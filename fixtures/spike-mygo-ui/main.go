// Command spike-mygo-ui checks the Phase 4 prerequisites that need the real
// window and layout engine: a native ui.View around an embedded web view, the
// page's bindings and events inside that web view, and headless multi-frame
// tests of plugin-shaped views.
//
// It prints one JSON line. Without -headless it opens a hidden window; with it
// it builds frames in memory only.
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

// Tick is emitted once the page proves it can call Go.
var Tick = mygo.NewEvent[string]("phase4-tick")

// Report is what the page tells Go.
type Report struct {
	Stage  string `json:"stage"`
	Detail string `json:"detail,omitempty"`
}

// Node is one row of the fake orchestration tree.
type Node struct {
	ID    string
	Title string
	State string
}

// uiState is the plugin-shaped data a view draws; the window builds it anew
// every frame, so it holds no ui.Element.
type uiState struct {
	rows     []Node
	selected string
	menu     bool
	web      *mygo.WebView
	events   int
	steps    []string
	step     int
	window   *mygo.Window
}

func (s *uiState) view(c *ui.Context) {
	if s.web == nil {
		return
	}
	t := c.Theme()
	ui.Row(c).Fill().AlignItems(ui.Stretch).Children(func() {
		// session.sidebar slot, drawn natively left of the page.
		ui.Column(c).Width(200).Padding(8).Gap(4).Children(func() {
			ui.Text(c, "编排树").Bold()
			if len(s.rows) == 0 {
				ui.Text(c, "暂无").TextColor(t.TextMuted)
			}
			for _, r := range s.rows {
				row := ui.Row(c.Key(r.ID)).Padding(4, 6).Gap(6).Radius(6).AlignItems(ui.Center)
				color := t.TextMuted
				if r.ID == s.selected {
					row.Background(t.Accent)
					color = t.AccentText
				} else if row.Hovered() {
					row.Background(t.Border)
				}
				row.Children(func() {
					ui.Text(c, r.State).TextColor(color).FontSize(11)
					ui.Text(c, r.Title).TextColor(color)
				})
				if row.Clicked() {
					s.selected = r.ID
					s.window.Invalidate()
				}
			}
		})
		ui.Divider(c)
		ui.Column(c).Grow(1).MinWidth(0).Children(func() {
			// session.timeline.above slot, a strip over the page.
			ui.Row(c).Padding(6, 10).Gap(8).AlignItems(ui.Center).Children(func() {
				for i, step := range s.steps {
					color := t.TextMuted
					if i == s.step {
						color = t.Accent
					}
					ui.Text(c, step).TextColor(color).FontSize(12)
					if i < len(s.steps)-1 {
						ui.Text(c, "›").TextColor(t.TextMuted).FontSize(12)
					}
				}
				ui.Row(c).Grow(1)
				// A popover anchored natively shows over the page.
				more := ui.Button(c, "插件 ▾")
				if more.Clicked() {
					s.menu = !s.menu
				}
				ui.Popover(c, more, &s.menu, func() {
					panel := ui.Column(c).Padding(6).Gap(2).Width(180)
					panel.Children(func() {
						ui.Text(c, "orchestration-tree").FontSize(12)
						ui.Text(c, "workflow-timeline").FontSize(12)
						ui.Text(c, "tool-inspector").FontSize(12)
					})
				})
			})
			ui.Divider(c)
			// The page keeps the transcript and the composer.
			ui.WebView(c, s.web).Grow(1)
			// Painted after the page, so it shows over it.
			ui.Row(c).Absolute().Bottom(16).Right(16).Padding(4, 10).Radius(12).
				Background(t.Accent).Children(func() {
				ui.Textf(c, "events %d", s.events).TextColor(t.AccentText).FontSize(11)
			})
		})
		ui.Divider(c)
		// session.tool.detail slot, drawn natively right of the page.
		ui.Column(c).Width(240).Padding(8).Gap(4).Children(func() {
			ui.Text(c, "工具检查器").Bold()
			if s.selected == "" {
				ui.Text(c, "暂无").TextColor(t.TextMuted)
			} else {
				ui.Text(c, "节点 "+s.selected).FontSize(12)
				ui.Text(c, "参数与结果在此").TextColor(t.TextMuted).FontSize(12)
			}
		})
	})
}

// Probe records the stages of the window run.
type Probe struct {
	mu       sync.Mutex
	started  time.Time
	stages   map[string]float64
	web      *mygo.WebView
	finished bool
	state    *uiState
}

func (p *Probe) note(stage string) {
	p.mu.Lock()
	if _, ok := p.stages[stage]; !ok {
		p.stages[stage] = float64(time.Since(p.started).Microseconds()) / 1000
	}
	p.mu.Unlock()
}

// Report records one stage reached by the page and answers it.
func (p *Probe) Report(r Report) string {
	p.note(r.Stage)
	if r.Stage == "page" {
		p.mu.Lock()
		web := p.web
		p.mu.Unlock()
		if web != nil {
			_ = Tick.EmitPage(web.Page(), "from-go")
		}
	}
	if r.Stage == "event" {
		p.finish(true)
	}
	return "ack:" + r.Stage
}

