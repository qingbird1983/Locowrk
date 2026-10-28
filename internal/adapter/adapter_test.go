package adapter

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/qingbird1983/locowrk/internal/eventbus"
)

var mockBinary string

func TestMain(m *testing.M) {
	dir, err := os.MkdirTemp("", "locowrk-mock-")
	if err != nil {
		fmt.Fprintln(os.Stderr, "temp dir:", err)
		os.Exit(1)
	}
	defer os.RemoveAll(dir)
	exe := "mock-orchestrator"
	if runtime.GOOS == "windows" {
		exe += ".exe"
	}
	mockBinary = filepath.Join(dir, exe)
	cmd := exec.Command("go", "build", "-o", mockBinary, "./fixtures/plugins/mock-orchestrator")
	cmd.Dir = filepath.Join("..", "..")
	out, err := cmd.CombinedOutput()
	if err != nil {
		fmt.Fprintf(os.Stderr, "building mock plugin: %v\n%s\n", err, out)
		os.Exit(1)
	}
	os.Exit(m.Run())
}

type fixture struct {
	dir      string
	adapter  *Adapter
	bus      *eventbus.Bus
	manifest *Manifest
	mu       sync.Mutex
	notes    []Notification
}

// newFixture installs the mock plugin in a temp dir and starts it.
func newFixture(t *testing.T, mode string, tweak func(*Options), env ...string) *fixture {
	t.Helper()
	dir := t.TempDir()
	name := "mock-orchestrator"
	if runtime.GOOS == "windows" {
		name += ".exe"
	}
	data, err := os.ReadFile(mockBinary)
	if err != nil {
		t.Fatalf("reading mock binary: %v", err)
	}
	if err := os.WriteFile(filepath.Join(dir, name), data, 0o755); err != nil {
		t.Fatalf("installing mock binary: %v", err)
	}
	manifestJSON := `{
	  "name": "mock-orchestrator",
	  "version": "0.1.0",
	  "protocol": 1,
	  "entrypoint": "` + name + `",
	  "capabilities": [{"name": "fs.write", "scope": ["notes/**"]}]
	}`
	if err := os.WriteFile(filepath.Join(dir, ManifestFile), []byte(manifestJSON), 0o644); err != nil {
		t.Fatalf("writing manifest: %v", err)
	}

	bus := eventbus.New()
	f := &fixture{dir: dir, bus: bus}
	opts := Options{
		Bus:               bus,
		SessionID:         "session-1",
		Cwd:               dir,
		HandshakeTimeout:  5 * time.Second,
		RPCTimeout:        2 * time.Second,
		ReadyQuiet:        20 * time.Millisecond,
		ReadyGrace:        300 * time.Millisecond,
		HeartbeatInterval: time.Hour,
		ShutdownTimeout:   2 * time.Second,
		MaxRestarts:       0,
		Env:               append([]string{"LOCOWRK_MOCK_MODE=" + mode}, env...),
		Notify: func(n Notification) {
			f.mu.Lock()
			f.notes = append(f.notes, n)
			f.mu.Unlock()
		},
	}
	if tweak != nil {
		tweak(&opts)
	}
	adapter, err := New(opts)
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	m, err := Load(dir)
	if err != nil {
		t.Fatalf("Load: %v", err)
	}
	if err := adapter.Start(context.Background(), m); err != nil {
		t.Fatalf("Start: %v", err)
	}
	f.adapter = adapter
	f.manifest = m
	t.Cleanup(func() { _ = adapter.Close() })
	return f
}

func (f *fixture) notifications() []Notification {
	f.mu.Lock()
	defer f.mu.Unlock()
	return append([]Notification(nil), f.notes...)
}

func TestStartRegistersToolsAndCommands(t *testing.T) {
	f := newFixture(t, "supported", nil)
	if got := f.adapter.State(); got != StateReady {
		t.Fatalf("state = %s, want ready", got)
	}
	tools := f.adapter.Tools()
	if len(tools) != 2 || tools[0].Name != "mock_echo" || tools[1].Name != "mock_host_error" {
		t.Fatalf("tools = %+v", tools)
	}
	if tools[0].Plugin != "mock-orchestrator" {
		t.Fatalf("tool plugin = %q", tools[0].Plugin)
	}
	var schema map[string]any
	if err := json.Unmarshal(tools[0].Parameters, &schema); err != nil {
		t.Fatalf("parameters are not JSON: %v", err)
	}
	if schema["type"] != "object" {
		t.Fatalf("parameters type = %v, want object", schema["type"])
	}
	commands := f.adapter.Commands()
	if len(commands) != 4 {
		t.Fatalf("commands = %+v", commands)
	}
}

