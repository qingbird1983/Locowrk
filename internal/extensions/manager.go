package extensions

import (
	"context"
	"errors"
	"fmt"
	"sync"
	"time"

	"github.com/qingbird1983/locowrk/internal/adapter"
	"github.com/qingbird1983/locowrk/internal/eventbus"
)

// Options configures a Manager. Store and Bus are required in production.
type Options struct {
	Bus      *eventbus.Bus
	Cwd      string
	NodePath string
	// NodeVersion is the pinned runtime version to verify (for example
	// "v26.13.0"). Empty skips the check, which tests and development
	// overrides do.
	NodeVersion string
	// RuntimeRequired fails the A track when NodePath is empty (release
	// builds ship runtime/node and must not fall back silently).
	RuntimeRequired bool
	Store           Store
	UI              adapter.HostUI
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

	// A runtime that does not match the pin disables the A track; the desktop
	// itself stays usable (ADR-0015 §3).
	if err := m.checkRuntime(ctx); err != nil {
		m.failAll(err)
		return nil
	}

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
		// Keep a failed adapter so the panel can show its diagnostics; a
		// healthy-but-unusable one is closed and dropped.
		if a.State() == adapter.StateFailed {
			m.mu.Lock()
			m.adapters[c.Name] = a
			m.mu.Unlock()
		} else {
			_ = a.Close()
		}
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

// checkRuntime verifies the pinned Node runtime before any extension starts.
func (m *Manager) checkRuntime(ctx context.Context) error {
	if m.opts.NodePath == "" {
		if m.opts.RuntimeRequired {
			return errors.New("没有可用的 Node 运行时：发布构建必须随包 runtime/node，A 轨扩展已禁用")
		}
		return nil
	}
	if m.opts.NodeVersion == "" {
		return nil
	}
	return CheckNodeVersion(ctx, m.opts.NodePath, m.opts.NodeVersion)
}

// failAll marks every extension failed with the same reason, so the status
// panel explains why the A track is off instead of showing a silent empty list.
func (m *Manager) failAll(err error) {
	m.mu.Lock()
	defer m.mu.Unlock()
	for _, c := range m.candidates {
		c.State = "failed"
		c.Reason = err.Error()
		if info := m.infos[c.Name]; info != nil {
			info.State = "failed"
			info.Reason = err.Error()
		}
	}
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
