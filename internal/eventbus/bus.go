package eventbus

import (
	"encoding/json"
	"errors"
	"sync"
	"time"
)

// Defaults for a subscription. A consumer that needs a different trade-off
// passes its own values in SubscribeOptions.
const (
	DefaultBuffer   = 256
	DefaultMaxQueue = 1024
	DefaultTimeout  = 2 * time.Second
)

// ErrClosed is returned when publishing to a closed bus.
var ErrClosed = errors.New("eventbus: closed")

// SubscribeOptions configures a subscription.
type SubscribeOptions struct {
	// SessionID limits delivery to one session; empty means every session.
	SessionID string
	// Buffer is the handoff channel size (default DefaultBuffer).
	Buffer int
	// MaxQueue bounds the internal queue. Control events past it disconnect a
	// stalled subscriber instead of blocking the publisher
	// (architecture/02 §2.3).
	MaxQueue int
	// Timeout bounds how long the delivery goroutine may block on a stalled
	// reader before disconnecting it.
	Timeout time.Duration
}

// Stats is a snapshot of one subscription's counters.
type Stats struct {
	Delivered uint64
	Merged    uint64
	Dropped   uint64
	// Disconnected is the reason the subscription ended; empty while it is
	// still alive.
	Disconnected string
	// Pending is the number of events still queued for delivery.
	Pending int
}

// Bus fans events out to subscriptions. Seq is assigned per session, in publish
// order, so a consumer can detect gaps after reconnecting.
type Bus struct {
	mu     sync.Mutex
	subs   map[uint64]*Subscription
	nextID uint64
	seq    map[string]uint64
	closed bool
}

// New creates an empty bus.
func New() *Bus {
	return &Bus{subs: map[uint64]*Subscription{}, seq: map[string]uint64{}}
}

// Subscribe registers a subscriber and starts its delivery goroutine.
func (b *Bus) Subscribe(opts SubscribeOptions) *Subscription {
	if opts.Buffer <= 0 {
		opts.Buffer = DefaultBuffer
	}
	if opts.MaxQueue <= 0 {
		opts.MaxQueue = DefaultMaxQueue
	}
	if opts.MaxQueue < opts.Buffer {
		opts.MaxQueue = opts.Buffer
	}
	if opts.Timeout <= 0 {
		opts.Timeout = DefaultTimeout
	}

	s := &Subscription{
		bus:       b,
		sessionID: opts.SessionID,
		out:       make(chan Event, opts.Buffer),
		maxQueue:  opts.MaxQueue,
		timeout:   opts.Timeout,
	}
	s.cond = sync.NewCond(&s.mu)

	b.mu.Lock()
	b.nextID++
	s.id = b.nextID
	b.subs[s.id] = s
	b.mu.Unlock()

	go s.run()
	return s
}

// Publish assigns the next sequence number for sessionID and fans the event out
// to every matching subscription. It never blocks on a subscriber.
func (b *Bus) Publish(kind Kind, sessionID string, payload any) (Event, error) {
	raw, err := marshalPayload(payload)
	if err != nil {
		return Event{}, err
	}

	b.mu.Lock()
	if b.closed {
		b.mu.Unlock()
		return Event{}, ErrClosed
	}
	b.seq[sessionID]++
	ev := Event{
		Seq:       b.seq[sessionID],
		Kind:      kind,
		SessionID: sessionID,
		At:        time.Now().UTC(),
		Payload:   raw,
	}
	targets := make([]*Subscription, 0, len(b.subs))
	for _, s := range b.subs {
		if s.sessionID == "" || s.sessionID == sessionID {
			targets = append(targets, s)
		}
	}
	b.mu.Unlock()

	for _, s := range targets {
		s.enqueue(ev)
	}
	return ev, nil
}

// Close tears down every subscription. Later publishes return ErrClosed.
func (b *Bus) Close() {
	b.mu.Lock()
	if b.closed {
		b.mu.Unlock()
		return
	}
	b.closed = true
	subs := make([]*Subscription, 0, len(b.subs))
	for _, s := range b.subs {
		subs = append(subs, s)
	}
	b.mu.Unlock()

	for _, s := range subs {
		s.close("bus closed")
	}
}

// Subscription is one consumer's view of the bus. Read from C until it closes;
// Close disconnects early.
type Subscription struct {
	id        uint64
	bus       *Bus
	sessionID string
	out       chan Event
	maxQueue  int
	timeout   time.Duration

	mu        sync.Mutex
	cond      *sync.Cond
	queue     []Event
	closed    bool
	reason    string
	delivered uint64
	merged    uint64
	dropped   uint64
}

// C returns the delivery channel. It closes when the subscription ends, which
// includes the bus closing or the subscriber being disconnected for stalling.
func (s *Subscription) C() <-chan Event { return s.out }

