package kernel

import (
	"context"
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/qingbird1983/locowrk/pi/ai"
	"github.com/qingbird1983/locowrk/pi/ai/providers"
	"github.com/qingbird1983/locowrk/pi/coding"

	"github.com/qingbird1983/locowrk/internal/eventbus"
)

// testHome isolates session files from the developer's real ~/.pi.
func testHome(t *testing.T) {
	t.Helper()
	home := t.TempDir()
	t.Setenv("HOME", home)
	t.Setenv("USERPROFILE", home)
}

func newFaux(t *testing.T, opts providers.RegisterFauxProviderOptions) (*providers.FauxProviderRegistration, ModelRef) {
	t.Helper()
	if opts.Models == nil {
		opts.Models = []providers.FauxModelDefinition{{ID: "faux-1", ContextWindow: 200000, MaxTokens: 8192}}
	}
	reg := providers.RegisterFauxProvider(opts)
	t.Cleanup(reg.Unregister)
	model := reg.GetModel()
	return reg, ModelRef{Provider: string(model.Provider), ID: model.ID}
}

func serviceFor(t *testing.T, model *ai.Model) *Service {
	t.Helper()
	svc, err := New(Options{
		LookupModel: func(ModelRef) (*ai.Model, error) { return model, nil },
	})
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	t.Cleanup(func() { _ = svc.Close(context.Background()) })
	return svc
}

func collect(t *testing.T, sub *eventbus.Subscription, stop eventbus.Kind, timeout time.Duration) []eventbus.Event {
	t.Helper()
	deadline := time.After(timeout)
	var out []eventbus.Event
	for {
		select {
		case ev, ok := <-sub.C():
			if !ok {
				t.Fatalf("subscription closed before %s", stop)
			}
			out = append(out, ev)
			if ev.Kind == stop {
				return out
			}
		case <-deadline:
			t.Fatalf("timed out waiting for %s (got %d events)", stop, len(out))
		}
	}
}

func kinds(events []eventbus.Event) []eventbus.Kind {
	out := make([]eventbus.Kind, len(events))
	for i, ev := range events {
		out[i] = ev.Kind
	}
	return out
}

// assertSubsequence checks that want appears in got in order.
func assertSubsequence(t *testing.T, got []eventbus.Kind, want ...eventbus.Kind) {
	t.Helper()
	i := 0
	for _, k := range got {
		if i < len(want) && k == want[i] {
			i++
		}
	}
	if i != len(want) {
		t.Fatalf("event order missing %v\n got: %v", want[i:], got)
	}
}

