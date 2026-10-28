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

	"github.com/qingbird1983/locowrk/desktop"
	"github.com/qingbird1983/locowrk/internal/bridge"
	"github.com/qingbird1983/locowrk/internal/eventbus"
	"github.com/qingbird1983/locowrk/internal/kernel"
	"github.com/qingbird1983/locowrk/web"
)

const version = "0.8.0"

func main() {
	if err := run(); err != nil {
		fmt.Fprintln(os.Stderr, "locowrk:", err)
		os.Exit(1)
	}
}

func run() error {
	var (
		cwd      = flag.String("cwd", "", "workspace directory (default: current; temp for -smoke)")
		model    = flag.String("model", defaultModel(), "model as provider/id")
		thinking = flag.String("thinking", "off", "thinking level: off|low|medium|high")
		faux     = flag.Bool("faux", false, "use the scripted faux provider (no API key)")
		smoke    = flag.Bool("smoke", false, "hidden-window acceptance run driven by the page")
		scenario = flag.String("smoke-scenario", "faux", "smoke scenario: faux|real|tool|cancel|long|restore")
		expect   = flag.String("smoke-expect", "", "text the restore scenario must find")
		hidden   = flag.Bool("hidden", false, "run without showing the window")
		gen      = flag.String("generate-bindings", "", "write the TypeScript client to this path and exit")
	)
	flag.Parse()

	if *smoke && *scenario != "real" {
		*faux = true
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
	if *smoke && *scenario == "tool" {
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

	var app *desktop.App
	b, err := bridge.New(k, bridge.Options{OnEvent: func(ev eventbus.Event) {
		if app != nil {
			app.Emit(ev)
		}
	}})
	if err != nil {
		return err
	}
	app = desktop.New(b, desktop.Options{
		Cwd:      *cwd,
		Model:    ref.String(),
		Thinking: *thinking,
		Version:  version,
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
			Title:  "Locowrk",
			URL:    pageURL,
			Width:  1100,
			Height: 760,
			Hidden: *smoke || *hidden,
		})
		app.SetWindow(win)
		win.OnClose(func(*mygo.CloseEvent) {
			app.Shutdown()
		})
		if *smoke {
			go runSmoke(win)
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

// runSmoke drives the page through the requested acceptance scenario and exits
// the process with 0 or 1.
func runSmoke(win *mygo.Window) {
	deadline := time.Now().Add(120 * time.Second)
	for time.Now().Before(deadline) {
		kind, err := mygo.EvalAs[string](win.Page(), "return typeof window.__locowrkSmoke")
		if err == nil && kind == "function" {
			out, err := mygo.EvalAs[string](win.Page(), "return await window.__locowrkSmoke()")
			if err != nil {
				fmt.Fprintln(os.Stderr, "smoke eval:", err)
				mygo.App.Exit(1)
				return
			}
			var result struct {
				OK      bool   `json:"ok"`
				Text    string `json:"text"`
				Session string `json:"session"`
				Error   string `json:"error"`
				MS      int    `json:"ms"`
				Rows    int    `json:"rows"`
				Mounted int    `json:"mounted"`
				Tools   int    `json:"tools"`
			}
			if err := json.Unmarshal([]byte(out), &result); err != nil {
				fmt.Fprintln(os.Stderr, "smoke output:", err, out)
				mygo.App.Exit(1)
				return
			}
			encoded, _ := json.Marshal(result)
			fmt.Println(string(encoded))
			if !result.OK {
				if result.Error != "" {
					fmt.Fprintln(os.Stderr, "smoke:", result.Error)
				}
				mygo.App.Exit(1)
				return
			}
			mygo.App.Exit(0)
			return
		}
		time.Sleep(200 * time.Millisecond)
	}
	fmt.Fprintln(os.Stderr, "smoke: frontend never became ready")
	mygo.App.Exit(1)
}
