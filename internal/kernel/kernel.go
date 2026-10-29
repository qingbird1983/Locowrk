// Package kernel is the only package that imports github.com/qingbird1983/locowrk/pi.
// It owns sessions, turns their lifecycle events into project events and keeps
// upstream types out of every other package's signatures (architecture/02).
package kernel

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"strings"
	"sync"

	"github.com/qingbird1983/locowrk/pi/agent"
	"github.com/qingbird1983/locowrk/pi/ai"
	"github.com/qingbird1983/locowrk/pi/coding"

	"github.com/qingbird1983/locowrk/internal/eventbus"
)

// SessionID identifies a session inside one Service.
type SessionID string

// Well-known kernel errors. Callers switch on these instead of matching
// upstream message text.
var (
	// ErrCanceled reports a run stopped by Abort or a canceled context. It is
	// not a failure (architecture/02 搂7).
	ErrCanceled = errors.New("kernel: canceled")
	// ErrSessionNotFound reports an unknown SessionID.
	ErrSessionNotFound = errors.New("kernel: session not found")
	// ErrModelNotFound reports a model reference no runtime knows.
	ErrModelNotFound = errors.New("kernel: model not found")
	// ErrNotRunning reports work that needs a running turn, such as steering
	// while the session is idle.
	ErrNotRunning = errors.New("kernel: session is not running")
)

// ModelRef names a model without exposing the upstream type.
type ModelRef struct {
	Provider string
	ID       string
}

// String renders the reference as provider/id.
func (m ModelRef) String() string { return m.Provider + "/" + m.ID }

// Image is an image attached to a prompt.
type Image struct {
	Data     string
	MimeType string
}

// BeforeToolFunc runs after a tool call's arguments are validated and before
// the tool executes. A non-empty reason blocks the call; the model sees an
// error tool result carrying it. Hosts use it for extension permission gates.
type BeforeToolFunc func(ctx context.Context, name string, args json.RawMessage) (reason string, err error)

// Options configures a Service.
type Options struct {
	// Bus receives every event. Nil creates a private bus owned by the Service.
	Bus *eventbus.Bus
	// LookupModel resolves a ModelRef to an upstream model. Nil uses the
	// built-in catalog (ai.GetModel).
	LookupModel func(ModelRef) (*ai.Model, error)
	// Models is the runtime every session streams through. Nil lets the
	// upstream resolve credentials from the environment.
	Models ai.Models
}

// SessionOptions configures one new session.
type SessionOptions struct {
	Cwd          string
	Model        ModelRef
	Thinking     string
	Tools        []string
	ExcludeTools []string
	// NoTools disables the built-in tool set entirely.
	NoTools      bool
	SystemPrompt string
	TrustProject bool
	// ExtraTools are tools owned outside the kernel (orchestration plugins).
	ExtraTools []ToolSpec
	// BeforeTool gates tool execution (Phase 6 extension permission hook).
	BeforeTool BeforeToolFunc
}

// OpenOptions configures resuming a stored session.
type OpenOptions struct {
	// Path is the JSONL session file.
	Path         string
	Model        ModelRef
	Thinking     string
	Tools        []string
	ExcludeTools []string
	// NoTools disables the built-in tool set entirely.
	NoTools      bool
	SystemPrompt string
	TrustProject bool
	// ExtraTools are tools owned outside the kernel; pass the same set again
	// when resuming so the model keeps seeing them.
	ExtraTools []ToolSpec
	// BeforeTool gates tool execution; pass the same hook when resuming.
	BeforeTool BeforeToolFunc
}

// Service owns the sessions of one host (the desktop shell, the smoke CLI).
type Service struct {
	opts     Options
	bus      *eventbus.Bus
	ownsBus  bool
	mu       sync.Mutex
	sessions map[SessionID]*session
	closed   bool
}

type session struct {
	id   SessionID
	sess *coding.Session
	rec  *coding.SessionRecorder

	// ids assigns transcript positions to live message events; it must start
	// at the number of messages already stored when a session is resumed.
	ids idSource

	mu     sync.Mutex
	model  ModelRef
	path   string
	cwd    string
	closed bool
	// cancel stops the active run's context; nil while idle. Abort cancels it
	// so a stopped turn is classified as canceled, not as a provider failure.
	cancel context.CancelFunc
}

