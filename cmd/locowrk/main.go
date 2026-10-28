// Command locowrk is the desktop shell: it wires the kernel and the bridge into
// a MyGo window and serves the embedded frontend.
package main

import (
	"context"
	"encoding/json"
	"flag"
	"fmt"
	"net/url"
	"os"
	"path/filepath"
	"time"

	"github.com/egoist/mygo"
	"github.com/egoist/mygo/ui"

	"github.com/qingbird1983/locowrk/desktop"
	"github.com/qingbird1983/locowrk/internal/bridge"
	"github.com/qingbird1983/locowrk/internal/eventbus"
	"github.com/qingbird1983/locowrk/internal/kernel"
	"github.com/qingbird1983/locowrk/web"
)

const version = "0.9.0"

func main() {
	if err := run(); err != nil {
		fmt.Fprintln(os.Stderr, "locowrk:", err)
		os.Exit(1)
	}
}

func run() error {
	var (
		cwd      = flag.String("cwd", "", "workspace directory (default: current; temp for -smoke)")
		config   = flag.String("config", "", "settings file (default: per-user; temp for -smoke)")
		model    = flag.String("model", defaultModel(), "model as provider/id")
		thinking = flag.String("thinking", "off", "thinking level: off|low|medium|high")
		faux     = flag.Bool("faux", false, "use the scripted faux provider (no API key)")
		smoke    = flag.Bool("smoke", false, "hidden-window acceptance run driven by the page")
		scenario = flag.String("smoke-scenario", "faux", "smoke scenario: faux|real|tool|cancel|long|restore|plugin|plugin-crash")
		expect   = flag.String("smoke-expect", "", "text the restore scenario must find")
		hidden   = flag.Bool("hidden", false, "run without showing the window")
		show     = flag.Bool("show", false, "show the window during a smoke run")
		gen      = flag.String("generate-bindings", "", "write the TypeScript client to this path and exit")
	)
	flag.Parse()

	if *smoke && *scenario != "real" {
		*faux = true
	}
	// Windows does not build native content for a hidden window, so the crash
	// probe needs a real frame; its run shows the window briefly.
	if *smoke && *scenario == "plugin-crash" {
		*show = true
	}
	if *cwd == "" {
		if *smoke {
			dir, err := os.MkdirTemp("", "locowrk-smoke-*")
			if err != nil {
				return err
			}
			defer func() { _ = os.RemoveAll(dir) }()
			*cwd = dir
		} else {
			dir, err := os.Getwd()
			if err != nil {
				return err
			}
			*cwd = dir
		}
	}
	if *config == "" {
		if *smoke {
			// Keep smoke runs out of the user's real settings.
			*config = filepath.Join(*cwd, "locowrk-settings.json")
		} else {
			*config = desktop.DefaultSettingsPath()
		}
	}
	if *smoke && (*scenario == "tool" || *scenario == "plugin") {
		note := filepath.Join(*cwd, "smoke-note.txt")
		if err := os.WriteFile(note, []byte("locowrk smoke note\n"), 0o644); err != nil {
			return err
		}
	}

	kernelOpts := kernel.Options{}
	ref, err := kernel.ParseModelRef(*model)
	if err != nil {
		return err
	}
	var cleanup func()
	if *faux {
		kernelOpts, ref, cleanup = kernel.WithFauxScenario(kernelOpts, *scenario, "smoke-note.txt")
		defer cleanup()
	}

	k, err := kernel.New(kernelOpts)
	if err != nil {
		return err
	}
	defer func() { _ = k.Close(context.Background()) }()

	var extras []desktop.DesktopPlugin
	if *smoke && *scenario == "plugin-crash" {
		extras = append(extras, smokeCrashPlugin{})
	}
	host := desktop.NewPluginHost(desktop.HostOptions{SettingsPath: *config, Plugins: extras})

	var app *desktop.App
	b, err := bridge.New(k, bridge.Options{OnEvent: func(ev eventbus.Event) {
		if app != nil {
			app.Emit(ev)
		}
		host.Dispatch(ev)
	}})
	if err != nil {
		return err
	}
	app = desktop.New(b, desktop.Options{
		Cwd:      *cwd,
		Model:    ref.String(),
		Thinking: *thinking,
		Version:  version,
		Host:     host,
	})
	mygo.BindAs("Desktop", desktop.NewBindings(app))
	mygo.SetFrontend(web.FS())

	if *gen != "" {
		if err := mygo.WriteTypeScript(*gen); err != nil {
			return fmt.Errorf("writing TypeScript client: %w", err)
		}
		fmt.Fprintln(os.Stderr, "wrote", *gen)
		return nil
	}

	mygo.App.SetName("Locowrk")
	mygo.App.SetVersion(version)
	pageURL := "/"
	if *smoke {
		query := url.Values{}
		query.Set("smoke", *scenario)
		if *expect != "" {
			query.Set("expect", *expect)
		}
		pageURL = "/?" + query.Encode()
	}
	mygo.App.WhenReady(func() {
		win := mygo.NewWindow(mygo.WindowOptions{
			Title:     "Locowrk",
			Width:     1100,
			Height:    760,
			MinWidth:  900,
			MinHeight: 620,
			Hidden:    *hidden || (*smoke && !*show),
			Content:   ui.View(host.View),
		})
		page, err := win.NewWebView(mygo.WebViewOptions{URL: pageURL})
		if err != nil {
			fmt.Fprintln(os.Stderr, "locowrk: web view:", err)
			mygo.App.Exit(1)
			return
		}
		host.Attach(win, page)
		app.SetWindow(win)
		app.SetPage(page.Page())
		win.OnClose(func(*mygo.CloseEvent) {
			app.Shutdown()
		})
		if *smoke {
			go runSmoke(page, host, app, *scenario, *expect)
		}
	})

	if err := mygo.App.Run(); err != nil {
		return err
	}
	app.Shutdown()
	return nil
}