// SessionID is the filter this subscription was created with.
func (s *Subscription) SessionID() string { return s.sessionID }

// Stats returns the subscription's counters.
func (s *Subscription) Stats() Stats {
	s.mu.Lock()
	defer s.mu.Unlock()
	return Stats{
		Delivered:    s.delivered,
		Merged:       s.merged,
		Dropped:      s.dropped,
		Disconnected: s.reason,
		Pending:      len(s.queue),
	}
}

// Close disconnects the subscription; buffered events remain readable until the
// channel drains.
func (s *Subscription) Close() { s.close("closed by consumer") }

func (s *Subscription) close(reason string) {
	s.mu.Lock()
	if s.closed {
		s.mu.Unlock()
		return
	}
	s.closed = true
	if s.reason == "" {
		s.reason = reason
	}
	s.mu.Unlock()
	s.cond.Broadcast()
	s.detach()
}

// enqueue queues ev without blocking. Mergeable deltas coalesce with the tail;
// when the queue is full a delta is dropped and counted, while a control event
// that cannot fit ends the subscription.
func (s *Subscription) enqueue(ev Event) {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.closed {
		return
	}

	if isMergeable(ev.Kind) && len(s.queue) > 0 {
		if mergeInto(&s.queue[len(s.queue)-1], ev) {
			s.merged++
			return
		}
	}

	if len(s.queue) >= s.maxQueue {
		if !isMergeable(ev.Kind) {
			s.closed = true
			s.reason = "queue overflow"
			s.dropped++
			s.cond.Broadcast()
			go s.detach()
			return
		}
		if !dropOldestDelta(s.queue) {
			s.closed = true
			s.reason = "queue overflow"
			s.dropped++
			s.cond.Broadcast()
			go s.detach()
			return
		}
		s.dropped++
		s.queue = s.queue[:len(s.queue)-1]
	}

	s.queue = append(s.queue, ev)
	s.cond.Signal()
}

func (s *Subscription) run() {
	defer close(s.out)

	for {
		s.mu.Lock()
		for len(s.queue) == 0 && !s.closed {
			s.cond.Wait()
		}
		if len(s.queue) == 0 {
			s.mu.Unlock()
			return
		}
		ev := s.queue[0]
		s.queue = s.queue[1:]
		s.mu.Unlock()

		timer := time.NewTimer(s.timeout)
		select {
		case s.out <- ev:
			timer.Stop()
			s.mu.Lock()
			s.delivered++
			s.mu.Unlock()
		case <-timer.C:
			// The consumer stopped reading: stop blocking on it and record why.
			s.close("subscriber stalled")
			return
		}
	}
}

// detach removes the subscription from its bus.
func (s *Subscription) detach() {
	b := s.bus
	if b == nil {
		return
	}
	b.mu.Lock()
	delete(b.subs, s.id)
	b.mu.Unlock()
}

// isMergeable reports whether a kind may be coalesced.
func isMergeable(k Kind) bool { return k == KindMessageUpdate }

// mergeInto folds next into prev when both describe the same delta block.
func mergeInto(prev *Event, next Event) bool {
	if prev.Kind != KindMessageUpdate || next.Kind != KindMessageUpdate {
		return false
	}
	if prev.SessionID != next.SessionID {
		return false
	}
	var a, b MessageUpdateDTO
	if err := json.Unmarshal(prev.Payload, &a); err != nil {
		return false
	}
	if err := json.Unmarshal(next.Payload, &b); err != nil {
		return false
	}
	if a.MessageID == "" || a.MessageID != b.MessageID {
		return false
	}
	if a.BlockIndex != b.BlockIndex || a.BlockType != b.BlockType {
		return false
	}
	if !isDeltaEvent(a.Event) || !isDeltaEvent(b.Event) {
		return false
	}
	a.Delta += b.Delta
	if b.Text != "" {
		a.Text = b.Text
	}
	raw, err := json.Marshal(a)
	if err != nil {
		return false
	}
	prev.Payload = raw
	prev.At = next.At
	return true
}

func isDeltaEvent(e string) bool {
	return e == "text_delta" || e == "thinking_delta" || e == "toolcall_delta"
}

// dropOldestDelta removes the earliest mergeable event in queue and reports
// whether one was found. The caller holds s.mu.
func dropOldestDelta(queue []Event) bool {
	for i := range queue {
		if isMergeable(queue[i].Kind) {
			copy(queue[i:], queue[i+1:])
			queue[len(queue)-1] = Event{}
			return true
		}
	}
	return false
}

func marshalPayload(payload any) (json.RawMessage, error) {
	switch v := payload.(type) {
	case nil:
		return nil, nil
	case json.RawMessage:
		return v, nil
	default:
		return json.Marshal(v)
	}
}
