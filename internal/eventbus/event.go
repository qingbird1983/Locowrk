// Package eventbus carries project events from the kernel to consumers such as
// the desktop shell, the smoke CLI and, later, plugins.
//
// It exists so that a slow consumer can never stall the agent loop: every
// subscriber owns a bounded queue and its own delivery goroutine.
package eventbus

import (
	"encoding/json"
	"time"
)

// Kind identifies an event. The lifecycle kinds mirror the upstream
// agent.Ev* constants one-to-one; the orchestration kinds are the only ones the
// project adds (architecture/02 §2.2).
type Kind string

const (
	KindAgentStart          Kind = "agent_start"
	KindAgentEnd            Kind = "agent_end"
	KindTurnStart           Kind = "turn_start"
	KindTurnEnd             Kind = "turn_end"
	KindMessageStart        Kind = "message_start"
	KindMessageUpdate       Kind = "message_update"
	KindMessageEnd          Kind = "message_end"
	KindToolExecutionStart  Kind = "tool_execution_start"
	KindToolExecutionUpdate Kind = "tool_execution_update"
	KindToolExecutionEnd    Kind = "tool_execution_end"

	KindSubagentSpawned  Kind = "subagent_spawned"
	KindSubagentSettled  Kind = "subagent_settled"
	KindWorkflowStep     Kind = "workflow_step"
	KindSteeringApplied  Kind = "steering_applied"
	KindPluginDiagnostic Kind = "plugin_diagnostic"
)

// IsLifecycle reports whether kind corresponds to an upstream agent event.
func (k Kind) IsLifecycle() bool {
	switch k {
	case KindAgentStart, KindAgentEnd, KindTurnStart, KindTurnEnd,
		KindMessageStart, KindMessageUpdate, KindMessageEnd,
		KindToolExecutionStart, KindToolExecutionUpdate, KindToolExecutionEnd:
		return true
	}
	return false
}

// Event is one project event. Payload is a stable project DTO, never an
// upstream struct, so that upstream field renames cannot break consumers.
type Event struct {
	// Seq is monotonic within SessionID, starting at 1.
	Seq       uint64          `json:"seq"`
	Kind      Kind            `json:"kind"`
	SessionID string          `json:"sessionId"`
	At        time.Time       `json:"at"`
	Payload   json.RawMessage `json:"payload,omitempty"`
}

// MessageDTO is the transcript view of one message.
type MessageDTO struct {
	ID         string        `json:"id"`
	Role       string        `json:"role"`
	Text       string        `json:"text,omitempty"`
	Thinking   string        `json:"thinking,omitempty"`
	ToolCalls  []ToolCallDTO `json:"toolCalls,omitempty"`
	StopReason string        `json:"stopReason,omitempty"`
	Error      string        `json:"error,omitempty"`
	Timestamp  int64         `json:"timestamp,omitempty"`
}

// ToolCallDTO describes a tool invocation the model asked for.
type ToolCallDTO struct {
	ID        string          `json:"id"`
	Name      string          `json:"name"`
	Arguments json.RawMessage `json:"arguments,omitempty"`
}

// MessageUpdateDTO is the payload of KindMessageUpdate. Delta is the
// incremental text; Text/Thinking are the cumulative snapshot of that block at
// the time of the event.
type MessageUpdateDTO struct {
	MessageID  string       `json:"messageId"`
	BlockIndex int          `json:"blockIndex"`
	BlockType  string       `json:"blockType"`
	Event      string       `json:"event"`
	Delta      string       `json:"delta,omitempty"`
	Text       string       `json:"text,omitempty"`
	ToolCall   *ToolCallDTO `json:"toolCall,omitempty"`
}

// ToolExecutionDTO is the payload of the three tool execution kinds. Result is
// the tool's own text output; Details is deliberately not forwarded.
type ToolExecutionDTO struct {
	ToolCallID string          `json:"toolCallId"`
	ToolName   string          `json:"toolName"`
	Arguments  json.RawMessage `json:"arguments,omitempty"`
	Text       string          `json:"text,omitempty"`
	IsError    bool            `json:"isError,omitempty"`
	DurationMs *int64          `json:"durationMs,omitempty"`
}

// TurnEndDTO is the payload of KindTurnEnd.
type TurnEndDTO struct {
	Message     *MessageDTO  `json:"message,omitempty"`
	ToolResults []MessageDTO `json:"toolResults,omitempty"`
}

// AgentEndDTO is the payload of KindAgentEnd.
type AgentEndDTO struct {
	Messages []MessageDTO `json:"messages,omitempty"`
}
