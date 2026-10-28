package kernel

import (
	"context"
	"errors"
	"strings"
	"testing"
	"time"

	"github.com/qingbird1983/locowrk/pi/ai"
	"github.com/qingbird1983/locowrk/pi/ai/providers"

	"github.com/qingbird1983/locowrk/internal/eventbus"
)

// TestAbortClassifiesAsCanceled locks the UI contract: a stopped turn is not a
// provider failure, even when the stream reports an abort error.
func TestAbortClassifiesAsCanceled(t *testing.T) {
	testHome(t)
	cwd := t.TempDir()
	reg, ref := newFaux(t, providers.RegisterFauxProviderOptions{TokensPerSecond: 5})
	reg.SetResponses([]providers.FauxResponseStep{
		providers.FauxStatic(providers.FauxAssistantMessage(ai.ContentList{
			providers.FauxText(strings.Repeat("streaming ", 80)),
		}, ai.StopStop)),
		providers.FauxStatic(providers.FauxAssistantMessage(ai.ContentList{
			providers.FauxText("after stop"),
		}, ai.StopStop)),
	})
	svc := serviceFor(t, reg.GetModel())
	sub := svc.Subscribe(eventbus.SubscribeOptions{})
	defer sub.Close()

	id, err := svc.CreateSession(context.Background(), SessionOptions{Cwd: cwd, Model: ref, NoTools: true})
	if err != nil {
		t.Fatalf("CreateSession: %v", err)
	}

	done := make(chan error, 1)
	go func() { done <- svc.Prompt(context.Background(), id, "stream") }()

	deadline := time.After(10 * time.Second)
	for {
		select {
		case ev := <-sub.C():
			if ev.Kind != eventbus.KindMessageUpdate {
				continue
			}
			if err := svc.Abort(context.Background(), id); err != nil {
				t.Fatalf("Abort: %v", err)
			}
			goto aborted
		case <-deadline:
			t.Fatal("timed out waiting for a stream update")
		}
	}
aborted:
	if err := <-done; !errors.Is(err, ErrCanceled) {
		t.Fatalf("Prompt err = %v, want ErrCanceled", err)
	}

	if err := svc.Prompt(context.Background(), id, "again"); err != nil {
		t.Fatalf("second Prompt: %v", err)
	}
}
