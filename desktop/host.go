package desktop

import (
	"fmt"
	"sync"
	"time"

	"github.com/egoist/mygo"
	"github.com/egoist/mygo/ui"

	"github.com/qingbird1983/locowrk/internal/eventbus"
)

// HostOptions configure the plugin host.
type HostOptions struct {
	// SettingsPath is where plugin toggles persist; empty disables saving.
	SettingsPath string
	// Plugins are added after the built-in three, as smoke-only probes.
	Plugins []DesktopPlugin
}

// Host owns the plugin registry, the native rail and the embedded page. The
// window shows Host.View; the page is a *mygo.WebView it places.
type Host struct {
	mu       sync.Mutex
	entries  []*pluginEntry
	byKind   map[eventbus.Kind][]*pluginEntry
	session  string
	selected string
	settings settings
	path     string
	win      *mygo.Window
	web      *mygo.WebView
	diags    map[string]*PluginDiagnostic

	// menuOpen belongs to the view, which runs on the main thread only.
	menuOpen bool
	// frames counts View calls, for smoke evidence that the shell builds.
	frames int
}

type pluginEntry struct {
	plugin  DesktopPlugin
	enabled bool
	failed  bool
}

// NewPluginHost registers the built-in plugins and loads the user settings.
func NewPluginHost(opts HostOptions) *Host {
	plugins := append(defaultPlugins(), opts.Plugins...)
	names := make([]string, 0, len(plugins))
	for _, p := range plugins {
		names = append(names, p.Name())
	}
	h := &Host{
		byKind: map[eventbus.Kind][]*pluginEntry{},
		diags:  map[string]*PluginDiagnostic{},
		path:   opts.SettingsPath,
	}
	loaded, err := loadSettings(opts.SettingsPath, names)
	h.settings = loaded
	for _, p := range plugins {
		e := &pluginEntry{plugin: p, enabled: true}
		if on, ok := loaded.Plugins[p.Name()]; ok {
			e.enabled = on
		}
		h.entries = append(h.entries, e)
		for _, kind := range p.InterestedEvents() {
			h.byKind[kind] = append(h.byKind[kind], e)
		}
	}
	if err != nil {
		h.record("", DiagSettings, err.Error())
	}
	return h
}

// defaultPlugins returns the Phase 4 plugins in rail order.
func defaultPlugins() []DesktopPlugin {
	return []DesktopPlugin{
		newTreePlugin(),
		newTimelinePlugin(),
		newToolsPlugin(),
	}
}

// Attach gives the host its window and embedded page, then repaints.
func (h *Host) Attach(win *mygo.Window, web *mygo.WebView) {
	h.mu.Lock()
	h.win, h.web = win, web
	h.mu.Unlock()
	h.Invalidate()
}

// Dispatch sends one event to the plugins that asked for its kind. It never
// blocks the bridge: plugins only update data and ask for a frame.
func (h *Host) Dispatch(ev eventbus.Event) {
	h.mu.Lock()
	if h.session != "" && ev.SessionID != "" && ev.SessionID != h.session {
		h.mu.Unlock()
		return
	}
	targets := h.byKind[ev.Kind]
	active := make([]*pluginEntry, 0, len(targets))
	for _, e := range targets {
		if e.enabled && !e.failed {
			active = append(active, e)
		}
	}
	h.mu.Unlock()
	for _, e := range active {
		h.guardDispatch(e, ev)
	}
	if len(active) > 0 {
		h.Invalidate()
	}
}

// SetSession scopes plugins to id and clears per-session data.
func (h *Host) SetSession(id string) {
	h.mu.Lock()
	if h.session == id {
		h.mu.Unlock()
		return
	}
	h.session = id
	h.selected = ""
	entries := append([]*pluginEntry(nil), h.entries...)
	h.mu.Unlock()
	for _, e := range entries {
		if r, ok := e.plugin.(sessionResetter); ok {
			h.guardReset(e, r)
		}
	}
	h.Invalidate()
}

// SessionID returns the session the shell shows.
func (h *Host) SessionID() string {
	h.mu.Lock()
	defer h.mu.Unlock()
	return h.session
}

// SelectedToolCall returns the tool call selected in the page or the rail.
func (h *Host) SelectedToolCall() string {
	h.mu.Lock()
	defer h.mu.Unlock()
	return h.selected
}

// SelectToolCall records a selection and repaints.
func (h *Host) SelectToolCall(id string) {
	h.mu.Lock()
	changed := h.selected != id
	h.selected = id
	h.mu.Unlock()
	if changed {
		h.Invalidate()
	}
}

// Invalidate asks the window for the next frame.
func (h *Host) Invalidate() {
	h.mu.Lock()
	w := h.win
	h.mu.Unlock()
	if w != nil {
		w.Invalidate()
	}
}

// Diagnostic returns the latest message for name, empty when none.
func (h *Host) Diagnostic(name string) string {
	h.mu.Lock()
	defer h.mu.Unlock()
	if d := h.diags[name]; d != nil {
		return d.Message
	}
	return ""
}

// record keeps one diagnostic per plugin, counting repeats.
func (h *Host) record(plugin, code, message string) {
	h.mu.Lock()
	defer h.mu.Unlock()
	d := h.diags[plugin]
	if d == nil {
		d = &PluginDiagnostic{Plugin: plugin, At: time.Now().UTC()}
		h.diags[plugin] = d
	}
	d.Code, d.Message = code, message
	d.Count++
}

// fail disables a plugin after a panic and leaves a diagnostic behind.
func (h *Host) fail(e *pluginEntry, code string, cause any) {
	h.mu.Lock()
	e.failed = true
	h.mu.Unlock()
	h.record(e.plugin.Name(), code, fmt.Sprint(cause))
	h.Invalidate()
}

