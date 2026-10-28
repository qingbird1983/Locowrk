package eventbus

import (
	"encoding/json"
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
			if stats := sub.Stats(); stats.Merged == 0 {
				t.Fatalf("expected at least one merged delta, stats = %+v", stats)
			}
			return
		}
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
