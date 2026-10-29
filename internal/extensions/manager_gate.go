package extensions

import (
	"context"
	"encoding/json"
	"errors"

	"github.com/qingbird1983/locowrk/internal/adapter"
)

// AbortAll tells every running extension to stop its orchestration work
// (design/phase-6 §3.3: Desktop.stop aborts the kernel run and the extensions).
func (m *Manager) AbortAll(ctx context.Context) error {
	m.mu.Lock()
	type entry struct {
		name string
		a    *adapter.Adapter
	}
	adapters := make([]entry, 0, len(m.adapters))
	for n, a := range m.adapters {
		if a.State() != adapter.StateReady {
			continue
		}
		adapters = append(adapters, entry{name: n, a: a})
	}
	m.mu.Unlock()

	var first error
	for _, e := range adapters {
		if err := e.a.Abort(ctx); err != nil {
			m.addNotice(adapter.Notification{
				Plugin:  e.name,
				Message: "扩展中止失败：" + err.Error(),
				Type:    "warning",
			})
			if first == nil {
				first = err
			}
		}
	}
	return first
}

// ConfirmSessionSwitch asks extensions whether a session switch may proceed
// (rpc-demo uses session_before_switch to confirm clearing a session). A
// cancel answer aborts the switch; a plugin failure is reported but does not
// block, matching the tool gate's fail-open rule.
func (m *Manager) ConfirmSessionSwitch(ctx context.Context, reason string) error {
	m.mu.Lock()
	type entry struct {
		name string
		a    *adapter.Adapter
	}
	adapters := make([]entry, 0, len(m.adapters))
	for n, a := range m.adapters {
		if a.State() != adapter.StateReady {
			continue
		}
		adapters = append(adapters, entry{name: n, a: a})
	}
	m.mu.Unlock()

	for _, e := range adapters {
		if !e.a.WantsEventAck() {
			continue
		}
		result, err := e.a.RequestEvent(ctx, "session_before_switch", map[string]any{"reason": reason})
		if err != nil {
			m.addNotice(adapter.Notification{
				Plugin:  e.name,
				Message: "会话切换确认没有响应：" + err.Error(),
				Type:    "warning",
			})
			continue
		}
		if cancel, _ := result["cancel"].(bool); cancel {
			return errors.New("会话切换已被扩展取消")
		}
	}
	return nil
}

// GateToolCall asks every extension that accepts event requests whether a tool
// call may run. The first block wins; a failing extension is reported and does
// not block (fail-open, design/phase-6/runtime.md §10).
func (m *Manager) GateToolCall(ctx context.Context, name string, args json.RawMessage) (string, error) {
	m.mu.Lock()
	type entry struct {
		name string
		a    *adapter.Adapter
	}
	adapters := make([]entry, 0, len(m.adapters))
	for n, a := range m.adapters {
		if a.State() != adapter.StateReady {
			continue
		}
		adapters = append(adapters, entry{name: n, a: a})
	}
	m.mu.Unlock()

	for _, e := range adapters {
		if !e.a.WantsEventAck() {
			continue
		}
		result, err := e.a.RequestEvent(ctx, "tool_call", map[string]any{
			"toolName": name,
			"input":    args,
		})
		if err != nil {
			m.addNotice(adapter.Notification{
				Plugin:  e.name,
				Message: "工具授权请求没有响应：" + err.Error(),
				Type:    "warning",
			})
			continue
		}
		if block, _ := result["block"].(bool); block {
			reason, _ := result["reason"].(string)
			if reason == "" {
				reason = "blocked by " + e.name
			}
			return reason, nil
		}
	}
	return "", nil
}
