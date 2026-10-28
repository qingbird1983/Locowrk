// Command mock-orchestrator is the Phase 2 protocol fixture: a minimal
// orchestration plugin that exercises the supported, deferred, TUI no-op,
// crash and hang paths of proto/plugin-protocol.md.
//
// It is a test fixture, not a product plugin. Behaviour is selected with the
// LOCOWRK_MOCK_MODE environment variable: supported (default), crash, hang.
package main

import (
	"bufio"
	"encoding/json"
	"fmt"
	"os"
	"strings"
	"sync"
	"sync/atomic"
	"time"
)

type message struct {
	JSONRPC string          `json:"jsonrpc"`
	ID      json.RawMessage `json:"id,omitempty"`
	Method  string          `json:"method,omitempty"`
	Params  json.RawMessage `json:"params,omitempty"`
	Result  json.RawMessage `json:"result,omitempty"`
	Error   *rpcError       `json:"error,omitempty"`
}

type rpcError struct {
	Code    int    `json:"code"`
	Message string `json:"message"`
}

type plugin struct {
	mu      sync.Mutex
	out     *bufio.Writer
	pending map[string]chan message
	nextID  int
	mode    string
	hung    atomic.Bool

	lastHostError string
}

func main() {
	p := &plugin{
		out:     bufio.NewWriter(os.Stdout),
		pending: map[string]chan message{},
		mode:    envOr("LOCOWRK_MOCK_MODE", "supported"),
	}
	// Reading stdin is the process's main loop; there is no second long-lived
	// goroutine, so the runtime never mistakes the fixture for a deadlock.
	p.readLoop(bufio.NewReaderSize(os.Stdin, 64*1024))
}

func (p *plugin) readLoop(r *bufio.Reader) {
	for {
		line, err := r.ReadBytes('\n')
		if err != nil {
			os.Exit(0)
		}
		if len(line) == 0 {
			continue
		}
		var msg message
		if err := json.Unmarshal(line, &msg); err != nil {
			continue
		}
		if p.hung.Load() {
			continue
		}
		if msg.Method != "" {
			switch msg.Method {
			case "invoke_tool", "invoke_command":
				// Host calls may themselves call back into the host, so they run
				// outside the read loop.
				go p.handleHost(msg)
			default:
				p.handleHost(msg)
			}
			continue
		}
		if len(msg.ID) > 0 {
			p.mu.Lock()
			key := strings.Trim(string(msg.ID), "\"")
			ch := p.pending[key]
			delete(p.pending, key)
			p.mu.Unlock()
			if ch != nil {
				ch <- msg
			}
		}
	}
}

func (p *plugin) handleHost(msg message) {
	switch msg.Method {
	case "initialize":
		p.register()
		p.respond(msg.ID, map[string]any{
			"protocol": 1,
			"plugin":   map[string]string{"name": "mock-orchestrator", "version": "0.1.0"},
			"capabilities": []map[string]any{
				{"name": "fs.write", "scope": []string{"notes/**", "out/**"}},
			},
		})
		switch p.mode {
		case "crash":
			time.AfterFunc(150*time.Millisecond, func() {
				fmt.Fprintln(os.Stderr, "mock: crashing on purpose")
				os.Exit(3)
			})
		case "hang":
			// Stop answering anything, including heartbeats and RPCs.
			p.hung.Store(true)
		}
	case "ping":
		p.respond(msg.ID, map[string]any{})
	case "invoke_tool":
		p.invokeTool(msg)
	case "invoke_command":
		p.invokeCommand(msg)
	case "event":
		// Lifecycle notification; nothing to do in the fixture.
	case "shutdown_plugin":
		p.respond(msg.ID, map[string]any{"ok": true})
		os.Exit(0)
	default:
		p.respondError(msg.ID, 4400, "unsupported method "+msg.Method)
	}
}

func (p *plugin) register() {
	p.notify("register_tool", map[string]any{
		"name":        "mock_echo",
		"description": "Echo text back; the fixture's supported tool.",
		"parameters": map[string]any{
			"type":       "object",
			"properties": map[string]any{"text": map[string]any{"type": "string"}},
			"required":   []string{"text"},
		},
	})
	p.notify("register_tool", map[string]any{
		"name":        "mock_host_error",
		"description": "Return the last host-side capability error code.",
		"parameters":  map[string]any{"type": "object", "properties": map[string]any{}},
	})
	p.notify("register_command", map[string]any{
		"name":        "mock_ui",
		"description": "Exercise notify and the TUI no-op path.",
	})
	p.notify("register_command", map[string]any{
		"name":        "mock_events",
		"description": "Emit subagent and workflow events.",
	})
	p.notify("register_command", map[string]any{
		"name":        "mock_deferred",
		"description": "Ask for a deferred host capability.",
	})
	p.notify("register_command", map[string]any{
		"name":        "mock_denied",
		"description": "Attempt a file write through the host capability gate.",
	})
}

