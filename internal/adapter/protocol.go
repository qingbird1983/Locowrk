package adapter

import (
	"encoding/json"
	"errors"
	"fmt"
	"time"
)

// JSON-RPC and protocol error codes (proto/plugin-protocol.md §9).
const (
	CodeParseError          = -32700
	CodeInvalidRequest      = -32600
	CodeMethodNotFound      = -32601
	CodeInvalidParams       = -32602
	CodeInternalError       = -32603
	CodeUnsupported         = 4400
	CodeNotImplemented      = 4401
	CodePermissionDenied    = 4402
	CodeTimeout             = 4403
	CodeCrashed             = 4404
	CodeProtocolError       = 4405
	CodeIncompatibleVersion = 4406
)

// Well-known errors. Callers use errors.Is; every RPCError unwraps to one.
var (
	ErrUnsupported         = errors.New("adapter: unsupported")
	ErrNotImplemented      = errors.New("adapter: not implemented")
	ErrPermissionDenied    = errors.New("adapter: permission denied")
	ErrTimeout             = errors.New("adapter: timeout")
	ErrCrashed             = errors.New("adapter: plugin crashed")
	ErrProtocol            = errors.New("adapter: protocol error")
	ErrIncompatibleVersion = errors.New("adapter: incompatible protocol version")
)

// RPCError is a JSON-RPC error object from the plugin.
type RPCError struct {
	Code    int    `json:"code"`
	Message string `json:"message"`
}

func (e *RPCError) Error() string {
	return fmt.Sprintf("adapter: rpc %d: %s", e.Code, e.Message)
}

// Unwrap maps the numeric code onto the package's sentinel errors so callers can
// use errors.Is instead of comparing numbers.
func (e *RPCError) Unwrap() error {
	switch e.Code {
	case CodeUnsupported:
		return ErrUnsupported
	case CodeNotImplemented:
		return ErrNotImplemented
	case CodePermissionDenied:
		return ErrPermissionDenied
	case CodeTimeout:
		return ErrTimeout
	case CodeCrashed:
		return ErrCrashed
	case CodeProtocolError:
		return ErrProtocol
	case CodeIncompatibleVersion:
		return ErrIncompatibleVersion
	}
	return nil
}

type rpcMessage struct {
	JSONRPC string          `json:"jsonrpc"`
	ID      json.RawMessage `json:"id,omitempty"`
	Method  string          `json:"method,omitempty"`
	Params  json.RawMessage `json:"params,omitempty"`
	Result  json.RawMessage `json:"result,omitempty"`
	Error   *RPCError       `json:"error,omitempty"`
}

func (m rpcMessage) hasID() bool {
	id := string(m.ID)
	return id != "" && id != "null"
}

// rpcResult is the envelope of a successful response.
type rpcResult struct {
	JSONRPC string          `json:"jsonrpc"`
	ID      json.RawMessage `json:"id"`
	Result  any             `json:"result"`
}

type rpcErrorResult struct {
	JSONRPC string          `json:"jsonrpc"`
	ID      json.RawMessage `json:"id"`
	Error   *RPCError       `json:"error"`
}

type rpcRequest struct {
	JSONRPC string `json:"jsonrpc"`
	ID      string `json:"id,omitempty"`
	Method  string `json:"method"`
	Params  any    `json:"params,omitempty"`
}

// initializeParams is the handshake payload (proto §3).
type initializeParams struct {
	Protocol int `json:"protocol"`
	Host     struct {
		Name    string `json:"name"`
		Version string `json:"version"`
	} `json:"host"`
	Session struct {
		ID  string `json:"id"`
		Cwd string `json:"cwd"`
	} `json:"session"`
}

type initializeResult struct {
	Protocol int `json:"protocol"`
	Plugin   struct {
		Name    string `json:"name"`
		Version string `json:"version"`
	} `json:"plugin"`
	Capabilities []Capability `json:"capabilities"`
	// WantsEventAck asks the host to send lifecycle events as requests.
	WantsEventAck bool `json:"wantsEventAck,omitempty"`
	// Shim is the bundled shim version, empty for non-shim mock plugins.
	Shim string `json:"shim,omitempty"`
}

// Tool is a tool registered by a plugin. Parameters is a JSON Schema document
// and is validated by the kernel before execution.
type Tool struct {
	Name        string          `json:"name"`
	Description string          `json:"description"`
	Parameters  json.RawMessage `json:"parameters"`
	TimeoutMS   int             `json:"timeoutMs,omitempty"`
	Plugin      string          `json:"plugin"`
}

// Command is a command-panel entry registered by a plugin.
type Command struct {
	Name        string          `json:"name"`
	Description string          `json:"description"`
	Args        json.RawMessage `json:"args,omitempty"`
	Plugin      string          `json:"plugin"`
}

// ToolContent is one content block in an invoke_tool result.
type ToolContent struct {
	Type string `json:"type"`
	Text string `json:"text,omitempty"`
}

// ToolResult is the result of plugin tool execution (proto §6.1).
type ToolResult struct {
	Content []ToolContent   `json:"content"`
	Details json.RawMessage `json:"details,omitempty"`
	IsError bool            `json:"isError,omitempty"`
}

// Text joins the text blocks, which is what the model sees.
func (r ToolResult) Text() string {
	out := ""
	for _, c := range r.Content {
		if c.Type == "text" {
			out += c.Text
		}
	}
	return out
}

// Notification is a ui_notify payload mapped into the host UI.
type Notification struct {
	Plugin  string `json:"plugin"`
	Message string `json:"message"`
	Type    string `json:"type"`
}

// Diagnostic is one compatibility degradation (proto §7).
type Diagnostic struct {
	Plugin   string    `json:"plugin"`
	Member   string    `json:"member"`
	Category string    `json:"category"`
	Code     string    `json:"code"`
	Message  string    `json:"message"`
	Count    int       `json:"count"`
	FirstAt  time.Time `json:"firstAt"`
}

// Diagnostic categories.
const (
	CategoryNotImplemented = "not_implemented"
	CategoryUnsupported    = "unsupported"
)
