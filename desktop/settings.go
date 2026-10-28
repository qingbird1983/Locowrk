package desktop

import (
	"encoding/json"
	"os"
	"path/filepath"
	"time"
)

// settingsVersion is the schema version of the user settings file. Version 2
// adds A-track extension trust and workspace skill trust; version 1 files are
// read and migrated on the next save.
const settingsVersion = 2

// extensionRecord is one installed A-track extension and its trust state.
type extensionRecord struct {
	Path      string `json:"path"`
	SHA256    string `json:"sha256"`
	Enabled   bool   `json:"enabled"`
	TrustedAt string `json:"trustedAt,omitempty"`
}

// settings is the per-user shell state kept across restarts. It never holds
// credentials, session content or extension source.
type settings struct {
	Version      int                        `json:"version"`
	Plugins      map[string]bool            `json:"plugins"`
	Rail         string                     `json:"rail"` // "open" | "closed"
	Extensions   map[string]extensionRecord `json:"extensions,omitempty"`
	ProjectTrust map[string]string          `json:"projectTrust,omitempty"`
}

// DefaultSettingsPath returns the per-user settings file. An empty path means
// settings are not persisted (as in headless tests).
func DefaultSettingsPath() string {
	dir, err := os.UserConfigDir()
	if err != nil || dir == "" {
		return ""
	}
	return filepath.Join(dir, "Locowrk", "settings.json")
}

// defaultSettings enables every known plugin and opens the rail.
func defaultSettings(names []string) settings {
	plugins := make(map[string]bool, len(names))
	for _, name := range names {
		plugins[name] = true
	}
	return settings{
		Version:      settingsVersion,
		Plugins:      plugins,
		Rail:         "open",
		Extensions:   map[string]extensionRecord{},
		ProjectTrust: map[string]string{},
	}
}

// loadSettings reads path, falling back to defaults when the file is missing.
// A parse error also falls back to defaults and is returned for a diagnostic;
// the original file is left alone for a human to inspect.
func loadSettings(path string, names []string) (settings, error) {
	out := defaultSettings(names)
	if path == "" {
		return out, nil
	}
	data, err := os.ReadFile(path)
	if err != nil {
		if os.IsNotExist(err) {
			return out, nil
		}
		return out, err
	}
	var loaded settings
	if err := json.Unmarshal(data, &loaded); err != nil {
		return out, err
	}
	for _, name := range names {
		if on, ok := loaded.Plugins[name]; ok {
			out.Plugins[name] = on
		}
	}
	if loaded.Rail == "closed" || loaded.Rail == "open" {
		out.Rail = loaded.Rail
	}
	if loaded.Extensions != nil {
		out.Extensions = loaded.Extensions
	}
	if loaded.ProjectTrust != nil {
		out.ProjectTrust = loaded.ProjectTrust
	}
	return out, nil
}

// saveSettings writes s atomically: a temporary file in the same directory is
// renamed over path, so a crash never leaves a half-written file.
func saveSettings(path string, s settings) error {
	if path == "" {
		return nil
	}
	dir := filepath.Dir(path)
	if err := os.MkdirAll(dir, 0o755); err != nil {
		return err
	}
	data, err := json.MarshalIndent(s, "", "  ")
	if err != nil {
		return err
	}
	data = append(data, '\n')
	tmp, err := os.CreateTemp(dir, ".settings-*")
	if err != nil {
		return err
	}
	name := tmp.Name()
	if _, err := tmp.Write(data); err != nil {
		_ = tmp.Close()
		_ = os.Remove(name)
		return err
	}
	if err := tmp.Close(); err != nil {
		_ = os.Remove(name)
		return err
	}
	if err := os.Rename(name, path); err != nil {
		_ = os.Remove(name)
		return err
	}
	return nil
}

// Extension returns one stored extension record.
func (h *Host) Extension(name string) (path, sha string, enabled, trusted bool, ok bool) {
	h.mu.Lock()
	defer h.mu.Unlock()
	rec, found := h.settings.Extensions[name]
	return rec.Path, rec.SHA256, rec.Enabled, rec.TrustedAt != "", found
}

// SaveExtension writes one extension's trust and enable state.
func (h *Host) SaveExtension(name, path, sha string, enabled, trusted bool) error {
	h.mu.Lock()
	if h.settings.Extensions == nil {
		h.settings.Extensions = map[string]extensionRecord{}
	}
	rec := extensionRecord{Path: path, SHA256: sha, Enabled: enabled}
	if trusted {
		rec.TrustedAt = time.Now().UTC().Format(time.RFC3339)
	}
	h.settings.Extensions[name] = rec
	s := h.settings
	settingsPath := h.path
	h.mu.Unlock()
	return saveSettings(settingsPath, s)
}

// ProjectTrusted reports whether cwd is trusted for project skills/extensions.
func (h *Host) ProjectTrusted(cwd string) bool {
	cwd = filepath.Clean(cwd)
	h.mu.Lock()
	defer h.mu.Unlock()
	_, ok := h.settings.ProjectTrust[cwd]
	return ok
}

// SetProjectTrust records or removes a workspace trust decision.
func (h *Host) SetProjectTrust(cwd string, trusted bool) error {
	cwd = filepath.Clean(cwd)
	h.mu.Lock()
	if h.settings.ProjectTrust == nil {
		h.settings.ProjectTrust = map[string]string{}
	}
	if trusted {
		h.settings.ProjectTrust[cwd] = time.Now().UTC().Format(time.RFC3339)
	} else {
		delete(h.settings.ProjectTrust, cwd)
	}
	s := h.settings
	settingsPath := h.path
	h.mu.Unlock()
	return saveSettings(settingsPath, s)
}
