package adapter

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"sync"
	"sync/atomic"
	"time"

	"github.com/qingbird1983/locowrk/internal/eventbus"
)

// State is the adapter lifecycle state (proto §4).
type State string

// Lifecycle states.
const (
	StateNew      State = "new"
	StateStarting State = "starting"
	StateReady    State = "ready"
	StateFailed   State = "failed"
	StateStopped  State = "stopped"
)

// ExitInfo records how a plugin process ended.
type ExitInfo struct {
	At     time.Time `json:"at"`
	Code   int       `json:"code"`
	Err    string    `json:"err,omitempty"`
	Stderr string    `json:"stderr,omitempty"`
}

// Options configures an Adapter. Zero durations take the protocol defaults;
// tests shrink them.
type Options struct {
	Bus       *eventbus.Bus
	SessionID string
	Cwd       string

	HostName    string
	HostVersion string

	HandshakeTimeout  time.Duration
	RPCTimeout        time.Duration
	ReadyQuiet        time.Duration
	ReadyGrace        time.Duration
	HeartbeatInterval time.Duration
	HeartbeatTimeout  time.Duration
	MaxMissedPings    int
	ShutdownTimeout   time.Duration
	MaxRestarts       int
	RestartBackoff    []time.Duration

	// Env is appended to the child environment.
	Env []string
	// NodePath runs the entrypoint with this Node runtime. Empty executes the
	// entrypoint directly (mock plugins built as executables).
	NodePath string
	// Notify maps plugin ui_notify calls into the host UI.
	Notify func(Notification)
	// UI answers ctx.ui select/confirm/input/editor. Nil degrades them to an
	// unsupported diagnostic instead of throwing.
	UI HostUI
	// UITimeout bounds one user dialog (default 60s).
	UITimeout time.Duration
}

func (o *Options) applyDefaults() error {
	if o.Cwd == "" {
		cwd, err := os.Getwd()
		if err != nil {
			return fmt.Errorf("adapter: resolving cwd: %w", err)
		}
		o.Cwd = cwd
	}
	if o.HostName == "" {
		o.HostName = "Locowrk"
	}
	if o.HostVersion == "" {
		o.HostVersion = "0.1.0"
	}
	if o.HandshakeTimeout <= 0 {
		o.HandshakeTimeout = 10 * time.Second
	}
	if o.RPCTimeout <= 0 {
		o.RPCTimeout = 30 * time.Second
	}
	if o.ReadyQuiet <= 0 {
		o.ReadyQuiet = 50 * time.Millisecond
	}
	if o.ReadyGrace <= 0 {
		o.ReadyGrace = 500 * time.Millisecond
	}
	if o.HeartbeatInterval <= 0 {
		o.HeartbeatInterval = 5 * time.Second
	}
	if o.HeartbeatTimeout <= 0 {
		o.HeartbeatTimeout = 2 * time.Second
	}
	if o.MaxMissedPings <= 0 {
		o.MaxMissedPings = 3
	}
	if o.ShutdownTimeout <= 0 {
		o.ShutdownTimeout = 5 * time.Second
	}
	if o.MaxRestarts < 0 {
		o.MaxRestarts = 0
	}
	if len(o.RestartBackoff) == 0 {
		o.RestartBackoff = []time.Duration{time.Second, 2 * time.Second, 4 * time.Second}
	}
	if o.UITimeout <= 0 {
		o.UITimeout = 60 * time.Second
	}
	return nil
}

// Adapter owns one plugin process and its registrations.
type Adapter struct {
	opts     Options
	manifest *Manifest

	mu         sync.Mutex
	state      State
	peer       *peer
	pluginName string
	session    string
	accepted   map[CapabilityName]bool
	eventAck   bool
	tools      map[string]Tool
	toolNames  []string
	commands   map[string]Command
	audit      []AuditRecord
	exit       *ExitInfo
	lastErr    error
	restarts   int
	stopping   bool

	diagnostics *diagnosticStore
	events      chan eventbus.Event
	localSeq    uint64
	dropped     atomic.Uint64

	regCh  chan struct{}
	hbStop chan struct{}
	hbDone chan struct{}
}

// New validates options and returns an adapter in StateNew.
func New(opts Options) (*Adapter, error) {
	if err := opts.applyDefaults(); err != nil {
		return nil, err
	}
	return &Adapter{
		opts:        opts,
		state:       StateNew,
		session:     opts.SessionID,
		accepted:    map[CapabilityName]bool{},
		tools:       map[string]Tool{},
		commands:    map[string]Command{},
		diagnostics: newDiagnosticStore(),
		events:      make(chan eventbus.Event, 64),
		regCh:       make(chan struct{}, 1),
	}, nil
}