func TestToolCallRunEmitsLifecycleAndPersists(t *testing.T) {
	testHome(t)
	cwd := t.TempDir()
	if err := os.WriteFile(filepath.Join(cwd, "note.txt"), []byte("hello kernel"), 0o644); err != nil {
		t.Fatalf("write fixture: %v", err)
	}

	reg, ref := newFaux(t, providers.RegisterFauxProviderOptions{})
	reg.SetResponses([]providers.FauxResponseStep{
		providers.FauxStatic(providers.FauxAssistantMessage(ai.ContentList{
			providers.FauxToolCall("read", map[string]any{"path": "note.txt"}),
		}, ai.StopToolUse)),
		providers.FauxStatic(providers.FauxAssistantMessage(ai.ContentList{
			providers.FauxText("the note says hello kernel"),
		}, ai.StopStop)),
	})
	svc := serviceFor(t, reg.GetModel())
	sub := svc.Subscribe(eventbus.SubscribeOptions{})
	defer sub.Close()

	id, err := svc.CreateSession(context.Background(), SessionOptions{
		Cwd: cwd, Model: ref, Tools: []string{"read"},
	})
	if err != nil {
		t.Fatalf("CreateSession: %v", err)
	}
	if id == "" {
		t.Fatal("empty session id")
	}
	if err := svc.Prompt(context.Background(), id, "read note.txt and summarize it"); err != nil {
		t.Fatalf("Prompt: %v", err)
	}

	events := collect(t, sub, eventbus.KindAgentEnd, 10*time.Second)
	assertSubsequence(t, kinds(events),
		eventbus.KindAgentStart,
		eventbus.KindTurnStart,
		eventbus.KindMessageStart,
		eventbus.KindMessageEnd,
		eventbus.KindToolExecutionStart,
		eventbus.KindToolExecutionEnd,
		eventbus.KindAgentEnd,
	)

	var progress bool
	for _, ev := range events {
		if ev.Kind == eventbus.KindMessageUpdate {
			progress = true
		}
		if ev.Kind != eventbus.KindToolExecutionEnd {
			continue
		}
		var dto eventbus.ToolExecutionDTO
		if err := json.Unmarshal(ev.Payload, &dto); err != nil {
			t.Fatalf("tool payload: %v", err)
		}
		if dto.ToolName != "read" || !strings.Contains(dto.Text, "hello kernel") {
			t.Fatalf("tool end payload = %+v", dto)
		}
		if dto.IsError {
			t.Fatalf("tool reported an error: %+v", dto)
		}
	}
	if !progress {
		t.Fatal("no message_update events streamed")
	}

	history, err := svc.History(context.Background(), id)
	if err != nil {
		t.Fatalf("History: %v", err)
	}
	if len(history) < 4 {
		t.Fatalf("history = %d messages, want the full transcript: %+v", len(history), history)
	}
	last := history[len(history)-1]
	if !strings.Contains(last.Text, "hello kernel") {
		t.Fatalf("final assistant text = %q", last.Text)
	}

	path, ok := svc.SessionPath(id)
	if !ok || path == "" {
		t.Fatal("session has no file")
	}
	info, err := os.Stat(path)
	if err != nil || info.Size() == 0 {
		t.Fatalf("session file missing or empty: %v", err)
	}
	msgs, err := coding.LoadSessionMessages(path)
	if err != nil {
		t.Fatalf("LoadSessionMessages: %v", err)
	}
	if len(msgs) < 4 {
		t.Fatalf("stored messages = %d, want >= 4", len(msgs))
	}
}

func TestOpenSessionResumesAndAppends(t *testing.T) {
	testHome(t)
	cwd := t.TempDir()

	reg, ref := newFaux(t, providers.RegisterFauxProviderOptions{})
	reg.SetResponses([]providers.FauxResponseStep{
		providers.FauxStatic(providers.FauxAssistantMessage(ai.ContentList{providers.FauxText("first answer")}, ai.StopStop)),
		providers.FauxStatic(providers.FauxAssistantMessage(ai.ContentList{providers.FauxText("second answer")}, ai.StopStop)),
	})

	svc := serviceFor(t, reg.GetModel())
	id, err := svc.CreateSession(context.Background(), SessionOptions{Cwd: cwd, Model: ref, NoTools: true})
	if err != nil {
		t.Fatalf("CreateSession: %v", err)
	}
	if err := svc.Prompt(context.Background(), id, "first question"); err != nil {
		t.Fatalf("Prompt: %v", err)
	}
	path, _ := svc.SessionPath(id)
	before, err := coding.LoadSessionMessages(path)
	if err != nil {
		t.Fatalf("LoadSessionMessages: %v", err)
	}
	if err := svc.Close(context.Background()); err != nil {
		t.Fatalf("Close: %v", err)
	}

	svc2 := serviceFor(t, reg.GetModel())
	resumed, err := svc2.OpenSession(context.Background(), OpenOptions{Path: path, Model: ref, NoTools: true})
	if err != nil {
		t.Fatalf("OpenSession: %v", err)
	}
	history, err := svc2.History(context.Background(), resumed)
	if err != nil {
		t.Fatalf("History: %v", err)
	}
	if len(history) != len(before) {
		t.Fatalf("resumed history = %d messages, stored = %d", len(history), len(before))
	}
	if got := history[len(history)-1].Text; !strings.Contains(got, "first answer") {
		t.Fatalf("resumed last message = %q", got)
	}

	if err := svc2.Prompt(context.Background(), resumed, "second question"); err != nil {
		t.Fatalf("Prompt after resume: %v", err)
	}
	after, err := coding.LoadSessionMessages(path)
	if err != nil {
		t.Fatalf("LoadSessionMessages after resume: %v", err)
	}
	if len(after) <= len(before) {
		t.Fatalf("file did not grow: before %d, after %d", len(before), len(after))
	}
}

