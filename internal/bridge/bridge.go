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
	// Cwd is the workspace used for project trust and skills.
	Cwd string
	// Tools supplies external A-track tools when a session is created/opened.
	Tools func() []kernel.ToolSpec
	// BeforeTool gates every tool call (Phase 6 extension permission hook).
	BeforeTool kernel.BeforeToolFunc
	// ProjectTrusted is the workspace skill/extension trust decision.
	ProjectTrusted  func(cwd string) bool
	SetProjectTrust func(cwd string, trusted bool) error
	// Extensions and commands are UI-only callbacks; nil hides the feature.
	Extensions          func() []ExtensionRow
	TrustExtension      func(name string, trusted bool) error
	SetExtensionEnabled func(name string, enabled bool) error
	Commands            func() []CommandRow
	RunCommand          func(ctx context.Context, name, args string) error
}

// ExtensionRow is one A-track extension for the native status panel.
type ExtensionRow struct {
	Name        string   `json:"name"`
	Version     string   `json:"version,omitempty"`
	Source      string   `json:"source"`
	State       string   `json:"state"`
	Reason      string   `json:"reason,omitempty"`
	SHA         string   `json:"sha,omitempty"`
	Enabled     bool     `json:"enabled"`
	Trusted     bool     `json:"trusted"`
	Tools       int      `json:"tools"`
	Commands    int      `json:"commands"`
	Diagnostics []string `json:"diagnostics,omitempty"`
}

// CommandRow is one extension command.
type CommandRow struct {
	Name        string `json:"name"`
	Description string `json:"description,omitempty"`
	Plugin      string `json:"plugin"`
}

// SkillRow is one discovered skill.
type SkillRow struct {
	Name        string `json:"name"`
	Description string `json:"description"`
	Path        string `json:"path"`
	Source      string `json:"source"`
	Disabled    bool   `json:"disabled,omitempty"`
}

// SkillsView is the skill panel DTO.
type SkillsView struct {
	Trusted     bool       `json:"trusted"`
	User        []SkillRow `json:"user"`
	Project     []SkillRow `json:"project"`
	Diagnostics []string   `json:"diagnostics,omitempty"`
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
	opts    Options
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
	return &Service{kernel: k, opts: opts, onEvent: opts.OnEvent, watched: map[string]*watch{}}, nil
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
		Cwd:          opts.Cwd,
		Model:        model,
		Thinking:     opts.Thinking,
		TrustProject: s.projectTrusted(opts.Cwd),
		ExtraTools:   s.extraTools(),
		BeforeTool:   s.opts.BeforeTool,
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
		Path:         path,
		Model:        ref,
		Thinking:     thinking,
		TrustProject: s.projectTrusted(s.opts.Cwd),
		ExtraTools:   s.extraTools(),
		BeforeTool:   s.opts.BeforeTool,
	})
	return string(id), err
}

func (s *Service) extraTools() []kernel.ToolSpec {
	if s.opts.Tools == nil {
		return nil
	}
	return s.opts.Tools()
}

func (s *Service) projectTrusted(cwd string) bool {
	if s.opts.ProjectTrusted == nil {
		return false
	}
	return s.opts.ProjectTrusted(cwd)
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

// Steer appends a user instruction to the running turn.
func (s *Service) Steer(sessionID, text string) error {
	if strings.TrimSpace(text) == "" {
		return errors.New("bridge: steering text is empty")
	}
	return s.kernel.Steer(context.Background(), kernel.SessionID(sessionID), text)
}

// Skills lists user and (when trusted) project skills.
func (s *Service) Skills(cwd string) SkillsView {
	if cwd == "" {
		cwd = s.opts.Cwd
	}
	trusted := s.projectTrusted(cwd)
	skills, diags := s.kernel.Skills(cwd, trusted)
	view := SkillsView{Trusted: trusted, User: []SkillRow{}, Project: []SkillRow{}, Diagnostics: []string{}}
	for _, sk := range skills {
		row := SkillRow{Name: sk.Name, Description: sk.Description, Path: sk.Path, Source: sk.Source, Disabled: sk.Disabled}
		if sk.Source == "project" {
			view.Project = append(view.Project, row)
		} else {
			view.User = append(view.User, row)
		}
	}
	for _, d := range diags {
		view.Diagnostics = append(view.Diagnostics, d.Type+": "+d.Message)
	}
	return view
}

// Extensions returns the A-track status rows, nil when the feature is off.
func (s *Service) Extensions() []ExtensionRow {
	if s.opts.Extensions == nil {
		return nil
	}
	return s.opts.Extensions()
}

// TrustExtension records a user's trust decision.
func (s *Service) TrustExtension(name string, trusted bool) error {
	if s.opts.TrustExtension == nil {
		return errors.New("bridge: extensions are not enabled")
	}
	return s.opts.TrustExtension(name, trusted)
}

// SetExtensionEnabled toggles one extension.
func (s *Service) SetExtensionEnabled(name string, enabled bool) error {
	if s.opts.SetExtensionEnabled == nil {
		return errors.New("bridge: extensions are not enabled")
	}
	return s.opts.SetExtensionEnabled(name, enabled)
}

// Commands lists extension commands.
func (s *Service) Commands() []CommandRow {
	if s.opts.Commands == nil {
		return nil
	}
	return s.opts.Commands()
}

// RunCommand executes one extension command.
func (s *Service) RunCommand(name, args string) error {
	if s.opts.RunCommand == nil {
		return errors.New("bridge: commands are not enabled")
	}
	return s.opts.RunCommand(context.Background(), name, args)
}

// SetProjectTrust records a workspace skill trust decision.
func (s *Service) SetProjectTrust(cwd string, trusted bool) error {
	if s.opts.SetProjectTrust == nil {
		return errors.New("bridge: project trust is not enabled")
	}
	return s.opts.SetProjectTrust(cwd, trusted)
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
