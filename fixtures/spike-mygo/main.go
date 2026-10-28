// Command spike-mygo checks the Phase 3 prerequisites that need a real window:
// a hidden WebView2 window, page-to-Go calls, Go-to-page events, and exit
// behavior.
//
// It prints one JSON line and exits 0 when both directions work, 1 otherwise.
// The window is hidden so the spike does not interrupt the desktop.
package main

import (
	"encoding/json"
	"fmt"
	"os"
	"sync"
	"time"

	"github.com/egoist/mygo"
)

// Tick is emitted after the page proves it can call Go.
var Tick = mygo.NewEvent[string]("phase3-tick")

type report struct {
	Stage  string `json:"stage"`
	Detail string `json:"detail,omitempty"`
}

// Probe receives the page's reports.
type Probe struct {
	mu       sync.Mutex
	started  time.Time
	stages   map[string]time.Duration
	win      *mygo.Window
	finished bool
}

// Report records one stage reached by the page and answers it.
func (p *Probe) Report(r report) string {
	p.mu.Lock()
	if p.stages == nil {
		p.stages = map[string]time.Duration{}
	}
	if _, ok := p.stages[r.Stage]; !ok {
		p.stages[r.Stage] = time.Since(p.started)
	}
	win := p.win
	_, evented := p.stages["event"]
	_, paged := p.stages["page"]
	p.mu.Unlock()

	if r.Stage == "page" && win != nil {
		// Go -> page through the typed event API.
		_ = Tick.Emit(win, "from-go")
	}
	if paged && evented {
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
		stages[k] = float64(v.Microseconds()) / 1000.0
	}
	p.mu.Unlock()

	out := map[string]any{
		"ok":        ok,
		"stagesMs":  stages,
		"userAgent": userAgent,
	}
	data, _ := json.Marshal(out)
	fmt.Println(string(data))
	if ok {
		mygo.App.Exit(0)
		return
	}
	mygo.App.Exit(1)
}

var (
	probe     = &Probe{started: time.Now()}
	userAgent string
)

const page = `<!doctype html>
<html>
<head><meta charset="utf-8"><title>Locowrk Phase 3 spike</title></head>
<body>
<script>
  mygo.call("Probe.Report", {stage: "page"}).then(function (ack) {
    document.title = ack;
  });
  mygo.on("phase3-tick", function (payload) {
    mygo.call("Probe.Report", {stage: "event", detail: payload});
  });
</script>
</body>
</html>`

func main() {
	mygo.App.SetName("Locowrk Phase 3 Spike")
	mygo.App.SetVersion("0.0.0")
	mygo.Bind(probe)

	mygo.App.WhenReady(func() {
		win := mygo.NewWindow(mygo.WindowOptions{
			Title:  "Locowrk Phase 3 Spike",
			Width:  520,
			Height: 320,
			Hidden: true,
		})
		probe.mu.Lock()
		probe.win = win
		probe.mu.Unlock()

		if ua, err := mygo.EvalAs[string](win.Page(), "navigator.userAgent"); err == nil {
			userAgent = ua
		}
		win.Page().LoadHTML(page, "")

		// Without both directions the spike reports failure instead of hanging.
		go func() {
			time.Sleep(10 * time.Second)
			probe.finish(false)
		}()
	})

	if err := mygo.App.Run(); err != nil {
		fmt.Fprintln(os.Stderr, "spike-mygo:", err)
		os.Exit(1)
	}
}
