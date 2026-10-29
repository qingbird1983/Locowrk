// Package adapter runs A-track orchestration plugins as subprocesses and
// translates the wire protocol in proto/plugin-protocol.md into project events,
// tools and diagnostics.
//
// It deliberately does not import github.com/qingbird1983/locowrk/pi: tools are described
// with project DTOs and converted to upstream tools by internal/kernel
// (architecture/03 seam #12).
package adapter

import (
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"strings"
)

// ManifestFile is the manifest name inside a plugin directory.
const ManifestFile = "plugin.json"

// ProtocolVersion is the only subprocess protocol this adapter speaks.
const ProtocolVersion = 1

// ShimVersion is the shim bundled into A-track extension packages.
const ShimVersion = "locowrk-shim/1"

// CapabilityName is a capability declared by a manifest.
type CapabilityName string

// Capability names from proto/plugin-protocol.md §8.
const (
	CapFSRead   CapabilityName = "fs.read"
	CapFSWrite  CapabilityName = "fs.write"
	CapFSDelete CapabilityName = "fs.delete"
	CapExec     CapabilityName = "exec"
	CapNet      CapabilityName = "net"
)

var knownCapabilities = map[CapabilityName]bool{
	CapFSRead: true, CapFSWrite: true, CapFSDelete: true, CapExec: true, CapNet: true,
}

// Capability grants one capability within its scopes. An empty scope grants
// nothing: fail-closed is the default.
type Capability struct {
	Name  CapabilityName `json:"name"`
	Scope []string       `json:"scope"`
}

// Limits are the plugin's resource ceilings. Zero values take the defaults
// from the schema.
type Limits struct {
	MaxMemoryMiB            int `json:"maxMemoryMiB,omitempty"`
	MaxCPUSeconds           int `json:"maxCpuSeconds,omitempty"`
	MaxOutputBytesPerSecond int `json:"maxOutputBytesPerSecond,omitempty"`
}

// LongTask declares a method allowed to exceed the default RPC timeout.
type LongTask struct {
	Method    string `json:"method"`
	TimeoutMS int    `json:"timeoutMs"`
}

// Manifest is a validated plugin.json.
type Manifest struct {
	Name         string       `json:"name"`
	Version      string       `json:"version"`
	Protocol     int          `json:"protocol"`
	Description  string       `json:"description,omitempty"`
	Entrypoint   string       `json:"entrypoint"`
	Capabilities []Capability `json:"capabilities"`
	Limits       Limits       `json:"limits,omitempty"`
	LongTasks    []LongTask   `json:"longTasks,omitempty"`

	// Dir is the plugin directory. It is not part of the manifest file.
	Dir string `json:"-"`
}

// ErrInvalidManifest wraps every manifest validation failure.
var ErrInvalidManifest = errors.New("adapter: invalid manifest")

var (
	namePattern   = regexp.MustCompile(`^[a-z][a-z0-9-]{0,62}$`)
	verPattern    = regexp.MustCompile(`^\d+\.\d+\.\d+$`)
	toolPattern   = regexp.MustCompile(`^[a-z][a-z0-9_-]{0,62}$`)
	methodPattern = regexp.MustCompile(`^[a-z][a-z0-9_]{0,62}$`)
)

// Load reads and validates <dir>/plugin.json.
func Load(dir string) (*Manifest, error) {
	if dir == "" {
		return nil, fmt.Errorf("%w: empty directory", ErrInvalidManifest)
	}
	absDir, err := filepath.Abs(dir)
	if err != nil {
		return nil, fmt.Errorf("%w: %v", ErrInvalidManifest, err)
	}
	path := filepath.Join(absDir, ManifestFile)
	file, err := os.Open(path)
	if err != nil {
		return nil, fmt.Errorf("%w: opening %s: %v", ErrInvalidManifest, ManifestFile, err)
	}
	defer file.Close()

	dec := json.NewDecoder(file)
	dec.DisallowUnknownFields()
	var m Manifest
	if err := dec.Decode(&m); err != nil {
		return nil, fmt.Errorf("%w: decoding %s: %v", ErrInvalidManifest, ManifestFile, err)
	}
	m.Dir = absDir
	if err := m.validate(); err != nil {
		return nil, err
	}
	return &m, nil
}

