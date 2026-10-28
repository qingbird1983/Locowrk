package desktop

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"
	"time"

	"github.com/egoist/mygo/ui"

	"github.com/qingbird1983/locowrk/internal/eventbus"
)

// dispatch publishes one event as the bridge would.
func dispatch(t *testing.T, h *Host, session string, kind eventbus.Kind, payload string) {
	t.Helper()
	h.Dispatch(eventbus.Event{Kind: kind, SessionID: session, At: time.Now().UTC(), Payload: json.RawMessage(payload)})
}

func newTestHost(t *testing.T, extras ...DesktopPlugin) *Host {
	t.Helper()
	return NewPluginHost(HostOptions{
		SettingsPath: filepath.Join(t.TempDir(), "settings.json"),
		Plugins:      extras,
	})
}

func TestSettingsRoundTrip(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "Locowrk", "settings.json")
	s := defaultSettings([]string{"a", "b"})
	s.Plugins["b"] = false
	s.Rail = "closed"
	if err := saveSettings(path, s); err != nil {
		t.Fatal(err)
	}
	got, err := loadSettings(path, []string{"a", "b"})
	if err != nil {
		t.Fatal(err)
	}
	if !got.Plugins["a"] || got.Plugins["b"] || got.Rail != "closed" {
		t.Fatalf("loaded %+v", got)
	}
}

func TestSettingsCorruptFallsBack(t *testing.T) {
	path := filepath.Join(t.TempDir(), "settings.json")
	if err := os.WriteFile(path, []byte("{not json"), 0o644); err != nil {
		t.Fatal(err)
	}
	got, err := loadSettings(path, []string{"a"})
	if err == nil {
		t.Fatal("corrupt settings reported no error")
	}
	if !got.Plugins["a"] || got.Rail != "open" {
		t.Fatalf("defaults not used: %+v", got)
	}
	// The original file stays for a human to inspect.
	if data, _ := os.ReadFile(path); string(data) != "{not json" {
		t.Fatal("corrupt settings were rewritten")
	}
	h := NewPluginHost(HostOptions{SettingsPath: path})
	if h.Snapshot().Diagnostics == 0 {
		t.Fatal("no diagnostic for corrupt settings")
	}
}

func TestDispatchCountsAndSessionReset(t *testing.T) {
	h := newTestHost(t)
	h.SetSession("s1")
	dispatch(t, h, "s1", eventbus.KindSubagentSpawned, `{"id":"a1","name":"探索"}`)
	dispatch(t, h, "s1", eventbus.KindSubagentSpawned, `{"id":"a2","name":"实现","parentId":"a1"}`)
	dispatch(t, h, "s1", eventbus.KindSubagentSettled, `{"id":"a2"}`)
	dispatch(t, h, "s1", eventbus.KindWorkflowStep, `{"name":"收集","status":"done"}`)
	dispatch(t, h, "s1", eventbus.KindWorkflowStep, `{"name":"实现","status":"running"}`)
	dispatch(t, h, "s1", eventbus.KindToolExecutionStart, `{"toolCallId":"t1","toolName":"read","arguments":{"path":"a"}}`)
	dispatch(t, h, "s1", eventbus.KindToolExecutionUpdate, `{"toolCallId":"t1","text":"working"}`)
	dispatch(t, h, "s1", eventbus.KindToolExecutionEnd, `{"toolCallId":"t1","text":"ok","durationMs":12}`)

	snap := h.Snapshot()
	if snap.Nodes != 2 || snap.Steps != 2 || snap.Tools != 1 {
		t.Fatalf("counts %+v", snap)
	}
	if snap.Enabled == nil || len(snap.Enabled) != 3 {
		t.Fatalf("enabled %v", snap.Enabled)
	}

	h.SetSession("s2")
	snap = h.Snapshot()
	if snap.Nodes != 0 || snap.Steps != 0 || snap.Tools != 0 {
		t.Fatalf("session switch kept data: %+v", snap)
	}
	dispatch(t, h, "s1", eventbus.KindSubagentSpawned, `{"id":"late","name":"旧会话"}`)
	if snap = h.Snapshot(); snap.Nodes != 0 {
		t.Fatalf("event of another session was applied: %+v", snap)
	}
}

