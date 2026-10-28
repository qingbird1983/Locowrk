package desktop

import (
	"strings"
	"sync"
	"testing"

	"github.com/egoist/mygo"
	"github.com/egoist/mygo/ui"

	"github.com/qingbird1983/locowrk/internal/eventbus"
)

// fakeTabs creates zero-value web views for tab tests. MyGo's
// ui.NativeWebView cannot be implemented outside mygo, but ui.WebView and the
// headless tester only compare *mygo.WebView identity (P-009).
type fakeTabs struct {
	mu        sync.Mutex
	made      []TabRequest
	destroyed []TabID
	reloads   int
}

func (f *fakeTabs) new(id TabID, req TabRequest) (*tabEntry, error) {
	f.mu.Lock()
	f.made = append(f.made, req)
	f.mu.Unlock()
	return &tabEntry{
		web: new(mygo.WebView),
		reload: func() {
			f.mu.Lock()
			f.reloads++
			f.mu.Unlock()
		},
		destroy: func() {
			f.mu.Lock()
			f.destroyed = append(f.destroyed, id)
			f.mu.Unlock()
		},
	}, nil
}

func newTabHost(t *testing.T) (*Host, *fakeTabs) {
	t.Helper()
	h := newTestHost(t)
	f := &fakeTabs{}
	h.newTab = f.new
	h.SetSession("s1")
	return h, f
}

func tabWeb(h *Host, id TabID) *mygo.WebView {
	h.mu.Lock()
	defer h.mu.Unlock()
	for _, e := range h.tabs {
		if e.info.ID == id {
			return e.web
		}
	}
	return nil
}

func TestTabsOpenActivateClose(t *testing.T) {
	h, f := newTabHost(t)
	a, err := h.OpenTab(TabRequest{Key: "a", Title: "变更 A", URL: "/a.html", Data: "a.txt"})
	if err != nil {
		t.Fatal(err)
	}
	b, err := h.OpenTab(TabRequest{Key: "b", Title: "变更 B", URL: "/b.html"})
	if err != nil {
		t.Fatal(err)
	}
	if len(h.Tabs()) != 2 || h.ActiveTab() != b {
		t.Fatalf("tabs=%v active=%s", h.Tabs(), h.ActiveTab())
	}
	if !strings.Contains(f.made[0].URL, "tab=tab-1") {
		t.Fatalf("tab id not in URL: %q", f.made[0].URL)
	}

	// Reopening a key focuses it and asks it to reload.
	again, err := h.OpenTab(TabRequest{Key: "a", Title: "变更 A", URL: "/a.html"})
	if err != nil || again != a {
		t.Fatalf("reopen = %v, %v", again, err)
	}
	if h.ActiveTab() != a || f.reloads != 1 || len(f.made) != 2 {
		t.Fatalf("reopen active=%s reloads=%d made=%d", h.ActiveTab(), f.reloads, len(f.made))
	}

	h.CloseTab(b)
	if h.ActiveTab() != a || len(h.Tabs()) != 1 {
		t.Fatalf("close did not fall back: active=%s tabs=%v", h.ActiveTab(), h.Tabs())
	}
	f.mu.Lock()
	if len(f.destroyed) != 1 || f.destroyed[0] != b {
		t.Fatalf("destroyed %v", f.destroyed)
	}
	f.mu.Unlock()

	h.ActivateTab(TabSession)
	if h.ActiveTab() != TabSession {
		t.Fatal("did not return to the transcript")
	}
	h.CloseTab(a)
	if len(h.Tabs()) != 0 || h.ActiveTab() != TabSession {
		t.Fatalf("tabs left: %v", h.Tabs())
	}
}

func TestTabsCapAndCloseAll(t *testing.T) {
	h, f := newTabHost(t)
	for i := 0; i < maxWebTabs; i++ {
		if _, err := h.OpenTab(TabRequest{Key: "k" + string(rune('a'+i)), URL: "/x.html"}); err != nil {
			t.Fatal(err)
		}
	}
	if _, err := h.OpenTab(TabRequest{Key: "extra", URL: "/x.html"}); err != ErrTooManyTabs {
		t.Fatalf("cap error = %v", err)
	}
	h.CloseTabs()
	if len(h.Tabs()) != 0 || h.ActiveTab() != TabSession {
		t.Fatalf("close all left %v", h.Tabs())
	}
	f.mu.Lock()
	if len(f.destroyed) != maxWebTabs {
		t.Fatalf("destroyed %d of %d", len(f.destroyed), maxWebTabs)
	}
	f.mu.Unlock()

	// A session switch closes tabs too.
	if _, err := h.OpenTab(TabRequest{Key: "s", URL: "/x.html"}); err != nil {
		t.Fatal(err)
	}
	h.SetSession("s2")
	if len(h.Tabs()) != 0 {
		t.Fatalf("session switch kept %v", h.Tabs())
	}
}

