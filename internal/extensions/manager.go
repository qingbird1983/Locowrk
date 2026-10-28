package extensions

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"sort"
	"sync"
	"time"

	"github.com/qingbird1983/locowrk/internal/adapter"
	"github.com/qingbird1983/locowrk/internal/eventbus"
	"github.com/qingbird1983/locowrk/internal/kernel"
)

// Options configures a Manager. Store and Bus are required in production.
type Options struct {
	Bus      *eventbus.Bus
	Cwd      string
	NodePath string
	Store    Store
	UI       adapter.HostUI
	// ExtraDirs are development/冒烟 extension roots; normally empty.
	ExtraDirs   []string
	HostName    string
	HostVersion string
}

// Manager owns every A-track extension process for the current workspace.
type Manager struct {
	opts Options

	mu         sync.Mutex
	candidates []*Candidate
	adapters   map[string]*adapter.Adapter
	infos      map[string]*ExtensionInfo
	toolOwner  map[string]string
	session    string
	notices    []Notice
	closed     bool
}

// New returns an empty manager.
func New(opts Options) *Manager {
	if opts.HostName == "" {
		opts.HostName = "Locowrk"
	}
	if opts.HostVersion == "" {
		opts.HostVersion = "0.10.0"
	}
	return &Manager{
		opts:      opts,
		adapters:  map[string]*adapter.Adapter{},
		infos:     map[string]*ExtensionInfo{},
		toolOwner: map[string]string{},
	}
}

// Prepare discovers extensions and starts every trusted, enabled one. One
// failure never stops the others.
func (m *Manager) Prepare(ctx context.Context) error {
	candidates := Discover(m.opts.Cwd, m.opts.ExtraDirs)
	m.mu.Lock()
	m.candidates = candidates
	for _, c := range candidates {
		info := &ExtensionInfo{
			Name: c.Name, Version: c.Version, Source: c.Source, State: c.State,
			Reason: c.Reason, SHA: c.SHA, Trusted: c.Trusted, Enabled: c.Enabled,
		}
		m.infos[c.Name] = info
	}
	m.mu.Unlock()

	for _, c := range candidates {
		if c.State != "ready" {
			continue
		}
		if err := m.decide(c); err != nil {
			m.fail(c, err)
			continue
		}
		m.mu.Lock()
		if info := m.infos[c.Name]; info != nil {
			info.State = c.State
			info.Reason = c.Reason
			info.Enabled = c.Enabled
			info.Trusted = c.Trusted
		}
		m.mu.Unlock()
		if c.State != "ready" {
			continue
		}
		if err := m.start(ctx, c); err != nil {
			m.fail(c, err)
		}
	}
	return nil
}

// decide applies the saved trust/enable record to a ready candidate.
func (m *Manager) decide(c *Candidate) error {
	if m.opts.Store == nil {
		c.State = "untrusted"
		c.Reason = "no trust store"
		return nil
	}
	path, sha, enabled, trusted, ok := m.opts.Store.Extension(c.Name)
	if !ok || path != c.Dir {
		c.State = "untrusted"
		c.Reason = "extension is not trusted"
		return nil
	}
	c.Enabled = enabled
	c.Trusted = trusted
	if sha != c.SHA {
		c.State = "untrusted"
		c.Reason = "extension changed since it was trusted"
		return nil
	}
	if !enabled {
		c.State = "disabled"
		c.Reason = "disabled by the user"
		return nil
	}
	if !trusted {
		c.State = "untrusted"
		c.Reason = "extension is not trusted"
		return nil
	}
	return nil
}

// start launches one candidate and records its tools/commands.
func (m *Manager) start(ctx context.Context, c *Candidate) error {
	a, err := adapter.New(adapter.Options{
		Bus:         m.opts.Bus,
		SessionID:   m.currentSession(),
		Cwd:         m.opts.Cwd,
		NodePath:    m.opts.NodePath,
		HostName:    m.opts.HostName,
		HostVersion: m.opts.HostVersion,
		UI:          m.opts.UI,
		Notify: func(n adapter.Notification) {
			m.addNotice(n)
		},
	})
	if err != nil {
		return err
	}
	if err := a.Start(ctx, c.Manifest); err != nil {
		_ = a.Close()
		return err
	}
	// Stop this extension if it registers a tool another extension owns.
	for _, t := range a.Tools() {
		m.mu.Lock()
		owner, taken := m.toolOwner[t.Name]
		if !taken {
			m.toolOwner[t.Name] = c.Name
		}
		m.mu.Unlock()
		if taken && owner != c.Name {
			_ = a.Close()
			return fmt.Errorf("tool %q is already provided by %s", t.Name, owner)
		}
	}
	m.mu.Lock()
	m.adapters[c.Name] = a
	info := m.infos[c.Name]
	if info != nil {
		info.State = "running"
		info.Tools = len(a.Tools())
		info.Commands = len(a.Commands())
		info.Diagnostics = a.Diagnostics()
	}
	m.mu.Unlock()
	a.SetSession(m.currentSession())
	return nil
}

