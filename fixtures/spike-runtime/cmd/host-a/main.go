// Command host-a is the QuickJS route of the Phase 2 runtime spike.
//
// It runs one bundled extension inside modernc.org/quickjs (pure Go, no cgo)
// and speaks the same JSONL host protocol as host-b (Node) and host-c (Go
// rewrite). It is deliberately small and single-purpose: no production code
// imports it.
package main

import (
	"bufio"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"os"
	"sync"
	"time"

	"modernc.org/quickjs"
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
	vm    *quickjs.VM
	write *bufio.Writer
	mu    sync.Mutex
}

func main() {
	bundle := flag.String("bundle", "", "bundled extension (IIFE)")
	flag.Parse()
	if *bundle == "" {
		fmt.Fprintln(os.Stderr, "host-a: --bundle is required")
		os.Exit(2)
	}
	code, err := os.ReadFile(*bundle)
	if err != nil {
		fmt.Fprintf(os.Stderr, "host-a: reading bundle: %v\n", err)
		os.Exit(1)
	}

	h, err := newHost(os.Stdout)
	if err != nil {
		fmt.Fprintf(os.Stderr, "host-a: %v\n", err)
		os.Exit(1)
	}
	defer h.close()

	if _, err := h.vm.Eval(string(code), quickjs.EvalGlobal); err != nil {
		fmt.Fprintf(os.Stderr, "host-a: evaluating bundle: %v\n", err)
		os.Exit(1)
	}

	if err := h.serve(os.Stdin); err != nil {
		fmt.Fprintf(os.Stderr, "host-a: %v\n", err)
		os.Exit(1)
	}
}

func newHost(out *os.File) (*host, error) {
	vm, err := quickjs.NewVM()
	if err != nil {
		return nil, fmt.Errorf("creating quickjs vm: %w", err)
	}
	h := &host{vm: vm, write: bufio.NewWriter(out)}
	if err := vm.RegisterHostFunc("__hostSend", func(args []any) (any, error) {
		kind, _ := args[0].(string)
		payload, _ := args[1].(string)
		var params any
		if err := json.Unmarshal([]byte(payload), &params); err != nil {
			params = map[string]any{"raw": payload}
		}
		h.notify(kind, params)
		return nil, nil
	}); err != nil {
		vm.Close()
		return nil, fmt.Errorf("registering host bridge: %w", err)
	}
	return h, nil
}

func (h *host) close() { _ = h.vm.Close() }

func (h *host) send(v any) {
	h.mu.Lock()
	defer h.mu.Unlock()
	data, _ := json.Marshal(v)
	_, _ = h.write.Write(data)
	_ = h.write.WriteByte('\n')
	_ = h.write.Flush()
}

func (h *host) notify(method string, params any) {
	h.send(map[string]any{"jsonrpc": "2.0", "method": method, "params": params})
}

func (h *host) respond(id *int, result any, rerr *rpcError) {
	payload := map[string]any{"jsonrpc": "2.0", "id": id}
	if rerr != nil {
		payload["error"] = rerr
	} else {
		payload["result"] = result
	}
	h.send(payload)
}

func (h *host) serve(in *os.File) error {
	scanner := bufio.NewScanner(in)
	scanner.Buffer(make([]byte, 64*1024), 8*1024*1024)
	for scanner.Scan() {
		line := scanner.Bytes()
		if len(line) == 0 {
			continue
		}
		var msg message
		if err := json.Unmarshal(line, &msg); err != nil {
			h.respond(nil, nil, &rpcError{Code: 4405, Message: "protocol_error: " + err.Error()})
			continue
		}
		if h.handle(msg) {
			return nil
		}
	}
	return scanner.Err()
}

