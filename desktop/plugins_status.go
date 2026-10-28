package desktop

import (
	"sync"
	"time"

	"github.com/egoist/mygo/ui"

	"github.com/qingbird1983/locowrk/internal/bridge"
	"github.com/qingbird1983/locowrk/internal/eventbus"
)

// statusPlugin is the native A-track extensions + skills panel.
type statusPlugin struct {
	mu       sync.Mutex
	last     string
	skills   bridge.SkillsView
	skillsAt time.Time
}

func newStatusPlugin() *statusPlugin { return &statusPlugin{} }

func (p *statusPlugin) Name() string  { return "extensions-status" }
func (p *statusPlugin) Title() string { return "扩展与技能" }
func (p *statusPlugin) Slot() Slot    { return SlotSessionSidebar }

func (p *statusPlugin) InterestedEvents() []eventbus.Kind { return nil }

func (p *statusPlugin) Dispatch(eventbus.Event, PluginHost) {}

func (p *statusPlugin) setError(err error) {
	p.mu.Lock()
	if err != nil {
		p.last = err.Error()
	}
	p.mu.Unlock()
}

func (p *statusPlugin) Build(c *ui.Context, h PluginHost) {
	sh, ok := h.(StatusHost)
	if !ok {
		ui.Text(c, "扩展面板未接入").FontSize(11)
		return
	}
	t := c.Theme()
	exts := sh.StatusExtensions()
	if len(exts) == 0 {
		ui.Text(c, "暂无扩展；把预打包目录放到扩展目录后信任").FontSize(11).TextColor(t.TextMuted)
	} else {
		for _, e := range exts {
			ui.Column(c).Padding(2, 0).Gap(2).Children(func() {
				ui.Row(c).Gap(4).AlignItems(ui.Center).Children(func() {
					ui.Text(c, e.Name).FontSize(12).Bold()
					ui.Text(c, e.State).FontSize(10).TextColor(statusColor(t, e.State))
					ui.Box(c).Grow(1)
					if !e.Trusted {
						if ui.Button(c, "信任").FontSize(10).Clicked() {
							go func(name string) {
								p.setError(sh.TrustExtension(name, true))
								h.Invalidate()
							}(e.Name)
						}
					} else {
						label := "禁用"
						if !e.Enabled {
							label = "启用"
						}
						if ui.Button(c, label).FontSize(10).Clicked() {
							go func(name string, on bool) {
								p.setError(sh.EnableExtension(name, on))
								h.Invalidate()
							}(e.Name, !e.Enabled)
						}
					}
				})
				if e.Reason != "" {
					ui.Text(c, e.Reason).FontSize(10).TextColor(t.Danger)
				}
				if e.Tools > 0 || e.Commands > 0 {
					ui.Textf(c, "%d 工具 · %d 命令", e.Tools, e.Commands).FontSize(10).TextColor(t.TextMuted)
				}
				for _, d := range e.Diagnostics {
					ui.Text(c, d).FontSize(10).TextColor(t.TextMuted)
				}
			})
		}
	}

	ui.Divider(c)
	skills := p.skillsView(sh)
	ui.Row(c).Gap(4).AlignItems(ui.Center).Children(func() {
		ui.Text(c, "技能").FontSize(11).Bold()
		ui.Textf(c, "%d 用户 · %d 项目", len(skills.User), len(skills.Project)).FontSize(10).TextColor(t.TextMuted)
		if !skills.Trusted {
			if ui.Button(c, "信任工作区").FontSize(10).Clicked() {
				go func() {
					p.setError(sh.TrustProject(true))
					h.Invalidate()
				}()
			}
		}
	})
	for _, s := range append(skills.User, skills.Project...) {
		ui.Text(c, s.Name).FontSize(11)
	}
	for _, d := range skills.Diagnostics {
		ui.Text(c, d).FontSize(10).TextColor(t.Danger)
	}

	if cmds := sh.StatusCommands(); len(cmds) > 0 {
		ui.Divider(c)
		ui.Text(c, "命令").FontSize(11).Bold()
		for _, cmd := range cmds {
			cmd := cmd
			if ui.Button(c, cmd.Name).FontSize(10).Tooltip(cmd.Description).Clicked() {
				go func() {
					p.setError(sh.RunExtensionCommand(cmd.Name, ""))
					h.Invalidate()
				}()
			}
		}
	}

	p.mu.Lock()
	last := p.last
	p.mu.Unlock()
	if last != "" {
		ui.Text(c, last).FontSize(10).TextColor(t.Danger)
	}
}

// skillsView caches the disk scan; the panel is built every frame.
func (p *statusPlugin) skillsView(sh StatusHost) bridge.SkillsView {
	p.mu.Lock()
	defer p.mu.Unlock()
	if time.Since(p.skillsAt) < 2*time.Second {
		return p.skills
	}
	p.skills = sh.StatusSkills()
	p.skillsAt = time.Now()
	return p.skills
}

func statusColor(t *ui.Theme, state string) ui.Color {
	switch state {
	case "running":
		return t.Success
	case "failed", "untrusted":
		return t.Danger
	default:
		return t.TextMuted
	}
}