func (m *Manager) fail(c *Candidate, err error) {
	m.mu.Lock()
	c.State = "failed"
	c.Reason = err.Error()
	if info := m.infos[c.Name]; info != nil {
		info.State = "failed"
		info.Reason = err.Error()
	}
	m.mu.Unlock()
}

// Extensions returns the UI snapshot in name order.
func (m *Manager) Extensions() []ExtensionInfo {
	m.mu.Lock()
	defer m.mu.Unlock()
	out := make([]ExtensionInfo, 0, len(m.infos))
	for _, info := range m.infos {
		cp := *info
		if a := m.adapters[info.Name]; a != nil {
			cp.Diagnostics = a.Diagnostics()
			cp.Tools = len(a.Tools())
			cp.Commands = len(a.Commands())
		} else {
			cp.Tools = 0
			cp.Commands = 0
		}
		out = append(out, cp)
	}
	sort.Slice(out, func(i, j int) bool { return out[i].Name < out[j].Name })
	return out
}

// Tools returns every running extension tool as a kernel spec.
func (m *Manager) Tools() []kernel.ToolSpec {
	m.mu.Lock()
	adapters := make(map[string]*adapter.Adapter, len(m.adapters))
	for name, a := range m.adapters {
		adapters[name] = a
	}
	m.mu.Unlock()

	var out []kernel.ToolSpec
	for _, a := range adapters {
		for _, tool := range a.Tools() {
			name := tool.Name
			out = append(out, kernel.ToolSpec{
				Name:        name,
				Label:       name,
				Description: tool.Description,
				Parameters:  tool.Parameters,
				Execute: func(ctx context.Context, id string, args json.RawMessage) (kernel.ToolResult, error) {
					res, err := a.InvokeTool(ctx, name, args)
					if err != nil {
						return kernel.ToolResult{}, err
					}
					return kernel.ToolResult{Text: res.Text(), Details: res.Details, IsError: res.IsError}, nil
				},
			})
		}
	}
	return out
}

// Commands returns registered commands.
func (m *Manager) Commands() []CommandInfo {
	m.mu.Lock()
	adapters := make(map[string]*adapter.Adapter, len(m.adapters))
	for name, a := range m.adapters {
		adapters[name] = a
	}
	m.mu.Unlock()
	var out []CommandInfo
	for _, a := range adapters {
		for _, c := range a.Commands() {
			out = append(out, CommandInfo{Name: c.Name, Description: c.Description, Plugin: c.Plugin})
		}
	}
	sort.Slice(out, func(i, j int) bool { return out[i].Name < out[j].Name })
	return out
}

// RunCommand executes one extension command.
func (m *Manager) RunCommand(ctx context.Context, name, args string) error {
	m.mu.Lock()
	var target *adapter.Adapter
	for _, a := range m.adapters {
		for _, c := range a.Commands() {
			if c.Name == name {
				target = a
				break
			}
		}
	}
	m.mu.Unlock()
	if target == nil {
		return fmt.Errorf("extensions: unknown command %q", name)
	}
	return target.SendCommand(ctx, name, args)
}

// Dispatch forwards kernel lifecycle events to every extension. Orchestration
// events are skipped: they came from extensions and must not loop back.
func (m *Manager) Dispatch(ev eventbus.Event) {
	if !forwardedKind(ev.Kind) {
		return
	}
	m.mu.Lock()
	session := m.session
	adapters := make([]*adapter.Adapter, 0, len(m.adapters))
	for _, a := range m.adapters {
		adapters = append(adapters, a)
	}
	m.mu.Unlock()
	if session != "" && ev.SessionID != "" && ev.SessionID != session {
		return
	}
	for _, a := range adapters {
		_ = a.SendEvent(context.Background(), ev)
	}
}

// forwardedKind reports whether a bus event should reach extensions.
func forwardedKind(kind eventbus.Kind) bool {
	switch kind {
	case eventbus.KindAgentStart, eventbus.KindAgentEnd,
		eventbus.KindTurnStart, eventbus.KindTurnEnd,
		eventbus.KindMessageStart, eventbus.KindMessageUpdate, eventbus.KindMessageEnd,
		eventbus.KindToolExecutionStart, eventbus.KindToolExecutionUpdate, eventbus.KindToolExecutionEnd:
		return true
	}
	return false
}

// SetSession changes the active session without restarting extensions.
func (m *Manager) SetSession(id string) {
	m.mu.Lock()
	old := m.session
	m.session = id
	adapters := make([]*adapter.Adapter, 0, len(m.adapters))
	for _, a := range m.adapters {
		adapters = append(adapters, a)
	}
	m.mu.Unlock()
	for _, a := range adapters {
		a.SetSession(id)
		if old != "" {
			_ = a.SendHostEvent("session_shutdown", map[string]string{"sessionId": old})
		}
		if id != "" {
			_ = a.SendHostEvent("session_start", map[string]string{"sessionId": id, "cwd": m.opts.Cwd})
		}
	}
}