// handle returns true when the host should stop.
func (h *host) handle(msg message) bool {
	switch msg.Method {
	case "initialize":
		var opts struct {
			Cwd string `json:"cwd"`
		}
		_ = json.Unmarshal(msg.Params, &opts)
		boot, err := json.Marshal(map[string]string{"cwd": opts.Cwd, "mode": "rpc"})
		if err != nil {
			h.respond(msg.ID, nil, &rpcError{Code: 4405, Message: err.Error()})
			return false
		}
		if _, err := h.vm.Call("__locowrkBoot", string(boot)); err != nil {
			h.respond(msg.ID, nil, &rpcError{Code: 4404, Message: "boot: " + err.Error()})
			return false
		}
		if err := h.pump("__locowrkBootDone", 10*time.Second); err != nil {
			h.respond(msg.ID, nil, &rpcError{Code: 4403, Message: "boot timeout: " + err.Error()})
			return false
		}
		desc, err := h.vm.Call("__locowrkDescribe")
		if err != nil {
			h.respond(msg.ID, nil, &rpcError{Code: 4404, Message: "describe: " + err.Error()})
			return false
		}
		h.respond(msg.ID, json.RawMessage(desc.(string)), nil)

	case "event", "invoke_tool", "invoke_command":
		kind := "event"
		name := msg.Method
		switch msg.Method {
		case "invoke_tool":
			kind = "tool"
			name = field(msg.Params, "name")
		case "invoke_command":
			kind = "command"
			name = field(msg.Params, "name")
		case "event":
			name = field(msg.Params, "name")
		}
		payload := "null"
		if raw := payloadField(msg.Params, msg.Method); len(raw) > 0 {
			payload = string(raw)
		}
		if _, err := h.vm.Call("__locowrkStart", kind, name, payload); err != nil {
			h.respond(msg.ID, nil, &rpcError{Code: 4404, Message: "dispatch: " + err.Error()})
			return false
		}
		if err := h.pump("__locowrkRunDone", 30*time.Second); err != nil {
			h.respond(msg.ID, nil, &rpcError{Code: 4403, Message: "dispatch timeout: " + err.Error()})
			return false
		}
		raw, err := h.vm.Call("__locowrkRunResult")
		if err != nil {
			h.respond(msg.ID, nil, &rpcError{Code: 4404, Message: "result: " + err.Error()})
			return false
		}
		out, _ := raw.(string)
		var decoded struct {
			OK    bool            `json:"ok"`
			Value json.RawMessage `json:"value"`
			Error string          `json:"error"`
		}
		if err := json.Unmarshal([]byte(out), &decoded); err != nil || !decoded.OK {
			text := "dispatch failed"
			if err == nil {
				text = decoded.Error
			}
			h.respond(msg.ID, nil, &rpcError{Code: 4404, Message: text})
			return false
		}
		h.respond(msg.ID, decoded.Value, nil)

	case "metrics":
		desc, err := h.vm.Call("__locowrkDescribe")
		if err != nil {
			h.respond(msg.ID, nil, &rpcError{Code: 4404, Message: err.Error()})
			return false
		}
		h.respond(msg.ID, json.RawMessage(desc.(string)), nil)

	case "shutdown":
		h.respond(msg.ID, map[string]bool{"ok": true}, nil)
		return true

	default:
		h.respond(msg.ID, nil, &rpcError{Code: 4400, Message: "unsupported method: " + msg.Method})
	}
	return false
}

// pump runs pending promise jobs until the named global returns true.
func (h *host) pump(doneExpr string, timeout time.Duration) error {
	deadline := time.Now().Add(timeout)
	for {
		v, err := h.vm.Eval(doneExpr+"()", quickjs.EvalGlobal)
		if err != nil {
			return err
		}
		if done, ok := v.(bool); ok && done {
			return nil
		}
		if _, err := h.vm.ExecutePendingJobs(); err != nil {
			return err
		}
		if time.Now().After(deadline) {
			return errors.New("deadline exceeded")
		}
		time.Sleep(time.Millisecond)
	}
}

func field(raw json.RawMessage, key string) string {
	var m map[string]json.RawMessage
	if err := json.Unmarshal(raw, &m); err != nil {
		return ""
	}
	var s string
	_ = json.Unmarshal(m[key], &s)
	return s
}

func payloadField(raw json.RawMessage, method string) json.RawMessage {
	var m map[string]json.RawMessage
	if err := json.Unmarshal(raw, &m); err != nil {
		return nil
	}
	switch method {
	case "invoke_tool":
		return m["arguments"]
	case "invoke_command":
		return m["args"]
	default:
		return m["event"]
	}
}