// New validates options and returns a Service.
func New(opts Options) (*Service, error) {
	s := &Service{opts: opts, sessions: map[SessionID]*session{}}
	if opts.Bus != nil {
		s.bus = opts.Bus
	} else {
		s.bus = eventbus.New()
		s.ownsBus = true
	}
	return s, nil
}

// Events exposes the bus so a host can subscribe before the first prompt.
func (s *Service) Events() *eventbus.Bus { return s.bus }

// Subscribe is shorthand for Events().Subscribe.
func (s *Service) Subscribe(opts eventbus.SubscribeOptions) *eventbus.Subscription {
	return s.bus.Subscribe(opts)
}

// CreateSession starts a new persisted session.
func (s *Service) CreateSession(ctx context.Context, opts SessionOptions) (SessionID, error) {
	if ctx == nil {
		ctx = context.Background()
	}
	cwd := opts.Cwd
	model, err := s.resolveModel(opts.Model)
	if err != nil {
		return "", err
	}
	thinking := opts.Thinking
	if thinking == "" {
		thinking = string(coding.DefaultThinkingLevel)
	}

	rec, err := coding.StartSession(cwd, model, thinking)
	if err != nil {
		return "", fmt.Errorf("starting session: %w", err)
	}
	sess, err := s.newCodingSession(cwd, model, opts, rec.ID())
	if err != nil {
		_ = rec.Close()
		return "", err
	}
	sess.Record(rec)

	entry := &session{
		id:    SessionID(rec.ID()),
		sess:  sess,
		rec:   rec,
		model: ModelRef{Provider: string(model.Provider), ID: model.ID},
		path:  rec.Path(),
		cwd:   cwd,
	}
	entry.ids.reset(0)
	s.attach(entry)

	s.mu.Lock()
	defer s.mu.Unlock()
	if s.closed {
		return "", errors.New("kernel: service closed")
	}
	s.sessions[entry.id] = entry
	return entry.id, nil
}

// OpenSession resumes a stored JSONL session.
func (s *Service) OpenSession(ctx context.Context, opts OpenOptions) (SessionID, error) {
	if ctx == nil {
		ctx = context.Background()
	}
	if opts.Path == "" {
		return "", errors.New("kernel: open session needs a path")
	}
	model, err := s.resolveModel(opts.Model)
	if err != nil {
		return "", err
	}
	tree, err := coding.LoadSessionTree(opts.Path)
	if err != nil {
		return "", fmt.Errorf("loading session tree: %w", err)
	}
	rec, err := coding.ResumeSession(opts.Path)
	if err != nil {
		return "", fmt.Errorf("resuming session: %w", err)
	}
	cwd := tree.Header.Cwd
	base := SessionOptions{
		Cwd:          cwd,
		Tools:        opts.Tools,
		ExcludeTools: opts.ExcludeTools,
		NoTools:      opts.NoTools,
		SystemPrompt: opts.SystemPrompt,
		TrustProject: opts.TrustProject,
		ExtraTools:   opts.ExtraTools,
		BeforeTool:   opts.BeforeTool,
	}
	sess, err := s.newCodingSession(cwd, model, base, rec.ID())
	if err != nil {
		_ = rec.Close()
		return "", err
	}
	sess.LoadBranch(tree.BuildProjection())
	sess.Record(rec)

	entry := &session{
		id:    SessionID(rec.ID()),
		sess:  sess,
		rec:   rec,
		model: ModelRef{Provider: string(model.Provider), ID: model.ID},
		path:  opts.Path,
		cwd:   cwd,
	}
	entry.ids.reset(len(sess.History()))
	s.attach(entry)

	s.mu.Lock()
	defer s.mu.Unlock()
	s.sessions[entry.id] = entry
	return entry.id, nil
}

