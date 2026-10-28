package adapter

import (
	"encoding/json"
	"fmt"
	"strings"
)

// Tools returns the registered tools ordered by registration.
func (a *Adapter) Tools() []Tool {
	a.mu.Lock()
	defer a.mu.Unlock()
	out := make([]Tool, 0, len(a.tools))
	for _, name := range a.toolNames {
		out = append(out, a.tools[name])
	}
	return out
}

// Commands returns the registered commands.
func (a *Adapter) Commands() []Command {
	a.mu.Lock()
	defer a.mu.Unlock()
	out := make([]Command, 0, len(a.commands))
	for _, c := range a.commands {
		out = append(out, c)
	}
	return out
}

// Diagnostics returns every compatibility diagnostic with its count.
func (a *Adapter) Diagnostics() []Diagnostic { return a.diagnostics.snapshot() }

// Audit returns the capability decisions, newest last.
func (a *Adapter) Audit() []AuditRecord {
	a.mu.Lock()
	defer a.mu.Unlock()
	return append([]AuditRecord(nil), a.audit...)
}

// validToolName reports whether a registered tool name fits the protocol.
func validToolName(name string) bool { return toolPattern.MatchString(name) }

// validParameters reports whether parameters is a JSON object schema.
func validParameters(raw json.RawMessage) bool {
	if len(raw) == 0 {
		return false
	}
	var probe map[string]any
	if err := json.Unmarshal(raw, &probe); err != nil {
		return false
	}
	return strings.EqualFold(fmt.Sprint(probe["type"]), "object") || probe["properties"] != nil
}
