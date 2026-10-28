package kernel

import (
	"encoding/json"
	"strconv"

	"github.com/sky-valley/pi/agent"
	"github.com/sky-valley/pi/ai"

	"github.com/qingbird1983/locowrk/internal/eventbus"
)

// messageDTO flattens a transcript message into the project's stable shape.
func messageDTO(m agent.AgentMessage) eventbus.MessageDTO {
	dto := eventbus.MessageDTO{
		ID:        messageKey(m),
		Role:      string(m.MessageRole()),
		Timestamp: messageTimestamp(m),
	}
	switch v := m.(type) {
	case ai.SystemMessage:
		dto.Text, dto.Thinking, dto.ToolCalls = contentParts(v.Content)
	case *ai.SystemMessage:
		dto.Text, dto.Thinking, dto.ToolCalls = contentParts(v.Content)
	case ai.UserMessage:
		dto.Text, dto.Thinking, dto.ToolCalls = contentParts(v.Content)
	case *ai.UserMessage:
		dto.Text, dto.Thinking, dto.ToolCalls = contentParts(v.Content)
	case ai.AssistantMessage:
		dto.Text, dto.Thinking, dto.ToolCalls = contentParts(v.Content)
		dto.StopReason = string(v.StopReason)
		dto.Error = v.ErrorMessage
	case *ai.AssistantMessage:
		dto.Text, dto.Thinking, dto.ToolCalls = contentParts(v.Content)
		dto.StopReason = string(v.StopReason)
		dto.Error = v.ErrorMessage
	case ai.ToolResultMessage:
		dto.Text, _, _ = contentParts(v.Content)
		if v.IsError {
			dto.Error = "tool failed"
		}
	case *ai.ToolResultMessage:
		dto.Text, _, _ = contentParts(v.Content)
		if v.IsError {
			dto.Error = "tool failed"
		}
	}
	return dto
}

// contentParts splits a content list into text, thinking and tool calls.
func contentParts(cl ai.ContentList) (text, thinking string, calls []eventbus.ToolCallDTO) {
	for _, c := range cl {
		switch v := c.(type) {
		case ai.TextContent:
			text += v.Text
		case *ai.TextContent:
			text += v.Text
		case ai.ThinkingContent:
			thinking += v.Thinking
		case *ai.ThinkingContent:
			thinking += v.Thinking
		case ai.ToolCall:
			calls = append(calls, toolCallDTO(v))
		case *ai.ToolCall:
			calls = append(calls, toolCallDTO(*v))
		}
	}
	return text, thinking, calls
}

func toolCallDTO(tc ai.ToolCall) eventbus.ToolCallDTO {
	dto := eventbus.ToolCallDTO{ID: tc.ID, Name: tc.Name}
	if tc.Arguments != nil {
		if raw, err := json.Marshal(tc.Arguments); err == nil {
			dto.Arguments = raw
		}
	}
	return dto
}

// messageKey builds a stable identifier for a message. Upstream messages carry
// no ID, so the transcript timestamp plus role is the best available key; it is
// stable for the lifetime of a message and unique enough for ordering.
func messageKey(m agent.AgentMessage) string {
	return string(m.MessageRole()) + "-" + strconv.FormatInt(messageTimestamp(m), 10)
}

func messageTimestamp(m agent.AgentMessage) int64 {
	switch v := m.(type) {
	case ai.SystemMessage:
		return v.Timestamp
	case *ai.SystemMessage:
		return v.Timestamp
	case ai.UserMessage:
		return v.Timestamp
	case *ai.UserMessage:
		return v.Timestamp
	case ai.AssistantMessage:
		return v.Timestamp
	case *ai.AssistantMessage:
		return v.Timestamp
	case ai.ToolResultMessage:
		return v.Timestamp
	case *ai.ToolResultMessage:
		return v.Timestamp
	}
	return 0
}

// eventKind maps an upstream lifecycle event to its project kind.
func eventKind(t agent.EventType) (eventbus.Kind, bool) {
	switch t {
	case agent.EvAgentStart:
		return eventbus.KindAgentStart, true
	case agent.EvAgentEnd:
		return eventbus.KindAgentEnd, true
	case agent.EvTurnStart:
		return eventbus.KindTurnStart, true
	case agent.EvTurnEnd:
		return eventbus.KindTurnEnd, true
	case agent.EvMessageStart:
		return eventbus.KindMessageStart, true
	case agent.EvMessageUpdate:
		return eventbus.KindMessageUpdate, true
	case agent.EvMessageEnd:
		return eventbus.KindMessageEnd, true
	case agent.EvToolExecutionStart:
		return eventbus.KindToolExecutionStart, true
	case agent.EvToolExecutionUpdate:
		return eventbus.KindToolExecutionUpdate, true
	case agent.EvToolExecutionEnd:
		return eventbus.KindToolExecutionEnd, true
	}
	return "", false
}