func (p *plugin) invokeTool(msg message) {
	var params struct {
		Name      string          `json:"name"`
		Arguments json.RawMessage `json:"arguments"`
	}
	_ = json.Unmarshal(msg.Params, &params)
	switch params.Name {
	case "mock_echo":
		var args struct {
			Text string `json:"text"`
		}
		_ = json.Unmarshal(params.Arguments, &args)
		p.respond(msg.ID, map[string]any{
			"content": []map[string]string{{"type": "text", "text": "echo:" + args.Text}},
			"details": map[string]string{"text": args.Text},
		})
	case "mock_host_error":
		p.mu.Lock()
		last := p.lastHostError
		p.mu.Unlock()
		p.respond(msg.ID, map[string]any{
			"content": []map[string]string{{"type": "text", "text": last}},
			"details": map[string]string{},
		})
	default:
		p.respondError(msg.ID, 4401, "unknown tool "+params.Name)
	}
}

func (p *plugin) invokeCommand(msg message) {
	var params struct {
		CommandID string `json:"commandId"`
	}
	_ = json.Unmarshal(msg.Params, &params)
	switch params.CommandID {
	case "mock_ui":
		p.notify("ui_notify", map[string]string{"message": "mock ui ready", "type": "info"})
		p.notify("ui_set_widget", map[string]any{"key": "mock", "content": []string{"widget"}})
		p.notify("ui_set_widget", map[string]any{"key": "mock", "content": []string{"widget again"}})
		p.respond(msg.ID, nil)
	case "mock_events":
		p.notify("emit_event", map[string]any{"kind": "subagent_spawned", "payload": map[string]string{"id": "a1"}})
		p.notify("workflow_step", map[string]any{"name": "review", "status": "running"})
		p.notify("workflow_step", map[string]any{"name": "review", "status": "done"})
		p.notify("emit_event", map[string]any{"kind": "subagent_settled", "payload": map[string]string{"id": "a1"}})
		p.respond(msg.ID, nil)
	case "mock_deferred":
		resp, err := p.request("spawn_subagent", map[string]string{"prompt": "do work"})
		if err != nil {
			p.setHostError(fmt.Sprintf("%d", err.Code))
		} else {
			_ = resp
			p.setHostError("no error")
		}
		p.notify("ui_notify", map[string]string{"message": "deferred handled", "type": "info"})
		p.respond(msg.ID, nil)
	case "mock_denied":
		path := envOr("LOCOWRK_MOCK_FS_PATH", "notes/ok.txt")
		_, err := p.request("fs_write", map[string]string{"path": path, "data": "fixture"})
		if err != nil {
			p.setHostError(fmt.Sprintf("%d", err.Code))
		} else {
			p.setHostError("0")
		}
		p.respond(msg.ID, nil)
	default:
		p.respondError(msg.ID, 4401, "unknown command "+params.CommandID)
	}
}

func (p *plugin) setHostError(code string) {
	p.mu.Lock()
	p.lastHostError = code
	p.mu.Unlock()
}

// request sends a plugin→host request and waits for its response.
func (p *plugin) request(method string, params any) (json.RawMessage, *rpcError) {
	p.mu.Lock()
	p.nextID++
	id := fmt.Sprintf("%d", p.nextID)
	ch := make(chan message, 1)
	p.pending[id] = ch
	p.mu.Unlock()

	p.write(map[string]any{"jsonrpc": "2.0", "id": id, "method": method, "params": params})
	msg := <-ch
	return msg.Result, msg.Error
}

func (p *plugin) respond(id json.RawMessage, result any) {
	p.write(map[string]any{"jsonrpc": "2.0", "id": id, "result": result})
}

func (p *plugin) respondError(id json.RawMessage, code int, message string) {
	p.write(map[string]any{"jsonrpc": "2.0", "id": id, "error": map[string]any{"code": code, "message": message}})
}

func (p *plugin) notify(method string, params any) {
	p.write(map[string]any{"jsonrpc": "2.0", "method": method, "params": params})
}

func (p *plugin) write(v any) {
	data, err := json.Marshal(v)
	if err != nil {
		return
	}
	p.mu.Lock()
	defer p.mu.Unlock()
	_, _ = p.out.Write(append(data, '\n'))
	_ = p.out.Flush()
}

func envOr(key, fallback string) string {
	if v := os.Getenv(key); v != "" {
		return v
	}
	return fallback
}
