// Command host-c is the Go-rewrite route of the Phase 2 runtime spike.
//
// It does not execute the extension; it re-implements what the probe samples
// do. The point is a like-for-like startup/memory reading and a concrete
// reminder that route C buys compatibility one extension at a time.
package main

import (
	"bufio"
	"encoding/json"
	"flag"
	"os"
	"os/exec"
	"time"
)

type message struct {
	JSONRPC string          `json:"jsonrpc"`
	ID      *int            `json:"id,omitempty"`
	Method  string          `json:"method"`
	Params  json.RawMessage `json:"params,omitempty"`
	Result  json.RawMessage `json:"result,omitempty"`
	Error   *rpcError       `json:"error,omitempty"`
}

type rpcError struct {
	Code    int    `json:"code"`
	Message string `json:"message"`
}

type host struct {
	out    *bufio.Writer
	sample string
}

func main() {
	sample := flag.String("sample", "ui", "sample being rewritten: ui or orchestrator")
	flag.Parse()
	h := &host{out: bufio.NewWriter(os.Stdout), sample: *sample}
	scanner := bufio.NewScanner(os.Stdin)
	scanner.Buffer(make([]byte, 64*1024), 8*1024*1024)
	for scanner.Scan() {
		line := scanner.Bytes()
		if len(line) == 0 {
			continue
		}
		var msg message
		if err := json.Unmarshal(line, &msg); err != nil {
			h.respond(nil, nil, &rpcError{Code: 4405, Message: err.Error()})
			continue
		}
		if h.handle(msg) {
			return
		}
	}
}

func (h *host) send(v any) {
	data, _ := json.Marshal(v)
	_, _ = h.out.Write(data)
	_ = h.out.WriteByte('\n')
	_ = h.out.Flush()
}

func (h *host) notify(method string, params any) {
	h.send(message{JSONRPC: "2.0", Method: method, Params: mustRaw(params)})
}

func (h *host) respond(id *int, result any, rerr *rpcError) {
	h.send(message{JSONRPC: "2.0", ID: id, Result: mustRaw(result), Error: rerr})
}

func mustRaw(v any) json.RawMessage {
	if v == nil {
		return json.RawMessage("null")
	}
	data, _ := json.Marshal(v)
	return data
}

func (h *host) handle(msg message) bool {
	switch msg.Method {
	case "initialize":
		h.respond(msg.ID, map[string]any{
			"tools":     []string{"probe_echo", "probe_delegate"},
			"commands":  []string{"probe-ui"},
			"events":    []string{"session_start", "tool_call"},
			"coverage":  map[string]any{"used": []string{}, "implemented": []string{}, "missing": []string{}, "note": "route C has no shim; compatibility is per rewritten extension"},
			"bootError": nil,
		}, nil)
	case "event", "invoke_tool", "invoke_command":
		h.respond(msg.ID, h.dispatch(msg), nil)
	case "metrics":
		h.respond(msg.ID, map[string]any{
			"tools":    []string{"probe_echo", "probe_delegate"},
			"commands": []string{"probe-ui"},
			"events":   []string{"session_start", "tool_call"},
			"coverage": map[string]any{"used": []string{}, "implemented": []string{}, "missing": []string{}, "note": "route C has no shim"},
		}, nil)
	case "shutdown":
		h.respond(msg.ID, map[string]bool{"ok": true}, nil)
		return true
	default:
		h.respond(msg.ID, nil, &rpcError{Code: 4400, Message: "unsupported method: " + msg.Method})
	}
	return false
}

func (h *host) dispatch(msg message) any {
	var params struct {
		Name      string          `json:"name"`
		Arguments json.RawMessage `json:"arguments"`
	}
	_ = json.Unmarshal(msg.Params, &params)

	switch msg.Method {
	case "event":
		if params.Name == "session_start" {
			// What the probe's session_start handler does, rewritten by hand.
			message := "ui-probe started"
			if h.sample == "orchestrator" {
				message = "orchestrator-probe started"
			}
			h.notify("notify", map[string]string{"message": message, "type": "info"})
			h.notify("diagnostic", map[string]any{
				"member": "ui.setWidget", "code": "unsupported", "message": "ui.setWidget", "count": 1,
			})
		}
		return nil
	case "invoke_tool":
		switch params.Name {
		case "probe_echo":
			var args struct {
				Text string `json:"text"`
			}
			_ = json.Unmarshal(params.Arguments, &args)
			return map[string]any{
				"content": []map[string]string{{"type": "text", "text": "echo:" + args.Text}},
				"details": map[string]string{"text": args.Text},
			}
		case "probe_delegate":
			var args struct {
				Task string `json:"task"`
			}
			_ = json.Unmarshal(params.Arguments, &args)
			return h.delegate(args.Task)
		}
	}
	return nil
}

func (h *host) delegate(task string) any {
	start := time.Now()
	cmd := exec.Command("node", "-e", "process.stdout.write('worker:' + process.argv[1])", task)
	out, err := cmd.Output()
	if err != nil {
		return map[string]any{"content": []map[string]string{{"type": "text", "text": "worker failed: " + err.Error()}}, "details": map[string]any{"task": task}}
	}
	return map[string]any{
		"content": []map[string]string{{"type": "text", "text": string(out)}},
		"details": map[string]any{"task": task, "mode": "single", "ms": time.Since(start).Milliseconds()},
	}
}
