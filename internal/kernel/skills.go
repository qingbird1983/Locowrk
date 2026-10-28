package kernel

import (
	"path/filepath"
	"strings"

	"github.com/qingbird1983/locowrk/pi/coding"
)

// SkillInfo is one discovered Agent Skill for the desktop.
type SkillInfo struct {
	Name        string `json:"name"`
	Description string `json:"description"`
	Path        string `json:"path"`
	BaseDir     string `json:"baseDir,omitempty"`
	Source      string `json:"source"` // user|project
	Disabled    bool   `json:"disabled,omitempty"`
}

// SkillDiagnostic is one validation warning for a skill file.
type SkillDiagnostic struct {
	Type    string `json:"type"`
	Message string `json:"message"`
	Path    string `json:"path"`
}

// Skills discovers user skills and, when trusted, project skills under
// <cwd>/.pi/skills. It does not read skill bodies.
func (s *Service) Skills(cwd string, trusted bool) ([]SkillInfo, []SkillDiagnostic) {
	skills, diags := coding.LoadSkillsWithTrust(cwd, trusted)
	out := make([]SkillInfo, 0, len(skills))
	for _, sk := range skills {
		source := "user"
		if cwd != "" {
			rel, err := filepath.Rel(cwd, sk.BaseDir)
			if err == nil && rel != ".." && !strings.HasPrefix(rel, ".."+string(filepath.Separator)) {
				source = "project"
			}
		}
		out = append(out, SkillInfo{
			Name: sk.Name, Description: sk.Description,
			Path: sk.FilePath, BaseDir: sk.BaseDir, Source: source,
			Disabled: sk.DisableModelInvocation,
		})
	}
	diagOut := make([]SkillDiagnostic, 0, len(diags))
	for _, d := range diags {
		diagOut = append(diagOut, SkillDiagnostic{Type: d.Type, Message: d.Message, Path: d.Path})
	}
	return out, diagOut
}
