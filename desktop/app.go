// Package desktop is the MyGo shell: window, bindings and the event pump.
// It may import MyGo; internal/bridge and internal/kernel may not.
package desktop

import (
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"sync"

	"github.com/egoist/mygo"

	"github.com/qingbird1983/locowrk/internal/bridge"
	"github.com/qingbird1983/locowrk/internal/eventbus"
	"github.com/qingbird1983/locowrk/internal/kernel"
)

// Event carries one project event to the page.
var Event = mygo.NewEvent[Envelope]("harness-event")

// Envelope is the payload of Event.
type Envelope struct {
	Kind      string `json:"kind"`
	SessionID string `json:"sessionId"`
	Payload   any    `json:"payload"`
}

// Info describes the shell's defaults.
type Info struct {
	Cwd         string `json:"cwd"`
	Model       string `json:"model"`
	Thinking    string `json:"thinking"`
	Version     string `json:"version"`
	Credentials bool   `json:"credentials"`
}

// SessionRow is one item in the session list.
type SessionRow struct {
	ID        string `json:"id"`
	Path      string `json:"path"`
	Timestamp string `json:"timestamp"`
	Messages  int    `json:"messages"`
}

// ToolCall is a model-requested tool call, decoded for the page.
type ToolCall struct {
	ID        string `json:"id"`
	Name      string `json:"name"`
	Arguments any    `json:"arguments"`
}

// Message is the transcript view used by the page.
type Message struct {
	ID         string     `json:"id"`
	Role       string     `json:"role"`
	Text       string     `json:"text"`
	Thinking   string     `json:"thinking,omitempty"`
	ToolCalls  []ToolCall `json:"toolCalls,omitempty"`
	StopReason string     `json:"stopReason,omitempty"`
	Error      string     `json:"error,omitempty"`
}

// Options are the shell defaults.
type Options struct {
	Cwd      string
	Model    string
	Thinking string
	Version  string
	// Host draws the native plugin rail; nil leaves the window page-only.
	Host *Host
}

// App is the service bound to the page as "Desktop".
type App struct {
	bridge *bridge.Service
	opts   Options
	host   *Host

	mu      sync.Mutex
	win     *mygo.Window
	page    *mygo.Page
	current string
	closed  bool
}

// New wires the shell to the bridge.
func New(b *bridge.Service, opts Options) *App {
	if opts.Thinking == "" {
		opts.Thinking = "off"
	}
	return &App{bridge: b, opts: opts, host: opts.Host}
}

// SetWindow lets the app emit events to the window.
func (a *App) SetWindow(w *mygo.Window) {
	a.mu.Lock()
	a.win = w
	a.mu.Unlock()
}

// SetPage names the embedded page the app emits to. A native window has no
// page of its own; the shell's WebView does (ADR-0013).
func (a *App) SetPage(p *mygo.Page) {
	a.mu.Lock()
	a.page = p
	a.mu.Unlock()
}

// Emit forwards one bridge event to the page.
func (a *App) Emit(ev eventbus.Event) {
	a.mu.Lock()
	w := a.win
	p := a.page
	a.mu.Unlock()
	if w == nil && p == nil {
		return
	}
	var payload any
	if len(ev.Payload) > 0 {
		if err := json.Unmarshal(ev.Payload, &payload); err != nil {
			payload = string(ev.Payload)
		}
	}
	env := Envelope{Kind: string(ev.Kind), SessionID: ev.SessionID, Payload: payload}
	if p != nil {
		_ = Event.EmitPage(p, env)
	} else {
		_ = Event.Emit(w, env)
	}
}

// Info returns the defaults shown at startup.
func (a *App) Info() Info {
	return Info{
		Cwd:         a.opts.Cwd,
		Model:       a.opts.Model,
		Thinking:    a.opts.Thinking,
		Version:     a.opts.Version,
		Credentials: credentialsConfigured(a.opts.Model),
	}
}

// credentialsConfigured reports whether a known provider has its environment
// variable set. Unknown providers keep the indicator neutral (configured) so
// the page never claims a working setup is broken.
func credentialsConfigured(model string) bool {
	provider := model
	if i := strings.IndexByte(model, '/'); i > 0 {
		provider = model[:i]
	}
	vars := map[string]string{
		"deepseek":  "DEEPSEEK_API_KEY",
		"anthropic": "ANTHROPIC_API_KEY",
		"openai":    "OPENAI_API_KEY",
	}
	name, ok := vars[provider]
	if !ok {
		return true
	}
	return os.Getenv(name) != ""
}

// Models lists the models offered in the picker.
func (a *App) Models() []string {
	seen := map[string]bool{}
	out := []string{}
	for _, m := range []string{a.opts.Model, "deepseek/deepseek-flash", "deepseek/deepseek-v4-pro"} {
		if m == "" || seen[m] {
			continue
		}
		seen[m] = true
		out = append(out, m)
	}
	return out
}

// Sessions lists stored sessions for cwd.
func (a *App) Sessions(cwd string) []SessionRow {
	if cwd == "" {
		cwd = a.opts.Cwd
	}
	rows := a.bridge.ListSessions(cwd)
	out := make([]SessionRow, 0, len(rows))
	for _, r := range rows {
		out = append(out, SessionRow{ID: r.ID, Path: r.Path, Timestamp: r.Timestamp, Messages: r.Messages})
	}
	return out
}

