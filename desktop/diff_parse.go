package desktop

import (
	"path"
	"regexp"
	"strconv"
	"strings"
)

// hunkHeader matches @@ -old,count +new,count @@.
var hunkHeader = regexp.MustCompile(`^@@ -([0-9]+)(?:,([0-9]+))? \+([0-9]+)(?:,([0-9]+))? @@`)

// diffParser turns a unified diff into the structured DTO. It never trusts
// hunk counts for slicing: every line is classified, caps are enforced, and
// malformed headers become skipped files rather than a failed document.
type diffParser struct {
	doc        *DiffDoc
	cur        *DiffFile
	hunk       *DiffHunk
	oldNo      int
	newNo      int
	curAdds    int
	curDels    int
	size       int
	lines      int
	skipping   bool // inside a malformed hunk
	binaryData bool // inside a GIT binary patch
	stop       bool
}

// parseDiff parses patch into doc. doc must have a non-nil Files slice.
func parseDiff(patch []byte, doc *DiffDoc) {
	p := &diffParser{doc: doc}
	for _, raw := range strings.Split(string(patch), "\n") {
		if p.stop {
			break
		}
		line := strings.TrimSuffix(raw, "\r")
		p.size += len(line) + 1
		if p.size > maxDiffDocBytes {
			p.truncate("文档超过 4 MiB")
			break
		}
		p.line(line)
	}
	p.finishFile()
	if len(doc.Files) == 0 && doc.Hidden == 0 && doc.Skipped == 0 && !doc.Truncated {
		doc.State = "clean"
	}
	doc.Stats.Files = len(doc.Files)
}

func (p *diffParser) truncate(reason string) {
	if !p.doc.Truncated || p.doc.Reason == "" {
		p.doc.Truncated = true
		p.doc.Reason = reason
	}
}

func (p *diffParser) line(line string) {
	switch {
	case strings.HasPrefix(line, "diff --git "):
		p.finishFile()
		p.cur = &DiffFile{Status: "modified", Hunks: []DiffHunk{}}
		p.binaryData = false
		p.skipping = false
		p.oldNo, p.newNo = 0, 0
		p.applyGitLine(line)
		return
	case strings.HasPrefix(line, "new file mode "):
		p.ensureFile()
		p.cur.Status = "added"
		return
	case strings.HasPrefix(line, "deleted file mode "):
		p.ensureFile()
		p.cur.Status = "deleted"
		return
	case strings.HasPrefix(line, "old mode "), strings.HasPrefix(line, "new mode "):
		p.ensureFile()
		if p.cur.Status == "modified" {
			p.cur.Status = "mode"
		}
		return
	case strings.HasPrefix(line, "rename from "):
		p.ensureFile()
		p.cur.OldPath = unquoteGitPath(strings.TrimPrefix(line, "rename from "))
		return
	case strings.HasPrefix(line, "rename to "):
		p.ensureFile()
		p.cur.Path = unquoteGitPath(strings.TrimPrefix(line, "rename to "))
		return
	case strings.HasPrefix(line, "similarity index "), strings.HasPrefix(line, "index "),
		strings.HasPrefix(line, "dissimilarity index "):
		return
	case strings.HasPrefix(line, "--- "):
		p.ensureFile()
		p.cur.OldPath = fileHeaderPath(strings.TrimPrefix(line, "--- "))
		if p.cur.OldPath == "" {
			p.cur.Status = "added"
		}
		return
	case strings.HasPrefix(line, "+++ "):
		p.ensureFile()
		p.cur.Path = fileHeaderPath(strings.TrimPrefix(line, "+++ "))
		if p.cur.Path == "" {
			p.cur.Status = "deleted"
		}
		return
	case strings.HasPrefix(line, "@@"):
		p.startHunk(line)
		return
	case strings.HasPrefix(line, "Binary files "):
		p.ensureFile()
		p.cur.Binary = true
		p.cur.Status = "binary"
		p.binaryPaths(line)
		return
	case strings.HasPrefix(line, "GIT binary patch"):
		p.ensureFile()
		p.cur.Binary = true
		p.cur.Status = "binary"
		p.binaryData = true
		return
	case strings.HasPrefix(line, `\ No newline`):
		if p.hunk != nil {
			p.addLine("meta", 0, 0, line)
		}
		return
	}
	if p.binaryData || p.skipping {
		return
	}
	if p.hunk == nil {
		return
	}
	if line == "" {
		return
	}
	switch line[0] {
	case '+':
		p.addLine("add", 0, p.newNo, line[1:])
		p.newNo++
		p.curAdds++
	case '-':
		p.addLine("del", p.oldNo, 0, line[1:])
		p.oldNo++
		p.curDels++
	case ' ':
		p.addLine("context", p.oldNo, p.newNo, line[1:])
		p.oldNo++
		p.newNo++
	default:
		// Anything else inside a hunk is a malformed section; keep the lines
		// already parsed and wait for the next header.
		p.doc.Skipped++
		p.hunk = nil
		p.skipping = true
	}
}

