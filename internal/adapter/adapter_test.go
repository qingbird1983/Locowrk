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
	"strconv"
	"strings"
	"sync"
	"syscall"
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
	f := installMock(t, mode, tweak, env...)
	if err := f.adapter.Start(context.Background(), f.manifest); err != nil {
		t.Fatalf("Start: %v", err)
	}
	return f
}

// installMock prepares the adapter and manifest without starting the plugin, so
// tests can exercise startup failures.
func installMock(t *testing.T, mode string, tweak func(*Options), env ...string) *fixture {
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
	start := time.Now()
	f := newFixture(t, "supported", nil)
	if elapsed := time.Since(start); elapsed > 10*time.Second {
		t.Fatalf("handshake to ready took %s", elapsed)
	} else {
		t.Logf("handshake to ready: %s", elapsed)
	}
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

func TestHyphenatedCommandNamesAndRejectedNotifications(t *testing.T) {
	a, err := New(Options{NodePath: "node"})
	if err != nil {
		t.Fatalf("new adapter: %v", err)
	}
	// Upstream examples name commands with hyphens (rpc-demo: rpc-input).
	if _, rerr := a.handlePluginMessage("register_command", json.RawMessage(`{"name":"rpc-input","description":"prompt"}`)); rerr != nil {
		t.Fatalf("hyphenated command rejected: %+v", rerr)
	}
	if _, ok := a.commands["rpc-input"]; !ok {
		t.Fatalf("command not registered: %+v", a.Commands())
	}

	// A rejected notification has no request id, so the reason must show up as
	// a diagnostic instead of disappearing.
	a.handleNotify("register_command", json.RawMessage(`{"name":"Bad Name","description":"invalid"}`))
	found := false
	for _, d := range a.Diagnostics() {
		if d.Member == "register_command" && d.Code == "rejected" {
			found = true
		}
	}
	if !found {
		t.Fatalf("rejected notification produced no diagnostic: %+v", a.Diagnostics())
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

func TestStopKillsProcessTree(t *testing.T) {
	pidFile := filepath.Join(t.TempDir(), "grandchild.pid")
	f := newFixture(t, "grandchild", func(o *Options) {
		o.ShutdownTimeout = 500 * time.Millisecond
	}, "LOCOWRK_MOCK_PID_FILE="+pidFile)

	var pid int
	deadline := time.Now().Add(3 * time.Second)
	for time.Now().Before(deadline) {
		data, err := os.ReadFile(pidFile)
		if err == nil && len(data) > 0 {
			pid, _ = strconv.Atoi(strings.TrimSpace(string(data)))
			break
		}
		time.Sleep(20 * time.Millisecond)
	}
	if pid == 0 {
		t.Fatal("mock never reported its grandchild pid")
	}
	if !processAlive(pid) {
		t.Fatalf("grandchild %d was not running", pid)
	}

	if err := f.adapter.Close(); err != nil {
		t.Fatalf("Close: %v", err)
	}

	deadline = time.Now().Add(5 * time.Second)
	for time.Now().Before(deadline) {
		if !processAlive(pid) {
			return
		}
		time.Sleep(50 * time.Millisecond)
	}
	t.Fatalf("grandchild %d survived plugin shutdown", pid)
}

// processAlive reports whether pid exists; it is only used by the process-tree
// cleanup test.
func processAlive(pid int) bool {
	if runtime.GOOS == "windows" {
		out, err := exec.Command("tasklist", "/FI", fmt.Sprintf("PID eq %d", pid), "/NH").Output()
		if err != nil {
			return false
		}
		return strings.Contains(string(out), strconv.Itoa(pid))
	}
	proc, err := os.FindProcess(pid)
	if err != nil {
		return false
	}
	return proc.Signal(syscall.Signal(0)) == nil
}

func TestHandshakeTimeout(t *testing.T) {
	f := installMock(t, "silent", func(o *Options) {
		o.HandshakeTimeout = 300 * time.Millisecond
		o.ShutdownTimeout = 200 * time.Millisecond
	})
	start := time.Now()
	err := f.adapter.Start(context.Background(), f.manifest)
	if !errors.Is(err, ErrTimeout) {
		t.Fatalf("Start err = %v, want timeout", err)
	}
	if elapsed := time.Since(start); elapsed > 3*time.Second {
		t.Fatalf("handshake timeout took %s", elapsed)
	}
	if got := f.adapter.State(); got != StateFailed {
		t.Fatalf("state = %s, want failed", got)
	}
}

func TestOversizedFrameIsProtocolError(t *testing.T) {
	f := newFixture(t, "flood", func(o *Options) {
		o.ShutdownTimeout = 500 * time.Millisecond
	})
	deadline := time.Now().Add(5 * time.Second)
	for time.Now().Before(deadline) {
		if f.adapter.State() == StateFailed {
			break
		}
		time.Sleep(20 * time.Millisecond)
	}
	if got := f.adapter.State(); got != StateFailed {
		t.Fatalf("state = %s, want failed", got)
	}
	if err := f.adapter.Err(); !errors.Is(err, ErrProtocol) {
		t.Fatalf("err = %v, want protocol error", err)
	}
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
