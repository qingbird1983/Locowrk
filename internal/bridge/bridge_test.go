package bridge

import (
	"context"
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/qingbird1983/locowrk/pi/ai"
	"github.com/qingbird1983/locowrk/pi/ai/providers"

	"github.com/qingbird1983/locowrk/internal/eventbus"
	"github.com/qingbird1983/locowrk/internal/kernel"
)

func testHome(t *testing.T) {
	t.Helper()
	home := t.TempDir()
	t.Setenv("HOME", home)
	t.Setenv("USERPROFILE", home)
}

func newService(t *testing.T, emit func(eventbus.Event), providerOpts providers.RegisterFauxProviderOptions) (*Service, *providers.FauxProviderRegistration, string) {
	t.Helper()
	testHome(t)
	cwd := t.TempDir()
	reg := providers.RegisterFauxProvider(providerOpts)
	t.Cleanup(reg.Unregister)
	model := reg.GetModel()
	k, err := kernel.New(kernel.Options{
		LookupModel: func(kernel.ModelRef) (*ai.Model, error) { return model, nil },
	})
	if err != nil {
		t.Fatalf("kernel.New: %v", err)
	}
	t.Cleanup(func() { _ = k.Close(context.Background()) })
	svc, err := New(k, Options{OnEvent: emit})
	if err != nil {
		t.Fatalf("bridge.New: %v", err)
	}
	t.Cleanup(svc.Close)
	return svc, reg, cwd
}

func modelRef(reg *providers.FauxProviderRegistration) string {
	m := reg.GetModel()
	return string(m.Provider) + "/" + m.ID
}

