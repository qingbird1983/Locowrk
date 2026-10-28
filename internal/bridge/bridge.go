// Package bridge is the desktop-facing facade over the kernel. It moves only
// project DTOs, never upstream or UI types, so a WebView frontend, a native
// view or a future CLI can all consume it (architecture/03 seam #2).
package bridge

import (
	"context"
	"errors"
	"strings"
	"sync"

	"github.com/qingbird1983/locowrk/internal/eventbus"
	"github.com/qingbird1983/locowrk/internal/kernel"
)

// Options configures a Service.
type Options struct {
	// OnEvent receives every event of a watched session, in Seq order. It runs
	// on the eventbus delivery goroutine and must not block.
	OnEvent func(eventbus.Event)
}

// CreateOptions describes a new session.
type CreateOptions struct {
	Cwd      string `json:"cwd"`
	Model    string `json:"model"`
	Thinking string `json:"thinking,omitempty"`
}

// Service is the DTO-only facade bound to the desktop frontend.
type Service struct {
	kernel  *kernel.Service
	onEvent func(eventbus.Event)
	mu      sync.Mutex
	watched map[string]*watch
	closed  bool
}

type watch struct {
	sub  *eventbus.Subscription
	done chan struct{}
}

// New returns a facade around k.
func New(k *kernel.Service, opts Options) (*Service, error) {
	if k == nil {
		return nil, errors.New("bridge: nil kernel service")
	}
	return &Service{kernel: k, onEvent: opts.OnEvent, watched: map[string]*watch{}}, nil
}

// ListSessions returns stored sessions for cwd, newest first.
func (s *Service) ListSessions(cwd string) []kernel.SessionSummary {
	return s.kernel.ListSessions(cwd)
}

// CreateSession starts a new persisted session and returns its id.
func (s *Service) CreateSession(opts CreateOptions) (string, error) {
	model, err := kernel.ParseModelRef(opts.Model)
	if err != nil {
		return "", err
	}
	id, err := s.kernel.CreateSession(context.Background(), kernel.SessionOptions{
		Cwd:      opts.Cwd,
		Model:    model,
		Thinking: opts.Thinking,
	})
	return string(id), err
}

// OpenSession resumes a stored session file and returns its id.
func (s *Service) OpenSession(path, model, thinking string) (string, error) {
	if strings.TrimSpace(path) == "" {
		return "", errors.New("bridge: session path is required")
	}
	ref, err := kernel.ParseModelRef(model)
	if err != nil {
		return "", err
	}
	id, err := s.kernel.OpenSession(context.Background(), kernel.OpenOptions{
		Path:     path,
		Model:    ref,
		Thinking: thinking,
	})
	return string(id), err
}

// Prompt runs one turn and blocks until it finishes. The frontend calls it from
// its own goroutine and stops it with Cancel.
func (s *Service) Prompt(sessionID, text string) error {
	if strings.TrimSpace(text) == "" {
		return errors.New("bridge: prompt is empty")
	}
	return s.kernel.Prompt(context.Background(), kernel.SessionID(sessionID), text)
}

// Cancel stops the session's active turn. A canceled turn is not an error.
func (s *Service) Cancel(sessionID string) error {
	return s.kernel.Abort(context.Background(), kernel.SessionID(sessionID))
}

// History returns the session transcript as DTOs.
func (s *Service) History(sessionID string) ([]eventbus.MessageDTO, error) {
	return s.kernel.History(context.Background(), kernel.SessionID(sessionID))
}

// SetModel switches the session's model.
func (s *Service) SetModel(sessionID, model string) error {
	ref, err := kernel.ParseModelRef(model)
	if err != nil {
		return err
	}
	return s.kernel.SetModel(context.Background(), kernel.SessionID(sessionID), ref)
}

// SetThinking switches the session's thinking level.
func (s *Service) SetThinking(sessionID, level string) error {
	return s.kernel.SetThinking(context.Background(), kernel.SessionID(sessionID), level)
}

// Watch starts forwarding one session's events to OnEvent. Watching twice is a
// no-op; events already delivered before Watch are not replayed.
func (s *Service) Watch(sessionID string) error {
	if sessionID == "" {
		return errors.New("bridge: watch needs a session id")
	}
	s.mu.Lock()
	if s.closed {
		s.mu.Unlock()
		return errors.New("bridge: service closed")
	}
	if _, ok := s.watched[sessionID]; ok {
		s.mu.Unlock()
		return nil
	}
	sub := s.kernel.Subscribe(eventbus.SubscribeOptions{SessionID: sessionID})
	entry := &watch{sub: sub, done: make(chan struct{})}
	s.watched[sessionID] = entry
	s.mu.Unlock()

	go func() {
		defer close(entry.done)
		for ev := range sub.C() {
			if s.onEvent != nil {
				s.onEvent(ev)
			}
		}
	}()
	return nil
}

// Unwatch stops forwarding a session's events.
func (s *Service) Unwatch(sessionID string) {
	s.mu.Lock()
	entry := s.watched[sessionID]
	delete(s.watched, sessionID)
	s.mu.Unlock()
	if entry != nil {
		entry.sub.Close()
		<-entry.done
	}
}

// Close stops every watch. It does not close the kernel service.
func (s *Service) Close() {
	s.mu.Lock()
	s.closed = true
	entries := make([]*watch, 0, len(s.watched))
	for id, entry := range s.watched {
		entries = append(entries, entry)
		delete(s.watched, id)
	}
	s.mu.Unlock()
	for _, entry := range entries {
		entry.sub.Close()
		<-entry.done
	}
}

// SessionPath returns the JSONL file backing a live session.
func (s *Service) SessionPath(sessionID string) (string, bool) {
	return s.kernel.SessionPath(kernel.SessionID(sessionID))
}
