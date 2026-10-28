package desktop

import (
	"context"
	"errors"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
)

func readPatch(t *testing.T, name string) []byte {
	t.Helper()
	data, err := os.ReadFile(filepath.Join("testdata", "diff", name))
	if err != nil {
		t.Fatal(err)
	}
	return data
}

func parseFixture(t *testing.T, name string) DiffDoc {
	t.Helper()
	doc := DiffDoc{State: "ok", Files: []DiffFile{}}
	parseDiff(readPatch(t, name), &doc)
	return doc
}

func TestParseBasicDiff(t *testing.T) {
	doc := parseFixture(t, "basic.patch")
	if len(doc.Files) != 2 || doc.Stats.Additions != 4 || doc.Stats.Deletions != 1 {
		t.Fatalf("files=%d adds=%d dels=%d", len(doc.Files), doc.Stats.Additions, doc.Stats.Deletions)
	}
	hello := doc.Files[0]
	if hello.Path != "hello.txt" || hello.Status != "modified" || len(hello.Hunks) != 1 {
		t.Fatalf("hello: %+v", hello)
	}
	h := hello.Hunks[0]
	if h.OldStart != 1 || h.NewStart != 1 || h.NewCount != 4 {
		t.Fatalf("hunk header %+v", h)
	}
	if len(h.Lines) != 6 {
		t.Fatalf("lines %d", len(h.Lines))
	}
	if h.Lines[1].Kind != "del" || h.Lines[1].OldNo != 2 || h.Lines[1].Text != "two" {
		t.Fatalf("del line %+v", h.Lines[1])
	}
	if h.Lines[2].Kind != "add" || h.Lines[2].NewNo != 2 || h.Lines[2].Text != "2" {
		t.Fatalf("add line %+v", h.Lines[2])
	}
	if doc.Files[1].Status != "added" || doc.Files[1].Path != "new.txt" {
		t.Fatalf("new file %+v", doc.Files[1])
	}
}

func TestParseRenameBinaryDelete(t *testing.T) {
	doc := parseFixture(t, "rename-binary-delete.patch")
	if len(doc.Files) != 3 {
		t.Fatalf("files %+v", doc.Files)
	}
	ren := doc.Files[0]
	if ren.Status != "renamed" || ren.Path != "new/name.txt" || ren.OldPath != "old/name.txt" {
		t.Fatalf("rename %+v", ren)
	}
	if doc.Files[1].Status != "binary" || !doc.Files[1].Binary {
		t.Fatalf("binary %+v", doc.Files[1])
	}
	if doc.Files[2].Status != "deleted" || doc.Files[2].Path != "old.txt" {
		t.Fatalf("delete %+v", doc.Files[2])
	}
}

func TestParseSensitivePathsAreHidden(t *testing.T) {
	doc := parseFixture(t, "sensitive.patch")
	if doc.Hidden != 2 {
		t.Fatalf("hidden %d", doc.Hidden)
	}
	if len(doc.Files) != 1 || doc.Files[0].Path != "visible.txt" {
		t.Fatalf("visible files %+v", doc.Files)
	}
	if doc.Stats.Additions != 1 || doc.Stats.Deletions != 1 {
		t.Fatalf("stats counted hidden files: %+v", doc.Stats)
	}
}

func TestParseMalformedHunkIsSkipped(t *testing.T) {
	patch := []byte("diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ nonsense @@\n+line\n")
	doc := DiffDoc{State: "ok", Files: []DiffFile{}}
	parseDiff(patch, &doc)
	if doc.Skipped != 1 || len(doc.Files) != 0 {
		t.Fatalf("skipped=%d files=%+v", doc.Skipped, doc.Files)
	}
}

func TestParseLineCap(t *testing.T) {
	var b strings.Builder
	b.WriteString("diff --git a/big b/big\n--- a/big\n+++ b/big\n@@ -1 +1,30000 @@\n")
	for i := 0; i < maxDiffLines+10; i++ {
		b.WriteString("+line\n")
	}
	doc := DiffDoc{State: "ok", Files: []DiffFile{}}
	parseDiff([]byte(b.String()), &doc)
	if !doc.Truncated || !strings.Contains(doc.Reason, "20,000") {
		t.Fatalf("not truncated: %+v", doc)
	}
	if doc.Stats.Lines != maxDiffLines {
		t.Fatalf("lines %d", doc.Stats.Lines)
	}
}

func TestParseLongLineAndPath(t *testing.T) {
	long := strings.Repeat("中", maxDiffLineRunes+50)
	patch := []byte("diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -1 +1 @@\n-" + long + "\n+" + long + "\n")
	doc := DiffDoc{State: "ok", Files: []DiffFile{}}
	parseDiff(patch, &doc)
	lines := doc.Files[0].Hunks[0].Lines
	for _, l := range lines {
		if len([]rune(l.Text)) != maxDiffLineRunes+1 || !strings.HasSuffix(l.Text, "…") {
			t.Fatalf("line not clipped: %d runes", len([]rune(l.Text)))
		}
	}
}

