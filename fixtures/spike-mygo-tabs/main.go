// Command spike-mygo-tabs checks the Phase 5 prerequisites: two web views in
// one native window, switching which one the view places while keeping the
// hidden page's state, a native popover over a page, and destroying a web view.
//
// It prints one JSON line. Without -show it runs the pages in a hidden window
// (no native frames); with -show it opens a small window, switches tabs,
// captures the native content and destroys the second web view.
package main

import (
	"embed"
	"encoding/json"
	"flag"
	"fmt"
	"io/fs"
	"os"
	"sync"
	"time"

	"github.com/egoist/mygo"
	"github.com/egoist/mygo/ui"
)

//go:embed page
var pages embed.FS

// Report is what a page tells Go when it is ready.
type Report struct {
	Page   string `json:"page"`
	Detail string `json:"detail,omitempty"`
}

// State is the shell the fixture draws: tab strip, popover and the active page.
type State struct {
	mu      sync.Mutex
	win     *mygo.Window
	a, b    *mygo.WebView
	active  string
	menu    bool
	frames  int
	ready   map[string]bool
	reloads int
}

func newState() *State {
	return &State{active: "a", ready: map[string]bool{}}
}

func (s *State) setActive(name string) {
	s.mu.Lock()
	if s.active == name {
		s.mu.Unlock()
		return
	}
	s.active = name
	w := s.win
	s.mu.Unlock()
	if w != nil {
		w.Invalidate()
	}
}

func (s *State) setMenu(open bool) {
	s.mu.Lock()
	s.menu = open
	w := s.win
	s.mu.Unlock()
	if w != nil {
		w.Invalidate()
	}
}

// view draws the tab strip and places only the active page.
func (s *State) view(c *ui.Context) {
	s.mu.Lock()
	s.frames++
	active, menu, a, b := s.active, s.menu, s.a, s.b
	s.mu.Unlock()
	if a == nil || b == nil {
		return
	}
	ui.Column(c).Fill().Children(func() {
		ui.Row(c).Height(38).Padding(6, 10).Gap(6).AlignItems(ui.Center).Children(func() {
			for _, tab := range []struct {
				name  string
				label string
			}{{"a", "A"}, {"b", "B"}} {
				tab := tab
				label := tab.label
				if active == tab.name {
					label = "● " + label
				}
				if ui.Button(c, label).Clicked() {
					s.setActive(tab.name)
				}
			}
			ui.Box(c).Grow(1)
			more := ui.Button(c, "菜单")
			if more.Clicked() {
				menu = !menu
				s.setMenu(menu)
			}
			ui.Popover(c, more, &menu, func() {
				ui.Column(c).Width(200).Padding(6).Gap(4).Children(func() {
					if ui.Button(c, "刷新数据").Clicked() {
						s.mu.Lock()
						s.reloads++
						s.mu.Unlock()
						s.setMenu(false)
					}
					if ui.Button(c, "关闭标签页").Clicked() {
						s.setMenu(false)
					}
				})
			})
		})
		ui.Divider(c)
		if active == "b" {
			ui.WebView(c, b).Grow(1)
		} else {
			ui.WebView(c, a).Grow(1)
		}
	})
}

// Probe is bound to the pages as "Probe".
type Probe struct {
	state   *State
	started time.Time
	stages  map[string]float64
	mu      sync.Mutex
	once    sync.Once
}

func (p *Probe) note(stage string) {
	p.mu.Lock()
	if _, ok := p.stages[stage]; !ok {
		p.stages[stage] = float64(time.Since(p.started).Microseconds()) / 1000
	}
	p.mu.Unlock()
}

// Ready records that a page loaded and could call Go.
func (p *Probe) Ready(r Report) string {
	p.note(r.Page)
	p.state.mu.Lock()
	p.state.ready[r.Page] = true
	p.state.mu.Unlock()
	return "ack:" + r.Page
}

