package kernel

import (
	"context"
	"encoding/json"

	"github.com/qingbird1983/locowrk/pi/agent"
)

// BeforeToolFunc runs after a tool call's arguments are validated and before
// the tool executes. A non-empty reason blocks the call; the model sees an
// error tool result carrying it. Hosts use it for extension permission gates.
type BeforeToolFunc func(ctx context.Context, name string, args json.RawMessage) (reason string, err error)

// beforeToolHook adapts a host gate to pi's BeforeToolCall. A nil gate keeps
// the upstream behaviour (no hook).
func beforeToolHook(gate BeforeToolFunc) agent.BeforeToolCallFunc {
	if gate == nil {
		return nil
	}
	return func(ctx context.Context, call agent.BeforeToolCallContext) *agent.BeforeToolCallResult {
		raw, err := json.Marshal(call.Args)
		if err != nil {
			return &agent.BeforeToolCallResult{Block: true, Reason: "tool arguments: " + err.Error()}
		}
		reason, err := gate(ctx, call.ToolCall.Name, raw)
		if err != nil {
			return &agent.BeforeToolCallResult{Block: true, Reason: err.Error()}
		}
		if reason == "" {
			return nil
		}
		return &agent.BeforeToolCallResult{Block: true, Reason: reason}
	}
}