func (s *Service) newCodingSession(cwd string, model *ai.Model, opts SessionOptions, sessionID string) (*coding.Session, error) {
	customTools, err := agentTools(opts.ExtraTools)
	if err != nil {
		return nil, err
	}
	co := coding.SessionOptions{
		Model:         model,
		Cwd:           cwd,
		SystemPrompt:  opts.SystemPrompt,
		ThinkingLevel: agent.ThinkingLevel(opts.Thinking),
		SessionID:     sessionID,
		TrustProject:  opts.TrustProject,
		ToolNames:     opts.Tools,
		ExcludeTools:  opts.ExcludeTools,
		CustomTools:   customTools,
		Models:        s.opts.Models,
	}
	if opts.NoTools {
		co.NoTools = coding.NoToolsAll
		co.ToolNames = nil
	}
	if opts.BeforeTool != nil {
		gate := opts.BeforeTool
		co.BeforeToolCall = func(ctx context.Context, call agent.BeforeToolCallContext) *agent.BeforeToolCallResult {
			raw, err := json.Marshal(call.Args)
			if err != nil {
				return &agent.BeforeToolCallResult{Block: true, Reason: "tool arguments: " + err.Error()}
			}
			reason, err := gate(ctx, call.ToolCall.Name, raw)
			if err != nil {
				return &agent.BeforeToolCallResult{Block: true, Reason: err.Error()}
			}
			if reason == "" {
				return nil
			}
			return &agent.BeforeToolCallResult{Block: true, Reason: reason}
		}
	}
	sess, err := coding.NewSession(co)
	if err != nil {
		return nil, fmt.Errorf("creating session: %w", err)
	}
	return sess, nil
}

// attach wires the session's lifecycle events into the bus.
func (s *Service) attach(entry *session) {
	entry.sess.Subscribe(func(_ context.Context, e agent.AgentEvent) error {
		kind, ok := eventKind(e.Type)
		if !ok {
			return nil
		}
		index, turnResults := entry.ids.observe(e)
		// Publish never blocks the loop and a marshal failure must not abort
		// the run, so the error is intentionally dropped here.
		_, _ = s.bus.Publish(kind, string(entry.id), payloadFor(e, index, turnResults))
		return nil
	})
}

// Prompt runs one turn, blocking until the run completes. Cancellation returns
// ErrCanceled and keeps whatever the run produced.
func (s *Service) Prompt(ctx context.Context, id SessionID, text string, images ...Image) error {
	entry, err := s.lookup(id)
	if err != nil {
		return err
	}
	aiImages := make([]ai.ImageContent, 0, len(images))
	for _, img := range images {
		aiImages = append(aiImages, ai.ImageContent{Data: img.Data, MimeType: img.MimeType})
	}
	runCtx, cancel := context.WithCancel(ctx)
	entry.mu.Lock()
	entry.cancel = cancel
	entry.mu.Unlock()
	defer func() {
		cancel()
		entry.mu.Lock()
		entry.cancel = nil
		entry.mu.Unlock()
	}()

	_, runErr := entry.sess.Run(runCtx, text, aiImages...)
	if runErr == nil {
		return nil
	}
	if errors.Is(runErr, context.Canceled) || errors.Is(runErr, context.DeadlineExceeded) || runCtx.Err() != nil {
		return fmt.Errorf("%w: %w", ErrCanceled, runErr)
	}
	return fmt.Errorf("prompt: %w", runErr)
}

// Steer queues a message for the running turn. Steering an idle session is an
// error: the caller must send a normal prompt instead (phase-6 搂5.4).
func (s *Service) Steer(ctx context.Context, id SessionID, text string) error {
	entry, err := s.lookup(id)
	if err != nil {
		return err
	}
	if !entry.sess.Agent.State().IsStreaming {
		return ErrNotRunning
	}
	entry.sess.Steer(ai.UserMessage{Content: ai.ContentList{ai.TextContent{Text: text}}})
	if _, err := s.bus.Publish(eventbus.KindSteeringApplied, string(id), map[string]string{"text": text}); err != nil {
		return fmt.Errorf("publishing steering event: %w", err)
	}
	return nil
}

