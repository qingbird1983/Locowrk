package extensions

import (
	"context"
	"encoding/json"
	"fmt"
	"sort"

	"github.com/qingbird1983/locowrk/internal/adapter"
	"github.com/qingbird1983/locowrk/internal/kernel"
)

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
			// A crashed or stopped process must not keep showing "running".
			switch a.State() {
			case adapter.StateFailed:
				cp.State = "failed"
				if err := a.Err(); err != nil {
					cp.Reason = err.Error()
				}
			case adapter.StateReady:
				cp.State, cp.Reason = "running", ""
			}
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
		if a.State() != adapter.StateReady {
			continue
		}
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