func defaultModel() string {
	if m := os.Getenv("LOCOWRK_MODEL"); m != "" {
		return m
	}
	return "deepseek/deepseek-flash"
}

// smokeResult is the JSON line the smoke run prints.
type smokeResult struct {
	OK      bool              `json:"ok"`
	Text    string            `json:"text,omitempty"`
	Session string            `json:"session,omitempty"`
	Error   string            `json:"error,omitempty"`
	MS      int               `json:"ms,omitempty"`
	Rows    int               `json:"rows,omitempty"`
	Mounted int               `json:"mounted,omitempty"`
	Tools   int               `json:"tools,omitempty"`
	Plugins *desktop.Snapshot `json:"plugins,omitempty"`
}

// runSmoke drives the page through the requested acceptance scenario and exits
// the process with 0 or 1.
func runSmoke(page *mygo.WebView, host *desktop.Host, app *desktop.App, scenario, expect string) {
	deadline := time.Now().Add(120 * time.Second)
	for time.Now().Before(deadline) {
		kind, err := mygo.EvalAs[string](page.Page(), "return typeof window.__locowrkSmoke")
		if err == nil && kind == "function" {
			out, err := mygo.EvalAs[string](page.Page(), "return await window.__locowrkSmoke()")
			if err != nil {
				smokeFail("smoke eval: " + err.Error())
				return
			}
			var result smokeResult
			if err := json.Unmarshal([]byte(out), &result); err != nil {
				smokeFail("smoke output: " + err.Error() + " " + out)
				return
			}
			if !result.OK {
				if result.Error == "" {
					result.Error = "page reported failure"
				}
				smokeFail(result.Error)
				return
			}
			if err := checkPlugins(host, app, scenario, &result); err != nil {
				smokeFail(err.Error())
				return
			}
			encoded, _ := json.Marshal(result)
			fmt.Println(string(encoded))
			mygo.App.Exit(0)
			return
		}
		time.Sleep(200 * time.Millisecond)
	}
	smokeFail("smoke: frontend never became ready")
}

// checkPlugins injects orchestration events and asserts the host state.
func checkPlugins(host *desktop.Host, app *desktop.App, scenario string, result *smokeResult) error {
	session := app.Current()
	switch scenario {
	case "plugin":
		deadline := time.Now().Add(2 * time.Second)
		for host.SelectedToolCall() == "" && time.Now().Before(deadline) {
			time.Sleep(20 * time.Millisecond)
		}
		if host.SelectedToolCall() == "" {
			return fmt.Errorf("page tool click did not reach the native inspector")
		}
		seedPluginEvents(host, session)
		if err := app.SelectToolCall(session, "smoke-t1"); err != nil {
			return fmt.Errorf("select tool call: %w", err)
		}
		snapshot := host.Snapshot()
		result.Plugins = &snapshot
		if snapshot.Nodes < 2 || snapshot.Steps < 2 || snapshot.Tools < 1 {
			return fmt.Errorf("plugins did not receive events: %+v", snapshot)
		}
		if snapshot.Selected != "smoke-t1" {
			return fmt.Errorf("tool call selection not recorded: %+v", snapshot)
		}
	case "plugin-crash":
		// The window shows for this scenario; wait for the frame that builds
		// the failing plugin.
		deadline := time.Now().Add(5 * time.Second)
		for time.Now().Before(deadline) {
			if len(host.Snapshot().Failed) > 0 {
				break
			}
			time.Sleep(50 * time.Millisecond)
		}
		snapshot := host.Snapshot()
		result.Plugins = &snapshot
		if snapshot.Frames == 0 {
			return fmt.Errorf("the native shell never built a frame; run with -show: %+v", snapshot)
		}
		found := false
		for _, name := range snapshot.Failed {
			if name == "smoke-crash" {
				found = true
			}
		}
		if !found {
			return fmt.Errorf("crashing plugin was not isolated: %+v", snapshot)
		}
	}
	return nil
}

// seedPluginEvents sends the same shapes the A-track adapter publishes.
func seedPluginEvents(host *desktop.Host, session string) {
	at := time.Now().UTC()
	send := func(kind eventbus.Kind, payload string) {
		host.Dispatch(eventbus.Event{Kind: kind, SessionID: session, At: at, Payload: json.RawMessage(payload)})
	}
	send(eventbus.KindSubagentSpawned, `{"id":"smoke-a1","name":"探索会话格式"}`)
	send(eventbus.KindSubagentSpawned, `{"id":"smoke-a2","name":"事件流映射","parentId":"smoke-a1"}`)
	send(eventbus.KindSubagentSettled, `{"id":"smoke-a2"}`)
	send(eventbus.KindWorkflowStep, `{"name":"收集","status":"done"}`)
	send(eventbus.KindWorkflowStep, `{"name":"实现","status":"running"}`)
	send(eventbus.KindToolExecutionStart, `{"toolCallId":"smoke-t1","toolName":"read","arguments":{"path":"smoke-note.txt"}}`)
	send(eventbus.KindToolExecutionEnd, `{"toolCallId":"smoke-t1","toolName":"read","text":"locowrk smoke note","durationMs":12}`)
}

func smokeFail(message string) {
	encoded, _ := json.Marshal(smokeResult{OK: false, Error: message})
	fmt.Println(string(encoded))
	fmt.Fprintln(os.Stderr, "smoke:", message)
	mygo.App.Exit(1)
}