// Start launches the plugin, performs the handshake and waits for the initial
// registrations.
func (a *Adapter) Start(ctx context.Context, m *Manifest) error {
	if m == nil {
		return errors.New("adapter: nil manifest")
	}
	entry, err := m.EntryPath()
	if err != nil {
		return err
	}
	if _, err := os.Stat(entry); err != nil {
		return fmt.Errorf("%w: entrypoint: %v", ErrInvalidManifest, err)
	}

	a.mu.Lock()
	if a.state == StateReady || a.state == StateStarting {
		a.mu.Unlock()
		return errors.New("adapter: already started")
	}
	a.manifest = m
	a.pluginName = m.Name
	a.state = StateStarting
	a.lastErr = nil
	a.mu.Unlock()

	var p *peer
	if a.opts.NodePath != "" {
		p, err = startPeer(a.opts.NodePath, []string{entry}, m.Dir, a.opts.Env)
	} else {
		p, err = startPeer(entry, nil, m.Dir, a.opts.Env)
	}
	if err != nil {
		a.setFailed(err)
		return err
	}
	p.setHandlers(a.handleRequest, a.handleNotify)

	a.mu.Lock()
	a.peer = p
	a.mu.Unlock()

	params := initializeParams{Protocol: ProtocolVersion}
	params.Host.Name = a.opts.HostName
	params.Host.Version = a.opts.HostVersion
	params.Session.ID = a.opts.SessionID
	params.Session.Cwd = a.opts.Cwd

	raw, err := p.call(ctx, a.opts.HandshakeTimeout, "initialize", params)
	if err != nil {
		p.kill()
		a.setFailed(fmt.Errorf("adapter: handshake: %w", err))
		return a.lastError()
	}
	var result initializeResult
	if err := json.Unmarshal(raw, &result); err != nil {
		p.kill()
		a.setFailed(fmt.Errorf("%w: initialize result: %v", ErrProtocol, err))
		return a.lastError()
	}
	if result.Protocol != ProtocolVersion {
		p.kill()
		a.setFailed(fmt.Errorf("%w: plugin speaks %d", ErrIncompatibleVersion, result.Protocol))
		return a.lastError()
	}
	if result.Shim != "" && result.Shim != ShimVersion {
		p.kill()
		a.setFailed(fmt.Errorf("%w: shim %q", ErrIncompatibleVersion, result.Shim))
		return a.lastError()
	}
	accepted := map[CapabilityName]bool{}
	for _, c := range result.Capabilities {
		if _, declared := m.Capability(c.Name); !declared {
			continue
		}
		// The host grants the intersection; a capability without scope is not
		// granted even if the plugin echoes one.
		if len(c.Scope) == 0 {
			continue
		}
		accepted[c.Name] = true
	}
	a.mu.Lock()
	a.accepted = accepted
	a.eventAck = result.WantsEventAck
	a.mu.Unlock()

	a.waitForRegistrations(ctx)

	a.mu.Lock()
	a.state = StateReady
	a.mu.Unlock()

	go a.watchExit(p)
	a.startHeartbeat(p)
	return nil
}

// Stop asks the plugin to exit, then tears the process tree down.
func (a *Adapter) Stop(_ context.Context) error {
	a.mu.Lock()
	if a.state == StateNew || a.state == StateStopped {
		a.mu.Unlock()
		return nil
	}
	a.stopping = true
	p := a.peer
	stopHeartbeat := a.hbStop
	a.mu.Unlock()

	if stopHeartbeat != nil {
		close(stopHeartbeat)
		select {
		case <-a.hbDone:
		case <-time.After(time.Second):
		}
	}

	var stopErr error
	if p != nil {
		shutdownCtx, cancel := context.WithTimeout(context.Background(), a.opts.ShutdownTimeout)
		_, err := p.call(shutdownCtx, a.opts.ShutdownTimeout, "shutdown_plugin", map[string]string{"reason": "host shutdown"})
		cancel()
		if err != nil && !errors.Is(err, ErrCrashed) {
			stopErr = fmt.Errorf("adapter: shutdown: %w", err)
		}
		p.closeStdin()
		select {
		case <-p.done:
		case <-time.After(2 * time.Second):
			p.kill()
		}
		p.kill()
	}

	a.mu.Lock()
	a.state = StateStopped
	a.peer = nil
	a.stopping = false
	a.mu.Unlock()
	return stopErr
}

// Close is Stop with a background context.
func (a *Adapter) Close() error { return a.Stop(context.Background()) }

// Events returns translated orchestration events. Hosts normally subscribe to
// the EventBus; this channel is a convenience for embedders.
func (a *Adapter) Events() <-chan eventbus.Event { return a.events }

// DroppedEvents counts events dropped because the Events channel was full.
func (a *Adapter) DroppedEvents() uint64 { return a.dropped.Load() }

// State returns the current lifecycle state.
func (a *Adapter) State() State {
	a.mu.Lock()
	defer a.mu.Unlock()
	return a.state
}

// Exit returns how the last plugin process ended, if it did.
func (a *Adapter) Exit() *ExitInfo {
	a.mu.Lock()
	defer a.mu.Unlock()
	if a.exit == nil {
		return nil
	}
	copy := *a.exit
	return &copy
}

// Err returns the last fatal error, if any.
func (a *Adapter) Err() error {
	a.mu.Lock()
	defer a.mu.Unlock()
	return a.lastErr
}
