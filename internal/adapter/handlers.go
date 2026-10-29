package adapter

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"

	"github.com/qingbird1983/locowrk/internal/eventbus"
)

// handleRequest serves one request from the plugin and produces its response.
func (a *Adapter) handleRequest(_ json.RawMessage, method string, params json.RawMessage) (any, *RPCError) {
	return a.handlePluginMessage(method, params)
}

// handleNotify serves one plugin notification; its result is discarded.
func (a *Adapter) handleNotify(method string, params json.RawMessage) {
	// A rejected notification would otherwise vanish: the plugin gets no id to
	// match a response to, so record the reason as a diagnostic instead.
	if _, rerr := a.handlePluginMessage(method, params); rerr != nil {
		a.addDiagnostic(Diagnostic{
			Member:   method,
			Category: CategoryUnsupported,
			Code:     "rejected",
			Message:  rerr.Message,
		})
	}
}

func (a *Adapter) handlePluginMessage(method string, params json.RawMessage) (any, *RPCError) {
	switch method {
	case "register_tool":
		return a.registerTool(params)
	case "register_command":
		return a.registerCommand(params)
	case "emit_event":
		return a.emitEvent(params)
	case "workflow_step":
		return a.workflowStep(params)
	case "spawn_subagent":
		a.addDiagnostic(Diagnostic{
			Member:   "spawn_subagent",
			Category: CategoryNotImplemented,
			Code:     "not_implemented",
			Message:  "host-managed subagents arrive in Phase 6",
		})
		return nil, &RPCError{Code: CodeNotImplemented, Message: "spawn_subagent is not implemented"}
	case "diagnostic":
		return a.recordDiagnostic(params)
	case "tool_result":
		// Phase 2 invokes tools synchronously; async completion is accepted and
		// ignored rather than treated as a protocol error.
		return nil, nil
	case "tool_update":
		return a.toolUpdate(params)
	case "event_result":
		// Phase 6 accepts tool_call/event acknowledgements; hosts that do not
		// send requests can ignore the result.
		return nil, nil
	case "ui_notify":
		return a.uiNotify(params)
	case "ui_select":
		return a.uiSelect(params)
	case "ui_confirm":
		return a.uiConfirm(params)
	case "ui_input":
		return a.uiInput(params)
	case "ui_editor":
		return a.uiEditor(params)
	case "ui_set_widget":
		a.addDiagnostic(Diagnostic{
			Member:   "ui.setWidget",
			Category: CategoryUnsupported,
			Code:     "unsupported",
			Message:  "terminal widgets have no desktop equivalent",
		})
		return nil, nil
	case "fs_write":
		return a.fsWriteRequest(params)
	default:
		return nil, &RPCError{Code: CodeUnsupported, Message: "unsupported method: " + method}
	}
}

func (a *Adapter) registerTool(params json.RawMessage) (any, *RPCError) {
	var req struct {
		Name        string          `json:"name"`
		Description string          `json:"description"`
		Parameters  json.RawMessage `json:"parameters"`
		TimeoutMS   int             `json:"timeoutMs,omitempty"`
	}
	if err := json.Unmarshal(params, &req); err != nil {
		return nil, &RPCError{Code: CodeInvalidParams, Message: err.Error()}
	}
	if !validToolName(req.Name) {
		return nil, &RPCError{Code: CodeInvalidParams, Message: "invalid tool name: " + req.Name}
	}
	if req.Description == "" {
		return nil, &RPCError{Code: CodeInvalidParams, Message: "tool description is required"}
	}
	if !validParameters(req.Parameters) {
		return nil, &RPCError{Code: CodeInvalidParams, Message: "parameters must be a JSON Schema object"}
	}
	if req.TimeoutMS != 0 && (req.TimeoutMS < 1000 || req.TimeoutMS > 600000) {
		return nil, &RPCError{Code: CodeInvalidParams, Message: "timeoutMs out of range"}
	}

	a.mu.Lock()
	if a.state == StateReady {
		a.mu.Unlock()
		a.addDiagnostic(Diagnostic{
			Member:   "register_tool",
			Category: CategoryUnsupported,
			Code:     "late_registration",
			Message:  "tools must be registered during the handshake",
		})
		return nil, &RPCError{Code: CodeInvalidRequest, Message: "registration closed"}
	}
	if _, exists := a.tools[req.Name]; exists {
		a.mu.Unlock()
		return nil, &RPCError{Code: CodeInvalidParams, Message: "duplicate tool: " + req.Name}
	}
	a.tools[req.Name] = Tool{
		Name:        req.Name,
		Description: req.Description,
		Parameters:  req.Parameters,
		TimeoutMS:   req.TimeoutMS,
		Plugin:      a.pluginName,
	}
	a.toolNames = append(a.toolNames, req.Name)
	a.mu.Unlock()

	a.signalRegistration()
	return nil, nil
}

