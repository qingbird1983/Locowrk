package extensions

import (
	"context"
	"encoding/json"
	"os/exec"
	"path/filepath"
	"testing"
)

// TestToolCallGate checks the host-side permission gate: the fixture answers
// tool_call as a request because the shim declares wantsEventAck.
func TestToolCallGate(t *testing.T) {
	node, err := exec.LookPath("node")
	if err != nil {
		t.Skip("node not installed")
	}
	root, err := filepath.Abs(filepath.Join("..", "..", "fixtures", "extensions"))
	if err != nil {
		t.Fatal(err)
	}
	var sample *Candidate
	for _, c := range Discover(t.TempDir(), []string{root}) {
		if c.Name == "orchestrator-sample" {
			sample = c
		}
	}
	if sample == nil || sample.State != "ready" {
		t.Fatalf("sample not found: %+v", sample)
	}
	store := newStore()
	store.recs[sample.Name] = record{path: sample.Dir, sha: sample.SHA, enabled: true, trusted: true}
	m := New(Options{Cwd: t.TempDir(), NodePath: node, Store: store, ExtraDirs: []string{root}})
	if err := m.Prepare(context.Background()); err != nil {
		t.Fatalf("Prepare: %v", err)
	}
	defer m.Stop()

	reason, err := m.GateToolCall(context.Background(), "danger", json.RawMessage(`{"command":"rm -rf /"}`))
	if err != nil {
		t.Fatalf("gate: %v", err)
	}
	if reason != "fixture blocks destructive commands" {
		t.Fatalf("block reason = %q", reason)
	}
	reason, err = m.GateToolCall(context.Background(), "sample_echo", json.RawMessage(`{"text":"fine"}`))
	if err != nil || reason != "" {
		t.Fatalf("harmless call gated: %q %v", reason, err)
	}
}
