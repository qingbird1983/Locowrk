package adapter

import (
	"bufio"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"os/exec"
	"strconv"
	"strings"
	"sync"
	"sync/atomic"
	"time"
)

// maxLineBytes is the frame limit from proto §2.
const maxLineBytes = 1 << 20

// stderrTailLines is how much plugin stderr the host keeps for diagnostics.
const stderrTailLines = 100

// peer owns one plugin process and its JSONL connection.
//
// Requests from the plugin are handled synchronously in the read loop so that
// registration order is preserved; handlers must therefore never call back into
// the peer.
type peer struct {
	cmd    *exec.Cmd
	stdin  io.WriteCloser
	stdout io.ReadCloser

	writeMu sync.Mutex

	mu      sync.Mutex
	pending map[string]chan rpcMessage
	nextID  int64
	closed  bool

	onRequest func(id json.RawMessage, method string, params json.RawMessage) (any, *RPCError)
	onNotify  func(method string, params json.RawMessage)

	done     chan struct{} // closed when stdout ends
	waitCh   chan error    // receives the process wait error once
	readErr  error
	stderr   *tailBuffer
	stopOnce sync.Once
}

// startPeer launches the executable and begins reading frames.
func startPeer(path string, args []string, dir string, env []string) (*peer, error) {
	cmd := exec.Command(path, args...)
	cmd.Dir = dir
	cmd.Env = append(os.Environ(), env...)
	stdin, err := cmd.StdinPipe()
	if err != nil {
		return nil, fmt.Errorf("adapter: stdin pipe: %w", err)
	}
	stdout, err := cmd.StdoutPipe()
	if err != nil {
		return nil, fmt.Errorf("adapter: stdout pipe: %w", err)
	}
	tail := &tailBuffer{maxLines: stderrTailLines}
	cmd.Stderr = tail
	prepareCommand(cmd)

	if err := cmd.Start(); err != nil {
		return nil, fmt.Errorf("adapter: starting plugin: %w", err)
	}
	p := &peer{
		cmd:     cmd,
		stdin:   stdin,
		stdout:  stdout,
		pending: map[string]chan rpcMessage{},
		done:    make(chan struct{}),
		waitCh:  make(chan error, 1),
		stderr:  tail,
	}
	go p.readLoop()
	go func() { p.waitCh <- cmd.Wait() }()
	return p, nil
}

// setHandlers installs the inbound handlers before the handshake starts.
func (p *peer) setHandlers(onRequest func(json.RawMessage, string, json.RawMessage) (any, *RPCError), onNotify func(string, json.RawMessage)) {
	p.onRequest = onRequest
	p.onNotify = onNotify
}

func (p *peer) readLoop() {
	defer close(p.done)
	reader := bufio.NewReaderSize(p.stdout, 64*1024)
	for {
		line, err := readLine(reader, maxLineBytes)
		if err != nil {
			if !errors.Is(err, io.EOF) && !errors.Is(err, os.ErrClosed) {
				p.setReadErr(err)
				p.kill()
			}
			return
		}
		if len(line) == 0 {
			continue
		}
		var msg rpcMessage
		if err := json.Unmarshal(line, &msg); err != nil {
			p.setReadErr(fmt.Errorf("%w: parse error: %v", ErrProtocol, err))
			p.kill()
			return
		}
		switch {
		case msg.Method != "":
			if msg.hasID() {
				p.handleRequest(msg)
			} else if p.onNotify != nil {
				p.onNotify(msg.Method, msg.Params)
			}
		case msg.hasID():
			p.deliver(msg)
		default:
			// A response without an id or a malformed object: count and drop.
		}
	}
}

func (p *peer) handleRequest(msg rpcMessage) {
	if p.onRequest == nil {
		p.respond(msg.ID, nil, &RPCError{Code: CodeMethodNotFound, Message: "no handler"})
		return
	}
	result, rerr := p.onRequest(msg.ID, msg.Method, msg.Params)
	p.respond(msg.ID, result, rerr)
}

func (p *peer) respond(id json.RawMessage, result any, rerr *RPCError) {
	if rerr != nil {
		p.write(rpcErrorResult{JSONRPC: "2.0", ID: id, Error: rerr})
		return
	}
	p.write(rpcResult{JSONRPC: "2.0", ID: id, Result: result})
}

func (p *peer) deliver(msg rpcMessage) {
	key := idKey(msg.ID)
	p.mu.Lock()
	ch := p.pending[key]
	if ch != nil {
		delete(p.pending, key)
	}
	p.mu.Unlock()
	if ch == nil {
		return
	}
	select {
	case ch <- msg:
	default:
	}
}

// idKey normalises a JSON-RPC id so numeric and string ids match their pending
// entry ("1" and 1 are the same request id here).
func idKey(raw json.RawMessage) string {
	return strings.Trim(string(raw), "\"")
}