// Abort cancels the session's active run.
func (s *Service) Abort(ctx context.Context, id SessionID) error {
	entry, err := s.lookup(id)
	if err != nil {
		return err
	}
	entry.sess.Abort()
	entry.mu.Lock()
	cancel := entry.cancel
	entry.mu.Unlock()
	if cancel != nil {
		cancel()
	}
	return nil
}

// History returns the session transcript as project DTOs.
func (s *Service) History(ctx context.Context, id SessionID) ([]eventbus.MessageDTO, error) {
	entry, err := s.lookup(id)
	if err != nil {
		return nil, err
	}
	msgs := entry.sess.History()
	out := make([]eventbus.MessageDTO, 0, len(msgs))
	for i, m := range msgs {
		out = append(out, messageDTO(m, i))
	}
	return out, nil
}

// SessionPath returns the JSONL file backing the session.
func (s *Service) SessionPath(id SessionID) (string, bool) {
	entry, err := s.lookup(id)
	if err != nil {
		return "", false
	}
	return entry.path, entry.path != ""
}

// SetModel switches the model of a live session.
func (s *Service) SetModel(ctx context.Context, id SessionID, ref ModelRef) error {
	entry, err := s.lookup(id)
	if err != nil {
		return err
	}
	model, err := s.resolveModel(ref)
	if err != nil {
		return err
	}
	entry.sess.SetModel(model)
	entry.mu.Lock()
	entry.model = ref
	entry.mu.Unlock()
	return nil
}

// SetThinking switches the reasoning level of a live session.
func (s *Service) SetThinking(ctx context.Context, id SessionID, level string) error {
	entry, err := s.lookup(id)
	if err != nil {
		return err
	}
	entry.sess.SetThinkingLevel(agent.ThinkingLevel(level))
	return nil
}

// Close flushes every recorder and shuts the bus down.
func (s *Service) Close(ctx context.Context) error {
	s.mu.Lock()
	if s.closed {
		s.mu.Unlock()
		return nil
	}
	s.closed = true
	entries := make([]*session, 0, len(s.sessions))
	for _, e := range s.sessions {
		entries = append(entries, e)
	}
	s.sessions = map[SessionID]*session{}
	s.mu.Unlock()

	var errs []error
	for _, e := range entries {
		e.mu.Lock()
		if !e.closed {
			e.closed = true
			if err := e.rec.Close(); err != nil {
				errs = append(errs, err)
			}
		}
		e.mu.Unlock()
	}
	if s.ownsBus {
		s.bus.Close()
	}
	return errors.Join(errs...)
}

func (s *Service) lookup(id SessionID) (*session, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	entry, ok := s.sessions[id]
	if !ok {
		return nil, fmt.Errorf("%w: %s", ErrSessionNotFound, id)
	}
	return entry, nil
}

func (s *Service) resolveModel(ref ModelRef) (*ai.Model, error) {
	if ref.Provider == "" || ref.ID == "" {
		return nil, fmt.Errorf("%w: empty reference", ErrModelNotFound)
	}
	if s.opts.LookupModel != nil {
		m, err := s.opts.LookupModel(ref)
		if err != nil {
			return nil, fmt.Errorf("%w: %s: %w", ErrModelNotFound, ref, err)
		}
		if m == nil {
			return nil, fmt.Errorf("%w: %s", ErrModelNotFound, ref)
		}
		return m, nil
	}
	if m := ai.GetModel(ref.Provider, ref.ID); m != nil {
		return m, nil
	}
	return nil, fmt.Errorf("%w: %s", ErrModelNotFound, ref)
}

// ParseModelRef parses "provider/id".
func ParseModelRef(s string) (ModelRef, error) {
	provider, id, ok := strings.Cut(s, "/")
	if !ok || provider == "" || id == "" {
		return ModelRef{}, fmt.Errorf("kernel: model must be provider/id, got %q", s)
	}
	return ModelRef{Provider: provider, ID: id}, nil
}

// IsCanceled reports whether err is a canceled run rather than a failure.
func IsCanceled(err error) bool { return errors.Is(err, ErrCanceled) }