func TestSupportedToolAndTUIDiagnostics(t *testing.T) {
	f := newFixture(t, "supported", nil)
	ctx := context.Background()

	result, err := f.adapter.InvokeTool(ctx, "mock_echo", json.RawMessage(`{"text":"hello"}`))
	if err != nil {
		t.Fatalf("InvokeTool: %v", err)
	}
	if got := result.Text(); got != "echo:hello" {
		t.Fatalf("tool text = %q", got)
	}

	if err := f.adapter.SendCommand(ctx, "mock_ui", ""); err != nil {
		t.Fatalf("SendCommand: %v", err)
	}
	notes := f.notifications()
	if len(notes) != 1 || notes[0].Message != "mock ui ready" {
		t.Fatalf("notifications = %+v", notes)
	}

	var widget *Diagnostic
	for _, d := range f.adapter.Diagnostics() {
		d := d
		if d.Member == "ui.setWidget" {
			widget = &d
		}
	}
	if widget == nil {
		t.Fatal("no ui.setWidget diagnostic")
	}
	if widget.Category != CategoryUnsupported || widget.Count != 2 {
		t.Fatalf("widget diagnostic = %+v", widget)
	}

	// The extension kept running after the no-op twice: the next command works.
	if err := f.adapter.SendCommand(ctx, "mock_events", ""); err != nil {
		t.Fatalf("second command: %v", err)
	}
}

func TestEventForwardingOrder(t *testing.T) {
	f := newFixture(t, "supported", nil)
	sub := f.bus.Subscribe(eventbus.SubscribeOptions{SessionID: "session-1"})
	defer sub.Close()

	if err := f.adapter.SendCommand(context.Background(), "mock_events", ""); err != nil {
		t.Fatalf("SendCommand: %v", err)
	}
	want := []eventbus.Kind{
		eventbus.KindSubagentSpawned,
		eventbus.KindWorkflowStep,
		eventbus.KindWorkflowStep,
		eventbus.KindSubagentSettled,
	}
	for i, kind := range want {
		select {
		case ev := <-sub.C():
			if ev.Kind != kind {
				t.Fatalf("event %d = %s, want %s", i, ev.Kind, kind)
			}
			if ev.SessionID != "session-1" {
				t.Fatalf("event %d session = %q", i, ev.SessionID)
			}
			if ev.Seq != uint64(i+1) {
				t.Fatalf("event %d seq = %d, want %d", i, ev.Seq, i+1)
			}
		case <-time.After(2 * time.Second):
			t.Fatalf("timed out waiting for event %d (%s)", i, kind)
		}
	}
}

func TestDeferredHostCall(t *testing.T) {
	f := newFixture(t, "supported", nil)
	ctx := context.Background()
	if err := f.adapter.SendCommand(ctx, "mock_deferred", ""); err != nil {
		t.Fatalf("SendCommand: %v", err)
	}
	result, err := f.adapter.InvokeTool(ctx, "mock_host_error", json.RawMessage(`{}`))
	if err != nil {
		t.Fatalf("InvokeTool: %v", err)
	}
	if got := result.Text(); got != "4401" {
		t.Fatalf("host error code = %q, want 4401", got)
	}
	found := false
	for _, d := range f.adapter.Diagnostics() {
		if d.Member == "spawn_subagent" && d.Category == CategoryNotImplemented {
			found = true
		}
	}
	if !found {
		t.Fatalf("diagnostics = %+v", f.adapter.Diagnostics())
	}
}

func TestPermissionDeniedIsSeparateFromDiagnostics(t *testing.T) {
	f := newFixture(t, "supported", nil, "LOCOWRK_MOCK_FS_PATH=secrets.txt")
	ctx := context.Background()
	if err := f.adapter.SendCommand(ctx, "mock_denied", ""); err != nil {
		t.Fatalf("SendCommand: %v", err)
	}
	result, err := f.adapter.InvokeTool(ctx, "mock_host_error", json.RawMessage(`{}`))
	if err != nil {
		t.Fatalf("InvokeTool: %v", err)
	}
	if got := result.Text(); got != "4402" {
		t.Fatalf("host error = %q, want 4402", got)
	}
	audit := f.adapter.Audit()
	if len(audit) != 1 || audit[0].Allowed || audit[0].Capability != "fs.write" {
		t.Fatalf("audit = %+v", audit)
	}
	if diagnostics := f.adapter.Diagnostics(); len(diagnostics) != 0 {
		t.Fatalf("permission denial leaked into diagnostics: %+v", diagnostics)
	}
	if _, err := os.Stat(filepath.Join(f.dir, "secrets.txt")); !os.IsNotExist(err) {
		t.Fatalf("denied write created a file (stat err = %v)", err)
	}
}

