package desktop

import (
	"encoding/json"
	"fmt"
	"sync"

	"github.com/egoist/mygo/ui"

	"github.com/qingbird1983/locowrk/internal/eventbus"
)

const (
	maxTreeNodes = 200
	maxTreeDepth = 2
)

// treeNode is one orchestration node: the session root or a subagent.
type treeNode struct {
	id     string
	name   string
	parent string
	state  string
}

// treePlugin draws the orchestration tree in the rail's top section.
type treePlugin struct {
	mu       sync.Mutex
	order    []string
	nodes    map[string]*treeNode
	selected string
	tools    int
	failures int
}

func newTreePlugin() *treePlugin {
	return &treePlugin{nodes: map[string]*treeNode{}}
}

func (p *treePlugin) Name() string  { return "orchestration-tree" }
func (p *treePlugin) Title() string { return "编排树" }
func (p *treePlugin) Slot() Slot    { return SlotSessionSidebar }

func (p *treePlugin) InterestedEvents() []eventbus.Kind {
	return []eventbus.Kind{
		eventbus.KindSubagentSpawned,
		eventbus.KindSubagentSettled,
		eventbus.KindToolExecutionEnd,
	}
}

func (p *treePlugin) Dispatch(ev eventbus.Event, h PluginHost) {
	switch ev.Kind {
	case eventbus.KindSubagentSpawned, eventbus.KindSubagentSettled:
		var d struct {
			ID       string `json:"id"`
			Name     string `json:"name"`
			ParentID string `json:"parentId"`
			State    string `json:"state"`
		}
		if err := json.Unmarshal(ev.Payload, &d); err != nil {
			h.Report(DiagPayload, "编排事件无法解析："+err.Error())
			return
		}
		if d.ID == "" {
			h.Report(DiagPayload, "编排事件缺少 id")
			return
		}
		p.mu.Lock()
		defer p.mu.Unlock()
		n := p.nodes[d.ID]
		if n == nil {
			n = &treeNode{id: d.ID, name: d.Name, state: "running"}
			if n.name == "" {
				n.name = d.ID
			}
			p.attach(n, d.ParentID)
			p.nodes[n.id] = n
			p.order = append(p.order, n.id)
			p.trim()
		}
		if d.Name != "" {
			n.name = d.Name
		}
		if ev.Kind == eventbus.KindSubagentSettled {
			n.state = "done"
		}
		if d.State != "" {
			n.state = d.State
		}
	case eventbus.KindToolExecutionEnd:
		var d struct {
			IsError bool `json:"isError"`
		}
		_ = json.Unmarshal(ev.Payload, &d)
		p.mu.Lock()
		p.tools++
		if d.IsError {
			p.failures++
		}
		p.mu.Unlock()
	}
}

// attach sets a valid parent within the allowed depth; anything else hangs
// from the session root (empty parent).
func (p *treePlugin) attach(n *treeNode, parent string) {
	if parent == "" || parent == n.id {
		return
	}
	up, ok := p.nodes[parent]
	if !ok || p.depth(parent) >= maxTreeDepth {
		return
	}
	n.parent = up.id
}

// depth counts edges to the root, capped so a malformed cycle cannot spin.
func (p *treePlugin) depth(id string) int {
	n := p.nodes[id]
	for d := 0; n != nil && d <= maxTreeDepth; d++ {
		if n.parent == "" {
			return d
		}
		n = p.nodes[n.parent]
	}
	return maxTreeDepth
}

// trim keeps the tree within its cap: the oldest settled node goes first, and
// the oldest node when every node is still running.
func (p *treePlugin) trim() {
	for len(p.order) > maxTreeNodes {
		idx := -1
		for i, id := range p.order {
			if n := p.nodes[id]; n != nil && n.state == "done" {
				idx = i
				break
			}
		}
		if idx < 0 {
			idx = 0
		}
		id := p.order[idx]
		delete(p.nodes, id)
		p.order = append(p.order[:idx], p.order[idx+1:]...)
	}
}

func (p *treePlugin) Counts() (nodes, steps, tools int) {
	p.mu.Lock()
	defer p.mu.Unlock()
	return len(p.order), 0, 0
}

func (p *treePlugin) Reset() {
	p.mu.Lock()
	p.order = nil
	p.nodes = map[string]*treeNode{}
	p.selected = ""
	p.tools, p.failures = 0, 0
	p.mu.Unlock()
}

func (p *treePlugin) Build(c *ui.Context, h PluginHost) {
	t := c.Theme()
	p.mu.Lock()
	order := append([]string(nil), p.order...)
	nodes := make([]treeNode, 0, len(order))
	for _, id := range order {
		if n := p.nodes[id]; n != nil {
			nodes = append(nodes, *n)
		}
	}
	selected := p.selected
	tools, failures := p.tools, p.failures
	p.mu.Unlock()

	summary := fmt.Sprintf("会话 · %d 次工具", tools)
	if failures > 0 {
		summary = fmt.Sprintf("会话 · %d 次工具 · %d 次失败", tools, failures)
	}
	ui.Text(c, summary).FontSize(11).TextColor(t.TextMuted)
	if len(nodes) == 0 {
		ui.Text(c, "暂无子代理").TextColor(t.TextMuted).FontSize(12)
		return
	}
	for _, n := range nodes {
		n := n
		row := ui.Row(c.Key("node:"+n.id)).Padding(3, 5).Gap(6).Radius(4).AlignItems(ui.Center)
		color := t.Text
		if n.id == selected {
			row.Background(t.Accent)
			color = t.AccentText
		} else if row.Hovered() {
			row.Background(t.SurfaceHover)
		}
		row.Children(func() {
			ui.Box(c).Size(6, 6).Radius(3).Background(stateColor(t, n.state))
			if n.parent != "" {
				ui.Box(c).Size(10, 1)
			}
			name := n.name
			if len([]rune(name)) > 18 {
				name = string([]rune(name)[:18]) + "…"
			}
			ui.Text(c, name).TextColor(color).FontSize(12)
		})
		if row.Clicked() {
			p.mu.Lock()
			p.selected = n.id
			p.mu.Unlock()
			h.Invalidate()
		}
	}
}

// stateColor maps an orchestration state to a status dot.
func stateColor(t *ui.Theme, state string) ui.Color {
	switch state {
	case "done", "settled", "success":
		return t.Success
	case "failed", "error":
		return t.Danger
	case "pending", "waiting", "queued":
		return t.TextMuted
	default:
		return t.Warning
	}
}
