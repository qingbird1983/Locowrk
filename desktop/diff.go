package desktop

import (
	"bytes"
	"context"
	"errors"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"time"

	"github.com/egoist/mygo"
	"github.com/egoist/mygo/transfer"
)

const (
	maxDiffFiles      = 200
	maxDiffLines      = 20000
	maxDiffBytes      = 8 << 20
	maxDiffDocBytes   = 4 << 20
	maxDiffLineRunes  = 2000
	maxDiffPathRunes  = 300
	maxDiffErrorRunes = 300
	diffTimeout       = 5 * time.Second
)

// DiffReload asks a diff page to reload its document without losing the
// current view mode or scroll anchor.
var DiffReload = mygo.NewEvent[struct{}]("diff-reload")

// DiffDoc is one workspace diff as the review page renders it.
type DiffDoc struct {
	State     string     `json:"state"` // ok|clean|notRepo|noGit|timeout|error
	Message   string     `json:"message,omitempty"`
	Workspace string     `json:"workspace"`
	Revision  string     `json:"revision,omitempty"`
	Focus     string     `json:"focus,omitempty"`
	Hidden    int        `json:"hidden"`
	Skipped   int        `json:"skipped"`
	Truncated bool       `json:"truncated"`
	Reason    string     `json:"reason,omitempty"`
	Stats     DiffStats  `json:"stats"`
	Files     []DiffFile `json:"files"`
}

// DiffStats summarizes a DiffDoc.
type DiffStats struct {
	Files     int `json:"files"`
	Additions int `json:"additions"`
	Deletions int `json:"deletions"`
	Lines     int `json:"lines"`
}

// DiffFile is one changed file.
type DiffFile struct {
	Path    string     `json:"path"`
	OldPath string     `json:"oldPath,omitempty"`
	Status  string     `json:"status"` // modified|added|deleted|renamed|binary|mode
	Binary  bool       `json:"binary,omitempty"`
	Hunks   []DiffHunk `json:"hunks"`
}

// DiffHunk is one @@ section.
type DiffHunk struct {
	Header   string     `json:"header"`
	OldStart int        `json:"oldStart"`
	OldCount int        `json:"oldCount"`
	NewStart int        `json:"newStart"`
	NewCount int        `json:"newCount"`
	Lines    []DiffLine `json:"lines"`
}

// DiffLine is one context, added or removed line.
type DiffLine struct {
	Kind  string `json:"kind"` // context|add|del|meta
	OldNo int    `json:"oldNo,omitempty"`
	NewNo int    `json:"newNo,omitempty"`
	Text  string `json:"text"`
}

// gitRunner runs one git command in cwd and returns stdout, stderr and whether
// stdout hit the byte cap. It exists so tests can inject failures and timeouts.
type gitRunner func(ctx context.Context, cwd string, args ...string) (stdout, stderr []byte, limited bool, err error)

// DiffService loads the workspace diff for a tab. It is bound to the page as
// "Diff"; it reads only tracked, unstaged changes.
type DiffService struct {
	host *Host
	cwd  string
	git  gitRunner
}

// NewDiffService wires the service to the host and workspace. A nil host is
// allowed for tests that only exercise parsing.
func NewDiffService(host *Host, cwd string) *DiffService {
	return &DiffService{host: host, cwd: cwd, git: execGit}
}

// DiffBindings is the page-facing surface of DiffService.
type DiffBindings struct {
	svc *DiffService
}

// NewDiffBindings wraps a service for mygo.BindAs("Diff", ...).
func NewDiffBindings(svc *DiffService) *DiffBindings { return &DiffBindings{svc: svc} }

// Load returns the diff of a tab's workspace. It is called by the diff page
// after it loads; expected git failures are states in DiffDoc, not errors.
func (b *DiffBindings) Load(tabID string) (DiffDoc, error) {
	return b.svc.Load(tabID)
}

// CopyPath copies one file path to the host clipboard.
func (b *DiffBindings) CopyPath(tabID, path string) error {
	return b.svc.CopyPath(tabID, path)
}

// Load reads the diff for tabID.
func (s *DiffService) Load(tabID string) (DiffDoc, error) {
	info, ok := s.host.TabData(TabID(tabID))
	if !ok {
		return DiffDoc{}, ErrTabClosed
	}
	ctx, cancel := context.WithTimeout(context.Background(), diffTimeout)
	defer cancel()
	return s.load(ctx, info.Data), nil
}

// CopyPath puts path on the clipboard after checking the tab is open.
func (s *DiffService) CopyPath(tabID, path string) error {
	if _, ok := s.host.TabData(TabID(tabID)); !ok {
		return ErrTabClosed
	}
	path = strings.TrimSpace(path)
	if path == "" {
		return errors.New("desktop: empty path")
	}
	path = clipRunes(path, maxDiffPathRunes)
	if err := mygo.Clipboard.Write(transfer.TextData(path)); err != nil {
		return fmt.Errorf("copying path: %w", err)
	}
	return nil
}

