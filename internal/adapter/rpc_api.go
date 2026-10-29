package adapter

import (
	"context"
	"encoding/json"
	"fmt"
	"sync/atomic"
	"time"

	"github.com/qingbird1983/locowrk/internal/eventbus"
)

// SetSession changes the session id used by later calls and published events.
// It does not restart the plugin.
func (a *Adapter) SetSession(id string) {
	a.mu.Lock()
	a.session = id
	a.mu.Unlock()
}

// sessionID returns the current session (falling back to the start option).
func (a *Adapter) sessionID() string {
	a.mu.Lock()
	defer a.mu.Unlock()
	if a.session != "" {
		return a.session
	}
	return a.opts.SessionID
}

// InvokeTool asks the plugin to execute one of its tools.
func (a *Adapter) InvokeTool(ctx context.Context, name string, args json.RawMessage) (ToolResult, error) {
	a.mu.Lock()
	tool, ok := a.tools[name]
	p := a.peer
	state := a.state
	a.mu.Unlock()
	if !ok {
		return ToolResult{}, fmt.Errorf("adapter: unknown tool %q", name)
	}
	if state != StateReady || p == nil {
		return ToolResult{}, fmt.Errorf("%w: adapter is %s", ErrCrashed, state)
	}
	if len(args) == 0 {
		args = json.RawMessage("{}")
	}
	timeout := a.opts.RPCTimeout
	if tool.TimeoutMS > 0 {
		timeout = time.Duration(tool.TimeoutMS) * time.Millisecond
	} else if ms, ok := a.manifest.LongTaskTimeout("invoke_tool"); ok {
		timeout = time.Duration(ms) * time.Millisecond
	}
	raw, err := p.call(ctx, timeout, "invoke_tool", map[string]any{
		"sessionId":  a.sessionID(),
		"toolCallId": "call-" + name,
		"name":       name,
		"arguments":  args,
	})
	if err != nil {
		return ToolResult{}, err
	}
	var result ToolResult
	if err := json.Unmarshal(raw, &result); err != nil {
		return ToolResult{}, fmt.Errorf("%w: invoke_tool result: %v", ErrProtocol, err)
	}
	if len(result.Content) == 0 {
		return ToolResult{}, fmt.Errorf("%w: invoke_tool returned no content", ErrProtocol)
	}
	return result, nil
}

// SendCommand invokes a command registered by the plugin.
func (a *Adapter) SendCommand(ctx context.Context, name, args string) error {
	a.mu.Lock()
	_, ok := a.commands[name]
	p := a.peer
	state := a.state
	a.mu.Unlock()
	if !ok {
		return fmt.Errorf("adapter: unknown command %q", name)
	}
	if state != StateReady || p == nil {
		return fmt.Errorf("%w: adapter is %s", ErrCrashed, state)
	}
	_, err := p.call(ctx, a.opts.RPCTimeout, "invoke_command", map[string]any{
		"sessionId": a.sessionID(),
		"commandId": name,
		"args":      args,
	})
	return err
}

// SendEvent delivers one lifecycle event to the plugin as a notification.
func (a *Adapter) SendEvent(_ context.Context, ev eventbus.Event) error {
	a.mu.Lock()
	p := a.peer
	state := a.state
	a.mu.Unlock()
	if state != StateReady || p == nil {
		return fmt.Errorf("%w: adapter is %s", ErrCrashed, state)
	}
	return p.notify("event", map[string]any{
		"sessionId": firstNonEmpty(ev.SessionID, a.sessionID()),
		"kind":      string(ev.Kind),
		"payload":   ev.Payload,
		"seq":       ev.Seq,
	})
}

// SendHostEvent delivers a host-only lifecycle event (session_start, etc.)
// that has no eventbus kind.
func (a *Adapter) SendHostEvent(kind string, payload any) error {
	a.mu.Lock()
	p := a.peer
	state := a.state
	a.mu.Unlock()
	if state != StateReady || p == nil {
		return fmt.Errorf("%w: adapter is %s", ErrCrashed, state)
	}
	return p.notify("event", map[string]any{
		"sessionId": a.sessionID(),
		"kind":      kind,
		"payload":   payload,
	})
}

