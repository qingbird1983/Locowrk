package desktop

import (
	"time"

	"github.com/egoist/mygo/ui"

	"github.com/qingbird1983/locowrk/internal/eventbus"
)

// Slot names a region of the shell a plugin may fill. A slot is a layout
// contract, not a permission: whether an element shows is decided by the
// registry and the user's enable/disable state.
type Slot string

const (
	// SlotSessionSidebar is the top half of the native plugin rail.
	SlotSessionSidebar Slot = "session.sidebar"
	// SlotTimelineAbove is the native strip over the page.
	SlotTimelineAbove Slot = "session.timeline.above"
	// SlotToolDetail is the bottom half of the native plugin rail.
	SlotToolDetail Slot = "session.tool.detail"
	// SlotSessionTab is reserved for Phase 5 (main-area tabs).
	SlotSessionTab Slot = "session.tab"
	// SlotSettingsSection is reserved for a native settings panel.
	SlotSettingsSection Slot = "settings.section"
)

// DesktopPlugin is one native panel. It holds data only: Build runs on every
// frame and must not keep ui.Element anywhere.
type DesktopPlugin interface {
	// Name is the stable ID stored in the settings file.
	Name() string
	// Title is the text shown in the shell and the plugin menu.
	Title() string
	Slot() Slot
	// InterestedEvents lists the kinds the host should dispatch.
	InterestedEvents() []eventbus.Kind
	// Dispatch updates data. It may run on the bridge's delivery goroutine.
	Dispatch(ev eventbus.Event, h PluginHost)
	// Build draws the plugin on the main thread; it must not retain c or any
	// element it builds.
	Build(c *ui.Context, h PluginHost)
}

// PluginHost is what a plugin may ask of the shell.
type PluginHost interface {
	// SessionID is the session the shell shows, empty before one is selected.
	SessionID() string
	// SelectedToolCall is the tool call the page or the inspector selected.
	SelectedToolCall() string
	// SelectToolCall records the selection and repaints.
	SelectToolCall(id string)
	// Invalidate asks for the next frame; safe from any goroutine.
	Invalidate()
	// Diagnostic returns the plugin's latest diagnostic, empty when none.
	Diagnostic() string
	// Report records a non-fatal diagnostic for the plugin, as a malformed
	// event payload.
	Report(code, message string)
}

// PluginDiagnostic is one plugin failure or degradation.
type PluginDiagnostic struct {
	Plugin  string    `json:"plugin"`
	Code    string    `json:"code"`
	Message string    `json:"message"`
	At      time.Time `json:"at"`
	Count   int       `json:"count"`
}

// Diagnostic codes.
const (
	DiagPanic    = "panic"
	DiagPayload  = "payload"
	DiagSettings = "settings"
)

// sessionResetter is optional: a plugin that keeps per-session data clears it
// when the shell switches sessions.
type sessionResetter interface {
	Reset()
}

// counter is optional: a plugin reports its data sizes for the smoke snapshot.
type counter interface {
	Counts() (nodes, steps, tools int)
}

// scopedHost binds a host to one plugin, so diagnostics carry its name.
type scopedHost struct {
	h    *Host
	name string
}

func (s scopedHost) SessionID() string           { return s.h.SessionID() }
func (s scopedHost) SelectedToolCall() string    { return s.h.SelectedToolCall() }
func (s scopedHost) SelectToolCall(id string)    { s.h.SelectToolCall(id) }
func (s scopedHost) Invalidate()                 { s.h.Invalidate() }
func (s scopedHost) Diagnostic() string          { return s.h.Diagnostic(s.name) }
func (s scopedHost) Report(code, message string) { s.h.record(s.name, code, message) }
