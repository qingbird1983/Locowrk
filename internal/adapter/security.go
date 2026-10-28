package adapter

import (
	"fmt"
	"os"
	"path"
	"path/filepath"
	"regexp"
	"strings"
	"sync"
	"time"
)

// AuditRecord is one host-side capability decision. Security decisions are kept
// apart from compatibility diagnostics (proto §7, ADR-0003).
type AuditRecord struct {
	At         time.Time `json:"at"`
	Plugin     string    `json:"plugin"`
	Capability string    `json:"capability"`
	Target     string    `json:"target"`
	Allowed    bool      `json:"allowed"`
	Reason     string    `json:"reason,omitempty"`
}

// splitScopes is the workspace-relative form used for glob matching.
func relativeTarget(cwd, target string) (string, bool) {
	if target == "" {
		return "", false
	}
	abs := target
	if !filepath.IsAbs(abs) {
		abs = filepath.Join(cwd, target)
	}
	abs = filepath.Clean(abs)
	rel, err := filepath.Rel(cwd, abs)
	if err != nil || rel == ".." || strings.HasPrefix(rel, ".."+string(filepath.Separator)) {
		return "", false
	}
	return filepath.ToSlash(rel), true
}

// matchesScope reports whether rel matches one manifest glob. Globs are
// workspace-relative and use path.Match per segment with ** meaning "any depth".
func matchesScope(scope, rel string) bool {
	scope = strings.TrimPrefix(strings.TrimSpace(scope), "./")
	scope = filepath.ToSlash(scope)
	if scope == "" {
		return false
	}
	return matchSegments(strings.Split(scope, "/"), strings.Split(rel, "/"))
}

func matchSegments(pattern, target []string) bool {
	for len(pattern) > 0 {
		switch pattern[0] {
		case "**":
			if len(pattern) == 1 {
				return true
			}
			for i := 0; i <= len(target); i++ {
				if matchSegments(pattern[1:], target[i:]) {
					return true
				}
			}
			return false
		default:
			if len(target) == 0 {
				return false
			}
			ok, err := path.Match(pattern[0], target[0])
			if err != nil || !ok {
				return false
			}
			pattern, target = pattern[1:], target[1:]
		}
	}
	return len(target) == 0
}

// hardDenied reports paths that no capability may grant (proto §8).
func hardDenied(rel string) bool {
	base := path.Base(rel)
	if strings.HasPrefix(base, ".env") {
		return true
	}
	segments := strings.Split(rel, "/")
	for _, s := range segments {
		switch s {
		case ".git", ".ssh":
			return true
		}
	}
	return false
}

// checkCapability enforces the manifest and records the decision. It is
// fail-closed: an undeclared capability, an out-of-scope path or a hard-denied
// path is refused.
func (a *Adapter) checkCapability(name CapabilityName, cwd, target string) error {
	record := AuditRecord{
		At:         time.Now().UTC(),
		Plugin:     a.pluginName,
		Capability: string(name),
		Target:     target,
	}
	rel, ok := relativeTarget(cwd, target)
	if !ok {
		record.Reason = "outside workspace"
		a.addAudit(record)
		return fmt.Errorf("%w: %s outside workspace", ErrPermissionDenied, target)
	}
	if hardDenied(rel) {
		record.Reason = "hard denied path"
		a.addAudit(record)
		return fmt.Errorf("%w: %s is hard denied", ErrPermissionDenied, target)
	}
	capability, declared := a.manifest.Capability(name)
	if !declared {
		record.Reason = "capability not declared"
		a.addAudit(record)
		return fmt.Errorf("%w: %s not declared", ErrPermissionDenied, name)
	}
	if len(a.accepted) > 0 && !a.accepted[name] {
		record.Reason = "capability not accepted by plugin"
		a.addAudit(record)
		return fmt.Errorf("%w: %s was not accepted during initialize", ErrPermissionDenied, name)
	}
	for _, scope := range capability.Scope {
		if matchesScope(scope, rel) {
			record.Allowed = true
			a.addAudit(record)
			return nil
		}
	}
	record.Reason = "out of declared scope"
	a.addAudit(record)
	return fmt.Errorf("%w: %s not in scope of %s", ErrPermissionDenied, target, name)
}

func (a *Adapter) addAudit(r AuditRecord) {
	a.mu.Lock()
	a.audit = append(a.audit, r)
	if len(a.audit) > 256 {
		a.audit = a.audit[len(a.audit)-256:]
	}
	a.mu.Unlock()
}

// fsWrite performs a host-managed write after the capability check.
func (a *Adapter) fsWrite(cwd, target string, data []byte) (int, error) {
	if err := a.checkCapability(CapFSWrite, cwd, target); err != nil {
		return 0, err
	}
	abs := target
	if !filepath.IsAbs(abs) {
		abs = filepath.Join(cwd, target)
	}
	if err := os.WriteFile(abs, data, 0o644); err != nil {
		return 0, fmt.Errorf("adapter: writing %s: %w", target, err)
	}
	return len(data), nil
}

// diagnosticStore deduplicates per plugin+member+category+code (proto §7).
type diagnosticStore struct {
	mu   sync.Mutex
	seen map[string]*Diagnostic
	all  []*Diagnostic
}

func newDiagnosticStore() *diagnosticStore {
	return &diagnosticStore{seen: map[string]*Diagnostic{}}
}

// add records a diagnostic and reports whether it is the first occurrence.
func (s *diagnosticStore) add(d Diagnostic) (Diagnostic, bool) {
	s.mu.Lock()
	defer s.mu.Unlock()
	key := d.Plugin + "\x00" + d.Member + "\x00" + d.Category + "\x00" + d.Code
	if existing, ok := s.seen[key]; ok {
		existing.Count++
		return *existing, false
	}
	d.Count = 1
	d.FirstAt = time.Now().UTC()
	entry := d
	s.seen[key] = &entry
	s.all = append(s.all, &entry)
	return entry, true
}

// snapshot returns a copy ordered by first occurrence.
func (s *diagnosticStore) snapshot() []Diagnostic {
	s.mu.Lock()
	defer s.mu.Unlock()
	out := make([]Diagnostic, 0, len(s.all))
	for _, d := range s.all {
		out = append(out, *d)
	}
	return out
}

var secretPattern = regexp.MustCompile(`(?i)(sk-[a-z0-9]{8,}|(?:api[_-]?key|token|authorization)\s*[:=]\s*)\S+`)

// redact removes obvious credentials from plugin stderr before it is stored.
func redact(s string) string {
	return secretPattern.ReplaceAllString(s, "$1[redacted]")
}
