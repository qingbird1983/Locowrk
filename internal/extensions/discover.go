// Package extensions loads A-track orchestration extensions, owns their
// subprocesses and exposes tools, commands, diagnostics and notices to the
// desktop through DTOs.
package extensions

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"

	"github.com/qingbird1983/locowrk/internal/adapter"
)

// Source is where an extension directory came from.
type Source string

// Extension sources.
const (
	SourceUser    Source = "user"
	SourceProject Source = "project"
	SourceDev     Source = "dev"
)

// Store persists trust and enable state. desktop.Host implements it.
type Store interface {
	Extension(name string) (path, sha string, enabled, trusted bool, ok bool)
	SaveExtension(name, path, sha string, enabled, trusted bool) error
	ProjectTrusted(cwd string) bool
	SetProjectTrust(cwd string, trusted bool) error
}

// Candidate is one discovered extension before it is started.
type Candidate struct {
	Name     string
	Version  string
	Dir      string
	Source   Source
	SHA      string
	Manifest *adapter.Manifest
	Enabled  bool
	Trusted  bool
	State    string // ready|untrusted|disabled|failed|duplicate
	Reason   string
}

// Discover scans the user, project and extra directories. Invalid manifests
// become candidates in the failed state so the UI can show why.
func Discover(cwd string, extraDirs []string) []*Candidate {
	type root struct {
		dir    string
		source Source
	}
	roots := []root{}
	if dir, err := os.UserConfigDir(); err == nil && dir != "" {
		roots = append(roots, root{filepath.Join(dir, "Locowrk", "extensions"), SourceUser})
	}
	if cwd != "" {
		roots = append(roots, root{filepath.Join(cwd, ".pi", "extensions"), SourceProject})
	}
	for _, dir := range extraDirs {
		if dir != "" {
			roots = append(roots, root{dir, SourceDev})
		}
	}

	seen := map[string]bool{}
	var out []*Candidate
	for _, r := range roots {
		entries, err := os.ReadDir(r.dir)
		if err != nil {
			continue
		}
		for _, entry := range entries {
			if !entry.IsDir() || strings.HasPrefix(entry.Name(), ".") {
				continue
			}
			dir := filepath.Join(r.dir, entry.Name())
			m, err := adapter.Load(dir)
			if err != nil {
				out = append(out, &Candidate{
					Name:   entry.Name(),
					Dir:    dir,
					Source: r.source,
					State:  "failed",
					Reason: err.Error(),
				})
				continue
			}
			if seen[m.Name] {
				out = append(out, &Candidate{
					Name: m.Name, Version: m.Version, Dir: dir, Source: r.source,
					State: "duplicate", Reason: "another source already provides " + m.Name,
				})
				continue
			}
			seen[m.Name] = true
			sha, err := bundleHash(dir, m)
			if err != nil {
				out = append(out, &Candidate{
					Name: m.Name, Version: m.Version, Dir: dir, Source: r.source,
					State: "failed", Reason: err.Error(),
				})
				continue
			}
			out = append(out, &Candidate{
				Name: m.Name, Version: m.Version, Dir: dir, Source: r.source,
				SHA: sha, Manifest: m, State: "ready",
			})
		}
	}
	sort.Slice(out, func(i, j int) bool { return out[i].Name < out[j].Name })
	return out
}

// bundleHash hashes the manifest and entrypoint so a changed capability list
// or bundle invalidates trust.
func bundleHash(dir string, m *adapter.Manifest) (string, error) {
	entry, err := m.EntryPath()
	if err != nil {
		return "", err
	}
	data, err := os.ReadFile(entry)
	if err != nil {
		return "", fmt.Errorf("reading entrypoint: %w", err)
	}
	manifest, err := os.ReadFile(filepath.Join(dir, adapter.ManifestFile))
	if err != nil {
		return "", fmt.Errorf("reading manifest: %w", err)
	}
	h := sha256.New()
	_, _ = h.Write(manifest)
	_, _ = h.Write([]byte{0})
	_, _ = h.Write(data)
	return hex.EncodeToString(h.Sum(nil)), nil
}

// ExtensionInfo is the UI DTO for one extension.
type ExtensionInfo struct {
	Name        string
	Version     string
	Source      Source
	State       string
	Reason      string
	Enabled     bool
	Trusted     bool
	SHA         string
	Tools       int
	Commands    int
	Diagnostics []adapter.Diagnostic
}

// Notice is one ui_notify message.
type Notice struct {
	Plugin  string
	Message string
	Type    string
	At      string
}

// CommandInfo is one extension command.
type CommandInfo struct {
	Name        string
	Description string
	Plugin      string
}

// MarshalJSON keeps the UI DTO stable for logs and tests.
func (c Candidate) MarshalJSON() ([]byte, error) {
	type alias Candidate
	return json.Marshal(alias(c))
}
