// Command locowrk is the desktop shell: it wires the kernel and the bridge into
// a MyGo window and serves the embedded frontend.
package main

import (
	"context"
	"flag"
	"fmt"
	"net/url"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"

	"github.com/egoist/mygo"
	"github.com/egoist/mygo/ui"

	"github.com/qingbird1983/locowrk/desktop"
	"github.com/qingbird1983/locowrk/internal/bridge"
	"github.com/qingbird1983/locowrk/internal/eventbus"
	"github.com/qingbird1983/locowrk/internal/extensions"
	"github.com/qingbird1983/locowrk/internal/kernel"
	"github.com/qingbird1983/locowrk/web"
)

const version = "0.11.0"

func main() {
	if err := run(); err != nil {
		fmt.Fprintln(os.Stderr, "locowrk:", err)
		os.Exit(1)
	}
}

func run() error {
	var (
		cwd       = flag.String("cwd", "", "workspace directory (default: current; temp for -smoke)")
		config    = flag.String("config", "", "settings file (default: per-user; temp for -smoke)")
		model     = flag.String("model", defaultModel(), "model as provider/id")
		thinking  = flag.String("thinking", "off", "thinking level: off|low|medium|high")
		faux      = flag.Bool("faux", false, "use the scripted faux provider (no API key)")
		smoke     = flag.Bool("smoke", false, "hidden-window acceptance run driven by the page")
		scenario  = flag.String("smoke-scenario", "faux", "smoke scenario: faux|real|steer|steer-real|tool|cancel|long|restore|plugin|plugin-crash|diff|diff-tool")
		expect    = flag.String("smoke-expect", "", "text the restore scenario must find")
		hidden    = flag.Bool("hidden", false, "run without showing the window")
		show      = flag.Bool("show", false, "show the window during a smoke run")
		smokeHold = flag.Duration("smoke-hold", 0, "keep a passing smoke window open this long")
		smokeShot = flag.String("smoke-shot", "", "write a PNG of the smoke window's active tab page")
		diffLines = flag.Int("smoke-diff-lines", 0, "diff scenario: baseline lines to rewrite (0 for the small fixture)")
		extDir    = flag.String("extensions", "", "extra A-track extension directory (development/smoke)")
		nodePath  = flag.String("node", "", "Node runtime for A-track extensions (default: env, bundled runtime, PATH)")
		gen       = flag.String("generate-bindings", "", "write the TypeScript client to this path and exit")
	)
	flag.Parse()

	if *smoke && *scenario != "real" && *scenario != "steer-real" {
		*faux = true
	}
	// Windows does not build native content for a hidden window, so the crash
	// probe needs a real frame; its run shows the window briefly.
	if *smoke && *scenario == "plugin-crash" {
		*show = true
	}
	autoCwd := false
	if *cwd == "" {
		if *smoke {
			dir, err := os.MkdirTemp("", "locowrk-smoke-*")
			if err != nil {
				return err
			}
			defer func() { _ = os.RemoveAll(dir) }()
			*cwd = dir
			autoCwd = true
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
	if *smoke && (*scenario == "diff" || *scenario == "diff-tool") && autoCwd {
		if err := prepareDiffSmoke(*cwd, *diffLines, *scenario == "diff"); err != nil {
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
	diffService := desktop.NewDiffService(host, *cwd)
	var extraDirs []string
	if *extDir != "" {
		extraDirs = append(extraDirs, *extDir)
	}
	node := ""
	nodeVersion := ""
	if *gen == "" {
		node, nodeVersion = resolveNodePath(*nodePath)
	}
	if node == "" && !mygo.IsDev() {
		fmt.Fprintln(os.Stderr, "locowrk: bundled Node runtime missing; A-track extensions are disabled")
	}
	extMgr := extensions.New(extensions.Options{
		Bus:             k.Events(),
		Cwd:             *cwd,
		NodePath:        node,
		NodeVersion:     nodeVersion,
		RuntimeRequired: !mygo.IsDev(),
		Store:           host,
		UI:              desktop.NewDialogUI(host),
		ExtraDirs:       extraDirs,
		HostName:        "Locowrk",
		HostVersion:     version,
	})
	if *gen == "" {
		if err := extMgr.Prepare(context.Background()); err != nil {
			fmt.Fprintln(os.Stderr, "locowrk: extensions:", err)
		}
	}
	defer extMgr.Stop()

	var app *desktop.App
	b, err := bridge.New(k, bridge.Options{
		Cwd:             *cwd,
		Tools:           extMgr.Tools,
		BeforeTool:      extMgr.GateToolCall,
		ProjectTrusted:  extMgr.ProjectTrusted,
		SetProjectTrust: extMgr.SetProjectTrust,
		Extensions:      func() []bridge.ExtensionRow { return extensionRows(extMgr) },
		TrustExtension: func(name string, trusted bool) error {
			return extMgr.SetTrusted(name, trusted)
		},
		SetExtensionEnabled: extMgr.SetEnabled,
		Commands:            func() []bridge.CommandRow { return extensionCommands(extMgr) },
		RunCommand:          extMgr.RunCommand,
		OnEvent: func(ev eventbus.Event) {
			if app != nil {
				app.Emit(ev)
			}
			host.Dispatch(ev)
			extMgr.Dispatch(ev)
		},
	})
	if err != nil {
		return err
	}
	app = desktop.New(b, desktop.Options{
		Cwd:            *cwd,
		Model:          ref.String(),
		Thinking:       *thinking,
		Version:        version,
		Host:           host,
		SessionChanged: extMgr.SetSession,
		BeforeSessionSwitch: func(reason string) error {
			return extMgr.ConfirmSessionSwitch(context.Background(), reason)
		},
		AbortExtensions: func() {
			_ = extMgr.AbortAll(context.Background())
		},
	})
	mygo.BindAs("Desktop", desktop.NewBindings(app))
	mygo.BindAs("Diff", desktop.NewDiffBindings(diffService))
	host.SetStatusSource(desktop.StatusSource{
		Extensions:     func() []bridge.ExtensionRow { return extensionRows(extMgr) },
		Skills:         app.Skills,
		Commands:       func() []bridge.CommandRow { return extensionCommands(extMgr) },
		TrustExtension: extMgr.SetTrusted,
		Enable:         extMgr.SetEnabled,
		TrustProject:   app.SetProjectTrust,
		RunCommand:     func(name, args string) error { return extMgr.RunCommand(context.Background(), name, args) },
	})
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
		pageScenario := *scenario
		if pageScenario == "steer-real" {
			pageScenario = "steer"
		}
		query := url.Values{}
		query.Set("smoke", pageScenario)
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
		desktop.GuardPage(page.Page())
		host.Attach(win, page)
		app.SetWindow(win)
		app.SetPage(page.Page())
		win.OnClose(func(*mygo.CloseEvent) {
			host.CloseTabs()
			app.Shutdown()
		})
		if *smoke {
			go runSmoke(page, host, app, *scenario, *expect, *smokeHold, *smokeShot)
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

// resolveNodePath picks the Node runtime for A-track extensions: flag, env,
// bundled runtime, then PATH (development). It also returns the version to
// enforce: only the bundled runtime is pinned, because flag/env and PATH are
// development overrides. A missing runtime surfaces as an extension start
// diagnostic; the desktop itself stays usable.
func resolveNodePath(flagValue string) (path, version string) {
	if flagValue != "" {
		return flagValue, ""
	}
	if v := os.Getenv("LOCOWRK_NODE"); v != "" {
		return v, ""
	}
	if exe, err := os.Executable(); err == nil {
		name := "node"
		if runtime.GOOS == "windows" {
			name = "node.exe"
		}
		p := filepath.Join(filepath.Dir(exe), "runtime", "node", name)
		if _, err := os.Stat(p); err == nil {
			return p, extensions.PinnedNodeVersion
		}
	}
	if _, err := exec.LookPath("node"); err == nil && mygo.IsDev() {
		fmt.Fprintln(os.Stderr, "locowrk: no bundled Node runtime; using PATH node (development)")
		return "node", ""
	}
	return "", ""
}

func extensionRows(m *extensions.Manager) []bridge.ExtensionRow {
	infos := m.Extensions()
	out := make([]bridge.ExtensionRow, 0, len(infos))
	for _, e := range infos {
		row := bridge.ExtensionRow{
			Name: e.Name, Version: e.Version, Source: string(e.Source),
			State: e.State, Reason: e.Reason, SHA: e.SHA,
			Enabled: e.Enabled, Trusted: e.Trusted,
			Tools: e.Tools, Commands: e.Commands,
		}
		for _, d := range e.Diagnostics {
			row.Diagnostics = append(row.Diagnostics, d.Member+": "+d.Category+"/"+d.Code)
		}
		out = append(out, row)
	}
	return out
}

func extensionCommands(m *extensions.Manager) []bridge.CommandRow {
	cmds := m.Commands()
	out := make([]bridge.CommandRow, 0, len(cmds))
	for _, c := range cmds {
		out = append(out, bridge.CommandRow{Name: c.Name, Description: c.Description, Plugin: c.Plugin})
	}
	return out
}