func TestSkillsTrustRoundTrip(t *testing.T) {
	testHome(t)
	cwd := t.TempDir()
	writeSkillFile := func(dir, name, desc string) {
		t.Helper()
		path := filepath.Join(dir, "SKILL.md")
		if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
			t.Fatal(err)
		}
		body := "---\nname: " + name + "\ndescription: " + desc + "\n---\n\nbody\n"
		if err := os.WriteFile(path, []byte(body), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	home := os.Getenv("USERPROFILE")
	writeSkillFile(filepath.Join(home, ".pi", "agent", "skills", "user-skill"), "user-skill", "user skill")
	writeSkillFile(filepath.Join(cwd, ".pi", "skills", "project-skill"), "project-skill", "project skill")

	k, err := kernel.New(kernel.Options{})
	if err != nil {
		t.Fatalf("kernel.New: %v", err)
	}
	t.Cleanup(func() { _ = k.Close(context.Background()) })
	trusted := false
	svc, err := New(k, Options{
		Cwd:            cwd,
		ProjectTrusted: func(string) bool { return trusted },
		SetProjectTrust: func(_ string, value bool) error {
			trusted = value
			return nil
		},
	})
	if err != nil {
		t.Fatalf("bridge.New: %v", err)
	}
	t.Cleanup(svc.Close)

	view := svc.Skills(cwd)
	if view.Trusted || len(view.Project) != 0 {
		t.Fatalf("untrusted workspace exposes project skills: %+v", view)
	}
	if len(view.User) != 1 || view.User[0].Name != "user-skill" {
		t.Fatalf("user skills missing: %+v", view.User)
	}

	if err := svc.SetProjectTrust(cwd, true); err != nil {
		t.Fatalf("trust: %v", err)
	}
	view = svc.Skills(cwd)
	if !view.Trusted || len(view.Project) != 1 || view.Project[0].Name != "project-skill" {
		t.Fatalf("trusted workspace does not expose the project skill: %+v", view)
	}

	// Revoking hides the project skill again for later sessions.
	if err := svc.SetProjectTrust(cwd, false); err != nil {
		t.Fatalf("revoke: %v", err)
	}
	view = svc.Skills(cwd)
	if view.Trusted || len(view.Project) != 0 || len(view.User) != 1 {
		t.Fatalf("revoke did not take effect: %+v", view)
	}
}

func TestSessionLifecycleThroughBridge(t *testing.T) {
	events := make(chan eventbus.Event, 256)
	svc, reg, cwd := newService(t, func(ev eventbus.Event) { events <- ev }, providers.RegisterFauxProviderOptions{})
	reg.SetResponses([]providers.FauxResponseStep{
		providers.FauxStatic(providers.FauxAssistantMessage(ai.ContentList{
			providers.FauxText("bridge answer"),
		}, ai.StopStop)),
	})

	id, err := svc.CreateSession(CreateOptions{Cwd: cwd, Model: modelRef(reg)})
	if err != nil {
		t.Fatalf("CreateSession: %v", err)
	}
	if id == "" {
		t.Fatal("empty session id")
	}
	if err := svc.Watch(id); err != nil {
		t.Fatalf("Watch: %v", err)
	}
	if err := svc.Prompt(id, "hello bridge"); err != nil {
		t.Fatalf("Prompt: %v", err)
	}

	deadline := time.After(10 * time.Second)
	for {
		select {
		case ev := <-events:
			if ev.SessionID != id {
				t.Fatalf("event for %q, want %q", ev.SessionID, id)
			}
			if ev.Kind == eventbus.KindAgentEnd {
				goto ended
			}
		case <-deadline:
			t.Fatal("timed out waiting for agent_end")
		}
	}
ended:

	history, err := svc.History(id)
	if err != nil {
		t.Fatalf("History: %v", err)
	}
	found := false
	for _, msg := range history {
		if strings.Contains(msg.Text, "bridge answer") {
			found = true
		}
	}
	if !found {
		t.Fatalf("history lacks the answer: %+v", history)
	}

	list := svc.ListSessions(cwd)
	if len(list) != 1 || list[0].ID != id || list[0].Messages == 0 {
		t.Fatalf("ListSessions = %+v", list)
	}
	path, ok := svc.SessionPath(id)
	if !ok || !strings.HasSuffix(path, ".jsonl") {
		t.Fatalf("SessionPath = %q, %v", path, ok)
	}

	svc.Unwatch(id)
	svc.Unwatch(id)
	svc.Close()
	svc.Close()
}

func TestCancelStopsTheTurn(t *testing.T) {
	events := make(chan eventbus.Event, 256)
	svc, reg, cwd := newService(t, func(ev eventbus.Event) { events <- ev }, providers.RegisterFauxProviderOptions{TokensPerSecond: 5})
	reg.SetResponses([]providers.FauxResponseStep{
		providers.FauxStatic(providers.FauxAssistantMessage(ai.ContentList{
			providers.FauxText(strings.Repeat("streaming ", 80)),
		}, ai.StopStop)),
		providers.FauxStatic(providers.FauxAssistantMessage(ai.ContentList{
			providers.FauxText("second answer"),
		}, ai.StopStop)),
	})

	id, err := svc.CreateSession(CreateOptions{Cwd: cwd, Model: modelRef(reg)})
	if err != nil {
		t.Fatalf("CreateSession: %v", err)
	}
	if err := svc.Watch(id); err != nil {
		t.Fatalf("Watch: %v", err)
	}

	done := make(chan error, 1)
	go func() { done <- svc.Prompt(id, "stream please") }()

	deadline := time.After(10 * time.Second)
	for {
		select {
		case ev := <-events:
			if ev.Kind == eventbus.KindMessageUpdate {
				if err := svc.Cancel(id); err != nil {
					t.Fatalf("Cancel: %v", err)
				}
				goto canceled
			}
		case <-deadline:
			t.Fatal("timed out waiting for a stream update")
		}
	}
canceled:
	if err := <-done; !errors.Is(err, kernel.ErrCanceled) {
		t.Fatalf("Prompt err = %v, want ErrCanceled", err)
	}

	// The session stays usable after cancellation.
	if err := svc.Prompt(id, "again"); err != nil {
		t.Fatalf("second Prompt: %v", err)
	}
}

func TestValidation(t *testing.T) {
	svc, _, _ := newService(t, nil, providers.RegisterFauxProviderOptions{})
	if _, err := svc.CreateSession(CreateOptions{}); err == nil {
		t.Fatal("empty model accepted")
	}
	if _, err := svc.OpenSession("", "faux/faux-1", ""); err == nil {
		t.Fatal("empty path accepted")
	}
	if err := svc.Prompt("missing", "hello"); err == nil {
		t.Fatal("unknown session accepted")
	}
	if err := svc.Prompt("missing", "  "); err == nil {
		t.Fatal("empty prompt accepted")
	}
	if err := svc.Watch(""); err == nil {
		t.Fatal("empty watch id accepted")
	}
	if err := svc.SetModel("missing", "bad"); err == nil {
		t.Fatal("invalid model ref accepted")
	}
}