func TestPromptCancellationReturnsCanceled(t *testing.T) {
	testHome(t)
	cwd := t.TempDir()

	reg, ref := newFaux(t, providers.RegisterFauxProviderOptions{TokensPerSecond: 20})
	reg.SetResponses([]providers.FauxResponseStep{
		providers.FauxStatic(providers.FauxAssistantMessage(ai.ContentList{
			providers.FauxText(strings.Repeat("streaming text ", 40)),
		}, ai.StopStop)),
	})
	svc := serviceFor(t, reg.GetModel())
	sub := svc.Subscribe(eventbus.SubscribeOptions{})
	defer sub.Close()

	id, err := svc.CreateSession(context.Background(), SessionOptions{Cwd: cwd, Model: ref, NoTools: true})
	if err != nil {
		t.Fatalf("CreateSession: %v", err)
	}

	ctx, cancel := context.WithCancel(context.Background())
	done := make(chan error, 1)
	go func() { done <- svc.Prompt(ctx, id, "stream please") }()

	deadline := time.After(5 * time.Second)
	for {
		select {
		case ev := <-sub.C():
			if ev.Kind == eventbus.KindMessageUpdate {
				goto cancelling
			}
		case <-deadline:
			cancel()
			t.Fatal("no streaming update within 5s")
		}
	}

cancelling:
	start := time.Now()
	cancel()
	select {
	case err := <-done:
		if !errors.Is(err, ErrCanceled) {
			t.Fatalf("Prompt error = %v; want ErrCanceled", err)
		}
		if elapsed := time.Since(start); elapsed > 2*time.Second {
			t.Fatalf("cancel took %v", elapsed)
		}
	case <-time.After(3 * time.Second):
		t.Fatal("Prompt did not return after cancel")
	}
}

func TestSteeringRequiresRunningTurnAndReachesNextRequest(t *testing.T) {
	testHome(t)
	cwd := t.TempDir()
	ctx := context.Background()

	reg, ref := newFaux(t, providers.RegisterFauxProviderOptions{TokensPerSecond: 40})
	reg.SetResponses([]providers.FauxResponseStep{
		providers.FauxStatic(providers.FauxAssistantMessage(ai.ContentList{
			providers.FauxText(strings.Repeat("streaming text ", 20)),
		}, ai.StopStop)),
		providers.FauxStatic(providers.FauxAssistantMessage(ai.ContentList{
			providers.FauxText("closed with the steering note"),
		}, ai.StopStop)),
	})
	svc := serviceFor(t, reg.GetModel())

	id, err := svc.CreateSession(ctx, SessionOptions{Cwd: cwd, Model: ref, NoTools: true})
	if err != nil {
		t.Fatalf("CreateSession: %v", err)
	}
	// An idle session cannot be steered; the caller must send a prompt.
	if err := svc.Steer(ctx, id, "idle note"); !errors.Is(err, ErrNotRunning) {
		t.Fatalf("idle Steer error = %v; want ErrNotRunning", err)
	}

	sub := svc.Subscribe(eventbus.SubscribeOptions{})
	defer sub.Close()
	done := make(chan error, 1)
	go func() { done <- svc.Prompt(ctx, id, "写一段说明") }()

	// Steer once the turn is running.
	deadline := time.Now().Add(10 * time.Second)
	steerErr := ErrNotRunning
	for time.Now().Before(deadline) {
		if steerErr = svc.Steer(ctx, id, "STEER-OK"); steerErr == nil {
			break
		}
		if !errors.Is(steerErr, ErrNotRunning) {
			t.Fatalf("Steer error = %v", steerErr)
		}
		time.Sleep(5 * time.Millisecond)
	}
	if steerErr != nil {
		t.Fatalf("steering was never accepted: %v", steerErr)
	}

	select {
	case err := <-done:
		if err != nil {
			t.Fatalf("Prompt: %v", err)
		}
	case <-time.After(30 * time.Second):
		t.Fatal("run with steering did not finish")
	}

	var kinds []eventbus.Kind
	applied := false
	settle := time.Now().Add(2 * time.Second)
	for !applied && time.Now().Before(settle) {
		select {
		case ev, ok := <-sub.C():
			if !ok {
				settle = time.Time{}
				break
			}
			kinds = append(kinds, ev.Kind)
			if ev.Kind == eventbus.KindSteeringApplied {
				applied = true
			}
		case <-time.After(20 * time.Millisecond):
		}
	}
	if !applied {
		t.Fatalf("steering_applied missing from %v", kinds)
	}

	msgs, err := svc.History(ctx, id)
	if err != nil {
		t.Fatalf("History: %v", err)
	}
	foundSteer, foundReply := false, false
	for _, m := range msgs {
		if strings.Contains(m.Text, "STEER-OK") {
			foundSteer = true
		}
		if strings.Contains(m.Text, "closed with the steering note") {
			foundReply = true
		}
	}
	if !foundSteer || !foundReply {
		t.Fatalf("transcript missing steering turn (steer=%v reply=%v): %+v", foundSteer, foundReply, msgs)
	}
}

