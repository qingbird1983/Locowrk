package desktop

import (
	"errors"
	"fmt"
	"net/url"
	"strings"

	"github.com/egoist/mygo"
	"github.com/egoist/mygo/transfer"
)

// TabID identifies a main-area tab. TabSession is the transcript page; other
// IDs belong to web tabs opened by the shell.
type TabID string

// TabSession is the tab that shows the transcript page. It always exists and
// cannot be closed.
const TabSession TabID = "session"

// maxWebTabs caps open web tabs, each of which owns a system web view.
const maxWebTabs = 4

var (
	// ErrTooManyTabs means the web tab cap is reached.
	ErrTooManyTabs = errors.New("desktop: too many web tabs")
	// ErrTabURL means a tab URL is not an app frontend path.
	ErrTabURL = errors.New("desktop: tab URL must be an app path")
	// ErrTabClosed means the tab was closed before the call.
	ErrTabClosed = errors.New("desktop: tab is closed")
)

// TabRequest opens one web tab. Reopening a Key activates that tab instead of
// creating a second one.
type TabRequest struct {
	Key   string // stable within the session
	Title string // shown in the tab strip
	URL   string // app frontend path, starting with "/"
	Data  string // opaque payload for the page, such as a relative path
}

// TabInfo is a tab as the shell and page see it.
type TabInfo struct {
	ID    TabID
	Key   string
	Title string
	URL   string
	Data  string
}

// tabEntry owns one web view. reload and destroy are callbacks so tests can
// observe lifecycle without a real window.
type tabEntry struct {
	info    TabInfo
	web     *mygo.WebView
	page    *mygo.Page
	reload  func()
	destroy func()
}

// newTabFunc creates a tab's web view. Tests replace it; production uses the
// window's NewWebView.
type newTabFunc func(id TabID, req TabRequest) (*tabEntry, error)

// OpenTab creates a web tab or activates the one with the same Key. It is safe
// from any goroutine but must not be called while building a frame.
func (h *Host) OpenTab(req TabRequest) (TabID, error) {
	if !strings.HasPrefix(req.URL, "/") || strings.Contains(req.URL, "://") {
		return "", ErrTabURL
	}
	req.Title = clipTabTitle(req.Title)
	if req.Title == "" {
		req.Title = "标签页"
	}

	h.tabMu.Lock()
	defer h.tabMu.Unlock()

	h.mu.Lock()
	if req.Key != "" {
		for _, e := range h.tabs {
			if e.info.Key == req.Key {
				h.active = e.info.ID
				reload := e.reload
				h.mu.Unlock()
				if reload != nil {
					reload()
				}
				h.Invalidate()
				return e.info.ID, nil
			}
		}
	}
	if len(h.tabs) >= maxWebTabs {
		h.mu.Unlock()
		return "", ErrTooManyTabs
	}
	h.tabSeq++
	id := TabID(fmt.Sprintf("tab-%d", h.tabSeq))
	win := h.win
	factory := h.newTab
	h.mu.Unlock()

	req.URL = withTabQuery(req.URL, id)
	var e *tabEntry
	var err error
	if factory != nil {
		e, err = factory(id, req)
	} else if win == nil {
		err = errors.New("desktop: no window for a tab")
	} else {
		e, err = newWindowTab(win, id, req)
	}
	if err != nil {
		return "", err
	}
	e.info = TabInfo{ID: id, Key: req.Key, Title: req.Title, URL: req.URL, Data: req.Data}

	h.mu.Lock()
	h.tabs = append(h.tabs, e)
	h.active = id
	h.mu.Unlock()
	h.Invalidate()
	return id, nil
}

// newWindowTab creates a tab's web view in the host's window.
func newWindowTab(win *mygo.Window, id TabID, req TabRequest) (*tabEntry, error) {
	web, err := win.NewWebView(mygo.WebViewOptions{URL: req.URL})
	if err != nil {
		return nil, fmt.Errorf("creating tab %s: %w", id, err)
	}
	return &tabEntry{
		web:  web,
		page: web.Page(),
		reload: func() {
			if err := DiffReload.EmitPage(web.Page(), struct{}{}); err != nil {
				web.Page().Reload()
			}
		},
		destroy: web.Destroy,
	}, nil
}

// withTabQuery adds the tab id to a frontend URL.
func withTabQuery(raw string, id TabID) string {
	u, err := url.Parse(raw)
	if err != nil {
		return raw
	}
	q := u.Query()
	q.Set("tab", string(id))
	u.RawQuery = q.Encode()
	return u.String()
}

// clipTabTitle keeps a tab label within 24 runes.
func clipTabTitle(s string) string {
	r := []rune(strings.TrimSpace(s))
	if len(r) <= 24 {
		return string(r)
	}
	return string(r[:24]) + "…"
}

