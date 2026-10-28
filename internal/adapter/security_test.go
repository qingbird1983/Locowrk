package adapter

import (
	"context"
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestPermissionDeniedIsSeparateFromDiagnostics(t *testing.T) {
	f := newFixture(t, "supported", nil, "LOCOWRK_MOCK_FS_PATH=secrets.txt")
	ctx := context.Background()
	if err := f.adapter.SendCommand(ctx, "mock_denied", ""); err != nil {
		t.Fatalf("SendCommand: %v", err)
	}
	result, err := f.adapter.InvokeTool(ctx, "mock_host_error", json.RawMessage(`{}`))
	if err != nil {
		t.Fatalf("InvokeTool: %v", err)
	}
	if got := result.Text(); got != "4402" {
		t.Fatalf("host error = %q, want 4402", got)
	}
	audit := f.adapter.Audit()
	if len(audit) != 1 || audit[0].Allowed || audit[0].Capability != "fs.write" {
		t.Fatalf("audit = %+v", audit)
	}
	if diagnostics := f.adapter.Diagnostics(); len(diagnostics) != 0 {
		t.Fatalf("permission denial leaked into diagnostics: %+v", diagnostics)
	}
	if _, err := os.Stat(filepath.Join(f.dir, "secrets.txt")); !os.IsNotExist(err) {
		t.Fatalf("denied write created a file (stat err = %v)", err)
	}
}

func TestAllowedWrite(t *testing.T) {
	f := newFixture(t, "supported", nil, "LOCOWRK_MOCK_FS_PATH=notes/ok.txt")
	if err := os.MkdirAll(filepath.Join(f.dir, "notes"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := f.adapter.SendCommand(context.Background(), "mock_denied", ""); err != nil {
		t.Fatalf("SendCommand: %v", err)
	}
	data, err := os.ReadFile(filepath.Join(f.dir, "notes", "ok.txt"))
	if err != nil {
		t.Fatalf("reading written file: %v", err)
	}
	if string(data) != "fixture" {
		t.Fatalf("file = %q", data)
	}
	audit := f.adapter.Audit()
	if len(audit) != 1 || !audit[0].Allowed {
		t.Fatalf("audit = %+v", audit)
	}
}

func TestManifestValidation(t *testing.T) {
	write := func(t *testing.T, body string) string {
		dir := t.TempDir()
		if err := os.WriteFile(filepath.Join(dir, ManifestFile), []byte(body), 0o644); err != nil {
			t.Fatal(err)
		}
		return dir
	}
	valid := `{"name":"demo","version":"0.1.0","protocol":1,"entrypoint":"bin/demo","capabilities":[{"name":"fs.read","scope":["src/**"]}]}`
	if _, err := Load(write(t, valid)); err != nil {
		t.Fatalf("valid manifest rejected: %v", err)
	}

	cases := map[string]string{
		"bad name":       `{"name":"Demo","version":"0.1.0","protocol":1,"entrypoint":"bin/demo","capabilities":[{"name":"fs.read","scope":["src/**"]}]}`,
		"bad version":    `{"name":"demo","version":"0.1.0-beta","protocol":1,"entrypoint":"bin/demo","capabilities":[{"name":"fs.read","scope":["src/**"]}]}`,
		"bad protocol":   `{"name":"demo","version":"0.1.0","protocol":2,"entrypoint":"bin/demo","capabilities":[{"name":"fs.read","scope":["src/**"]}]}`,
		"absolute entry": `{"name":"demo","version":"0.1.0","protocol":1,"entrypoint":"/bin/demo","capabilities":[{"name":"fs.read","scope":["src/**"]}]}`,
		"escaping entry": `{"name":"demo","version":"0.1.0","protocol":1,"entrypoint":"../demo","capabilities":[{"name":"fs.read","scope":["src/**"]}]}`,
		"empty scope":    `{"name":"demo","version":"0.1.0","protocol":1,"entrypoint":"bin/demo","capabilities":[{"name":"fs.read","scope":[]}]}`,
		"whole tree":     `{"name":"demo","version":"0.1.0","protocol":1,"entrypoint":"bin/demo","capabilities":[{"name":"fs.write","scope":["**"]}]}`,
		"duplicate cap":  `{"name":"demo","version":"0.1.0","protocol":1,"entrypoint":"bin/demo","capabilities":[{"name":"fs.read","scope":["src/**"]},{"name":"fs.read","scope":["lib/**"]}]}`,
		"unknown field":  `{"name":"demo","version":"0.1.0","protocol":1,"entrypoint":"bin/demo","capabilities":[],"extra":true}`,
	}
	for name, body := range cases {
		t.Run(name, func(t *testing.T) {
			if _, err := Load(write(t, body)); err == nil {
				t.Fatal("manifest accepted, want error")
			}
		})
	}
}

func TestScopeMatching(t *testing.T) {
	cases := []struct {
		scope, target string
		want          bool
	}{
		{"notes/**", "notes/a.txt", true},
		{"notes/**", "notes/deep/a.txt", true},
		{"notes/*.md", "notes/a.md", true},
		{"notes/*.md", "notes/a.txt", false},
		{"src/**/*.go", "src/a/b/c.go", true},
		{"src/*.go", "src/b/c.go", false},
		{"**", "anything", true},
	}
	for _, c := range cases {
		if got := matchesScope(c.scope, c.target); got != c.want {
			t.Fatalf("matchesScope(%q, %q) = %v, want %v", c.scope, c.target, got, c.want)
		}
	}
	if !hardDenied(".env") || !hardDenied("a/.git/config") || !hardDenied(".ssh/id_rsa") {
		t.Fatal("hard-deny paths not recognised")
	}
	if hardDenied("src/main.go") {
		t.Fatal("ordinary path treated as hard denied")
	}
}

func TestRedact(t *testing.T) {
	in := "token=supersecret sk-abcdefgh12345"
	out := redact(in)
	if strings.Contains(out, "supersecret") || strings.Contains(out, "abcdefgh12345") {
		t.Fatalf("redaction left a secret: %s", out)
	}
}