func (p *Probe) finish(ok bool) {
	p.mu.Lock()
	if p.finished {
		p.mu.Unlock()
		return
	}
	p.finished = true
	stages := map[string]float64{}
	for k, v := range p.stages {
		stages[k] = v
	}
	p.mu.Unlock()

	out := map[string]any{"ok": ok, "stagesMs": stages}
	data, _ := json.Marshal(out)
	fmt.Println(string(data))
	if ok {
		mygo.App.Exit(0)
		return
	}
	mygo.App.Exit(1)
}

var probe = &Probe{started: time.Now(), stages: map[string]float64{}}

func main() {
	headless := flag.Bool("headless", false, "build frames in memory and report timings")
	flag.Parse()
	if *headless {
		runHeadless()
		return
	}

	sub, err := fs.Sub(pages, "page")
	if err != nil {
		fmt.Fprintln(os.Stderr, "spike-mygo-ui:", err)
		os.Exit(1)
	}
	state := &uiState{
		steps:  []string{"收集", "实现", "验收"},
		step:   1,
		events: 500,
		rows: []Node{
			{ID: "a1", Title: "探索会话格式", State: "完成"},
			{ID: "a2", Title: "事件流映射", State: "运行"},
			{ID: "a3", Title: "工具卡片", State: "等待"},
		},
		window: nil,
	}

	mygo.App.SetName("Locowrk Phase 4 Spike")
	mygo.App.SetVersion("0.0.0")
	mygo.Bind(probe)
	mygo.SetFrontend(sub)

	mygo.App.WhenReady(func() {
		probe.note("ready")
		win := mygo.NewWindow(mygo.WindowOptions{
			Title:   "Locowrk Phase 4 Spike",
			Width:   900,
			Height:  600,
			Hidden:  true,
			Content: ui.View(state.view),
		})
		state.window = win
		probe.note("window")

		web, err := win.NewWebView(mygo.WebViewOptions{URL: "/"})
		if err != nil {
			fmt.Fprintln(os.Stderr, "spike-mygo-ui: new web view:", err)
			mygo.App.Exit(1)
			return
		}
		probe.mu.Lock()
		probe.web = web
		probe.mu.Unlock()
		state.web = web
		win.Invalidate()
		probe.note("webview")

		go func() {
			time.Sleep(20 * time.Second)
			probe.finish(false)
		}()
	})

	if err := mygo.App.Run(); err != nil {
		fmt.Fprintln(os.Stderr, "spike-mygo-ui:", err)
		os.Exit(1)
	}
}

// benchState is a view with the Phase 4 load: 100 tree rows, 500 tool events
// folded into a count, and a detail rail.
type benchState struct {
	rows     []Node
	selected string
	events   int
}

func (s *benchState) view(c *ui.Context) {
	t := c.Theme()
	ui.Row(c).Fill().AlignItems(ui.Stretch).Children(func() {
		ui.Column(c).Width(200).Padding(8).Gap(4).Children(func() {
			ui.Text(c, "编排树").Bold()
			ui.Scroll(c).Grow(1).Children(func() {
				for _, r := range s.rows {
					row := ui.Row(c.Key(r.ID)).Padding(4, 6).Gap(6).Radius(6)
					if r.ID == s.selected {
						row.Background(t.Accent)
					}
					row.Children(func() {
						ui.Text(c, r.State).FontSize(11).TextColor(t.TextMuted)
						ui.Text(c, r.Title)
					})
				}
			})
		})
		ui.Column(c).Grow(1).MinWidth(0).Children(func() {
			ui.Textf(c, "%d 个工具事件", s.events).TextColor(t.TextMuted)
			ui.Row(c).Grow(1)
		})
		ui.Column(c).Width(240).Padding(8).Children(func() {
			ui.Text(c, "工具检查器").Bold()
			ui.Textf(c, "%d 条记录", s.events).FontSize(12)
		})
	})
}

// runHeadless times frames of a plugin-shaped view without a window.
func runHeadless() {
	rows := make([]Node, 100)
	for i := range rows {
		rows[i] = Node{ID: fmt.Sprintf("n%d", i), Title: fmt.Sprintf("子代理 %d", i), State: "运行"}
	}
	var frames []float64
	state := &benchState{rows: rows, events: 500}
	tester := ui.NewTester(state.view, 900, 600)
	start := time.Now()
	for i := 0; i < 120; i++ {
		if i == 60 {
			state.selected = "n99"
		}
		t := time.Now()
		tester.Frame()
		frames = append(frames, float64(time.Since(t).Microseconds())/1000)
	}
	total := time.Since(start)
	var max float64
	for _, f := range frames {
		if f > max {
			max = f
		}
	}
	out := map[string]any{
		"ok":     tester.HasText("子代理 99") && tester.HasText("500 个工具事件"),
		"frames": len(frames),
		"avgMs":  float64(total.Microseconds()) / 1000 / float64(len(frames)),
		"maxMs":  max,
		"texts":  len(tester.Texts()),
	}
	data, _ := json.Marshal(out)
	fmt.Println(string(data))
}