// retry clears a failure so the next frame builds the plugin again.
func (h *Host) retry(name string) {
	h.mu.Lock()
	for _, e := range h.entries {
		if e.plugin.Name() == name {
			e.failed = false
		}
	}
	delete(h.diags, name)
	h.mu.Unlock()
	h.Invalidate()
}

// setEnabled toggles one plugin and persists the choice.
func (h *Host) setEnabled(name string, on bool) {
	h.mu.Lock()
	for _, e := range h.entries {
		if e.plugin.Name() != name {
			continue
		}
		if e.enabled == on {
			h.mu.Unlock()
			return
		}
		e.enabled = on
		if on {
			e.failed = false
			delete(h.diags, name)
		}
		h.settings.Plugins[name] = on
		break
	}
	s := h.settings
	path := h.path
	err := saveSettings(path, s)
	h.mu.Unlock()
	if err != nil {
		h.record("", DiagSettings, err.Error())
	}
	h.Invalidate()
}

// setRail opens or closes the plugin rail and persists the choice.
func (h *Host) setRail(open bool) {
	h.mu.Lock()
	if (h.settings.Rail == "open") == open {
		h.mu.Unlock()
		return
	}
	if open {
		h.settings.Rail = "open"
	} else {
		h.settings.Rail = "closed"
	}
	s := h.settings
	path := h.path
	err := saveSettings(path, s)
	h.mu.Unlock()
	if err != nil {
		h.record("", DiagSettings, err.Error())
	}
	h.Invalidate()
}

// RailOpen reports whether the plugin rail shows.
func (h *Host) RailOpen() bool {
	h.mu.Lock()
	defer h.mu.Unlock()
	return h.settings.Rail != "closed"
}

// Snapshot is the smoke-visible state of the host.
type Snapshot struct {
	SessionID   string   `json:"sessionId"`
	Enabled     []string `json:"enabled"`
	Failed      []string `json:"failed"`
	Diagnostics int      `json:"diagnostics"`
	Nodes       int      `json:"nodes"`
	Steps       int      `json:"steps"`
	Tools       int      `json:"tools"`
	Selected    string   `json:"selected,omitempty"`
	Frames      int      `json:"frames"`
}

// Snapshot returns counters for acceptance assertions. It is not exposed to
// the page; only the smoke CLI calls it.
func (h *Host) Snapshot() Snapshot {
	h.mu.Lock()
	out := Snapshot{SessionID: h.session, Selected: h.selected}
	out.Enabled = []string{}
	out.Failed = []string{}
	entries := append([]*pluginEntry(nil), h.entries...)
	for _, e := range entries {
		if e.enabled {
			out.Enabled = append(out.Enabled, e.plugin.Name())
		}
		if e.failed {
			out.Failed = append(out.Failed, e.plugin.Name())
		}
	}
	out.Diagnostics = len(h.diags)
	out.Frames = h.frames
	h.mu.Unlock()
	for _, e := range entries {
		if c, ok := e.plugin.(counter); ok {
			nodes, steps, tools := c.Counts()
			out.Nodes += nodes
			out.Steps += steps
			out.Tools += tools
		}
	}
	return out
}

// guardDispatch contains a plugin panic during event handling.
func (h *Host) guardDispatch(e *pluginEntry, ev eventbus.Event) {
	defer func() {
		if r := recover(); r != nil {
			h.fail(e, DiagPanic, r)
		}
	}()
	e.plugin.Dispatch(ev, scopedHost{h: h, name: e.plugin.Name()})
}

// guardReset contains a plugin panic while clearing a session.
func (h *Host) guardReset(e *pluginEntry, r sessionResetter) {
	defer func() {
		if rec := recover(); rec != nil {
			h.fail(e, DiagPanic, rec)
		}
	}()
	r.Reset()
}

// guardBuild builds one plugin inside a frame, containing a panic. The frame
// that panicked loses the plugin's subtree; the next frame shows the failure
// card instead, and MyGo resets the build depth per frame.
func (h *Host) guardBuild(c *ui.Context, e *pluginEntry) (ok bool) {
	defer func() {
		if r := recover(); r != nil {
			h.fail(e, DiagPanic, r)
			ok = false
		}
	}()
	e.plugin.Build(c, scopedHost{h: h, name: e.plugin.Name()})
	return true
}

// slotEntries returns the plugins of slot in registration order, including
// failed ones so their card still shows. Disabled plugins are omitted.
func (h *Host) slotEntries(slot Slot) []*pluginEntry {
	h.mu.Lock()
	defer h.mu.Unlock()
	var out []*pluginEntry
	for _, e := range h.entries {
		if e.enabled && e.plugin.Slot() == slot {
			out = append(out, e)
		}
	}
	return out
}

// buildPlugin draws one plugin or its failure card.
func (h *Host) buildPlugin(c *ui.Context, e *pluginEntry) {
	if e.failed {
		h.buildFailure(c, e)
		return
	}
	if !h.guardBuild(c, e) {
		h.buildFailure(c, e)
	}
}

// buildFailure shows why a plugin is off, with a retry.
func (h *Host) buildFailure(c *ui.Context, e *pluginEntry) {
	t := c.Theme()
	ui.Column(c).Padding(6).Gap(2).Border(1, t.Border).Radius(t.Radius).Children(func() {
		ui.Textf(c, "%s 已停用（异常）", e.plugin.Title()).TextColor(t.Danger).FontSize(12)
		if msg := h.Diagnostic(e.plugin.Name()); msg != "" {
			ui.Text(c, msg).TextColor(t.TextMuted).FontSize(11)
		}
		if ui.Button(c, "重试").Clicked() {
			h.retry(e.plugin.Name())
		}
	})
}