func (m *Manager) currentSession() string {
	m.mu.Lock()
	defer m.mu.Unlock()
	return m.session
}

// Trust records a trust decision and starts a newly trusted extension.
func (m *Manager) Trust(name string) error {
	return m.SetTrusted(name, true)
}

// SetTrusted records or revokes trust. Revoking stops the process immediately.
func (m *Manager) SetTrusted(name string, trusted bool) error {
	m.mu.Lock()
	c := m.candidate(name)
	info := m.infos[name]
	a := m.adapters[name]
	m.mu.Unlock()
	if c == nil || c.Manifest == nil {
		return fmt.Errorf("extensions: unknown extension %q", name)
	}
	if m.opts.Store == nil {
		return errors.New("extensions: no trust store")
	}
	enabled := c.Enabled
	if !trusted {
		enabled = false
	} else if !c.Trusted {
		enabled = true
	}
	if err := m.opts.Store.SaveExtension(name, c.Dir, c.SHA, enabled, trusted); err != nil {
		return err
	}
	c.Enabled, c.Trusted = enabled, trusted
	if info != nil {
		info.Enabled, info.Trusted = enabled, trusted
	}
	if !trusted {
		if a != nil {
			_ = a.Close()
			m.mu.Lock()
			delete(m.adapters, name)
			m.mu.Unlock()
		}
		if info != nil {
			info.State = "untrusted"
			info.Reason = "extension is not trusted"
		}
		return nil
	}
	c.State = "ready"
	if info != nil {
		info.State, info.Reason = "stopped", ""
	}
	return m.startIfNeeded(context.Background(), c)
}

// SetEnabled persists an enable toggle and starts/stops the process.
func (m *Manager) SetEnabled(name string, on bool) error {
	m.mu.Lock()
	c := m.candidate(name)
	info := m.infos[name]
	a := m.adapters[name]
	m.mu.Unlock()
	if c == nil || c.Manifest == nil {
		return fmt.Errorf("extensions: unknown extension %q", name)
	}
	if m.opts.Store == nil {
		return errors.New("extensions: no trust store")
	}
	path, sha, _, trusted, ok := m.opts.Store.Extension(name)
	if !ok {
		path, sha, trusted = c.Dir, c.SHA, false
	}
	if err := m.opts.Store.SaveExtension(name, path, sha, on, trusted); err != nil {
		return err
	}
	c.Enabled = on
	if info != nil {
		info.Enabled = on
	}
	if !on {
		if a != nil {
			_ = a.Close()
			m.mu.Lock()
			delete(m.adapters, name)
			m.mu.Unlock()
		}
		if info != nil {
			info.State = "disabled"
		}
		return nil
	}
	return m.startIfNeeded(context.Background(), c)
}

func (m *Manager) startIfNeeded(ctx context.Context, c *Candidate) error {
	m.mu.Lock()
	_, running := m.adapters[c.Name]
	m.mu.Unlock()
	if running {
		return nil
	}
	if err := m.decide(c); err != nil {
		return err
	}
	if c.State != "ready" {
		return nil
	}
	if err := m.start(ctx, c); err != nil {
		m.fail(c, err)
		return err
	}
	return nil
}

func (m *Manager) candidate(name string) *Candidate {
	for _, c := range m.candidates {
		if c.Name == name {
			return c
		}
	}
	return nil
}

func (m *Manager) addNotice(n adapter.Notification) {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.notices = append([]Notice{{
		Plugin: n.Plugin, Message: n.Message, Type: n.Type,
		At: time.Now().UTC().Format(time.RFC3339),
	}}, m.notices...)
	if len(m.notices) > 50 {
		m.notices = m.notices[:50]
	}
}

// Notices returns recent ui_notify messages, newest first.
func (m *Manager) Notices() []Notice {
	m.mu.Lock()
	defer m.mu.Unlock()
	return append([]Notice(nil), m.notices...)
}

// ProjectTrusted reports workspace trust for skills.
func (m *Manager) ProjectTrusted(cwd string) bool {
	if m.opts.Store == nil {
		return false
	}
	return m.opts.Store.ProjectTrusted(cwd)
}

// SetProjectTrust records a workspace skill trust decision.
func (m *Manager) SetProjectTrust(cwd string, trusted bool) error {
	if m.opts.Store == nil {
		return errors.New("extensions: no trust store")
	}
	return m.opts.Store.SetProjectTrust(cwd, trusted)
}

// Stop closes every extension process.
func (m *Manager) Stop() {
	m.mu.Lock()
	if m.closed {
		m.mu.Unlock()
		return
	}
	m.closed = true
	adapters := make([]*adapter.Adapter, 0, len(m.adapters))
	for _, a := range m.adapters {
		adapters = append(adapters, a)
	}
	m.adapters = map[string]*adapter.Adapter{}
	m.mu.Unlock()
	for _, a := range adapters {
		_ = a.Close()
	}
}