func (p *diffParser) ensureFile() {
	if p.cur == nil {
		p.cur = &DiffFile{Status: "modified", Hunks: []DiffHunk{}}
	}
}

func (p *diffParser) startHunk(line string) {
	p.ensureFile()
	m := hunkHeader.FindStringSubmatch(line)
	if m == nil {
		p.doc.Skipped++
		p.hunk = nil
		p.skipping = true
		p.cur = nil // the file's path is known but its content is not usable
		return
	}
	oldStart, _ := strconv.Atoi(m[1])
	oldCount := 1
	if m[2] != "" {
		oldCount, _ = strconv.Atoi(m[2])
	}
	newStart, _ := strconv.Atoi(m[3])
	newCount := 1
	if m[4] != "" {
		newCount, _ = strconv.Atoi(m[4])
	}
	h := DiffHunk{
		Header:   line,
		OldStart: oldStart,
		OldCount: oldCount,
		NewStart: newStart,
		NewCount: newCount,
		Lines:    []DiffLine{},
	}
	p.cur.Hunks = append(p.cur.Hunks, h)
	p.hunk = &p.cur.Hunks[len(p.cur.Hunks)-1]
	p.oldNo, p.newNo = oldStart, newStart
	p.skipping = false
}

func (p *diffParser) addLine(kind string, oldNo, newNo int, text string) {
	if p.hunk == nil {
		return
	}
	if p.lines >= maxDiffLines {
		p.truncate("超过 20,000 行")
		p.stop = true
		return
	}
	text = clipRunes(strings.ToValidUTF8(text, "�"), maxDiffLineRunes)
	p.hunk.Lines = append(p.hunk.Lines, DiffLine{Kind: kind, OldNo: oldNo, NewNo: newNo, Text: text})
	p.lines++
	if kind != "meta" {
		p.doc.Stats.Lines = p.lines
	}
}

func (p *diffParser) finishFile() {
	defer func() {
		p.curAdds, p.curDels = 0, 0
	}()
	if p.cur == nil {
		return
	}
	if p.cur.Path == "" {
		p.cur.Path = p.cur.OldPath
	}
	if p.cur.Path == "" {
		p.doc.Skipped++
		p.cur = nil
		p.hunk = nil
		return
	}
	if sensitivePath(p.cur.Path) || sensitivePath(p.cur.OldPath) {
		p.doc.Hidden++
		p.cur = nil
		p.hunk = nil
		return
	}
	if len(p.doc.Files) >= maxDiffFiles {
		p.truncate("超过 200 个文件")
		p.stop = true
		p.cur = nil
		p.hunk = nil
		return
	}
	if p.cur.OldPath != "" && p.cur.OldPath != p.cur.Path && p.cur.Status == "modified" {
		p.cur.Status = "renamed"
	}
	if p.cur.Status == "" {
		p.cur.Status = "modified"
	}
	p.doc.Files = append(p.doc.Files, *p.cur)
	p.doc.Stats.Additions += p.curAdds
	p.doc.Stats.Deletions += p.curDels
	p.cur = nil
	p.hunk = nil
	p.curAdds, p.curDels = 0, 0
}