func TestEnablePersistsAcrossHosts(t *testing.T) {
	path := filepath.Join(t.TempDir(), "settings.json")
	h := NewPluginHost(HostOptions{SettingsPath: path})
	h.SetSession("s1")
	h.setEnabled("orchestration-tree", false)
	dispatch(t, h, "s1", eventbus.KindSubagentSpawned, `{"id":"a1"}`)
	if snap := h.Snapshot(); snap.Nodes != 0 {
		t.Fatalf("disabled plugin still received events: %+v", snap)
	}
	again := NewPluginHost(HostOptions{SettingsPath: path})
	snap := again.Snapshot()
	for _, name := range snap.Enabled {
		if name == "orchestration-tree" {
			t.Fatal("disabled plugin came back enabled")
		}
	}
}

func TestRailTogglePersists(t *testing.T) {
	path := filepath.Join(t.TempDir(), "settings.json")
	h := NewPluginHost(HostOptions{SettingsPath: path})
	if !h.RailOpen() {
		t.Fatal("rail starts closed")
	}
	h.setRail(false)
	if h.RailOpen() {
		t.Fatal("rail did not close")
	}
	if again := NewPluginHost(HostOptions{SettingsPath: path}); again.RailOpen() {
		t.Fatal("closed rail did not persist")
	}
}

func TestMalformedPayloadIsReported(t *testing.T) {
	h := newTestHost(t)
	h.SetSession("s1")
	dispatch(t, h, "s1", eventbus.KindSubagentSpawned, `{"name":"no id"}`)
	if h.Diagnostic("orchestration-tree") == "" {
		t.Fatal("missing id produced no diagnostic")
	}
	if snap := h.Snapshot(); snap.Nodes != 0 {
		t.Fatalf("malformed event created a node: %+v", snap)
	}
}

// panicPlugin fails in Build, as a plugin bug would.
type panicPlugin struct{}

func (panicPlugin) Name() string                        { return "test-panic" }
func (panicPlugin) Title() string                       { return "崩溃插件" }
func (panicPlugin) Slot() Slot                          { return SlotSessionSidebar }
func (panicPlugin) InterestedEvents() []eventbus.Kind   { return nil }
func (panicPlugin) Dispatch(eventbus.Event, PluginHost) {}

func (panicPlugin) Build(*ui.Context, PluginHost) { panic("boom") }

func TestPluginPanicIsIsolated(t *testing.T) {
	h := newTestHost(t, panicPlugin{})
	h.SetSession("s1")
	dispatch(t, h, "s1", eventbus.KindSubagentSpawned, `{"id":"a1","name":"探索"}`)
	dispatch(t, h, "s1", eventbus.KindToolExecutionStart, `{"toolCallId":"t1","toolName":"read"}`)

	view := func(c *ui.Context) {
		ui.Column(c).Fill().Children(func() {
			h.buildSection(c, SlotSessionSidebar)
			h.buildSection(c, SlotToolDetail)
		})
	}
	tester := ui.NewTester(view, 420, 520)
	if !tester.HasText("崩溃插件 已停用（异常）") {
		t.Fatalf("failure card missing; texts: %v", tester.Texts())
	}
	if !tester.HasText("探索") {
		t.Fatalf("healthy plugin did not draw; texts: %v", tester.Texts())
	}
	if !tester.HasText("read") {
		t.Fatalf("tool inspector did not draw; texts: %v", tester.Texts())
	}
	// The next frame still builds the failure card instead of calling the
	// plugin again, and nothing panics out of the tester.
	tester.Frame()
	if !tester.HasText("崩溃插件 已停用（异常）") {
		t.Fatal("failure card disappeared on the next frame")
	}
	snap := h.Snapshot()
	if len(snap.Failed) != 1 || snap.Failed[0] != "test-panic" {
		t.Fatalf("failed list %v", snap.Failed)
	}
}