func (a *Adapter) registerCommand(params json.RawMessage) (any, *RPCError) {
	var req struct {
		Name        string          `json:"name"`
		Description string          `json:"description"`
		Args        json.RawMessage `json:"args,omitempty"`
	}
	if err := json.Unmarshal(params, &req); err != nil {
		return nil, &RPCError{Code: CodeInvalidParams, Message: err.Error()}
	}
	if !validToolName(req.Name) {
		return nil, &RPCError{Code: CodeInvalidParams, Message: "invalid command name: " + req.Name}
	}

	a.mu.Lock()
	if a.state == StateReady {
		a.mu.Unlock()
		a.addDiagnostic(Diagnostic{
			Member:   "register_command",
			Category: CategoryUnsupported,
			Code:     "late_registration",
			Message:  "commands must be registered during the handshake",
		})
		return nil, &RPCError{Code: CodeInvalidRequest, Message: "registration closed"}
	}
	if _, exists := a.commands[req.Name]; exists {
		a.mu.Unlock()
		return nil, &RPCError{Code: CodeInvalidParams, Message: "duplicate command: " + req.Name}
	}
	a.commands[req.Name] = Command{
		Name:        req.Name,
		Description: req.Description,
		Args:        req.Args,
		Plugin:      a.pluginName,
	}
	a.mu.Unlock()

	a.signalRegistration()
	return nil, nil
}

func (a *Adapter) emitEvent(params json.RawMessage) (any, *RPCError) {
	var req struct {
		Kind    string          `json:"kind"`
		Payload json.RawMessage `json:"payload"`
	}
	if err := json.Unmarshal(params, &req); err != nil {
		return nil, &RPCError{Code: CodeInvalidParams, Message: err.Error()}
	}
	kind, ok := eventKind(req.Kind)
	if !ok {
		return nil, &RPCError{Code: CodeUnsupported, Message: "event kind not allowed: " + req.Kind}
	}
	a.publish(kind, json.RawMessage(req.Payload))
	return nil, nil
}

func (a *Adapter) workflowStep(params json.RawMessage) (any, *RPCError) {
	var req struct {
		Name   string `json:"name"`
		Status string `json:"status"`
		Detail string `json:"detail,omitempty"`
	}
	if err := json.Unmarshal(params, &req); err != nil {
		return nil, &RPCError{Code: CodeInvalidParams, Message: err.Error()}
	}
	if req.Name == "" {
		return nil, &RPCError{Code: CodeInvalidParams, Message: "workflow step needs a name"}
	}
	switch req.Status {
	case "pending", "running", "done", "failed", "skipped":
	default:
		return nil, &RPCError{Code: CodeInvalidParams, Message: "invalid workflow status: " + req.Status}
	}
	a.publish(eventbus.KindWorkflowStep, map[string]string{
		"name":   req.Name,
		"status": req.Status,
		"detail": req.Detail,
	})
	return nil, nil
}

func (a *Adapter) recordDiagnostic(params json.RawMessage) (any, *RPCError) {
	var req struct {
		Member   string `json:"member"`
		Category string `json:"category"`
		Code     string `json:"code"`
		Message  string `json:"message"`
	}
	if err := json.Unmarshal(params, &req); err != nil {
		return nil, &RPCError{Code: CodeInvalidParams, Message: err.Error()}
	}
	if req.Member == "" || req.Code == "" {
		return nil, &RPCError{Code: CodeInvalidParams, Message: "diagnostic needs member and code"}
	}
	if req.Category != CategoryNotImplemented && req.Category != CategoryUnsupported {
		return nil, &RPCError{Code: CodeInvalidParams, Message: "invalid diagnostic category: " + req.Category}
	}
	a.addDiagnostic(Diagnostic{
		Member:   req.Member,
		Category: req.Category,
		Code:     req.Code,
		Message:  req.Message,
	})
	// A factory that threw means the extension cannot work at all: disable it
	// instead of leaving a live process with no tools (phase-6 §5.8).
	if req.Code == "factory_error" {
		a.setFailed(fmt.Errorf("extension failed to initialize: %s", req.Message))
		a.mu.Lock()
		p := a.peer
		a.mu.Unlock()
		if p != nil {
			go p.kill()
		}
	}
	return nil, nil
}

func (a *Adapter) uiNotify(params json.RawMessage) (any, *RPCError) {
	var req struct {
		Message string `json:"message"`
		Type    string `json:"type"`
	}
	if err := json.Unmarshal(params, &req); err != nil {
		return nil, &RPCError{Code: CodeInvalidParams, Message: err.Error()}
	}
	if req.Message == "" {
		return nil, &RPCError{Code: CodeInvalidParams, Message: "notify needs a message"}
	}
	switch req.Type {
	case "", "info", "warning", "error":
	default:
		return nil, &RPCError{Code: CodeInvalidParams, Message: "invalid notification type: " + req.Type}
	}
	if req.Type == "" {
		req.Type = "info"
	}
	if a.opts.Notify != nil {
		a.opts.Notify(Notification{Plugin: a.pluginName, Message: req.Message, Type: req.Type})
	}
	return nil, nil
}

