package desktop

import (
	"encoding/json"
	"sync"

	"github.com/egoist/mygo/ui"

	"github.com/qingbird1983/locowrk/internal/eventbus"
)

const (
	maxTimelineSteps = 50
	maxTimelineShown = 6
)

func timelineStatusOk(s string) bool {
	switch s {
	case "pending", "running", "done", "failed", "skipped":
		return true
	}
	return false
}

func timelineTerminal(s string) bool {
	return s == "done" || s == "failed" || s == "skipped"
}

// timelineStep is one workflow stage.
type timelineStep struct {
	name   string
	status string
	detail string
}

// timelinePlugin draws the workflow strip over the page.
type timelinePlugin struct {
	mu    sync.Mutex
	order []string
	steps map[string]*timelineStep
}

func newTimelinePlugin() *timelinePlugin {
	return &timelinePlugin{steps: map[string]*timelineStep{}}
}

func (p *timelinePlugin) Name() string  { return "workflow-timeline" }
func (p *timelinePlugin) Title() string { return "workflow 时间线" }
func (p *timelinePlugin) Slot() Slot    { return SlotTimelineAbove }

func (p *timelinePlugin) InterestedEvents() []eventbus.Kind {
	return []eventbus.Kind{eventbus.KindWorkflowStep}
}

func (p *timelinePlugin) Dispatch(ev eventbus.Event, h PluginHost) {
	var d struct {
		Name   string `json:"name"`
		Status string `json:"status"`
		Detail string `json:"detail"`
	}
	if err := json.Unmarshal(ev.Payload, &d); err != nil {
		h.Report(DiagPayload, "workflow 事件无法解析："+err.Error())
		return
	}
	if d.Name == "" || !timelineStatusOk(d.Status) {
		h.Report(DiagPayload, "workflow 事件缺少名称或状态非法")
		return
	}
	p.mu.Lock()
	defer p.mu.Unlock()
	s := p.steps[d.Name]
	if s == nil {
		if len(p.order) >= maxTimelineSteps {
			return
		}
		s = &timelineStep{name: d.Name}
		p.steps[d.Name] = s
		p.order = append(p.order, d.Name)
	}
	// A finished step is not dragged back to running by a late event.
	if timelineTerminal(s.status) && !timelineTerminal(d.Status) {
		return
	}
	s.status, s.detail = d.Status, d.Detail
}

func (p *timelinePlugin) Counts() (nodes, steps, tools int) {
	p.mu.Lock()
	defer p.mu.Unlock()
	return 0, len(p.order), 0
}

func (p *timelinePlugin) Reset() {
	p.mu.Lock()
	p.order = nil
	p.steps = map[string]*timelineStep{}
	p.mu.Unlock()
}

func (p *timelinePlugin) Build(c *ui.Context, h PluginHost) {
	t := c.Theme()
	p.mu.Lock()
	order := append([]string(nil), p.order...)
	steps := make([]timelineStep, 0, len(order))
	for _, name := range order {
		if s := p.steps[name]; s != nil {
			steps = append(steps, *s)
		}
	}
	p.mu.Unlock()
	if len(steps) == 0 {
		return
	}
	shown := steps
	hidden := 0
	if len(shown) > maxTimelineShown {
		hidden = len(shown) - maxTimelineShown
		shown = shown[:maxTimelineShown]
	}
	ui.Row(c).Gap(6).AlignItems(ui.Center).Children(func() {
		for i, s := range shown {
			if i > 0 {
				ui.Text(c, "›").FontSize(11).TextColor(t.TextMuted)
			}
			name := s.name
			if len([]rune(name)) > 8 {
				name = string([]rune(name)[:8]) + "…"
			}
			text := ui.Text(c, name).FontSize(11).TextColor(timelineColor(t, s.status))
			if s.detail != "" {
				text.Tooltip(s.detail)
			}
		}
		if hidden > 0 {
			ui.Textf(c, "+%d", hidden).FontSize(11).TextColor(t.TextMuted)
		}
	})
}

// timelineColor maps a workflow status to the strip's text color.
func timelineColor(t *ui.Theme, status string) ui.Color {
	switch status {
	case "running":
		return t.Accent
	case "done":
		return t.Success
	case "failed":
		return t.Danger
	case "skipped":
		return t.TextMuted
	default:
		return t.TextMuted
	}
}