// WantsEventAck reports whether the plugin asked for lifecycle events as
// requests, so the host can wait for a decision (phase-6 runtime §5).
func (a *Adapter) WantsEventAck() bool {
	a.mu.Lock()
	defer a.mu.Unlock()
	return a.eventAck
}

// RequestEvent sends one lifecycle event as a request and returns the plugin's
// merged answer ({block, reason, cancel}). The budget covers a dialog the
// handler may open, so a permission gate is not cut short by the RPC timeout.
func (a *Adapter) RequestEvent(ctx context.Context, kind string, payload any) (map[string]any, error) {
	a.mu.Lock()
	p := a.peer
	state := a.state
	budget := a.opts.RPCTimeout + a.opts.UITimeout
	a.mu.Unlock()
	if state != StateReady || p == nil {
		return nil, fmt.Errorf("%w: adapter is %s", ErrCrashed, state)
	}
	raw, err := p.call(ctx, budget, "event", map[string]any{
		"sessionId": a.sessionID(),
		"kind":      kind,
		"payload":   payload,
	})
	if err != nil {
		return nil, err
	}
	var out map[string]any
	if len(raw) > 0 {
		if err := json.Unmarshal(raw, &out); err != nil {
			return nil, fmt.Errorf("%w: event result: %v", ErrProtocol, err)
		}
	}
	return out, nil
}

// Abort tells the plugin to stop its current orchestration.
func (a *Adapter) Abort(ctx context.Context) error {
	a.mu.Lock()
	p := a.peer
	state := a.state
	a.mu.Unlock()
	if state != StateReady || p == nil {
		return fmt.Errorf("%w: adapter is %s", ErrCrashed, state)
	}
	_, err := p.call(ctx, a.opts.RPCTimeout, "abort", map[string]any{"sessionId": a.sessionID()})
	return err
}

func firstNonEmpty(a, b string) string {
	if a != "" {
		return a
	}
	return b
}

// Ping performs one heartbeat round trip.
func (a *Adapter) Ping(ctx context.Context) error {
	a.mu.Lock()
	p := a.peer
	state := a.state
	a.mu.Unlock()
	if state != StateReady || p == nil {
		return fmt.Errorf("%w: adapter is %s", ErrCrashed, state)
	}
	_, err := p.call(ctx, a.opts.HeartbeatTimeout, "ping", map[string]any{})
	return err
}

// publish sends an orchestration event to the bus (Seq assigned there) and to
// the convenience channel.
func (a *Adapter) publish(kind eventbus.Kind, payload any) {
	session := a.sessionID()
	ev := eventbus.Event{Kind: kind, SessionID: session}
	if a.opts.Bus != nil {
		published, err := a.opts.Bus.Publish(kind, session, payload)
		if err != nil {
			return
		}
		ev = published
	} else {
		raw, err := json.Marshal(payload)
		if err != nil {
			return
		}
		ev.Seq = atomic.AddUint64(&a.localSeq, 1)
		ev.At = time.Now().UTC()
		ev.Payload = raw
	}
	select {
	case a.events <- ev:
	default:
		a.dropped.Add(1)
	}
}

// eventKind maps an orchestration event name from the plugin.
func eventKind(name string) (eventbus.Kind, bool) {
	switch name {
	case string(eventbus.KindSubagentSpawned):
		return eventbus.KindSubagentSpawned, true
	case string(eventbus.KindSubagentSettled):
		return eventbus.KindSubagentSettled, true
	case string(eventbus.KindWorkflowStep):
		return eventbus.KindWorkflowStep, true
	case string(eventbus.KindPluginDiagnostic):
		return eventbus.KindPluginDiagnostic, true
	}
	return "", false
}