// load runs git and parses the result. Expected failures become states.
func (s *DiffService) load(ctx context.Context, focus string) DiffDoc {
	doc := DiffDoc{State: "ok", Workspace: filepath.Base(filepath.Clean(s.cwd)), Files: []DiffFile{}}
	if s.cwd == "" {
		doc.State, doc.Message = "notRepo", "没有工作区"
		return doc
	}
	if _, err := exec.LookPath("git"); err != nil {
		doc.State, doc.Message = "noGit", "找不到 git"
		return doc
	}

	out, _, _, err := s.git(ctx, s.cwd, "rev-parse", "--is-inside-work-tree")
	if err != nil {
		if ctx.Err() != nil {
			doc.State, doc.Message = "timeout", "读取仓库超时"
			return doc
		}
		doc.State, doc.Message = "notRepo", "不是 git 仓库"
		return doc
	}
	if strings.TrimSpace(string(out)) != "true" {
		doc.State, doc.Message = "notRepo", "不是 git 仓库"
		return doc
	}
	if rev, _, _, err := s.git(ctx, s.cwd, "rev-parse", "--short", "HEAD"); err == nil {
		doc.Revision = strings.TrimSpace(string(rev))
	}

	patch, stderr, limited, err := s.git(ctx, s.cwd,
		"-c", "core.quotepath=false", "diff", "--no-color", "--no-ext-diff",
		"--no-textconv", "--unified=3", "--", ".")
	if ctx.Err() != nil {
		doc.State, doc.Message = "timeout", "读取 diff 超时"
		return doc
	}
	if err != nil {
		doc.State = "error"
		doc.Message = clipRunes(strings.TrimSpace(string(stderr)), maxDiffErrorRunes)
		if doc.Message == "" {
			doc.Message = "git diff 失败"
		}
		return doc
	}
	if len(bytes.TrimSpace(patch)) == 0 {
		doc.State = "clean"
		return doc
	}
	parseDiff(patch, &doc)
	doc.Truncated = doc.Truncated || limited
	if doc.Truncated && doc.Reason == "" {
		doc.Reason = "输出超过 8 MiB"
	}
	if doc.State == "ok" && len(doc.Files) == 0 && doc.Hidden == 0 && doc.Skipped == 0 {
		doc.State = "clean"
	}
	doc.Focus = focusPath(doc.Files, focus)
	return doc
}

// execGit runs git with a fixed, filtered environment and a capped stdout.
func execGit(ctx context.Context, cwd string, args ...string) ([]byte, []byte, bool, error) {
	cmd := exec.CommandContext(ctx, "git", args...)
	cmd.Dir = cwd
	cmd.Env = gitEnv()
	var out capBuffer
	out.limit = maxDiffBytes
	var errBuf capBuffer
	errBuf.limit = 4096
	cmd.Stdout = &out
	cmd.Stderr = &errBuf
	err := cmd.Run()
	return out.Bytes(), errBuf.Bytes(), out.truncated, err
}

// gitEnv passes the user's environment but removes diff overrides and forces
// non-interactive, read-only git.
func gitEnv() []string {
	base := os.Environ()
	out := make([]string, 0, len(base)+3)
	for _, e := range base {
		key := e
		if i := strings.IndexByte(e, '='); i >= 0 {
			key = e[:i]
		}
		switch strings.ToUpper(key) {
		case "GIT_EXTERNAL_DIFF", "GIT_DIFF_OPTS", "GIT_PAGER", "GIT_TERMINAL_PROMPT", "GIT_OPTIONAL_LOCKS":
			continue
		}
		out = append(out, e)
	}
	return append(out, "GIT_PAGER=cat", "GIT_TERMINAL_PROMPT=0", "GIT_OPTIONAL_LOCKS=0")
}

// capBuffer keeps at most limit bytes and reports truncation without failing
// the child's writes.
type capBuffer struct {
	buf       bytes.Buffer
	limit     int
	truncated bool
}

func (b *capBuffer) Write(p []byte) (int, error) {
	n := len(p)
	room := b.limit - b.buf.Len()
	if room <= 0 {
		b.truncated = true
		return n, nil
	}
	if len(p) > room {
		b.truncated = true
		p = p[:room]
	}
	_, _ = b.buf.Write(p)
	return n, nil
}

func (b *capBuffer) Bytes() []byte { return b.buf.Bytes() }

// clipRunes limits s to n runes, marking the cut.
func clipRunes(s string, n int) string {
	r := []rune(s)
	if len(r) <= n {
		return s
	}
	return string(r[:n]) + "…"
}

// focusPath finds the relative path a tab opened for.
func focusPath(files []DiffFile, want string) string {
	want = strings.TrimPrefix(filepath.ToSlash(filepath.Clean(want)), "./")
	if want == "" || want == "." {
		return ""
	}
	for _, f := range files {
		if f.Path == want || strings.HasSuffix(f.Path, "/"+want) {
			return f.Path
		}
	}
	return ""
}

// relativeWorkspacePath turns a tool argument into a workspace-relative slash
// path, ignoring paths outside the workspace.
func relativeWorkspacePath(cwd, raw string) string {
	raw = strings.TrimSpace(strings.Trim(raw, `"`))
	if raw == "" || cwd == "" {
		return ""
	}
	p := filepath.FromSlash(raw)
	if filepath.IsAbs(p) {
		rel, err := filepath.Rel(cwd, p)
		if err != nil {
			return ""
		}
		p = rel
	}
	p = filepath.Clean(p)
	if p == "." || p == ".." || strings.HasPrefix(p, ".."+string(filepath.Separator)) {
		return ""
	}
	return filepath.ToSlash(p)
}
