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
)

func TestNodeSampleEndToEnd(t *testing.T) {
	node, err := exec.LookPath("node")
	if err != nil {
		t.Skip("node not installed")
	}
	root, err := filepath.Abs(filepath.Join("..", "..", "fixtures", "extensions"))
	if err != nil {
		t.Fatal(err)
	}
	cands := Discover(t.TempDir(), []string{root})
	var sample *Candidate
	for _, c := range cands {
		if c.Name == "orchestrator-sample" {
			sample = c
		}
	}
	if sample == nil || sample.State != "ready" {
		t.Fatalf("sample not found: %+v", cands)
	}
	store := newStore()
	store.recs[sample.Name] = record{path: sample.Dir, sha: sample.SHA, enabled: true, trusted: true}
	bus := eventbus.New()
	sub := bus.Subscribe(eventbus.SubscribeOptions{})
	defer sub.Close()
	m := New(Options{Bus: bus, Cwd: t.TempDir(), NodePath: node, Store: store, ExtraDirs: []string{root}})
	if err := m.Prepare(context.Background()); err != nil {
		t.Fatal(err)
	}
	defer m.Stop()
	info := m.Extensions()
	if len(info) != 1 || info[0].State != "running" {
		t.Fatalf("info %+v", info)
	}
	found := false
	for _, tool := range m.Tools() {
		if tool.Name == "sample_echo" {
			found = true
			res, err := tool.Execute(context.Background(), "t1", json.RawMessage(`{"text":"hello"}`))
			if err != nil || !strings.Contains(res.Text, "sample: hello") {
				t.Fatalf("tool %+v %v", res, err)
			}
		}
	}
	if !found {
		t.Fatalf("sample_echo missing: %+v", m.Tools())
	}
	if err := m.RunCommand(context.Background(), "sample_events", ""); err != nil {
		t.Fatal(err)
	}
	seen := map[eventbus.Kind]bool{}
	deadline := time.After(5 * time.Second)
	for !seen[eventbus.KindSubagentSpawned] || !seen[eventbus.KindSubagentSettled] || !seen[eventbus.KindWorkflowStep] {
		select {
		case ev := <-sub.C():
			seen[ev.Kind] = true
		case <-deadline:
			t.Fatalf("events missing: %+v", seen)
		}
	}
}
