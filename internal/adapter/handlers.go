package adapter

import (
	"encoding/json"
	"errors"

	"github.com/qingbird1983/locowrk/internal/eventbus"
)

// handleRequest serves one request from the plugin and produces its response.
func (a *Adapter) handleRequest(_ json.RawMessage, method string, params json.RawMessage) (any, *RPCError) {
	return a.handlePluginMessage(method, params)
}

// handleNotify serves one plugin notification; its result is discarded.
func (a *Adapter) handleNotify(method string, params json.RawMessage) {
	_, _ = a.handlePluginMessage(method, params)
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
	case "ui_notify":
		return a.uiNotify(params)
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

func (a *Adapter) signalRegistration() {
	select {
	case a.regCh <- struct{}{}:
	default:
	}
}
