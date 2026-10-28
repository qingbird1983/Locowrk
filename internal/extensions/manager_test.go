package extensions

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/qingbird1983/locowrk/internal/adapter"
	"github.com/qingbird1983/locowrk/internal/eventbus"
	"github.com/qingbird1983/locowrk/internal/kernel"
)

var mockBinary string

func TestMain(m *testing.M) {
	dir, err := os.MkdirTemp("", "locowrk-ext-mock-")
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
	defer os.RemoveAll(dir)
	name := "mock-orchestrator"
	if runtime.GOOS == "windows" {
		name += ".exe"
	}
	mockBinary = filepath.Join(dir, name)
	cmd := exec.Command("go", "build", "-o", mockBinary, "./fixtures/plugins/mock-orchestrator")
	cmd.Dir = filepath.Join("..", "..")
	if out, err := cmd.CombinedOutput(); err != nil {
		fmt.Fprintf(os.Stderr, "building mock: %v\n%s", err, out)
		os.Exit(1)
	}
	os.Exit(m.Run())
}

type memStore struct {
	mu    sync.Mutex
	recs  map[string]record
	trust map[string]bool
}

type record struct {
	path, sha string
	enabled   bool
	trusted   bool
}

func newStore() *memStore {
	return &memStore{recs: map[string]record{}, trust: map[string]bool{}}
}

func (s *memStore) Extension(name string) (string, string, bool, bool, bool) {
	s.mu.Lock()
	defer s.mu.Unlock()
	r, ok := s.recs[name]
	return r.path, r.sha, r.enabled, r.trusted, ok
}
func (s *memStore) SaveExtension(name, path, sha string, enabled, trusted bool) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.recs[name] = record{path: path, sha: sha, enabled: enabled, trusted: trusted}
	return nil
}
func (s *memStore) ProjectTrusted(cwd string) bool {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.trust[cwd]
}
func (s *memStore) SetProjectTrust(cwd string, trusted bool) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.trust[cwd] = trusted
	return nil
}

func installMock(t *testing.T, root string) (*adapter.Manifest, string) {
	t.Helper()
	dir := filepath.Join(root, "mock")
	if err := os.MkdirAll(dir, 0o755); err != nil {
		t.Fatal(err)
	}
	name := "mock-orchestrator"
	if runtime.GOOS == "windows" {
		name += ".exe"
	}
	data, err := os.ReadFile(mockBinary)
	if err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(dir, name), data, 0o755); err != nil {
		t.Fatal(err)
	}
	manifest := `{"name":"mock-orchestrator","version":"0.1.0","protocol":1,"entrypoint":"` + name + `"}`
	if err := os.WriteFile(filepath.Join(dir, adapter.ManifestFile), []byte(manifest), 0o644); err != nil {
		t.Fatal(err)
	}
	m, err := adapter.Load(dir)
	if err != nil {
		t.Fatal(err)
	}
	return m, dir
}

func TestDiscoverAndTrust(t *testing.T) {
	root := t.TempDir()
	_, dir := installMock(t, root)
	cands := Discover(t.TempDir(), []string{root})
	if len(cands) != 1 || cands[0].State != "ready" || cands[0].Name != "mock-orchestrator" {
		t.Fatalf("candidates %+v", cands)
	}
	store := newStore()
	m := New(Options{Bus: eventbus.New(), Cwd: dir, Store: store, ExtraDirs: []string{root}})
	if err := m.Prepare(context.Background()); err != nil {
		t.Fatal(err)
	}
	info := m.Extensions()
	if len(info) != 1 || info[0].State != "untrusted" {
		t.Fatalf("untrusted info %+v", info)
	}
	if err := m.SetTrusted("mock-orchestrator", true); err != nil {
		t.Fatal(err)
	}
	info = m.Extensions()
	if info[0].State != "running" || info[0].Tools != 2 || info[0].Commands != 4 {
		t.Fatalf("trusted info %+v", info)
	}
	tools := m.Tools()
	if len(tools) != 2 {
		t.Fatalf("tools %+v", tools)
	}
	var echo *kernel.ToolSpec
	for i := range tools {
		if tools[i].Name == "mock_echo" {
			echo = &tools[i]
		}
	}
	if echo == nil {
		t.Fatalf("mock_echo missing: %+v", tools)
	}
	res, err := echo.Execute(context.Background(), "t1", json.RawMessage(`{"text":"hi"}`))
	if err != nil || !strings.Contains(res.Text, "hi") {
		t.Fatalf("execute: %+v %v", res, err)
	}
	if err := m.RunCommand(context.Background(), "mock_ui", ""); err != nil {
		t.Fatal(err)
	}
	deadline := time.Now().Add(time.Second)
	for len(m.Notices()) == 0 && time.Now().Before(deadline) {
		time.Sleep(10 * time.Millisecond)
	}
	if len(m.Notices()) == 0 || m.Notices()[0].Type != "info" {
		t.Fatalf("notices %+v", m.Notices())
	}
	m.SetSession("s2")
	if err := m.SetTrusted("mock-orchestrator", false); err != nil {
		t.Fatal(err)
	}
	if info := m.Extensions(); info[0].State != "untrusted" || info[0].Tools != 0 {
		t.Fatalf("after revoke %+v", info)
	}
	m.Stop()
}

func TestHashChangeLosesTrust(t *testing.T) {
	root := t.TempDir()
	_, dir := installMock(t, root)
	store := newStore()
	m := New(Options{Bus: eventbus.New(), Cwd: dir, Store: store, ExtraDirs: []string{root}})
	if err := m.Prepare(context.Background()); err != nil {
		t.Fatal(err)
	}
	if err := m.SetTrusted("mock-orchestrator", true); err != nil {
		t.Fatal(err)
	}
	m.Stop()
	// Change the entrypoint: the stored hash no longer matches.
	entry := filepath.Join(dir, "mock-orchestrator")
	if runtime.GOOS == "windows" {
		entry += ".exe"
	}
	if err := os.WriteFile(entry, []byte("changed"), 0o755); err != nil {
		t.Fatal(err)
	}
	again := New(Options{Bus: eventbus.New(), Cwd: dir, Store: store, ExtraDirs: []string{root}})
	if err := again.Prepare(context.Background()); err != nil {
		t.Fatal(err)
	}
	info := again.Extensions()
	if len(info) != 1 || info[0].State != "untrusted" {
		t.Fatalf("changed bundle stayed trusted: %+v", info)
	}
}

func TestProjectTrust(t *testing.T) {
	store := newStore()
	m := New(Options{Store: store})
	cwd := t.TempDir()
	if m.ProjectTrusted(cwd) {
		t.Fatal("new workspace trusted")
	}
	if err := m.SetProjectTrust(cwd, true); err != nil {
		t.Fatal(err)
	}
	if !m.ProjectTrusted(cwd) {
		t.Fatal("trust not stored")
	}
}
