package extensions

import (
	"context"
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

// realSampleUI answers the rpc-demo dialogs without a window and records what
// the extension asked for.
type realSampleUI struct {
	selects  []string
	confirms []string
	inputs   []string
	editors  []string
	input    string
	editor   string
}

func (u *realSampleUI) Select(_ context.Context, title string, _ []string) (string, bool) {
	u.selects = append(u.selects, title)
	return "", false
}

func (u *realSampleUI) Confirm(_ context.Context, title, _ string) (bool, bool) {
	u.confirms = append(u.confirms, title)
	return false, true
}

func (u *realSampleUI) Input(_ context.Context, title, _ string) (string, bool) {
	u.inputs = append(u.inputs, title)
	return u.input, true
}

func (u *realSampleUI) Editor(_ context.Context, title, _ string) (string, bool) {
	u.editors = append(u.editors, title)
	return u.editor, true
}

// TestRealPiSamples runs the upstream examples (pi coding-agent todo.ts and
// rpc-demo.ts) after tools/extension-build packed them. The sources are not in
// the repository; point LOCOWRK_REAL_SAMPLE_DIR at a directory that holds the
// two packages:
//
//	LOCOWRK_REAL_SAMPLE_DIR=<dir> go test ./internal/extensions -run TestRealPiSamples -v
//
// Results are recorded manually in docs/design/phase-6/report.md.
func TestRealPiSamples(t *testing.T) {
	root := os.Getenv("LOCOWRK_REAL_SAMPLE_DIR")
	if root == "" {
		t.Skip("set LOCOWRK_REAL_SAMPLE_DIR to a directory with the built todo and rpc-demo packages")
	}
	root, err := filepath.Abs(root)
	if err != nil {
		t.Fatal(err)
	}
	cands := Discover(t.TempDir(), []string{root})
	byName := map[string]*Candidate{}
	for _, c := range cands {
		byName[c.Name] = c
	}
	for _, name := range []string{"todo", "rpc-demo"} {
		c := byName[name]
		if c == nil || c.State != "ready" {
			t.Fatalf("%s not discovered as ready: %+v", name, cands)
		}
	}

	store := newStore()
	for _, name := range []string{"todo", "rpc-demo"} {
		c := byName[name]
		store.recs[name] = record{path: c.Dir, sha: c.SHA, enabled: true, trusted: true}
	}
	ui := &realSampleUI{input: "hello", editor: "line one\nline two\nline three"}
	m := New(Options{
		Cwd: t.TempDir(), NodePath: "node", Store: store,
		ExtraDirs: []string{root}, UI: ui,
	})
	if err := m.Prepare(context.Background()); err != nil {
		t.Fatalf("Prepare: %v", err)
	}
	defer m.Stop()
	for _, info := range m.Extensions() {
		if info.State != "running" {
			t.Fatalf("%s did not start: %+v", info.Name, info)
		}
	}

	// todo: tool registration, state in tool results, command.
	tools := m.Tools()
	var todoTool *struct{ name string }
	for i := range tools {
		if tools[i].Name == "todo" {
			todoTool = &struct{ name string }{tools[i].Name}
			res, err := tools[i].Execute(context.Background(), "t1", json.RawMessage(`{"action":"list"}`))
			if err != nil || !strings.Contains(res.Text, "No todos") {
				t.Fatalf("todo list: %+v %v", res, err)
			}
			res, err = tools[i].Execute(context.Background(), "t2", json.RawMessage(`{"action":"add","text":"write the report"}`))
			if err != nil || !strings.Contains(res.Text, "Added todo #1") {
				t.Fatalf("todo add: %+v %v", res, err)
			}
			res, err = tools[i].Execute(context.Background(), "t3", json.RawMessage(`{"action":"list"}`))
			if err != nil || !strings.Contains(res.Text, "[ ] #1: write the report") {
				t.Fatalf("todo list after add: %+v %v", res, err)
			}
		}
	}
	if todoTool == nil {
		t.Fatalf("todo tool missing: %+v", tools)
	}
	if err := m.RunCommand(context.Background(), "todos", ""); err != nil {
		t.Fatalf("todos command: %v", err)
	}
	waitNotice(t, m, "requires interactive mode")

	// rpc-demo: UI no-ops produce diagnostics, dialogs reach the host, notify
	// reaches the notice list.
	m.SetSession("sess-real")
	time.Sleep(100 * time.Millisecond)
	if err := m.RunCommand(context.Background(), "rpc-input", ""); err != nil {
		t.Fatalf("rpc-input: %v", err)
	}
	waitNotice(t, m, "You entered: hello")
	if err := m.RunCommand(context.Background(), "rpc-editor", ""); err != nil {
		t.Fatalf("rpc-editor: %v", err)
	}
	waitNotice(t, m, "Editor submitted (3 lines)")
	if len(ui.inputs) == 0 || len(ui.editors) == 0 {
		t.Fatalf("dialogs did not reach the host: inputs=%v editors=%v", ui.inputs, ui.editors)
	}
	var diagMembers []string
	for _, info := range m.Extensions() {
		if info.Name != "rpc-demo" {
			continue
		}
		for _, d := range info.Diagnostics {
			diagMembers = append(diagMembers, d.Member)
		}
	}
	for _, member := range []string{"ui.setTitle", "ui.setWidget", "ui.setStatus"} {
		found := false
		for _, got := range diagMembers {
			if got == member {
				found = true
			}
		}
		if !found {
			t.Fatalf("%s produced no diagnostic: %v", member, diagMembers)
		}
	}
	t.Logf("commands: %v; todo tool ok; diagnostics: %v; notices: %d",
		commandNames(m.Commands()), diagMembers, len(m.Notices()))
}

func commandNames(commands []CommandInfo) []string {
	out := make([]string, 0, len(commands))
	for _, c := range commands {
		out = append(out, c.Name)
	}
	return out
}

func waitNotice(t *testing.T, m *Manager, contains string) {
	t.Helper()
	deadline := time.Now().Add(5 * time.Second)
	for time.Now().Before(deadline) {
		for _, n := range m.Notices() {
			if strings.Contains(n.Message, contains) {
				return
			}
		}
		time.Sleep(20 * time.Millisecond)
	}
	t.Fatalf("notice %q never arrived (have %+v)", contains, m.Notices())
}
