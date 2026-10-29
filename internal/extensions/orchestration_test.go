package extensions

import (
	"context"
	"encoding/json"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/qingbird1983/locowrk/internal/eventbus"
)

// startSample loads the orchestrator fixture and returns a manager plus a
// subscription that sees everything it publishes.
func startSample(t *testing.T) (*Manager, *eventbus.Subscription) {
	t.Helper()
	node, err := exec.LookPath("node")
	if err != nil {
		t.Skip("node not installed")
	}
	root, err := filepath.Abs(filepath.Join("..", "..", "fixtures", "extensions"))
	if err != nil {
		t.Fatal(err)
	}
	var sample *Candidate
	for _, c := range Discover(t.TempDir(), []string{root}) {
		if c.Name == "orchestrator-sample" {
			sample = c
		}
	}
	if sample == nil || sample.State != "ready" {
		t.Fatalf("sample not found: %+v", sample)
	}
	store := newStore()
	store.recs[sample.Name] = record{path: sample.Dir, sha: sample.SHA, enabled: true, trusted: true}
	bus := eventbus.New()
	m := New(Options{Bus: bus, Cwd: t.TempDir(), NodePath: node, Store: store, ExtraDirs: []string{root}})
	if err := m.Prepare(context.Background()); err != nil {
		t.Fatalf("Prepare: %v", err)
	}
	t.Cleanup(m.Stop)
	return m, bus.Subscribe(eventbus.SubscribeOptions{})
}

type sampleEvent struct {
	kind    eventbus.Kind
	seq     uint64
	id      string
	name    string
	status  string
	detail  string
	message string
}

func decodeSampleEvent(t *testing.T, ev eventbus.Event) sampleEvent {
	t.Helper()
	var payload struct {
		ID      string `json:"id"`
		Name    string `json:"name"`
		Status  string `json:"status"`
		Detail  string `json:"detail"`
		Message string `json:"message"`
	}
	if len(ev.Payload) > 0 {
		if err := json.Unmarshal(ev.Payload, &payload); err != nil {
			t.Fatalf("payload %s: %v", ev.Payload, err)
		}
	}
	return sampleEvent{kind: ev.Kind, seq: ev.Seq, id: payload.ID, name: payload.Name, status: payload.Status, detail: payload.Detail, message: payload.Message}
}

// collectUntil drains events until the predicate matches or the deadline hits.
func collectUntil(t *testing.T, sub *eventbus.Subscription, timeout time.Duration, done func(sampleEvent) bool) []sampleEvent {
	t.Helper()
	deadline := time.Now().Add(timeout)
	var out []sampleEvent
	for time.Now().Before(deadline) {
		select {
		case ev := <-sub.C():
			decoded := decodeSampleEvent(t, ev)
			out = append(out, decoded)
			if done(decoded) {
				return out
			}
		case <-time.After(20 * time.Millisecond):
		}
	}
	t.Fatalf("timed out after %d events: %+v", len(out), out)
	return out
}

// TestParallelDelegationSettlesInCompletionOrder covers acceptance 2: two
// subagents appear and the one that finishes first settles first, with events
// carrying increasing Seq values.
func TestParallelDelegationSettlesInCompletionOrder(t *testing.T) {
	m, sub := startSample(t)
	defer sub.Close()
	if err := m.RunCommand(context.Background(), "sample_events", ""); err != nil {
		t.Fatalf("sample_events: %v", err)
	}
	events := collectUntil(t, sub, 5*time.Second, func(e sampleEvent) bool {
		return e.kind == eventbus.KindWorkflowStep && e.status == "done"
	})

	var order []string
	for _, e := range events {
		switch e.kind {
		case eventbus.KindSubagentSpawned:
			order = append(order, "spawn:"+e.id)
		case eventbus.KindSubagentSettled:
			order = append(order, "settle:"+e.id)
		case eventbus.KindWorkflowStep:
			order = append(order, "step:"+e.status)
		}
	}
	want := []string{"spawn:sample-a", "spawn:sample-b", "step:running", "settle:sample-b", "settle:sample-a", "step:done"}
	if strings.Join(order, ",") != strings.Join(want, ",") {
		t.Fatalf("event order = %v, want %v", order, want)
	}
	for i := 1; i < len(events); i++ {
		if events[i].seq <= events[i-1].seq {
			t.Fatalf("Seq not increasing: %+v", events)
		}
	}
}