func (p *Probe) result(ok, layout bool, check, bCheck string, capture int, captureErr string, destroyed bool, err string) {
	p.once.Do(func() {
		p.mu.Lock()
		stages := map[string]float64{}
		for k, v := range p.stages {
			stages[k] = v
		}
		p.mu.Unlock()
		p.state.mu.Lock()
		out := map[string]any{
			"ok":           ok,
			"layout":       layout,
			"frames":       p.state.frames,
			"check":        check,
			"bCheck":       bCheck,
			"captureBytes": capture,
			"captureErr":   captureErr,
			"destroyed":    destroyed,
			"stagesMs":     stages,
			"err":          err,
		}
		p.state.mu.Unlock()
		data, _ := json.Marshal(out)
		fmt.Println(string(data))
		if ok {
			mygo.App.Exit(0)
			return
		}
		mygo.App.Exit(1)
	})
}

func (p *Probe) waitReady(timeout time.Duration) bool {
	deadline := time.Now().Add(timeout)
	for time.Now().Before(deadline) {
		p.state.mu.Lock()
		ok := p.state.ready["a"] && p.state.ready["b"]
		p.state.mu.Unlock()
		if ok {
			return true
		}
		time.Sleep(50 * time.Millisecond)
	}
	return false
}

func (p *Probe) eval(web *mygo.WebView, js string) (string, error) {
	if web == nil || web.IsDestroyed() {
		return "", fmt.Errorf("web view is not available")
	}
	return mygo.EvalAs[string](web.Page(), js)
}

// runScenario drives the visible-window checks.
func (p *Probe) runScenario(show bool, out string, hold time.Duration) {
	fail := func(err error) {
		p.result(false, show, "", "", 0, "", false, err.Error())
	}
	if !p.waitReady(10 * time.Second) {
		fail(fmt.Errorf("pages did not become ready"))
		return
	}
	p.note("ready")
	time.Sleep(300 * time.Millisecond)

	p.state.mu.Lock()
	a, b := p.state.a, p.state.b
	p.state.mu.Unlock()
	check, err := p.eval(a, "return window.__check()")
	if err != nil {
		fail(err)
		return
	}
	if check != "320|hello" {
		fail(fmt.Errorf("page A state before switching = %q", check))
		return
	}

	bCheck := ""
	capture := 0
	captureErr := ""
	if show {
		p.state.setActive("b")
		time.Sleep(300 * time.Millisecond)
		p.note("tab-b")
		if got, err := p.eval(b, "return window.__check()"); err != nil || got != "b" {
			fail(fmt.Errorf("page B check = %q, %v", got, err))
			return
		} else {
			bCheck = got
		}
		p.state.setActive("a")
		time.Sleep(300 * time.Millisecond)
		p.note("tab-a")

		check, err = p.eval(a, "return window.__check()")
		if err != nil {
			fail(err)
			return
		}
		if check != "320|hello" {
			fail(fmt.Errorf("page A state after switching = %q", check))
			return
		}

		p.state.setMenu(true)
		time.Sleep(300 * time.Millisecond)
		p.note("popover")
		if data, err := p.state.win.CapturePage(); err != nil {
			captureErr = err.Error()
		} else {
			capture = len(data)
			if out != "" {
				if err := os.WriteFile(out, data, 0o644); err != nil {
					captureErr = err.Error()
				}
			}
		}
		if hold <= 0 {
			hold = 1500 * time.Millisecond
		}
		time.Sleep(hold)
		p.state.setMenu(false)
		time.Sleep(150 * time.Millisecond)
	}

	p.state.setActive("a")
	time.Sleep(200 * time.Millisecond)
	b.Destroy()
	time.Sleep(300 * time.Millisecond)
	p.note("destroy")
	destroyed := b.IsDestroyed()
	ok := check == "320|hello" && destroyed && (capture > 0 || !show)
	p.result(ok, show, check, bCheck, capture, captureErr, destroyed, "")
}

