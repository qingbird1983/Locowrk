package extensions

import (
	"context"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"
	"testing"
)

func TestBundledRuntimeMatchesPin(t *testing.T) {
	name := "node"
	if runtime.GOOS == "windows" {
		name = "node.exe"
	}
	path, err := filepath.Abs(filepath.Join("..", "..", "runtime", "node", name))
	if err != nil {
		t.Fatal(err)
	}
	if _, err := os.Stat(path); err != nil {
		t.Skip("bundled runtime not fetched (scripts/fetch-node-runtime.mjs)")
	}
	if err := CheckNodeVersion(context.Background(), path, PinnedNodeVersion); err != nil {
		t.Fatalf("bundled runtime does not match the pin: %v", err)
	}
}

func TestNodeRuntimeVersionGate(t *testing.T) {
	node, err := exec.LookPath("node")
	if err != nil {
		t.Skip("node not installed")
	}
	out, err := exec.Command(node, "--version").Output()
	if err != nil {
		t.Fatalf("node --version: %v", err)
	}
	installed := strings.TrimSpace(string(out))
	if err := CheckNodeVersion(context.Background(), node, installed); err != nil {
		t.Fatalf("pinned version rejected: %v", err)
	}
	if err := CheckNodeVersion(context.Background(), node, "v0.0.1"); err == nil {
		t.Fatal("mismatched runtime was accepted")
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
	if sample == nil {
		t.Fatalf("sample not found: %+v", cands)
	}
	store := newStore()
	store.recs[sample.Name] = record{path: sample.Dir, sha: sample.SHA, enabled: true, trusted: true}
	m := New(Options{
		Cwd: t.TempDir(), NodePath: node, NodeVersion: "v0.0.1",
		Store: store, ExtraDirs: []string{root},
	})
	if err := m.Prepare(context.Background()); err != nil {
		t.Fatalf("Prepare: %v", err)
	}
	defer m.Stop()
	infos := m.Extensions()
	if len(infos) != 1 || infos[0].State != "failed" {
		t.Fatalf("mismatched runtime did not disable the A track: %+v", infos)
	}
	if !strings.Contains(infos[0].Reason, "needs v0.0.1") {
		t.Fatalf("reason does not name the pin: %q", infos[0].Reason)
	}
}