// TestChainStepsAsyncNotifyAndCancel covers acceptance 3: chain steps advance in
// order, the async completion notifies, and abort stops the work.
func TestChainStepsAsyncNotifyAndCancel(t *testing.T) {
	m, sub := startSample(t)
	defer sub.Close()

	if err := m.RunCommand(context.Background(), "sample_chain", ""); err != nil {
		t.Fatalf("sample_chain: %v", err)
	}
	events := collectUntil(t, sub, 5*time.Second, func(e sampleEvent) bool {
		return e.kind == eventbus.KindWorkflowStep && e.name == "验证" && e.status == "done"
	})
	var steps []string
	for _, e := range events {
		if e.kind == eventbus.KindWorkflowStep {
			steps = append(steps, e.name+":"+e.status)
		}
	}
	want := []string{"收集:running", "收集:done", "实现:running", "实现:done", "验证:running", "验证:done"}
	if strings.Join(steps, ",") != strings.Join(want, ",") {
		t.Fatalf("chain order = %v, want %v", steps, want)
	}
	waitNotice(t, m, "sample: chain finished")

	// A long chain stops when the host aborts the extensions.
	if err := m.RunCommand(context.Background(), "sample_chain_long", ""); err != nil {
		t.Fatalf("sample_chain_long: %v", err)
	}
	collectUntil(t, sub, 3*time.Second, func(e sampleEvent) bool {
		return e.kind == eventbus.KindWorkflowStep && e.name == "长任务" && e.status == "running"
	})
	if err := m.AbortAll(context.Background()); err != nil {
		t.Fatalf("AbortAll: %v", err)
	}
	collectUntil(t, sub, 3*time.Second, func(e sampleEvent) bool {
		return e.kind == eventbus.KindWorkflowStep && e.name == "长任务" && e.status == "skipped" && e.detail == "cancelled"
	})
	waitNotice(t, m, "sample: cancelled")

	// Nothing may keep ticking after the abort.
	quiet := time.After(300 * time.Millisecond)
	for {
		select {
		case ev := <-sub.C():
			decoded := decodeSampleEvent(t, ev)
			if decoded.kind == eventbus.KindWorkflowStep && strings.HasPrefix(decoded.detail, "tick") {
				t.Fatalf("work continued after abort: %+v", decoded)
			}
		case <-quiet:
			return
		}
	}
}

// TestDeferredMembersReportDiagnostics covers acceptance 6: every deferred or
// unsupported member answers with a diagnostic instead of throwing, and the
// extension keeps working afterwards.
func TestDeferredMembersReportDiagnostics(t *testing.T) {
	m, _ := startSample(t)
	if err := m.RunCommand(context.Background(), "sample_deferred", ""); err != nil {
		t.Fatalf("sample_deferred: %v", err)
	}
	waitNotice(t, m, "deferred members exercised")

	var members []string
	for _, info := range m.Extensions() {
		if info.Name != "orchestrator-sample" {
			continue
		}
		for _, d := range info.Diagnostics {
			members = append(members, d.Member)
		}
	}
	for _, want := range []string{
		"ctx.executeTool", "pi.sendMessage", "pi.sendUserMessage", "pi.appendEntry",
		"ctx.compact", "ui.setWidget", "ui.setEditorText", "ui.pasteToEditor",
		"ui.custom", "registerShortcut",
	} {
		found := false
		for _, got := range members {
			if got == want {
				found = true
			}
		}
		if !found {
			t.Fatalf("%s produced no diagnostic: %v", want, members)
		}
	}

	for _, spec := range m.Tools() {
		if spec.Name != "sample_echo" {
			continue
		}
		res, err := spec.Execute(context.Background(), "after-deferred", json.RawMessage(`{"text":"still works"}`))
		if err != nil || !strings.Contains(res.Text, "still works") {
			t.Fatalf("extension broke after deferred members: %+v %v", res, err)
		}
		return
	}
	t.Fatalf("tool missing after deferred members: %+v", m.Tools())
}