func (a *Adapter) fsWriteRequest(params json.RawMessage) (any, *RPCError) {
	var req struct {
		Path string `json:"path"`
		Data string `json:"data"`
	}
	if err := json.Unmarshal(params, &req); err != nil {
		return nil, &RPCError{Code: CodeInvalidParams, Message: err.Error()}
	}
	n, err := a.fsWrite(a.opts.Cwd, req.Path, []byte(req.Data))
	if err != nil {
		if errors.Is(err, ErrPermissionDenied) {
			return nil, &RPCError{Code: CodePermissionDenied, Message: err.Error()}
		}
		return nil, &RPCError{Code: CodeInternalError, Message: err.Error()}
	}
	return map[string]int{"bytes": n}, nil
}

func (a *Adapter) toolUpdate(params json.RawMessage) (any, *RPCError) {
	var req struct {
		ToolCallID string `json:"toolCallId"`
		Text       string `json:"text"`
	}
	if err := json.Unmarshal(params, &req); err != nil {
		return nil, &RPCError{Code: CodeInvalidParams, Message: err.Error()}
	}
	if req.ToolCallID == "" {
		return nil, &RPCError{Code: CodeInvalidParams, Message: "tool_update needs toolCallId"}
	}
	a.publish(eventbus.KindToolExecutionUpdate, map[string]string{
		"toolCallId": req.ToolCallID,
		"text":       req.Text,
	})
	return nil, nil
}

func (a *Adapter) uiSelect(params json.RawMessage) (any, *RPCError) {
	var req struct {
		Title   string   `json:"title"`
		Options []string `json:"options"`
	}
	if err := json.Unmarshal(params, &req); err != nil {
		return nil, &RPCError{Code: CodeInvalidParams, Message: err.Error()}
	}
	if req.Title == "" || len(req.Options) == 0 {
		return nil, &RPCError{Code: CodeInvalidParams, Message: "select needs a title and options"}
	}
	if a.opts.UI == nil {
		a.uiUnsupported("ui.select")
		return nil, nil
	}
	ctx, cancel := context.WithTimeout(context.Background(), a.opts.UITimeout)
	defer cancel()
	value, ok := a.opts.UI.Select(ctx, req.Title, req.Options)
	if !ok {
		return nil, nil
	}
	return value, nil
}

func (a *Adapter) uiConfirm(params json.RawMessage) (any, *RPCError) {
	var req struct {
		Title   string `json:"title"`
		Message string `json:"message"`
	}
	if err := json.Unmarshal(params, &req); err != nil {
		return nil, &RPCError{Code: CodeInvalidParams, Message: err.Error()}
	}
	if req.Title == "" {
		return nil, &RPCError{Code: CodeInvalidParams, Message: "confirm needs a title"}
	}
	if a.opts.UI == nil {
		a.uiUnsupported("ui.confirm")
		return false, nil
	}
	ctx, cancel := context.WithTimeout(context.Background(), a.opts.UITimeout)
	defer cancel()
	yes, ok := a.opts.UI.Confirm(ctx, req.Title, req.Message)
	if !ok {
		return false, nil
	}
	return yes, nil
}

func (a *Adapter) uiInput(params json.RawMessage) (any, *RPCError) {
	var req struct {
		Title       string `json:"title"`
		Placeholder string `json:"placeholder"`
	}
	if err := json.Unmarshal(params, &req); err != nil {
		return nil, &RPCError{Code: CodeInvalidParams, Message: err.Error()}
	}
	if req.Title == "" {
		return nil, &RPCError{Code: CodeInvalidParams, Message: "input needs a title"}
	}
	if a.opts.UI == nil {
		a.uiUnsupported("ui.input")
		return nil, nil
	}
	ctx, cancel := context.WithTimeout(context.Background(), a.opts.UITimeout)
	defer cancel()
	value, ok := a.opts.UI.Input(ctx, req.Title, req.Placeholder)
	if !ok {
		return nil, nil
	}
	return value, nil
}

func (a *Adapter) uiEditor(params json.RawMessage) (any, *RPCError) {
	var req struct {
		Title   string `json:"title"`
		Prefill string `json:"prefill"`
	}
	if err := json.Unmarshal(params, &req); err != nil {
		return nil, &RPCError{Code: CodeInvalidParams, Message: err.Error()}
	}
	if req.Title == "" {
		return nil, &RPCError{Code: CodeInvalidParams, Message: "editor needs a title"}
	}
	if a.opts.UI == nil {
		a.uiUnsupported("ui.editor")
		return nil, nil
	}
	ctx, cancel := context.WithTimeout(context.Background(), a.opts.UITimeout)
	defer cancel()
	value, ok := a.opts.UI.Editor(ctx, req.Title, req.Prefill)
	if !ok {
		return nil, nil
	}
	return value, nil
}

func (a *Adapter) uiUnsupported(member string) {
	a.addDiagnostic(Diagnostic{
		Member:   member,
		Category: CategoryUnsupported,
		Code:     "unsupported",
		Message:  "the host has no dialog UI attached",
	})
}

func (a *Adapter) signalRegistration() {
	select {
	case a.regCh <- struct{}{}:
	default:
	}
}