func TestSessionsAreIsolated(t *testing.T) {
	testHome(t)
	ctx := context.Background()

	regA, refA := newFaux(t, providers.RegisterFauxProviderOptions{Api: "faux-a", Provider: "faux-a"})
	regB, refB := newFaux(t, providers.RegisterFauxProviderOptions{Api: "faux-b", Provider: "faux-b"})
	regA.SetResponses([]providers.FauxResponseStep{
		providers.FauxStatic(providers.FauxAssistantMessage(ai.ContentList{providers.FauxText("answer A")}, ai.StopStop)),
	})
	regB.SetResponses([]providers.FauxResponseStep{
		providers.FauxStatic(providers.FauxAssistantMessage(ai.ContentList{providers.FauxText("answer B")}, ai.StopStop)),
	})

	models := map[ModelRef]*ai.Model{refA: regA.GetModel(), refB: regB.GetModel()}
	svc, err := New(Options{LookupModel: func(ref ModelRef) (*ai.Model, error) { return models[ref], nil }})
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	defer svc.Close(ctx)

	idA, err := svc.CreateSession(ctx, SessionOptions{Cwd: t.TempDir(), Model: refA, NoTools: true})
	if err != nil {
		t.Fatalf("CreateSession A: %v", err)
	}
	idB, err := svc.CreateSession(ctx, SessionOptions{Cwd: t.TempDir(), Model: refB, NoTools: true})
	if err != nil {
		t.Fatalf("CreateSession B: %v", err)
	}
	if idA == idB {
		t.Fatal("sessions share an id")
	}

	sub := svc.Subscribe(eventbus.SubscribeOptions{})
	defer sub.Close()

	var wg [2]chan error
	for i := range wg {
		wg[i] = make(chan error, 1)
	}
	go func() { wg[0] <- svc.Prompt(ctx, idA, "question A") }()
	go func() { wg[1] <- svc.Prompt(ctx, idB, "question B") }()
	for i := range wg {
		if err := <-wg[i]; err != nil {
			t.Fatalf("prompt %d: %v", i, err)
		}
	}

	lastSeq := map[string]uint64{}
	deadline := time.After(5 * time.Second)
	for {
		select {
		case ev := <-sub.C():
			if ev.Seq == 0 {
				t.Fatalf("event without seq: %+v", ev)
			}
			if prev := lastSeq[ev.SessionID]; ev.Seq != prev+1 {
				t.Fatalf("session %s seq = %d after %d", ev.SessionID, ev.Seq, prev)
			}
			lastSeq[ev.SessionID] = ev.Seq
		case <-deadline:
			if len(lastSeq) != 2 {
				t.Fatalf("events from %d sessions; want 2", len(lastSeq))
			}
			return
		}
	}
}

