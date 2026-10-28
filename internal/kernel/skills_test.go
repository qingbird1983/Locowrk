package kernel

import (
	"os"
	"path/filepath"
	"testing"
)

func writeSkill(t *testing.T, dir, name, desc string) {
	t.Helper()
	if err := os.MkdirAll(dir, 0o755); err != nil {
		t.Fatal(err)
	}
	body := "---\nname: " + name + "\ndescription: " + desc + "\n---\n\nbody\n"
	if err := os.WriteFile(filepath.Join(dir, "SKILL.md"), []byte(body), 0o644); err != nil {
		t.Fatal(err)
	}
}

func TestSkillsTrustGate(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)
	t.Setenv("USERPROFILE", home)
	cwd := t.TempDir()
	writeSkill(t, filepath.Join(home, ".pi", "agent", "skills", "user-skill"), "user-skill", "user skill")
	writeSkill(t, filepath.Join(cwd, ".pi", "skills", "project-skill"), "project-skill", "project skill")

	svc, err := New(Options{})
	if err != nil {
		t.Fatal(err)
	}
	skills, diags := svc.Skills(cwd, false)
	if len(diags) != 0 {
		t.Fatalf("diagnostics %+v", diags)
	}
	if len(skills) != 1 || skills[0].Name != "user-skill" || skills[0].Source != "user" {
		t.Fatalf("untrusted skills %+v", skills)
	}
	skills, _ = svc.Skills(cwd, true)
	if len(skills) != 2 {
		t.Fatalf("trusted skills %+v", skills)
	}
	found := false
	for _, s := range skills {
		if s.Name == "project-skill" && s.Source == "project" {
			found = true
		}
	}
	if !found {
		t.Fatalf("project skill missing: %+v", skills)
	}
}
