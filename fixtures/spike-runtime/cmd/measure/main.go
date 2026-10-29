// Command measure drives one runtime route and records the numbers the Phase 2
// spike compares: cold start to ready, resident memory, conformance of the
// required path, and shim coverage reported by the runtime itself.
//
// Exempt from the 500-line budget (standards/08 §4): this is a Phase 2 spike
// measurement fixture, not shipped code; it is registered in the Phase 6 report.
package main

import (
	"bufio"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"sort"
	"strconv"
	"strings"
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

type child struct {
	cmd     *exec.Cmd
	stdin   io.WriteCloser
	msgs    chan message
	stderr  *tailBuffer
	nextID  int
	notes   []message
	started time.Time
}

type tailBuffer struct {
	data []byte
	max  int
}

func (t *tailBuffer) Write(p []byte) (int, error) {
	t.data = append(t.data, p...)
	if len(t.data) > t.max {
		t.data = t.data[len(t.data)-t.max:]
	}
	return len(p), nil
}

func (t *tailBuffer) String() string { return strings.TrimSpace(string(t.data)) }

func spawn(name string, args []string, dir string) (*child, error) {
	cmd := exec.Command(name, args...)
	cmd.Dir = dir
	stdin, err := cmd.StdinPipe()
	if err != nil {
		return nil, err
	}
	stdout, err := cmd.StdoutPipe()
	if err != nil {
		return nil, err
	}
	stderr := &tailBuffer{max: 8 * 1024}
	cmd.Stderr = stderr
	if err := cmd.Start(); err != nil {
		return nil, err
	}
	c := &child{cmd: cmd, stdin: stdin, msgs: make(chan message, 256), stderr: stderr, started: time.Now()}
	go func() {
		scanner := bufio.NewScanner(stdout)
		scanner.Buffer(make([]byte, 64*1024), 8*1024*1024)
		for scanner.Scan() {
			line := scanner.Bytes()
			if len(line) == 0 {
				continue
			}
			var msg message
			if err := json.Unmarshal(line, &msg); err != nil {
				continue
			}
			c.msgs <- msg
		}
		close(c.msgs)
	}()
	return c, nil
}

func (c *child) call(method string, params any, timeout time.Duration) (json.RawMessage, error) {
	c.nextID++
	id := c.nextID
	req := map[string]any{"jsonrpc": "2.0", "id": id, "method": method, "params": params}
	data, _ := json.Marshal(req)
	if _, err := c.stdin.Write(append(data, '\n')); err != nil {
		return nil, fmt.Errorf("write %s: %w", method, err)
	}
	timer := time.NewTimer(timeout)
	defer timer.Stop()
	for {
		select {
		case msg, ok := <-c.msgs:
			if !ok {
				return nil, fmt.Errorf("%s: host closed stdout", method)
			}
			if msg.ID != nil && *msg.ID == id {
				if msg.Error != nil {
					return nil, fmt.Errorf("%s: %d %s", method, msg.Error.Code, msg.Error.Message)
				}
				return msg.Result, nil
			}
			if msg.Method != "" {
				c.notes = append(c.notes, msg)
			}
		case <-timer.C:
			return nil, fmt.Errorf("%s: timeout after %s", method, timeout)
		}
	}
}

func (c *child) notify(method string, params any) error {
	data, _ := json.Marshal(map[string]any{"jsonrpc": "2.0", "method": method, "params": params})
	_, err := c.stdin.Write(append(data, '\n'))
	return err
}

func (c *child) stop() {
	_, _ = c.call("shutdown", map[string]any{}, 5*time.Second)
	_ = c.stdin.Close()
	done := make(chan struct{})
	go func() { _ = c.cmd.Wait(); close(done) }()
	select {
	case <-done:
	case <-time.After(5 * time.Second):
		_ = c.cmd.Process.Kill()
	}
}

type description struct {
	Tools     []string        `json:"tools"`
	Commands  []string        `json:"commands"`
	Events    []string        `json:"events"`
	Coverage  json.RawMessage `json:"coverage"`
	BootError string          `json:"bootError"`
}

type check struct {
	Name   string `json:"name"`
	OK     bool   `json:"ok"`
	Detail string `json:"detail,omitempty"`
}

type runData struct {
	ReadyMs       float64         `json:"readyMs"`
	RSSMiB        float64         `json:"rssMiB"`
	RSSSamples    []float64       `json:"rssSamples,omitempty"`
	BundleBytes   int64           `json:"bundleBytes"`
	Tools         []string        `json:"tools"`
	Commands      []string        `json:"commands"`
	Events        []string        `json:"events"`
	Coverage      json.RawMessage `json:"coverage"`
	Notifications []note          `json:"notifications"`
	Diagnostics   []note          `json:"diagnostics"`
	ToolResult    json.RawMessage `json:"toolResult"`
	ToolError     string          `json:"toolError,omitempty"`
	Checks        []check         `json:"checks"`
	OK            bool            `json:"ok"`
	Stderr        string          `json:"stderr,omitempty"`
}

type note struct {
	Method string          `json:"method"`
	Params json.RawMessage `json:"params"`
}

type report struct {
	Route        string    `json:"route"`
	Sample       string    `json:"sample"`
	Runs         int       `json:"runs"`
	ColdStartMs  stats     `json:"coldStartMs"`
	Detail       runData   `json:"detail"`
	Notes        []string  `json:"notes,omitempty"`
	MeasuredAt   time.Time `json:"measuredAt"`
	CommandLine  string    `json:"commandLine"`
	PlatformNote string    `json:"platformNote"`
}

type stats struct {
	Median float64   `json:"median"`
	Min    float64   `json:"min"`
	Max    float64   `json:"max"`
	All    []float64 `json:"all"`
}

func main() {
	route := flag.String("route", "a", "runtime route: a (quickjs), b (node), c (go rewrite)")
	sample := flag.String("sample", "ui", "sample name: ui, orchestrator, state")
	root := flag.String("root", ".", "spike fixture root")
	bundle := flag.String("bundle", "", "bundle path for route a/b")
	runs := flag.Int("runs", 10, "cold-start runs")
	out := flag.String("out", "", "write the JSON report here")
	flag.Parse()

	spec := specFor(*sample)
	cmdName, cmdArgs, err := commandFor(*root, *route, *bundle, *sample)
	if err != nil {
		fail("%v", err)
	}

	report := report{
		Route:        *route,
		Sample:       *sample,
		Runs:         *runs,
		MeasuredAt:   time.Now().UTC(),
		CommandLine:  displayCommand(cmdName, cmdArgs, *root),
		PlatformNote: "Windows 11, Go 1.27.3, Node " + nodeVersion(),
	}

	var durations []float64
	for i := 0; i < *runs; i++ {
		d, err := readyOnly(cmdName, cmdArgs, *root, spec)
		if err != nil {
			fail("cold start run %d: %v", i+1, err)
		}
		durations = append(durations, d)
	}
	report.ColdStartMs = summarize(durations)

	detail, err := fullRun(cmdName, cmdArgs, *root, spec, *bundle, *route)
	if err != nil {
		fail("detail run: %v", err)
	}
	report.Detail = detail

	encoded, _ := json.MarshalIndent(report, "", "  ")
	fmt.Println(string(encoded))
	if *out != "" {
		if err := os.MkdirAll(filepath.Dir(*out), 0o755); err != nil {
			fail("creating output dir: %v", err)
		}
		if err := os.WriteFile(*out, append(encoded, '\n'), 0o644); err != nil {
			fail("writing report: %v", err)
		}
	}
	if !detail.OK {
		os.Exit(1)
	}
}

func displayCommand(name string, args []string, root string) string {
	absRoot, err := filepath.Abs(root)
	rel := func(s string) string {
		if err != nil {
			return s
		}
		r, err := filepath.Rel(absRoot, s)
		if err != nil || strings.HasPrefix(r, "..") {
			return s
		}
		return r
	}
	parts := []string{rel(name)}
	for _, a := range args {
		parts = append(parts, rel(a))
	}
	return strings.Join(parts, " ")
}

func commandFor(root, route, bundle, sample string) (string, []string, error) {
	absRoot, err := filepath.Abs(root)
	if err != nil {
		return "", nil, err
	}
	switch route {
	case "a":
		if bundle == "" {
			return "", nil, errors.New("route a needs --bundle")
		}
		return filepath.Join(absRoot, "build", "host-a.exe"), []string{"--bundle", bundle}, nil
	case "b":
		if bundle == "" {
			return "", nil, errors.New("route b needs --bundle")
		}
		return "node", []string{filepath.Join(absRoot, "runner", "host-b.mjs"), "--bundle", bundle}, nil
	case "c":
		return filepath.Join(absRoot, "build", "host-c.exe"), []string{"--sample", sample}, nil
	default:
		return "", nil, fmt.Errorf("unknown route %q", route)
	}
}

func readyOnly(name string, args []string, dir string, spec sampleSpec) (float64, error) {
	c, err := spawn(name, args, dir)
	if err != nil {
		return 0, err
	}
	defer c.stop()
	start := time.Now()
	if _, err := c.call("initialize", map[string]any{"protocol": 1, "cwd": dir}, 10*time.Second); err != nil {
		return 0, err
	}
	return float64(time.Since(start).Microseconds()) / 1000.0, nil
}

func fullRun(name string, args []string, dir string, spec sampleSpec, bundle string, route string) (runData, error) {
	var data runData
	c, err := spawn(name, args, dir)
	if err != nil {
		return data, err
	}
	defer c.stop()

	start := time.Now()
	raw, err := c.call("initialize", map[string]any{"protocol": 1, "cwd": dir}, 10*time.Second)
	if err != nil {
		return data, err
	}
	data.ReadyMs = float64(time.Since(start).Microseconds()) / 1000.0
	var desc description
	if err := json.Unmarshal(raw, &desc); err != nil {
		return data, fmt.Errorf("decoding initialize result: %w", err)
	}
	data.Tools, data.Commands, data.Events = desc.Tools, desc.Commands, desc.Events
	data.Coverage = desc.Coverage
	if bundle != "" {
		if st, err := os.Stat(bundle); err == nil {
			data.BundleBytes = st.Size()
		}
	}

	// Resident memory right after registration: sample a few times because Go's
	// heap (route A) and V8's heap (route B) settle at different moments.
	for i := 0; i < 5; i++ {
		if i > 0 {
			time.Sleep(100 * time.Millisecond)
		}
		data.RSSSamples = append(data.RSSSamples, rssMiB(c.cmd.Process.Pid))
	}
	data.RSSMiB = summarize(data.RSSSamples).Median

	if _, err := c.call("event", map[string]any{"name": "session_start", "event": map[string]any{"reason": "new"}}, 10*time.Second); err != nil {
		return data, fmt.Errorf("session_start: %w", err)
	}
	if spec.Command != "" {
		if _, err := c.call("invoke_command", map[string]any{"name": spec.Command, "args": ""}, 10*time.Second); err != nil {
			return data, fmt.Errorf("command %s: %w", spec.Command, err)
		}
	}
	toolRaw, err := json.RawMessage(nil), error(nil)
	if !spec.SkipTool {
		toolRaw, err = c.call("invoke_tool", map[string]any{"name": spec.Tool, "arguments": spec.Args}, 30*time.Second)
		if err != nil {
			data.ToolError = err.Error()
		} else {
			data.ToolResult = toolRaw
		}
	}

	if metricsRaw, err := c.call("metrics", map[string]any{}, 10*time.Second); err == nil {
		var m description
		if json.Unmarshal(metricsRaw, &m) == nil {
			data.Coverage = m.Coverage
		}
	}

	for _, n := range c.notes {
		entry := note{Method: n.Method, Params: n.Params}
		switch n.Method {
		case "notify":
			data.Notifications = append(data.Notifications, entry)
		case "diagnostic":
			data.Diagnostics = append(data.Diagnostics, entry)
		}
	}
	data.Stderr = c.stderr.String()
	data.Checks = evaluate(spec, data, desc.BootError, route)
	data.OK = true
	for _, chk := range data.Checks {
		if !chk.OK {
			data.OK = false
		}
	}
	return data, nil
}

type sampleSpec struct {
	Name       string
	Tool       string
	ExpectTool string
	SkipTool   bool
	Args       map[string]any
	Notify     string
	Command    string
	WantDiag   string
	ExpectText string
}

func specFor(sample string) sampleSpec {
	switch sample {
	case "orchestrator":
		return sampleSpec{
			Name:   "orchestrator",
			Tool:   "probe_delegate",
			Args:   map[string]any{"task": "spike"},
			Notify: "orchestrator-probe started",
		}
	case "state":
		return sampleSpec{
			Name:     "state",
			Tool:     "probe_state",
			Args:     map[string]any{"action": "add", "text": "first"},
			Command:  "probe-list",
			WantDiag: "session.getBranch",
		}
	case "real-rpc-demo":
		return sampleSpec{
			Name:     "real-rpc-demo",
			SkipTool: true,
			WantDiag: "ui.setWidget",
		}
	case "real-todo":
		return sampleSpec{
			Name:       "real-todo",
			Tool:       "todo",
			Args:       map[string]any{"action": "list"},
			WantDiag:   "session.getBranch",
			ExpectText: "No todos",
		}
	case "real-subagent":
		return sampleSpec{
			Name:       "real-subagent",
			ExpectTool: "subagent",
			SkipTool:   true,
		}
	default:
		return sampleSpec{
			Name:     "ui",
			Tool:     "probe_echo",
			Args:     map[string]any{"text": "spike"},
			Notify:   "ui-probe started",
			WantDiag: "ui.setWidget",
		}
	}
}

func evaluate(spec sampleSpec, data runData, bootError string, route string) []check {
	var checks []check
	add := func(name string, ok bool, detail string) {
		checks = append(checks, check{Name: name, OK: ok, Detail: detail})
	}
	add("boot", bootError == "", bootError)
	expectTool := spec.ExpectTool
	if expectTool == "" {
		expectTool = spec.Tool
	}
	if expectTool != "" {
		add("tool registered", contains(data.Tools, expectTool), strings.Join(data.Tools, ","))
	}
	if spec.Notify != "" {
		add("notify mapped", hasNotify(data, spec.Notify), spec.Notify)
	}
	if spec.WantDiag != "" {
		count, code := diagCount(data, spec.WantDiag)
		add("noop diagnostic", count >= 1, fmt.Sprintf("%s code=%s count=%d", spec.WantDiag, code, count))
	}
	if !spec.SkipTool {
		add("tool result", data.ToolError == "" && toolTextHas(data.ToolResult, expectedToolText(spec)), firstNonEmpty(data.ToolError, string(data.ToolResult)))
	}

	switch spec.Name {
	case "ui":
		// A real runtime must survive the no-op twice (session_start and the
		// tool call); a Go rewrite has no shim to diagnose, so skip it there.
		if route != "c" {
			count, _ := diagCount(data, "ui.setWidget")
			add("extension continued after no-op", count >= 2, fmt.Sprintf("setWidget diagnostics=%d", count))
		}
	case "orchestrator":
		add("worker output", toolTextHas(data.ToolResult, "worker:spike"), string(data.ToolResult))
	case "state":
		count, _ := diagCount(data, "ui.custom")
		add("TUI view ignored", count >= 1, fmt.Sprintf("ui.custom diagnostics=%d", count))
	}
	return checks
}

func expectedToolText(spec sampleSpec) string {
	if spec.ExpectText != "" {
		return spec.ExpectText
	}
	switch spec.Name {
	case "ui":
		return "echo:spike"
	case "state":
		return "#1 first"
	case "real-todo":
		return "No todos"
	default:
		return "worker:spike"
	}
}

func firstNonEmpty(values ...string) string {
	for _, v := range values {
		if v != "" {
			return v
		}
	}
	return ""
}

func contains(list []string, want string) bool {
	for _, item := range list {
		if item == want {
			return true
		}
	}
	return false
}

func hasNotify(data runData, want string) bool {
	for _, n := range data.Notifications {
		var p struct {
			Message string `json:"message"`
		}
		_ = json.Unmarshal(n.Params, &p)
		if strings.Contains(p.Message, want) {
			return true
		}
	}
	return false
}

func diagCount(data runData, member string) (int, string) {
	total, code := 0, ""
	for _, d := range data.Diagnostics {
		var p struct {
			Member string `json:"member"`
			Code   string `json:"code"`
			Count  int    `json:"count"`
		}
		if json.Unmarshal(d.Params, &p) != nil {
			continue
		}
		if p.Member == member {
			total += p.Count
			code = p.Code
		}
	}
	return total, code
}

func toolTextHas(raw json.RawMessage, want string) bool {
	var result struct {
		Content []struct {
			Type string `json:"type"`
			Text string `json:"text"`
		} `json:"content"`
	}
	if err := json.Unmarshal(raw, &result); err != nil {
		return false
	}
	for _, part := range result.Content {
		if strings.Contains(part.Text, want) {
			return true
		}
	}
	return false
}

func rssMiB(pid int) float64 {
	cmd := exec.Command("powershell", "-NoProfile", "-Command", fmt.Sprintf("(Get-Process -Id %d).WorkingSet64", pid))
	out, err := cmd.Output()
	if err != nil {
		return 0
	}
	bytes, err := strconv.ParseInt(strings.TrimSpace(string(out)), 10, 64)
	if err != nil {
		return 0
	}
	return float64(bytes) / (1024 * 1024)
}

func summarize(values []float64) stats {
	if len(values) == 0 {
		return stats{}
	}
	sorted := append([]float64(nil), values...)
	sort.Float64s(sorted)
	return stats{Median: sorted[len(sorted)/2], Min: sorted[0], Max: sorted[len(sorted)-1], All: values}
}

func nodeVersion() string {
	out, err := exec.Command("node", "--version").Output()
	if err != nil {
		return "unavailable"
	}
	return strings.TrimSpace(string(out))
}

func fail(format string, args ...any) {
	fmt.Fprintf(os.Stderr, format+"\n", args...)
	os.Exit(1)
}