// NewSession creates a session and selects it.
func (a *App) NewSession(cwd, model, thinking string) (string, error) {
	id, err := a.bridge.CreateSession(bridge.CreateOptions{Cwd: cwd, Model: model, Thinking: thinking})
	if err != nil {
		return "", err
	}
	if err := a.selectSession(id); err != nil {
		return "", err
	}
	return id, nil
}

// OpenSession resumes a stored session and selects it.
func (a *App) OpenSession(path, model, thinking string) (string, error) {
	id, err := a.bridge.OpenSession(path, model, thinking)
	if err != nil {
		return "", err
	}
	if err := a.selectSession(id); err != nil {
		return "", err
	}
	return id, nil
}

func (a *App) selectSession(id string) error {
	if err := a.bridge.Watch(id); err != nil {
		return err
	}
	a.mu.Lock()
	old := a.current
	a.current = id
	a.mu.Unlock()
	if old != "" && old != id {
		a.bridge.Unwatch(old)
	}
	if a.host != nil {
		a.host.SetSession(id)
	}
	return nil
}

// History returns the transcript as page DTOs.
func (a *App) History(sessionID string) ([]Message, error) {
	msgs, err := a.bridge.History(sessionID)
	if err != nil {
		return nil, err
	}
	out := make([]Message, 0, len(msgs))
	for _, m := range msgs {
		msg := Message{
			ID:         m.ID,
			Role:       m.Role,
			Text:       m.Text,
			Thinking:   m.Thinking,
			StopReason: m.StopReason,
			Error:      m.Error,
		}
		for _, tc := range m.ToolCalls {
			var args any
			if len(tc.Arguments) > 0 {
				_ = json.Unmarshal(tc.Arguments, &args)
			}
			msg.ToolCalls = append(msg.ToolCalls, ToolCall{ID: tc.ID, Name: tc.Name, Arguments: args})
		}
		out = append(out, msg)
	}
	return out, nil
}

// Send runs one turn and blocks until it finishes. Cancellation is not an error.
func (a *App) Send(sessionID, text string) error {
	if sessionID == "" {
		return errors.New("desktop: no session selected")
	}
	if err := a.bridge.Watch(sessionID); err != nil {
		return err
	}
	err := a.bridge.Prompt(sessionID, text)
	if errors.Is(err, kernel.ErrCanceled) {
		return nil
	}
	return err
}

// Stop cancels the session's active turn.
func (a *App) Stop(sessionID string) error {
	if sessionID == "" {
		return nil
	}
	return a.bridge.Cancel(sessionID)
}

// SetModel switches the session's model.
func (a *App) SetModel(sessionID, model string) error {
	return a.bridge.SetModel(sessionID, model)
}

// SetThinking switches the session's thinking level.
func (a *App) SetThinking(sessionID, level string) error {
	return a.bridge.SetThinking(sessionID, level)
}

// SelectToolCall shows a tool call's details in the native inspector. The page
// passes the ID of a clicked tool card; the host only records the selection.
func (a *App) SelectToolCall(sessionID, toolCallID string) error {
	if a.host == nil {
		return nil
	}
	a.mu.Lock()
	current := a.current
	a.mu.Unlock()
	if sessionID != "" && current != "" && sessionID != current {
		return fmt.Errorf("desktop: session %q is not selected", sessionID)
	}
	a.host.SelectToolCall(toolCallID)
	return nil
}

// OpenDiff opens a review tab for the workspace, focused on the file the tool
// call touched when it has one. An empty session means the workspace only.
func (a *App) OpenDiff(sessionID, toolCallID string) error {
	if a.host == nil {
		return errors.New("desktop: no plugin host")
	}
	a.mu.Lock()
	current := a.current
	a.mu.Unlock()
	if sessionID != "" && current != "" && sessionID != current {
		return fmt.Errorf("desktop: session %q is not selected", sessionID)
	}
	rel := relativeWorkspacePath(a.opts.Cwd, a.host.ToolPath(toolCallID))
	key, title := "diff:*", "工作区变更"
	if rel != "" {
		key = "diff:" + rel
		title = "变更：" + filepath.Base(rel)
	}
	_, err := a.host.OpenTab(TabRequest{Key: key, Title: title, URL: "/diff.html", Data: rel})
	return err
}

// Shutdown cancels the active turn and stops event forwarding. The kernel is
// closed by the caller after App.Run returns.
func (a *App) Shutdown() {
	a.mu.Lock()
	a.closed = true
	id := a.current
	a.mu.Unlock()
	if id != "" {
		_ = a.bridge.Cancel(id)
	}
	if a.host != nil {
		a.host.SetSession("")
	}
	a.bridge.Close()
}

// Current returns the selected session, for smoke assertions.
func (a *App) Current() string {
	a.mu.Lock()
	defer a.mu.Unlock()
	return a.current
}

// String is used by logs.
func (a *App) String() string { return fmt.Sprintf("desktop %s", a.opts.Version) }