func TestPluginMenuTogglesAndPersists(t *testing.T) {
	h := newTestHost(t)
	h.SetSession("s1")
	view := func(c *ui.Context) {
		ui.Column(c).Fill().Children(func() { h.buildMenu(c) })
	}
	tester := ui.NewTester(view, 340, 420)
	if err := tester.Click("编排树"); err != nil {
		t.Fatalf("click checkbox: %v", err)
	}
	tester.Frame()
	for _, name := range h.Snapshot().Enabled {
		if name == "orchestration-tree" {
			t.Fatal("plugin stayed enabled")
		}
	}
	loaded, err := loadSettings(h.path, []string{"orchestration-tree"})
	if err != nil {
		t.Fatal(err)
	}
	if loaded.Plugins["orchestration-tree"] {
		t.Fatal("toggle was not persisted")
	}
}

func TestEmptyStates(t *testing.T) {
	h := newTestHost(t)
	view := func(c *ui.Context) {
		ui.Column(c).Fill().Children(func() {
			h.buildSection(c, SlotSessionSidebar)
			h.buildSection(c, SlotToolDetail)
		})
	}
	tester := ui.NewTester(view, 420, 520)
	if !tester.HasText("暂无子代理") || !tester.HasText("暂无工具调用") {
		t.Fatalf("empty states missing; texts: %v", tester.Texts())
	}
}

func TestToolSelectionHighlights(t *testing.T) {
	h := newTestHost(t)
	h.SetSession("s1")
	dispatch(t, h, "s1", eventbus.KindToolExecutionStart, `{"toolCallId":"t1","toolName":"read","arguments":{"path":"a"}}`)
	dispatch(t, h, "s1", eventbus.KindToolExecutionEnd, `{"toolCallId":"t1","text":"contents","durationMs":7}`)
	h.SelectToolCall("t1")
	view := func(c *ui.Context) {
		ui.Column(c).Fill().Children(func() { h.buildSection(c, SlotToolDetail) })
	}
	tester := ui.NewTester(view, 360, 480)
	if !tester.HasText("read") || !tester.HasText("contents") || !tester.HasText("7 ms") {
		t.Fatalf("inspector details missing; texts: %v", tester.Texts())
	}
}

func TestFrameBudget(t *testing.T) {
	h := newTestHost(t)
	h.SetSession("s1")
	for i := 0; i < 100; i++ {
		dispatch(t, h, "s1", eventbus.KindSubagentSpawned, `{"id":"n`+itoa(i)+`","name":"子代理 `+itoa(i)+`"}`)
	}
	for i := 0; i < 500; i++ {
		id := "t" + itoa(i)
		dispatch(t, h, "s1", eventbus.KindToolExecutionStart, `{"toolCallId":"`+id+`","toolName":"read","arguments":{"path":"a"}}`)
		dispatch(t, h, "s1", eventbus.KindToolExecutionEnd, `{"toolCallId":"`+id+`","text":"ok","durationMs":3}`)
	}
	view := func(c *ui.Context) {
		ui.Column(c).Fill().Children(func() {
			h.buildSection(c, SlotSessionSidebar)
			h.buildSection(c, SlotToolDetail)
		})
	}
	tester := ui.NewTester(view, 900, 600)
	start := time.Now()
	for i := 0; i < 60; i++ {
		tester.Frame()
	}
	per := time.Since(start) / 60
	t.Logf("60 frames with 100 nodes and 500 calls: %v per frame", per)
	limit := 16 * time.Millisecond
	if raceBuild {
		limit = 100 * time.Millisecond
	}
	if per > limit {
		t.Fatalf("frame budget exceeded: %v (limit %v)", per, limit)
	}
}

// itoa formats a small int without pulling strconv into the test's hot path.
func itoa(n int) string {
	if n == 0 {
		return "0"
	}
	var buf [8]byte
	i := len(buf)
	for n > 0 {
		i--
		buf[i] = byte('0' + n%10)
		n /= 10
	}
	return string(buf[i:])
}
