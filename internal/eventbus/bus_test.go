package eventbus

import (
	"encoding/json"
	"sync"
	"testing"
	"time"
)

func recv(t *testing.T, ch <-chan Event) Event {
	t.Helper()
	select {
	case ev, ok := <-ch:
		if !ok {
			t.Fatal("subscription closed unexpectedly")
		}
		return ev
	case <-time.After(2 * time.Second):
		t.Fatal("timed out waiting for an event")
	}
	return Event{}
}

func TestPublishAssignsPerSessionSeq(t *testing.T) {
	bus := New()
	defer bus.Close()
	sub := bus.Subscribe(SubscribeOptions{})
	defer sub.Close()

	first, err := bus.Publish(KindAgentStart, "s1", nil)
	if err != nil {
		t.Fatalf("publish: %v", err)
	}
	second, err := bus.Publish(KindAgentStart, "s1", nil)
	if err != nil {
		t.Fatalf("publish: %v", err)
	}
	other, err := bus.Publish(KindAgentStart, "s2", nil)
	if err != nil {
		t.Fatalf("publish: %v", err)
	}
	if first.Seq != 1 || second.Seq != 2 {
		t.Fatalf("s1 seq = %d, %d; want 1, 2", first.Seq, second.Seq)
	}
	if other.Seq != 1 {
		t.Fatalf("s2 seq = %d; want 1", other.Seq)
	}
	for i := 0; i < 3; i++ {
		recv(t, sub.C())
	}
}

func TestSubscriptionFiltersSession(t *testing.T) {
	bus := New()
	defer bus.Close()
	sub := bus.Subscribe(SubscribeOptions{SessionID: "wanted"})
	defer sub.Close()

	if _, err := bus.Publish(KindTurnStart, "other", nil); err != nil {
		t.Fatalf("publish: %v", err)
	}
	if _, err := bus.Publish(KindTurnStart, "wanted", map[string]string{"k": "v"}); err != nil {
		t.Fatalf("publish: %v", err)
	}

	ev := recv(t, sub.C())
	if ev.SessionID != "wanted" {
		t.Fatalf("session = %q; want wanted", ev.SessionID)
	}
	var payload map[string]string
	if err := json.Unmarshal(ev.Payload, &payload); err != nil {
		t.Fatalf("payload: %v", err)
	}
	if payload["k"] != "v" {
		t.Fatalf("payload = %v", payload)
	}
}

func TestConsecutiveDeltasMerge(t *testing.T) {
	bus := New()
	defer bus.Close()
	sub := bus.Subscribe(SubscribeOptions{Buffer: 1})
	defer sub.Close()

	delta := func(text string) MessageUpdateDTO {
		return MessageUpdateDTO{
			MessageID: "assistant-1", BlockIndex: 0, BlockType: "text",
			Event: "text_delta", Delta: text, Text: text,
		}
	}
	for _, text := range []string{"a", "b", "c"} {
		if _, err := bus.Publish(KindMessageUpdate, "s1", delta(text)); err != nil {
			t.Fatalf("publish: %v", err)
		}
	}

	first := recv(t, sub.C())

	var collected string
	var a MessageUpdateDTO
	if err := json.Unmarshal(first.Payload, &a); err != nil {
		t.Fatalf("first payload: %v", err)
	}
	collected = a.Delta
	for {
		select {
		case ev := <-sub.C():
			var next MessageUpdateDTO
			if err := json.Unmarshal(ev.Payload, &next); err != nil {
				t.Fatalf("payload: %v", err)
			}
			collected += next.Delta
		case <-time.After(100 * time.Millisecond):
			if collected != "abc" {
				t.Fatalf("deltas = %q; want abc", collected)
			}
			return
		}
	}
}

// TestMergeableDeltasCoalesce checks the merge itself without racing the
// delivery goroutine: enqueue is called directly on a subscription whose loop
// is not running.
func TestMergeableDeltasCoalesce(t *testing.T) {
	s := &Subscription{maxQueue: 8}
	s.cond = sync.NewCond(&s.mu)
	delta := func(text string) Event {
		raw, err := json.Marshal(MessageUpdateDTO{
			MessageID: "assistant-1", BlockIndex: 0, BlockType: "text",
			Event: "text_delta", Delta: text, Text: text,
		})
		if err != nil {
			t.Fatalf("marshal: %v", err)
		}
		return Event{Kind: KindMessageUpdate, SessionID: "s1", Payload: raw}
	}
	for _, text := range []string{"a", "b", "c"} {
		s.enqueue(delta(text))
	}

	s.mu.Lock()
	queue := append([]Event(nil), s.queue...)
	s.mu.Unlock()
	if len(queue) != 1 {
		t.Fatalf("queue length = %d, want 1 merged event", len(queue))
	}
	var merged MessageUpdateDTO
	if err := json.Unmarshal(queue[0].Payload, &merged); err != nil {
		t.Fatalf("merged payload: %v", err)
	}
	if merged.Delta != "abc" {
		t.Fatalf("merged delta = %q, want abc", merged.Delta)
	}
	if stats := s.Stats(); stats.Merged != 2 {
		t.Fatalf("merged count = %d, want 2", stats.Merged)
	}
}

func TestSlowSubscriberDoesNotBlockPublisher(t *testing.T) {
	bus := New()
	defer bus.Close()
	sub := bus.Subscribe(SubscribeOptions{Buffer: 1, MaxQueue: 8, Timeout: 50 * time.Millisecond})

	start := time.Now()
	for i := 0; i < 5; i++ {
		if _, err := bus.Publish(KindAgentStart, "s1", nil); err != nil {
			t.Fatalf("publish: %v", err)
		}
	}
	if elapsed := time.Since(start); elapsed > time.Second {
		t.Fatalf("publish blocked for %v", elapsed)
	}

	deadline := time.Now().Add(2 * time.Second)
	for time.Now().Before(deadline) {
		if sub.Stats().Disconnected != "" {
			break
		}
		time.Sleep(10 * time.Millisecond)
	}
	if got := sub.Stats().Disconnected; got == "" {
		t.Fatal("stalled subscriber was never disconnected")
	}
}

func TestControlOverflowDisconnects(t *testing.T) {
	bus := New()
	defer bus.Close()
	sub := bus.Subscribe(SubscribeOptions{Buffer: 1, MaxQueue: 2, Timeout: time.Minute})

	for i := 0; i < 10; i++ {
		if _, err := bus.Publish(KindToolExecutionStart, "s1", ToolExecutionDTO{ToolCallID: "t"}); err != nil {
			t.Fatalf("publish: %v", err)
		}
	}
	if got := sub.Stats().Disconnected; got == "" {
		t.Fatalf("expected disconnection, stats = %+v", sub.Stats())
	}
}

func TestCloseEndsSubscriptions(t *testing.T) {
	bus := New()
	sub := bus.Subscribe(SubscribeOptions{})
	bus.Close()

	deadline := time.After(2 * time.Second)
	for {
		select {
		case _, ok := <-sub.C():
			if !ok {
				return
			}
		case <-deadline:
			t.Fatal("subscription did not close with the bus")
		}
	}
}