func TestRelativeWorkspacePath(t *testing.T) {
	cwd, err := filepath.Abs(filepath.Join("work", "repo"))
	if err != nil {
		t.Fatal(err)
	}
	cases := map[string]string{
		"a.txt":                     "a.txt",
		`a\b.txt`:                   "a/b.txt",
		filepath.Join(cwd, "x.go"):  "x.go",
		"../outside.txt":            "",
		filepath.Join("other", "x"): "other/x",
		"":                          "",
	}
	for in, want := range cases {
		if got := relativeWorkspacePath(cwd, in); got != want {
			t.Fatalf("%q -> %q, want %q", in, got, want)
		}
	}
}

func TestGitEnvFiltersOverrides(t *testing.T) {
	t.Setenv("GIT_EXTERNAL_DIFF", "evil")
	t.Setenv("GIT_DIFF_OPTS", "--unified=99")
	env := gitEnv()
	for _, e := range env {
		if strings.HasPrefix(e, "GIT_EXTERNAL_DIFF=") || strings.HasPrefix(e, "GIT_DIFF_OPTS=") {
			t.Fatalf("kept %q", e)
		}
	}
	if !contains(env, "GIT_OPTIONAL_LOCKS=0") || !contains(env, "GIT_TERMINAL_PROMPT=0") {
		t.Fatalf("missing safety env: %v", env)
	}
}

func contains(env []string, want string) bool {
	for _, e := range env {
		if e == want {
			return true
		}
	}
	return false
}

func TestDiffLoadStates(t *testing.T) {
	ctx := context.Background()
	notRepo := &DiffService{cwd: "somewhere", git: func(context.Context, string, ...string) ([]byte, []byte, bool, error) {
		return nil, []byte("fatal: not a git repository"), false, errors.New("exit 128")
	}}
	if doc := notRepo.load(ctx, ""); doc.State != "notRepo" {
		t.Fatalf("not repo state %+v", doc)
	}

	ok := &DiffService{cwd: "somewhere", git: func(_ context.Context, _ string, args ...string) ([]byte, []byte, bool, error) {
		switch {
		case len(args) >= 2 && args[0] == "rev-parse" && args[1] == "--is-inside-work-tree":
			return []byte("true\n"), nil, false, nil
		case len(args) >= 2 && args[0] == "rev-parse":
			return []byte("abc1234\n"), nil, false, nil
		default:
			return []byte("diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -1 +1 @@\n-a\n+b\n"), nil, true, nil
		}
	}}
	doc := ok.load(ctx, "x")
	if doc.State != "ok" || doc.Revision != "abc1234" || !doc.Truncated || doc.Focus != "x" {
		t.Fatalf("ok state %+v", doc)
	}

	canceled, cancel := context.WithCancel(ctx)
	cancel()
	timeout := &DiffService{cwd: "somewhere", git: func(c context.Context, _ string, args ...string) ([]byte, []byte, bool, error) {
		if len(args) >= 2 && args[0] == "rev-parse" && args[1] == "--is-inside-work-tree" {
			return []byte("true\n"), nil, false, nil
		}
		return nil, nil, false, c.Err()
	}}
	if doc := timeout.load(canceled, ""); doc.State != "timeout" {
		t.Fatalf("timeout state %+v", doc)
	}
}

func TestDiffServiceRealRepo(t *testing.T) {
	if _, err := exec.LookPath("git"); err != nil {
		t.Skip("git not installed")
	}
	dir := t.TempDir()
	runGit(t, dir, "init")
	runGit(t, dir, "config", "core.autocrlf", "false")
	base := filepath.Join(dir, "a.txt")
	if err := os.WriteFile(base, []byte("one\ntwo\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	runGit(t, dir, "add", "a.txt")
	runGit(t, dir, "-c", "user.name=Test", "-c", "user.email=test@example.com", "commit", "-m", "base")
	if err := os.WriteFile(base, []byte("one\n2\nthree\n"), 0o644); err != nil {
		t.Fatal(err)
	}

	svc := NewDiffService(nil, dir)
	doc := svc.load(context.Background(), "a.txt")
	if doc.State != "ok" || len(doc.Files) != 1 || doc.Files[0].Path != "a.txt" {
		t.Fatalf("doc %+v", doc)
	}
	if doc.Stats.Additions != 2 || doc.Stats.Deletions != 1 {
		t.Fatalf("stats %+v", doc.Stats)
	}
	if doc.Focus != "a.txt" {
		t.Fatalf("focus %q", doc.Focus)
	}
}

func runGit(t *testing.T, dir string, args ...string) {
	t.Helper()
	cmd := exec.Command("git", args...)
	cmd.Dir = dir
	cmd.Env = append(os.Environ(), "GIT_OPTIONAL_LOCKS=0")
	if out, err := cmd.CombinedOutput(); err != nil {
		t.Fatalf("git %v: %v\n%s", args, err, out)
	}
}