// payloadFor converts an upstream event into the project's DTO for its kind.
func payloadFor(e agent.AgentEvent) any {
	switch e.Type {
	case agent.EvMessageStart, agent.EvMessageEnd:
		dto := messageDTO(e.Message)
		return &dto
	case agent.EvMessageUpdate:
		return updateDTO(e)
	case agent.EvTurnEnd:
		out := eventbus.TurnEndDTO{}
		if e.Message != nil {
			msg := messageDTO(e.Message)
			out.Message = &msg
		}
		for _, tr := range e.ToolResults {
			out.ToolResults = append(out.ToolResults, messageDTO(tr))
		}
		return out
	case agent.EvAgentEnd:
		out := eventbus.AgentEndDTO{Messages: make([]eventbus.MessageDTO, 0, len(e.Messages))}
		for _, m := range e.Messages {
			out.Messages = append(out.Messages, messageDTO(m))
		}
		return out
	case agent.EvToolExecutionStart:
		dto := eventbus.ToolExecutionDTO{ToolCallID: e.ToolCallID, ToolName: e.ToolName}
		if e.Args != nil {
			if raw, err := json.Marshal(e.Args); err == nil {
				dto.Arguments = raw
			}
		}
		return dto
	case agent.EvToolExecutionUpdate:
		dto := eventbus.ToolExecutionDTO{ToolCallID: e.ToolCallID, ToolName: e.ToolName}
		dto.Text = resultText(e.PartialResult)
		return dto
	case agent.EvToolExecutionEnd:
		dto := eventbus.ToolExecutionDTO{
			ToolCallID: e.ToolCallID,
			ToolName:   e.ToolName,
			Text:       resultText(e.Result),
			IsError:    e.IsError,
			DurationMs: e.DurationMs,
		}
		return dto
	}
	return nil
}

// updateDTO turns a streaming assistant event into the delta DTO.
func updateDTO(e agent.AgentEvent) eventbus.MessageUpdateDTO {
	dto := eventbus.MessageUpdateDTO{MessageID: messageKey(e.Message)}
	ev := e.AssistantMessageEvent
	if ev == nil {
		return dto
	}
	dto.Event = string(ev.Type)
	dto.BlockIndex = ev.ContentIndex
	dto.BlockType = blockType(ev.Type)
	dto.Delta = ev.Delta

	if partial := ev.Partial; partial != nil && ev.ContentIndex >= 0 && ev.ContentIndex < len(partial.Content) {
		switch block := partial.Content[ev.ContentIndex].(type) {
		case ai.TextContent:
			dto.Text = block.Text
		case ai.ThinkingContent:
			dto.Text = block.Thinking
		case ai.ToolCall:
			call := toolCallDTO(block)
			dto.ToolCall = &call
		}
	}
	if ev.ToolCall != nil {
		call := toolCallDTO(*ev.ToolCall)
		dto.ToolCall = &call
	}
	return dto
}

func blockType(t ai.EventType) string {
	switch t {
	case ai.EventTextStart, ai.EventTextDelta, ai.EventTextEnd:
		return "text"
	case ai.EventThinkingStart, ai.EventThinkingDelta, ai.EventThinkingEnd:
		return "thinking"
	case ai.EventToolCallStart, ai.EventToolCallDelta, ai.EventToolCallEnd:
		return "toolcall"
	}
	return ""
}

// resultText renders an arbitrary tool result value as text without leaking
// upstream structs to consumers.
func resultText(v any) string {
	switch r := v.(type) {
	case nil:
		return ""
	case string:
		return r
	case ai.ContentList:
		text, _, _ := contentParts(r)
		return text
	case agent.AgentToolResult:
		text, _, _ := contentParts(r.Content)
		return text
	case *agent.AgentToolResult:
		text, _, _ := contentParts(r.Content)
		return text
	default:
		raw, err := json.Marshal(v)
		if err != nil {
			return ""
		}
		return string(raw)
	}
}
