package desktop

import (
	"encoding/json"
	"os"
	"path/filepath"
)

// settingsVersion is the schema version of the user settings file. Unknown
// versions are read as best effort; the file only holds plugin toggles.
const settingsVersion = 1

// settings is the per-user shell state kept across restarts. It never holds
// credentials, paths or session content.
type settings struct {
	Version int             `json:"version"`
	Plugins map[string]bool `json:"plugins"`
	Rail    string          `json:"rail"` // "open" | "closed"
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
	return settings{Version: settingsVersion, Plugins: plugins, Rail: "open"}
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