// ActivateTab shows a tab; TabSession returns to the transcript.
func (h *Host) ActivateTab(id TabID) {
	h.mu.Lock()
	if id != TabSession {
		found := false
		for _, e := range h.tabs {
			if e.info.ID == id {
				found = true
				break
			}
		}
		if !found {
			h.mu.Unlock()
			return
		}
	}
	changed := h.active != id
	h.active = id
	h.mu.Unlock()
	if changed {
		h.Invalidate()
	}
}

// CloseTab destroys one web tab and shows its neighbour. TabSession is
// ignored.
func (h *Host) CloseTab(id TabID) {
	if id == TabSession {
		return
	}
	h.tabMu.Lock()
	h.mu.Lock()
	idx := -1
	for i, e := range h.tabs {
		if e.info.ID == id {
			idx = i
			break
		}
	}
	if idx < 0 {
		h.mu.Unlock()
		h.tabMu.Unlock()
		return
	}
	e := h.tabs[idx]
	h.tabs = append(h.tabs[:idx], h.tabs[idx+1:]...)
	if h.active == id {
		if idx > 0 {
			h.active = h.tabs[idx-1].info.ID
		} else {
			h.active = TabSession
		}
	}
	h.mu.Unlock()
	if e.destroy != nil {
		e.destroy()
	}
	h.tabMu.Unlock()
	h.Invalidate()
}

// CloseTabs closes every web tab, as a session switch or window close does.
func (h *Host) CloseTabs() {
	h.closeTabs()
}

// closeTabs is CloseTabs without exporting the lifecycle to callers.
func (h *Host) closeTabs() {
	h.tabMu.Lock()
	h.mu.Lock()
	entries := h.tabs
	h.tabs = nil
	h.active = TabSession
	h.mu.Unlock()
	for _, e := range entries {
		if e.destroy != nil {
			e.destroy()
		}
	}
	h.tabMu.Unlock()
}

// Tabs returns the open web tabs in strip order.
func (h *Host) Tabs() []TabInfo {
	h.mu.Lock()
	defer h.mu.Unlock()
	out := make([]TabInfo, 0, len(h.tabs))
	for _, e := range h.tabs {
		out = append(out, e.info)
	}
	return out
}

// ActiveTab returns the visible tab, TabSession when the transcript shows.
func (h *Host) ActiveTab() TabID {
	h.mu.Lock()
	defer h.mu.Unlock()
	if h.active == "" {
		return TabSession
	}
	for _, e := range h.tabs {
		if e.info.ID == h.active {
			return h.active
		}
	}
	return TabSession
}

// TabData returns one open tab's info.
func (h *Host) TabData(id TabID) (TabInfo, bool) {
	h.mu.Lock()
	defer h.mu.Unlock()
	for _, e := range h.tabs {
		if e.info.ID == id {
			return e.info, true
		}
	}
	return TabInfo{}, false
}

// TabPage returns a tab's page, nil for the transcript tab or an unknown id.
// The smoke CLI uses it to read a diff page's DOM; pages do not get this.
func (h *Host) TabPage(id TabID) *mygo.Page {
	h.mu.Lock()
	defer h.mu.Unlock()
	for _, e := range h.tabs {
		if e.info.ID == id {
			return e.page
		}
	}
	return nil
}

// copyTabPath puts a tab's data on the clipboard. It is a shell action so the
// page never needs clipboard permissions.
func (h *Host) copyTabPath(id TabID) {
	info, ok := h.TabData(id)
	if !ok || info.Data == "" {
		return
	}
	if err := mygo.Clipboard.Write(transfer.TextData(info.Data)); err != nil {
		h.record("", "clipboard", err.Error())
	}
}

// reloadTab reloads one tab's page.
func (h *Host) reloadTab(id TabID) {
	h.mu.Lock()
	var reload func()
	for _, e := range h.tabs {
		if e.info.ID == id {
			reload = e.reload
			break
		}
	}
	h.mu.Unlock()
	if reload != nil {
		reload()
	}
}

// frameTab is one tab in a frame snapshot.
type frameTab struct {
	info TabInfo
	web  *mygo.WebView
}

// tabState returns tabs and the active page under one lock.
func (h *Host) tabState() ([]frameTab, TabID) {
	h.mu.Lock()
	defer h.mu.Unlock()
	tabs := make([]frameTab, 0, len(h.tabs))
	for _, e := range h.tabs {
		tabs = append(tabs, frameTab{info: e.info, web: e.web})
	}
	active := TabSession
	for _, e := range h.tabs {
		if e.info.ID == h.active {
			active = h.active
			break
		}
	}
	return tabs, active
}
