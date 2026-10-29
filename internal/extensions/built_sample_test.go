package extensions

import (
	"context"
	"encoding/json"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"

	"github.com/qingbird1983/locowrk/internal/eventbus"
)

// TestBuiltSampleEndToEnd builds fixtures/extensions/orchestrator-sample with
// tools/extension-build and runs the resulting package through the manager, so
// the production packaging path (single-file bundle with the shim inside) is
// covered, not just the committed hand-written bundle.
func TestBuiltSampleEndToEnd(t *testing.T) {
	node, err := exec.LookPath("node")
	if err != nil {
		t.Skip("node not installed")
	}
	tool, err := filepath.Abs(filepath.Join("..", "..", "tools", "extension-build", "build.mjs"))
	if err != nil {
		t.Fatal(err)
	}
	deps := filepath.Join(filepath.Dir(tool), "node_modules", "esbuild")
	if _, err := os.Stat(deps); err != nil {
		t.Skip("extension build deps not installed (cd tools/extension-build && npm install)")
	}
	src, err := filepath.Abs(filepath.Join("..", "..", "fixtures", "extensions", "orchestrator-sample"))
	if err != nil {
		t.Fatal(err)
	}
	root := t.TempDir()
	out := filepath.Join(root, "orchestrator-sample")
	build := exec.Command(node, tool, "--src", src, "--out", out)
	if data, err := build.CombinedOutput(); err != nil {
		t.Fatalf("build: %v\n%s", err, data)
	}

	cands := Discover(t.TempDir(), []string{root})
	if len(cands) != 1 || cands[0].Name != "orchestrator-sample" || cands[0].State != "ready" {
		t.Fatalf("built package not discoverable: %+v", cands)
	}
	sample := cands[0]
	if _, err := os.Stat(filepath.Join(sample.Dir, "build.json")); err != nil {
		t.Fatalf("build record missing: %v", err)
	}

	store := newStore()
	store.recs[sample.Name] = record{path: sample.Dir, sha: sample.SHA, enabled: true, trusted: true}
	bus := eventbus.New()
	m := New(Options{Bus: bus, Cwd: t.TempDir(), NodePath: node, Store: store, ExtraDirs: []string{root}})
	if err := m.Prepare(context.Background()); err != nil {
		t.Fatalf("Prepare: %v", err)
	}
	defer m.Stop()
	infos := m.Extensions()
	if len(infos) != 1 || infos[0].State != "running" {
		t.Fatalf("built package did not start: %+v", infos)
	}
	for _, spec := range m.Tools() {
		if spec.Name != "sample_echo" {
			continue
		}
		res, err := spec.Execute(context.Background(), "t1", json.RawMessage(`{"text":"built"}`))
		if err != nil || !strings.Contains(res.Text, "sample: built") {
			t.Fatalf("tool result %+v %v", res, err)
		}
		return
	}
	t.Fatalf("sample_echo missing: %+v", m.Tools())
}