// call sends one request and waits for its response.
func (p *peer) call(ctx context.Context, timeout time.Duration, method string, params any) (json.RawMessage, error) {
	if timeout <= 0 {
		timeout = 30 * time.Second
	}
	select {
	case <-p.done:
		return nil, fmt.Errorf("%w: plugin is not running", ErrCrashed)
	default:
	}

	id := strconv.FormatInt(atomic.AddInt64(&p.nextID, 1), 10)
	ch := make(chan rpcMessage, 1)
	p.mu.Lock()
	if p.closed {
		p.mu.Unlock()
		return nil, fmt.Errorf("%w: plugin is not running", ErrCrashed)
	}
	p.pending[id] = ch
	p.mu.Unlock()
	defer func() {
		p.mu.Lock()
		delete(p.pending, id)
		p.mu.Unlock()
	}()

	if err := p.write(rpcRequest{JSONRPC: "2.0", ID: id, Method: method, Params: params}); err != nil {
		return nil, err
	}

	timer := time.NewTimer(timeout)
	defer timer.Stop()
	select {
	case msg := <-ch:
		if msg.Error != nil {
			return nil, msg.Error
		}
		return msg.Result, nil
	case <-ctx.Done():
		return nil, fmt.Errorf("%w: %v", ErrTimeout, ctx.Err())
	case <-timer.C:
		return nil, fmt.Errorf("%w: %s after %s", ErrTimeout, method, timeout)
	case <-p.done:
		return nil, fmt.Errorf("%w: plugin exited during %s", ErrCrashed, method)
	}
}

// notify sends a one-way message.
func (p *peer) notify(method string, params any) error {
	return p.write(rpcRequest{JSONRPC: "2.0", Method: method, Params: params})
}

func (p *peer) write(v any) error {
	data, err := json.Marshal(v)
	if err != nil {
		return fmt.Errorf("adapter: encoding %T: %w", v, err)
	}
	if len(data) > maxLineBytes {
		return fmt.Errorf("%w: frame exceeds %d bytes", ErrProtocol, maxLineBytes)
	}
	p.writeMu.Lock()
	defer p.writeMu.Unlock()
	if _, err := p.stdin.Write(append(data, '\n')); err != nil {
		return fmt.Errorf("%w: writing to plugin: %v", ErrCrashed, err)
	}
	return nil
}

func (p *peer) setReadErr(err error) {
	p.mu.Lock()
	if p.readErr == nil {
		p.readErr = err
	}
	p.mu.Unlock()
}

// readError returns the fatal read error, if the stream ended abnormally.
func (p *peer) readError() error {
	p.mu.Lock()
	defer p.mu.Unlock()
	return p.readErr
}

// closeStdin closes the plugin's input, which is how a clean shutdown ends.
func (p *peer) closeStdin() {
	p.mu.Lock()
	p.closed = true
	p.mu.Unlock()
	_ = p.stdin.Close()
}

// kill terminates the process tree.
func (p *peer) kill() {
	p.stopOnce.Do(func() {
		if p.cmd.Process != nil {
			killTree(p.cmd.Process.Pid)
		}
	})
}

// stderrText returns the retained, redacted stderr tail.
func (p *peer) stderrText() string { return redact(p.stderr.Text()) }

// tailBuffer keeps the last n lines of a stream.
type tailBuffer struct {
	mu       sync.Mutex
	lines    []string
	partial  strings.Builder
	maxLines int
}

func (t *tailBuffer) Write(p []byte) (int, error) {
	t.mu.Lock()
	defer t.mu.Unlock()
	for _, r := range string(p) {
		if r == '\n' {
			t.lines = append(t.lines, t.partial.String())
			t.partial.Reset()
			if len(t.lines) > t.maxLines {
				t.lines = t.lines[len(t.lines)-t.maxLines:]
			}
			continue
		}
		t.partial.WriteRune(r)
	}
	return len(p), nil
}

func (t *tailBuffer) Text() string {
	t.mu.Lock()
	defer t.mu.Unlock()
	lines := append([]string(nil), t.lines...)
	if t.partial.Len() > 0 {
		lines = append(lines, t.partial.String())
	}
	return strings.Join(lines, "\n")
}

// readLine reads one \n-terminated frame, refusing lines over limit bytes.
func readLine(r *bufio.Reader, limit int) ([]byte, error) {
	var buf []byte
	for {
		chunk, err := r.ReadSlice('\n')
		if len(buf)+len(chunk) > limit {
			return nil, fmt.Errorf("%w: frame exceeds %d bytes", ErrProtocol, limit)
		}
		buf = append(buf, chunk...)
		switch {
		case err == nil:
			return trimEOL(buf), nil
		case errors.Is(err, bufio.ErrBufferFull):
			continue
		case errors.Is(err, io.EOF):
			if len(buf) > 0 {
				return trimEOL(buf), nil
			}
			return nil, io.EOF
		default:
			return nil, err
		}
	}
}

func trimEOL(b []byte) []byte {
	for len(b) > 0 && (b[len(b)-1] == '\n' || b[len(b)-1] == '\r') {
		b = b[:len(b)-1]
	}
	return b
}
