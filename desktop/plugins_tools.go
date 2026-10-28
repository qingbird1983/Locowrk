package desktop

import (
	"bytes"
	"encoding/json"
	"strings"
	"sync"

	"github.com/egoist/mygo/ui"

	"github.com/qingbird1983/locowrk/internal/eventbus"
)

const (
	maxToolCalls = 200
	maxToolText  = 2 << 10 // 2 KiB per argument or result
)

// toolCall is one tool invocation as the inspector keeps it.
type toolCall struct {
	id       string
	name     string
	path     string
	args     string
	text     string
	isError  bool
	duration *int64
}

// toolsPlugin is the tool inspector in the rail's bottom section.
type toolsPlugin struct {
	mu    sync.Mutex
	order []string
	calls map[string]*toolCall
}

func newToolsPlugin() *toolsPlugin {
	return &toolsPlugin{calls: map[string]*toolCall{}}
}

func (p *toolsPlugin) Name() string  { return "tool-inspector" }
func (p *toolsPlugin) Title() string { return "工具检查器" }
func (p *toolsPlugin) Slot() Slot    { return SlotToolDetail }

func (p *toolsPlugin) InterestedEvents() []eventbus.Kind {
	return []eventbus.Kind{
		eventbus.KindToolExecutionStart,
		eventbus.KindToolExecutionUpdate,
		eventbus.KindToolExecutionEnd,
	}
}

func (p *toolsPlugin) Dispatch(ev eventbus.Event, h PluginHost) {
	var d struct {
		ToolCallID string          `json:"toolCallId"`
		ToolName   string          `json:"toolName"`
		Arguments  json.RawMessage `json:"arguments"`
		Text       string          `json:"text"`
		IsError    bool            `json:"isError"`
		DurationMs *int64          `json:"durationMs"`
	}
	if err := json.Unmarshal(ev.Payload, &d); err != nil {
		h.Report(DiagPayload, "工具事件无法解析："+err.Error())
		return
	}
	if d.ToolCallID == "" {
		h.Report(DiagPayload, "工具事件缺少 toolCallId")
		return
	}
	p.mu.Lock()
	defer p.mu.Unlock()
	call := p.calls[d.ToolCallID]
	if call == nil {
		call = &toolCall{id: d.ToolCallID, name: d.ToolName, path: toolPathArg(d.Arguments), args: prettyArgs(d.Arguments)}
		if call.name == "" {
			call.name = "tool"
		}
		p.calls[call.id] = call
		p.order = append(p.order, call.id)
		p.trim()
	}
	if d.ToolName != "" {
		call.name = d.ToolName
	}
	switch ev.Kind {
	case eventbus.KindToolExecutionUpdate:
		call.text = clipText(call.text + d.Text)
	case eventbus.KindToolExecutionEnd:
		call.text = clipText(d.Text)
		call.isError = d.IsError
		call.duration = d.DurationMs
	}
}

// trim keeps the newest maxToolCalls entries.
func (p *toolsPlugin) trim() {
	for len(p.order) > maxToolCalls {
		oldest := p.order[0]
		p.order = p.order[1:]
		delete(p.calls, oldest)
	}
}

func (p *toolsPlugin) Counts() (nodes, steps, tools int) {
	p.mu.Lock()
	defer p.mu.Unlock()
	return 0, 0, len(p.order)
}

func (p *toolsPlugin) Reset() {
	p.mu.Lock()
	p.order = nil
	p.calls = map[string]*toolCall{}
	p.mu.Unlock()
}

func (p *toolsPlugin) Build(c *ui.Context, h PluginHost) {
	t := c.Theme()
	selected := h.SelectedToolCall()
	p.mu.Lock()
	order := append([]string(nil), p.order...)
	calls := make([]toolCall, 0, len(order))
	for _, id := range order {
		if call := p.calls[id]; call != nil {
			calls = append(calls, *call)
		}
	}
	p.mu.Unlock()

	if len(calls) == 0 {
		ui.Text(c, "暂无工具调用").TextColor(t.TextMuted).FontSize(12)
		return
	}
	for i := len(calls) - 1; i >= 0; i-- {
		call := calls[i]
		row := ui.Row(c.Key("call:"+call.id)).Padding(3, 5).Gap(6).Radius(4).AlignItems(ui.Center)
		color := t.Text
		if call.id == selected {
			row.Background(t.Accent)
			color = t.AccentText
		} else if row.Hovered() {
			row.Background(t.SurfaceHover)
		}
		row.Children(func() {
			ui.Text(c, call.name).TextColor(color).FontSize(12)
			ui.Box(c).Grow(1)
			if call.isError {
				ui.Text(c, "失败").TextColor(t.Danger).FontSize(10)
			}
			if call.duration != nil {
				ui.Textf(c, "%d ms", *call.duration).TextColor(t.TextMuted).FontSize(10)
			}
		})
		if row.Clicked() {
			h.SelectToolCall(call.id)
		}
	}
	ui.Divider(c)
	call := findCall(calls, selected)
	if call == nil {
		ui.Text(c, "选择一条调用查看详情").TextColor(t.TextMuted).FontSize(11)
		return
	}
	ui.Text(c, call.name+" · "+call.id).FontSize(11).TextColor(t.TextMuted)
	if call.args != "" {
		ui.Text(c, "参数").FontSize(10).TextColor(t.TextMuted)
		ui.Text(c, call.args).FontSize(11)
	}
	if call.text != "" {
		ui.Text(c, "结果").FontSize(10).TextColor(t.TextMuted)
		ui.Text(c, call.text).FontSize(11)
	} else {
		ui.Text(c, "运行中…").FontSize(11).TextColor(t.TextMuted)
	}
}

func findCall(calls []toolCall, id string) *toolCall {
	if id == "" {
		return nil
	}
	for i := range calls {
		if calls[i].id == id {
			return &calls[i]
		}
	}
	return nil
}

// Path returns a tool call's file argument, empty when it has none.
func (p *toolsPlugin) Path(id string) string {
	if id == "" {
		return ""
	}
	p.mu.Lock()
	defer p.mu.Unlock()
	if call := p.calls[id]; call != nil {
		return call.path
	}
	return ""
}

// toolPathArg reads the common path argument names without trusting more of
// the payload than needed.
func toolPathArg(raw json.RawMessage) string {
	if len(raw) == 0 {
		return ""
	}
	var args struct {
		Path     string `json:"path"`
		FilePath string `json:"file_path"`
		File     string `json:"file"`
		Target   string `json:"target"`
	}
	if err := json.Unmarshal(raw, &args); err != nil {
		return ""
	}
	for _, s := range []string{args.Path, args.FilePath, args.File, args.Target} {
		if s = strings.TrimSpace(s); s != "" {
			return s
		}
	}
	return ""
}

// prettyArgs formats arguments for reading, clipped to the inspector budget.
func prettyArgs(raw json.RawMessage) string {
	if len(raw) == 0 {
		return ""
	}
	var buf bytes.Buffer
	if err := json.Indent(&buf, raw, "", "  "); err == nil {
		return clipText(buf.String())
	}
	return clipText(string(raw))
}

// clipText keeps text within the inspector budget.
func clipText(s string) string {
	if len(s) <= maxToolText {
		return s
	}
	cut := maxToolText
	for cut > 0 && !utf8Start(s[cut]) {
		cut--
	}
	return s[:cut] + "\n…（截断）"
}

// utf8Start reports whether b starts a rune.
func utf8Start(b byte) bool { return b&0xc0 != 0x80 }