// runHeadless checks the tab strip and popover geometry without a window. It
// uses zero-value *mygo.WebView handles: the test engine only compares their
// identity and never calls their methods.
func runHeadless() {
	state := newState()
	state.a = new(mygo.WebView)
	state.b = new(mygo.WebView)
	tester := ui.NewTester(state.view, 900, 600)
	tester.Frame()
	aAt := tester.WebViewAt(450, 300)
	if aAt != state.a {
		reportHeadless(false, "initial page is not A")
		return
	}
	if err := tester.Click("B"); err != nil {
		reportHeadless(false, "click B: "+err.Error())
		return
	}
	tester.Frame()
	got := tester.WebViewAt(450, 300)
	if got != state.b {
		reportHeadless(false, fmt.Sprintf("switching to B did not place B (gotA=%v gotB=%v active=%q texts=%v)", got == state.a, got == state.b, state.active, tester.Texts()))
		return
	}
	if tester.WebViewAt(450, 300) == state.a {
		reportHeadless(false, "both pages placed")
		return
	}
	if err := tester.Click("菜单"); err != nil {
		reportHeadless(false, "open menu: "+err.Error())
		return
	}
	tester.Frame()
	rect, ok := tester.Find("刷新数据")
	if !ok {
		reportHeadless(false, "popover item missing")
		return
	}
	if tester.WebViewAt(rect.X+rect.W/2, rect.Y+rect.H/2) != nil {
		reportHeadless(false, "popover item does not cover the page")
		return
	}
	if err := tester.Click("刷新数据"); err != nil {
		reportHeadless(false, "click refresh: "+err.Error())
		return
	}
	tester.Frame()
	state.mu.Lock()
	reloads := state.reloads
	state.mu.Unlock()
	if reloads != 1 {
		reportHeadless(false, "popover click did not run")
		return
	}
	reportHeadless(true, "")
}

func reportHeadless(ok bool, err string) {
	out := map[string]any{"ok": ok, "headless": true, "err": err}
	data, _ := json.Marshal(out)
	fmt.Println(string(data))
	if ok {
		mygo.App.Exit(0)
		return
	}
	mygo.App.Exit(1)
}

func main() {
	show := flag.Bool("show", false, "show the window and test tab switching and capture")
	headless := flag.Bool("headless", false, "test the tab strip and popover without a window")
	out := flag.String("out", "", "write the native capture PNG to this path")
	hold := flag.Duration("hold", 0, "keep the popover open this long after the native capture")
	flag.Parse()

	if *headless {
		runHeadless()
		return
	}

	sub, err := fs.Sub(pages, "page")
	if err != nil {
		fmt.Fprintln(os.Stderr, "spike-mygo-tabs:", err)
		os.Exit(1)
	}
	state := newState()
	probe := &Probe{state: state, started: time.Now(), stages: map[string]float64{}}

	mygo.App.SetName("Locowrk Phase 5 Spike")
	mygo.App.SetVersion("0.0.0")
	mygo.Bind(probe)
	mygo.SetFrontend(sub)

	mygo.App.WhenReady(func() {
		win := mygo.NewWindow(mygo.WindowOptions{
			Title:       "Locowrk Phase 5 Spike",
			Width:       900,
			Height:      600,
			X:           80,
			Y:           80,
			Hidden:      !*show,
			SkipTaskbar: true,
			AlwaysOnTop: *show,
			Content:     ui.View(state.view),
		})
		state.mu.Lock()
		state.win = win
		state.mu.Unlock()

		a, err := win.NewWebView(mygo.WebViewOptions{URL: "/a.html"})
		if err != nil {
			fmt.Fprintln(os.Stderr, "spike-mygo-tabs: web view A:", err)
			mygo.App.Exit(1)
			return
		}
		b, err := win.NewWebView(mygo.WebViewOptions{URL: "/b.html"})
		if err != nil {
			fmt.Fprintln(os.Stderr, "spike-mygo-tabs: web view B:", err)
			mygo.App.Exit(1)
			return
		}
		state.mu.Lock()
		state.a, state.b = a, b
		state.mu.Unlock()
		win.Invalidate()

		go probe.runScenario(*show, *out, *hold)
		go func() {
			time.Sleep(20 * time.Second)
			probe.result(false, *show, "", "", 0, "", false, "timeout")
		}()
	})

	if err := mygo.App.Run(); err != nil {
		fmt.Fprintln(os.Stderr, "spike-mygo-tabs:", err)
		os.Exit(1)
	}
}