func (m *Manifest) validate() error {
	if !namePattern.MatchString(m.Name) {
		return invalid("name %q must match %s", m.Name, namePattern)
	}
	if !verPattern.MatchString(m.Version) {
		return invalid("version %q must be semver without prerelease", m.Version)
	}
	if m.Protocol != ProtocolVersion {
		return fmt.Errorf("%w: protocol %d, want %d", ErrIncompatibleVersion, m.Protocol, ProtocolVersion)
	}
	if err := validateEntrypoint(m.Entrypoint); err != nil {
		return err
	}
	if len(m.Capabilities) > 16 {
		return invalid("too many capabilities: %d", len(m.Capabilities))
	}
	seen := map[CapabilityName]bool{}
	for i := range m.Capabilities {
		c := &m.Capabilities[i]
		if !knownCapabilities[c.Name] {
			return invalid("unknown capability %q", c.Name)
		}
		if seen[c.Name] {
			return invalid("duplicate capability %q", c.Name)
		}
		seen[c.Name] = true
		if len(c.Scope) == 0 {
			return invalid("capability %q has no scope (fail-closed)", c.Name)
		}
		if len(c.Scope) > 64 {
			return invalid("capability %q has too many scopes", c.Name)
		}
		for _, scope := range c.Scope {
			if scope == "" || len(scope) > 260 {
				return invalid("capability %q has an empty or oversized scope", c.Name)
			}
			if (c.Name == CapFSWrite || c.Name == CapFSDelete) && wholeTree(scope) {
				return invalid("capability %q scope %q would grant the whole tree", c.Name, scope)
			}
		}
	}
	if m.Limits.MaxMemoryMiB != 0 && (m.Limits.MaxMemoryMiB < 64 || m.Limits.MaxMemoryMiB > 4096) {
		return invalid("limits.maxMemoryMiB out of range")
	}
	if m.Limits.MaxCPUSeconds != 0 && m.Limits.MaxCPUSeconds < 10 {
		return invalid("limits.maxCpuSeconds out of range")
	}
	if m.Limits.MaxOutputBytesPerSecond != 0 && m.Limits.MaxOutputBytesPerSecond < 65536 {
		return invalid("limits.maxOutputBytesPerSecond out of range")
	}
	if len(m.LongTasks) > 8 {
		return invalid("too many longTasks")
	}
	for _, lt := range m.LongTasks {
		if !methodPattern.MatchString(lt.Method) {
			return invalid("longTasks method %q is invalid", lt.Method)
		}
		if lt.TimeoutMS < 1000 || lt.TimeoutMS > 600000 {
			return invalid("longTasks timeoutMs out of range: %d", lt.TimeoutMS)
		}
	}
	return nil
}

func validateEntrypoint(entry string) error {
	if entry == "" || len(entry) > 260 {
		return invalid("entrypoint is empty or oversized")
	}
	if strings.Contains(entry, "..") {
		return invalid("entrypoint must not contain ..")
	}
	if strings.ContainsAny(entry, ":\\") {
		return invalid("entrypoint must be a relative path with forward slashes")
	}
	if strings.HasPrefix(entry, "/") {
		return invalid("entrypoint must be relative")
	}
	return nil
}

func invalid(format string, args ...any) error {
	return fmt.Errorf("%w: %s", ErrInvalidManifest, fmt.Sprintf(format, args...))
}

// wholeTree reports whether a scope would grant an unbounded subtree.
func wholeTree(scope string) bool {
	cleaned := strings.TrimPrefix(strings.TrimSpace(scope), "./")
	switch cleaned {
	case "*", "**", "**/*", "/**", "/**/*":
		return true
	}
	return false
}

// EntryPath resolves the executable inside the plugin directory and refuses
// anything that escapes it.
func (m *Manifest) EntryPath() (string, error) {
	cleaned := filepath.Clean(filepath.Join(m.Dir, filepath.FromSlash(m.Entrypoint)))
	rel, err := filepath.Rel(m.Dir, cleaned)
	if err != nil || rel == ".." || strings.HasPrefix(rel, ".."+string(filepath.Separator)) {
		return "", invalid("entrypoint escapes the plugin directory")
	}
	return cleaned, nil
}

// Capability returns the declared capability, if any.
func (m *Manifest) Capability(name CapabilityName) (Capability, bool) {
	for _, c := range m.Capabilities {
		if c.Name == name {
			return c, true
		}
	}
	return Capability{}, false
}

// LongTaskTimeout returns the declared timeout for a method, if any.
func (m *Manifest) LongTaskTimeout(method string) (int, bool) {
	for _, lt := range m.LongTasks {
		if lt.Method == method {
			return lt.TimeoutMS, true
		}
	}
	return 0, false
}