func TestEventsDoNotLeakAPIKeys(t *testing.T) {
	testHome(t)
	t.Setenv("ANTHROPIC_API_KEY", "sk-test-should-never-appear")

	reg, ref := newFaux(t, providers.RegisterFauxProviderOptions{})
	reg.SetResponses([]providers.FauxResponseStep{
		providers.FauxStatic(providers.FauxAssistantMessage(ai.ContentList{providers.FauxText("ok")}, ai.StopStop)),
	})
	svc := serviceFor(t, reg.GetModel())
	sub := svc.Subscribe(eventbus.SubscribeOptions{})
	defer sub.Close()

	id, err := svc.CreateSession(context.Background(), SessionOptions{Cwd: t.TempDir(), Model: ref, NoTools: true})
	if err != nil {
		t.Fatalf("CreateSession: %v", err)
	}
	if err := svc.Prompt(context.Background(), id, "hello"); err != nil {
		t.Fatalf("Prompt: %v", err)
	}
	for _, ev := range collect(t, sub, eventbus.KindAgentEnd, 10*time.Second) {
		if strings.Contains(string(ev.Payload), "sk-test-should-never-appear") {
			t.Fatalf("event %s leaked the API key", ev.Kind)
		}
	}
}

func TestMessageIDsAreUniqueAcrossTurns(t *testing.T) {
	testHome(t)
	reg, ref := newFaux(t, providers.RegisterFauxProviderOptions{})
	reg.SetResponses([]providers.FauxResponseStep{
		providers.FauxStatic(providers.FauxAssistantMessage(ai.ContentList{providers.FauxText("first answer")}, ai.StopStop)),
		providers.FauxStatic(providers.FauxAssistantMessage(ai.ContentList{providers.FauxText("second answer")}, ai.StopStop)),
	})
	svc := serviceFor(t, reg.GetModel())
	sub := svc.Subscribe(eventbus.SubscribeOptions{})
	defer sub.Close()

	id, err := svc.CreateSession(context.Background(), SessionOptions{Cwd: t.TempDir(), Model: ref, NoTools: true})
	if err != nil {
		t.Fatalf("CreateSession: %v", err)
	}

	started := map[string]bool{}
	for _, prompt := range []string{"one", "two"} {
		if err := svc.Prompt(context.Background(), id, prompt); err != nil {
			t.Fatalf("Prompt(%s): %v", prompt, err)
		}
		for _, ev := range collect(t, sub, eventbus.KindAgentEnd, 10*time.Second) {
			if ev.Kind != eventbus.KindMessageStart && ev.Kind != eventbus.KindMessageEnd {
				continue
			}
			var dto eventbus.MessageDTO
			if err := json.Unmarshal(ev.Payload, &dto); err != nil {
				t.Fatalf("message payload: %v", err)
			}
			if started[dto.ID] && ev.Kind == eventbus.KindMessageStart {
				t.Fatalf("duplicate message_start id %q (role %s)", dto.ID, dto.Role)
			}
			started[dto.ID] = true
		}
	}

	history, err := svc.History(context.Background(), id)
	if err != nil {
		t.Fatalf("History: %v", err)
	}
	seen := map[string]bool{}
	for _, m := range history {
		if seen[m.ID] {
			t.Fatalf("history repeats id %q", m.ID)
		}
		seen[m.ID] = true
		if !started[m.ID] {
			t.Fatalf("history id %q (role %s) never appeared in live events; live=%v", m.ID, m.Role, started)
		}
	}
	for id := range started {
		if !seen[id] {
			t.Fatalf("live id %q missing from history", id)
		}
	}
}