func (p *diffParser) applyGitLine(line string) {
	rest := strings.TrimPrefix(line, "diff --git ")
	oldPath, newPath := splitGitPaths(rest)
	if oldPath != "" {
		p.cur.OldPath = oldPath
	}
	if newPath != "" {
		p.cur.Path = newPath
	}
	if oldPath != "" && newPath == "" {
		p.cur.Path = oldPath
	}
}

func (p *diffParser) binaryPaths(line string) {
	rest := strings.TrimSuffix(strings.TrimPrefix(line, "Binary files "), " differ")
	if i := strings.LastIndex(rest, " and "); i >= 0 {
		a, b := rest[:i], rest[i+5:]
		if p.cur.Path == "" {
			p.cur.Path = unquoteGitPath(b)
		}
		if p.cur.OldPath == "" {
			p.cur.OldPath = unquoteGitPath(a)
		}
	}
}

// splitGitPaths parses "a/x b/x" including C-quoted paths with spaces.
func splitGitPaths(rest string) (oldPath, newPath string) {
	oldRaw, newRaw, ok := splitTwoGitPaths(rest)
	if !ok {
		// The ---/+++ lines will provide the paths.
		return "", ""
	}
	return trimGitPrefix(unquoteGitPath(oldRaw)), trimGitPrefix(unquoteGitPath(newRaw))
}

func splitTwoGitPaths(rest string) (string, string, bool) {
	rest = strings.TrimSpace(rest)
	if strings.HasPrefix(rest, `"`) {
		a, tail, ok := takeQuoted(rest)
		if !ok {
			return "", "", false
		}
		b, _, ok := takeQuoted(strings.TrimSpace(tail))
		if !ok {
			return "", "", false
		}
		return a, b, true
	}
	if i := strings.LastIndex(rest, " b/"); i >= 0 && strings.HasPrefix(rest, "a/") {
		return rest[:i], rest[i+1:], true
	}
	parts := strings.Fields(rest)
	if len(parts) >= 2 {
		return parts[0], parts[1], true
	}
	return "", "", false
}

func takeQuoted(s string) (string, string, bool) {
	if !strings.HasPrefix(s, `"`) {
		return "", "", false
	}
	escaped := false
	for i := 1; i < len(s); i++ {
		switch {
		case escaped:
			escaped = false
		case s[i] == '\\':
			escaped = true
		case s[i] == '"':
			return s[:i+1], s[i+1:], true
		}
	}
	return "", "", false
}

// fileHeaderPath reads the path after "---" or "+++". Git appends a tab and a
// timestamp on some paths; /dev/null means the file is added or deleted.
func fileHeaderPath(s string) string {
	s = strings.TrimSpace(s)
	if i := strings.IndexByte(s, '\t'); i >= 0 {
		s = s[:i]
	}
	if s == "/dev/null" {
		return ""
	}
	return trimGitPrefix(unquoteGitPath(s))
}

func trimGitPrefix(s string) string {
	if strings.HasPrefix(s, "a/") || strings.HasPrefix(s, "b/") {
		return s[2:]
	}
	return s
}

// unquoteGitPath removes git's C-style quotes when present.
func unquoteGitPath(s string) string {
	s = strings.TrimSpace(s)
	if len(s) >= 2 && strings.HasPrefix(s, `"`) && strings.HasSuffix(s, `"`) {
		if u, err := strconv.Unquote(s); err == nil {
			return u
		}
		return strings.Trim(s, `"`)
	}
	return s
}

// sensitivePath hides credentials and repository internals from the review.
func sensitivePath(p string) bool {
	p = strings.TrimPrefix(path.Clean(strings.ReplaceAll(p, `\`, "/")), "./")
	if p == "" || p == "." {
		return false
	}
	base := path.Base(p)
	lower := strings.ToLower(base)
	if base == ".env" || strings.HasPrefix(base, ".env.") ||
		strings.HasPrefix(lower, "id_rsa") || strings.HasPrefix(lower, "id_ed25519") ||
		strings.HasSuffix(lower, ".pem") || strings.HasSuffix(lower, ".key") {
		return true
	}
	for _, part := range strings.Split(p, "/") {
		if part == ".git" || part == ".ssh" {
			return true
		}
	}
	return false
}
