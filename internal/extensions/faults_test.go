package extensions

import (
	"context"
	"encoding/json"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/qingbird1983/locowrk/internal/eventbus"
	"github.com/qingbird1983/locowrk/internal/kernel"
)

// faultsRoot and samplesRoot are the two fixture roots used by the fault tests.
func faultRoots(t *testing.T) (string, string) {
	t.Helper()
	faults, err := filepath.Abs(filepath.Join("..", "..", "fixtures", "faults"))
	if err != nil {
		t.Fatal(err)
	}
	samples, err := filepath.Abs(filepath.Join("..", "..", "fixtures", "extensions"))
	if err != nil {
		t.Fatal(err)
	}
	return faults, samples
}

func trustAll(t *testing.T, names []string, roots ...string) (*memStore, []*Candidate) {
	t.Helper()
	cands := Discover(t.TempDir(), roots)
	byName := map[string]*Candidate{}
	for _, c := range cands {
		byName[c.Name] = c
	}
	store := newStore()
	for _, name := range names {
		c := byName[name]
		if c == nil || c.State != "ready" {
			t.Fatalf("%s not ready: %+v", name, cands)
		}
		store.recs[name] = record{path: c.Dir, sha: c.SHA, enabled: true, trusted: true}
	}
	return store, cands
}

// waitState polls the status rows until the extension reaches want.
func waitState(t *testing.T, m *Manager, name, want string) ExtensionInfo {
	t.Helper()
	deadline := time.Now().Add(10 * time.Second)
	var last ExtensionInfo
	for time.Now().Before(deadline) {
		for _, info := range m.Extensions() {
			if info.Name == name {
				last = info
				if info.State == want {
					return info
				}
			}
		}
		time.Sleep(20 * time.Millisecond)
	}
	t.Fatalf("%s never reached %q: %+v", name, want, last)
	return last
}

// TestCrashedExtensionIsDisabledAndIsolated checks acceptance 8: a crashing
// extension is recorded, shown as failed, and does not affect its sibling.
func TestCrashedExtensionIsDisabledAndIsolated(t *testing.T) {
	node, err := exec.LookPath("node")
	if err != nil {
		t.Skip("node not installed")
	}
	faults, samples := faultRoots(t)
	store, _ := trustAll(t, []string{"crash", "orchestrator-sample"}, faults, samples)
	bus := eventbus.New()
	m := New(Options{Bus: bus, Cwd: t.TempDir(), NodePath: node, Store: store, ExtraDirs: []string{faults, samples}})
	if err := m.Prepare(context.Background()); err != nil {
		t.Fatalf("Prepare: %v", err)
	}
	defer m.Stop()

	// The command exits the process before answering.
	if err := m.RunCommand(context.Background(), "crash", ""); err == nil {
		t.Fatal("crash command reported success")
	}
	info := waitState(t, m, "crash", "failed")
	if !strings.Contains(info.Reason, "exit") && !strings.Contains(info.Reason, "crash") {
		t.Fatalf("failure reason does not explain the crash: %q", info.Reason)
	}
	diag := false
	for _, d := range info.Diagnostics {
		if d.Code == "crashed" {
			diag = true
		}
	}
	if !diag {
		t.Fatalf("crash diagnostic missing: %+v", info.Diagnostics)
	}

	// The sibling keeps working.
	for _, spec := range m.Tools() {
		if spec.Name != "sample_echo" {
			continue
		}
		res, err := spec.Execute(context.Background(), "t1", json.RawMessage(`{"text":"still here"}`))
		if err != nil || !strings.Contains(res.Text, "still here") {
			t.Fatalf("sibling tool broke: %+v %v", res, err)
		}
		return
	}
	t.Fatalf("sibling tool missing: %+v", m.Tools())
}

// TestToolTimeoutRecordsDiagnostic checks that a tool which never answers times
// out through the manifest timeout instead of hanging the turn.
func TestToolTimeoutRecordsDiagnostic(t *testing.T) {
	node, err := exec.LookPath("node")
	if err != nil {
		t.Skip("node not installed")
	}
	faults, _ := faultRoots(t)
	store, _ := trustAll(t, []string{"slow"}, faults)
	m := New(Options{
		Bus: eventbus.New(), Cwd: t.TempDir(), NodePath: node, Store: store,
		ExtraDirs: []string{faults},
	})
	if err := m.Prepare(context.Background()); err != nil {
		t.Fatalf("Prepare: %v", err)
	}
	defer m.Stop()

	var slow kernel.ToolSpec
	for _, spec := range m.Tools() {
		if spec.Name == "fault_slow" {
			slow = spec
		}
	}
	if slow.Name == "" {
		t.Fatalf("slow tool missing: %+v", m.Tools())
	}
	start := time.Now()
	if _, err := slow.Execute(context.Background(), "t1", json.RawMessage(`{}`)); err == nil {
		t.Fatal("slow tool returned without error")
	}
	if elapsed := time.Since(start); elapsed > 3*time.Second {
		t.Fatalf("timeout took %s; the manifest timeout was ignored", elapsed)
	}
	info := waitState(t, m, "slow", "running")
	timeoutDiag := false
	for _, d := range info.Diagnostics {
		if d.Code == "timeout" {
			timeoutDiag = true
		}
	}
	if !timeoutDiag {
		t.Fatalf("timeout diagnostic missing: %+v", info.Diagnostics)
	}

	// The process stays usable after a timed-out call.
	for _, spec := range m.Tools() {
		if spec.Name != "fault_ok" {
			continue
		}
		if res, err := spec.Execute(context.Background(), "t2", json.RawMessage(`{}`)); err != nil || res.Text != "ok" {
			t.Fatalf("extension broke after timeout: %+v %v", res, err)
		}
		return
	}
	t.Fatalf("healthy tool missing: %+v", m.Tools())
}

// TestToolConflictDisablesOnlyTheSecond checks that a duplicate tool name
// disables the later extension and keeps the owner running.
func TestToolConflictDisablesOnlyTheSecond(t *testing.T) {
	node, err := exec.LookPath("node")
	if err != nil {
		t.Skip("node not installed")
	}
	faults, samples := faultRoots(t)
	store, _ := trustAll(t, []string{"tool-conflict", "orchestrator-sample"}, faults, samples)
	m := New(Options{
		Bus: eventbus.New(), Cwd: t.TempDir(), NodePath: node, Store: store,
		ExtraDirs: []string{faults, samples},
	})
	if err := m.Prepare(context.Background()); err != nil {
		t.Fatalf("Prepare: %v", err)
	}
	defer m.Stop()

	owner := waitState(t, m, "orchestrator-sample", "running")
	if owner.Tools != 1 {
		t.Fatalf("owner lost its tools: %+v", owner)
	}
	conflict := waitState(t, m, "tool-conflict", "failed")
	if !strings.Contains(conflict.Reason, "already provided by orchestrator-sample") {
		t.Fatalf("conflict reason = %q", conflict.Reason)
	}
}