func TestAllowedWrite(t *testing.T) {
	f := newFixture(t, "supported", nil, "LOCOWRK_MOCK_FS_PATH=notes/ok.txt")
	if err := os.MkdirAll(filepath.Join(f.dir, "notes"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := f.adapter.SendCommand(context.Background(), "mock_denied", ""); err != nil {
		t.Fatalf("SendCommand: %v", err)
	}
	data, err := os.ReadFile(filepath.Join(f.dir, "notes", "ok.txt"))
	if err != nil {
		t.Fatalf("reading written file: %v", err)
	}
	if string(data) != "fixture" {
		t.Fatalf("file = %q", data)
	}
	audit := f.adapter.Audit()
	if len(audit) != 1 || !audit[0].Allowed {
		t.Fatalf("audit = %+v", audit)
	}
}

func TestCrashIsolation(t *testing.T) {
	f := newFixture(t, "crash", nil)
	deadline := time.Now().Add(3 * time.Second)
	for time.Now().Before(deadline) {
		if f.adapter.State() == StateFailed {
			break
		}
		time.Sleep(20 * time.Millisecond)
	}
	if got := f.adapter.State(); got != StateFailed {
		t.Fatalf("state = %s, want failed", got)
	}
	exit := f.adapter.Exit()
	if exit == nil || exit.Code != 3 {
		t.Fatalf("exit = %+v", exit)
	}
	if err := f.adapter.Err(); !errors.Is(err, ErrCrashed) {
		t.Fatalf("err = %v", err)
	}
	// The host is still alive and can be stopped cleanly.
	if err := f.adapter.Close(); err != nil {
		t.Fatalf("Close after crash: %v", err)
	}
}

func TestRestartAfterCrash(t *testing.T) {
	f := newFixture(t, "crash", func(o *Options) {
		o.MaxRestarts = 1
		o.RestartBackoff = []time.Duration{20 * time.Millisecond}
	})
	deadline := time.Now().Add(5 * time.Second)
	for time.Now().Before(deadline) {
		crashed := 0
		for _, d := range f.adapter.Diagnostics() {
			if d.Member == "process" && d.Code == "crashed" {
				crashed = d.Count
			}
		}
		if crashed >= 2 && f.adapter.State() == StateFailed {
			return
		}
		time.Sleep(20 * time.Millisecond)
	}
	t.Fatalf("adapter did not retry once and stop: state=%s exit=%+v diagnostics=%+v",
		f.adapter.State(), f.adapter.Exit(), f.adapter.Diagnostics())
}

func TestRPCTimeout(t *testing.T) {
	f := newFixture(t, "hang", func(o *Options) {
		o.RPCTimeout = 150 * time.Millisecond
		o.ShutdownTimeout = 200 * time.Millisecond
	})
	start := time.Now()
	_, err := f.adapter.InvokeTool(context.Background(), "mock_echo", json.RawMessage(`{"text":"x"}`))
	if !errors.Is(err, ErrTimeout) {
		t.Fatalf("err = %v, want timeout", err)
	}
	if elapsed := time.Since(start); elapsed > 2*time.Second {
		t.Fatalf("timeout took %s", elapsed)
	}
}

func TestHeartbeatLoss(t *testing.T) {
	f := newFixture(t, "hang", func(o *Options) {
		o.HeartbeatInterval = 30 * time.Millisecond
		o.HeartbeatTimeout = 20 * time.Millisecond
		o.MaxMissedPings = 2
		o.ShutdownTimeout = 200 * time.Millisecond
	})
	deadline := time.Now().Add(3 * time.Second)
	for time.Now().Before(deadline) {
		if f.adapter.State() == StateFailed {
			break
		}
		time.Sleep(10 * time.Millisecond)
	}
	if got := f.adapter.State(); got != StateFailed {
		t.Fatalf("state = %s, want failed", got)
	}
	found := false
	for _, d := range f.adapter.Diagnostics() {
		if d.Member == "heartbeat" && d.Code == "timeout" {
			found = true
		}
	}
	if !found {
		t.Fatalf("diagnostics = %+v", f.adapter.Diagnostics())
	}
}

func TestStopIsIdempotentAndRestartable(t *testing.T) {
	f := newFixture(t, "supported", nil)
	if err := f.adapter.Close(); err != nil {
		t.Fatalf("first Close: %v", err)
	}
	if got := f.adapter.State(); got != StateStopped {
		t.Fatalf("state = %s, want stopped", got)
	}
	if err := f.adapter.Close(); err != nil {
		t.Fatalf("second Close: %v", err)
	}
	if err := f.adapter.Start(context.Background(), f.manifest); err != nil {
		t.Fatalf("restart: %v", err)
	}
	if got := f.adapter.State(); got != StateReady {
		t.Fatalf("state after restart = %s", got)
	}
}

func TestManifestValidation(t *testing.T) {
	write := func(t *testing.T, body string) string {
		dir := t.TempDir()
		if err := os.WriteFile(filepath.Join(dir, ManifestFile), []byte(body), 0o644); err != nil {
			t.Fatal(err)
		}
		return dir
	}
	valid := `{"name":"demo","version":"0.1.0","protocol":1,"entrypoint":"bin/demo","capabilities":[{"name":"fs.read","scope":["src/**"]}]}`
	if _, err := Load(write(t, valid)); err != nil {
		t.Fatalf("valid manifest rejected: %v", err)
	}

	cases := map[string]string{
		"bad name":       `{"name":"Demo","version":"0.1.0","protocol":1,"entrypoint":"bin/demo","capabilities":[{"name":"fs.read","scope":["src/**"]}]}`,
		"bad version":    `{"name":"demo","version":"0.1.0-beta","protocol":1,"entrypoint":"bin/demo","capabilities":[{"name":"fs.read","scope":["src/**"]}]}`,
		"bad protocol":   `{"name":"demo","version":"0.1.0","protocol":2,"entrypoint":"bin/demo","capabilities":[{"name":"fs.read","scope":["src/**"]}]}`,
		"absolute entry": `{"name":"demo","version":"0.1.0","protocol":1,"entrypoint":"/bin/demo","capabilities":[{"name":"fs.read","scope":["src/**"]}]}`,
		"escaping entry": `{"name":"demo","version":"0.1.0","protocol":1,"entrypoint":"../demo","capabilities":[{"name":"fs.read","scope":["src/**"]}]}`,
		"empty scope":    `{"name":"demo","version":"0.1.0","protocol":1,"entrypoint":"bin/demo","capabilities":[{"name":"fs.read","scope":[]}]}`,
		"whole tree":     `{"name":"demo","version":"0.1.0","protocol":1,"entrypoint":"bin/demo","capabilities":[{"name":"fs.write","scope":["**"]}]}`,
		"duplicate cap":  `{"name":"demo","version":"0.1.0","protocol":1,"entrypoint":"bin/demo","capabilities":[{"name":"fs.read","scope":["src/**"]},{"name":"fs.read","scope":["lib/**"]}]}`,
		"unknown field":  `{"name":"demo","version":"0.1.0","protocol":1,"entrypoint":"bin/demo","capabilities":[],"extra":true}`,
	}
	for name, body := range cases {
		t.Run(name, func(t *testing.T) {
			if _, err := Load(write(t, body)); err == nil {
				t.Fatal("manifest accepted, want error")
			}
		})
	}
}

func TestScopeMatching(t *testing.T) {
	cases := []struct {
		scope, target string
		want          bool
	}{
		{"notes/**", "notes/a.txt", true},
		{"notes/**", "notes/deep/a.txt", true},
		{"notes/*.md", "notes/a.md", true},
		{"notes/*.md", "notes/a.txt", false},
		{"src/**/*.go", "src/a/b/c.go", true},
		{"src/*.go", "src/b/c.go", false},
		{"**", "anything", true},
	}
	for _, c := range cases {
		if got := matchesScope(c.scope, c.target); got != c.want {
			t.Fatalf("matchesScope(%q, %q) = %v, want %v", c.scope, c.target, got, c.want)
		}
	}
	if !hardDenied(".env") || !hardDenied("a/.git/config") || !hardDenied(".ssh/id_rsa") {
		t.Fatal("hard-deny paths not recognised")
	}
	if hardDenied("src/main.go") {
		t.Fatal("ordinary path treated as hard denied")
	}
}

func TestRedact(t *testing.T) {
	in := "token=supersecret sk-abcdefgh12345"
	out := redact(in)
	if strings.Contains(out, "supersecret") || strings.Contains(out, "abcdefgh12345") {
		t.Fatalf("redaction left a secret: %s", out)
	}
}