func TestTabURLChecks(t *testing.T) {
	h, _ := newTabHost(t)
	for _, u := range []string{"https://example.com", "diff.html", ""} {
		if _, err := h.OpenTab(TabRequest{URL: u}); err != ErrTabURL {
			t.Fatalf("%q: err = %v", u, err)
		}
	}
	if h.Snapshot().Tabs != 0 {
		t.Fatal("rejected URL created a tab")
	}
}

func TestTabTitleClipping(t *testing.T) {
	h, f := newTabHost(t)
	long := strings.Repeat("标签", 20)
	if _, err := h.OpenTab(TabRequest{Key: "x", Title: long, URL: "/x.html"}); err != nil {
		t.Fatal(err)
	}
	f.mu.Lock()
	title := f.made[0].Title
	f.mu.Unlock()
	if len([]rune(title)) != 25 || !strings.HasSuffix(title, "…") {
		t.Fatalf("title %q is %d runes", title, len([]rune(title)))
	}
}

func TestTabBarPlacesActivePage(t *testing.T) {
	h, _ := newTabHost(t)
	h.web = new(mygo.WebView)
	a, err := h.OpenTab(TabRequest{Key: "a", Title: "变更 A", URL: "/a.html"})
	if err != nil {
		t.Fatal(err)
	}
	b, err := h.OpenTab(TabRequest{Key: "b", Title: "变更 B", URL: "/b.html"})
	if err != nil {
		t.Fatal(err)
	}
	tester := ui.NewTester(h.View, 900, 600)
	tester.Frame()
	if got := tester.WebViewAt(450, 300); got != tabWeb(h, b) {
		t.Fatalf("initial page is not B")
	}
	h.ActivateTab(a)
	tester.Frame()
	if got := tester.WebViewAt(450, 300); got != tabWeb(h, a) {
		t.Fatalf("switching did not place A")
	}
	h.ActivateTab(TabSession)
	tester.Frame()
	if got := tester.WebViewAt(450, 300); got != h.web {
		t.Fatalf("transcript tab did not place the main page")
	}
	if !tester.HasText("● 会话") || !tester.HasText("变更 A") {
		t.Fatalf("tab labels missing: %v", tester.Texts())
	}
}

func TestTabMenuCoversPage(t *testing.T) {
	h, f := newTabHost(t)
	h.web = new(mygo.WebView)
	if _, err := h.OpenTab(TabRequest{Key: "a", Title: "变更 A", URL: "/a.html", Data: "a.txt"}); err != nil {
		t.Fatal(err)
	}
	tester := ui.NewTester(h.View, 900, 600)
	tester.Frame()
	if err := tester.Click("⋯"); err != nil {
		t.Fatal(err)
	}
	tester.Frame()
	rect, ok := tester.Find("刷新")
	if !ok {
		t.Fatalf("tab menu missing: %v", tester.Texts())
	}
	if tester.WebViewAt(rect.X+rect.W/2, rect.Y+rect.H/2) != nil {
		t.Fatal("tab menu does not cover the page")
	}
	if err := tester.Click("刷新"); err != nil {
		t.Fatal(err)
	}
	f.mu.Lock()
	reloads := f.reloads
	f.mu.Unlock()
	if reloads != 1 {
		t.Fatalf("refresh clicked %d times", reloads)
	}
	tester.Frame()
	if !tester.HasText("变更 A") {
		t.Fatal("tab bar disappeared after refresh")
	}
}

func TestTabsSnapshot(t *testing.T) {
	h, _ := newTabHost(t)
	if _, err := h.OpenTab(TabRequest{Key: "a", URL: "/a.html"}); err != nil {
		t.Fatal(err)
	}
	snap := h.Snapshot()
	if snap.Tabs != 1 || snap.ActiveTab != "tab-1" {
		t.Fatalf("snapshot %+v", snap)
	}
}

func TestOpenDiffUsesToolPath(t *testing.T) {
	h, _ := newTabHost(t)
	dispatch(t, h, "s1", eventbus.KindToolExecutionStart, `{"toolCallId":"t1","toolName":"write","arguments":{"path":"sub/a.txt"}}`)
	dir := t.TempDir()
	app := New(nil, Options{Cwd: dir, Host: h, Version: "test"})
	app.current = "s1"
	if err := app.OpenDiff("s1", "t1"); err != nil {
		t.Fatal(err)
	}
	tabs := h.Tabs()
	if len(tabs) != 1 || tabs[0].Data != "sub/a.txt" || tabs[0].Title != "变更：a.txt" {
		t.Fatalf("tabs %+v", tabs)
	}
	if err := app.OpenDiff("other", "t1"); err == nil {
		t.Fatal("another session opened a diff")
	}
}
