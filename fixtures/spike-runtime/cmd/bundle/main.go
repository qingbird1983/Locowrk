// Command bundle packages one spike sample for a runtime route.
//
// Route A (--platform neutral): the sample is bundled against the shim and must
// not reach Node built-ins; esbuild fails when it does.
// Route B (--platform node): Node built-ins stay external and resolve at run
// time under Node.
package main

import (
	"flag"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/evanw/esbuild/pkg/api"
)

func main() {
	sample := flag.String("sample", "", "path to the extension entry (.ts or .js)")
	out := flag.String("out", "", "output bundle path")
	platform := flag.String("platform", "neutral", "esbuild platform: neutral (route A) or node (route B)")
	shimDir := flag.String("shim", "shim", "directory holding the shim modules")
	flag.Parse()

	if *sample == "" || *out == "" {
		fail("bundle: --sample and --out are required")
	}
	absSample, err := filepath.Abs(*sample)
	if err != nil {
		fail("bundle: resolving sample: %v", err)
	}
	absShim, err := filepath.Abs(*shimDir)
	if err != nil {
		fail("bundle: resolving shim: %v", err)
	}

	alias := map[string]string{
		"@earendil-works/pi-coding-agent": filepath.Join(absShim, "pi-coding-agent.js"),
		"@earendil-works/pi-ai":           filepath.Join(absShim, "pi-ai.js"),
		"@earendil-works/pi-agent-core":   filepath.Join(absShim, "pi-agent-core.js"),
		"@earendil-works/pi-tui":          filepath.Join(absShim, "pi-tui.js"),
		"@earendil-works/pi":              filepath.Join(absShim, "pi-coding-agent.js"),
		"typebox":                         filepath.Join(absShim, "typebox.js"),
	}

	if *platform == "neutral" {
		// Route A has no Node runtime. Rather than pretend, route the Node
		// built-ins the orchestration sample uses to capability stubs so the
		// run can report "needs a host capability" instead of failing to load.
		for _, name := range []string{"child_process", "fs", "os", "path"} {
			alias["node:"+name] = filepath.Join(absShim, "node-"+name+".js")
		}
	}

	entry := fmt.Sprintf(
		"import %q;\nimport ext from %q;\nglobalThis.__locowrkExtension = ext;\n",
		filepath.ToSlash(filepath.Join(absShim, "runtime.js")),
		filepath.ToSlash(absSample),
	)

	buildPlatform := api.PlatformNeutral
	buildFormat := api.FormatIIFE
	if *platform == "node" {
		buildPlatform = api.PlatformNode
		// Route B loads the bundle with require(), so built-ins resolve through
		// Node's own loader instead of a bundler-generated shim.
		buildFormat = api.FormatCommonJS
	}

	result := api.Build(api.BuildOptions{
		Stdin: &api.StdinOptions{
			Contents:   entry,
			ResolveDir: filepath.Dir(absSample),
			Sourcefile: "spike-entry.ts",
			Loader:     api.LoaderTS,
		},
		Bundle:   true,
		Write:    false,
		Platform: buildPlatform,
		Format:   buildFormat,
		Target:   api.ES2020,
		Alias:    alias,
		LogLevel: api.LogLevelSilent,
	})
	if len(result.Errors) > 0 {
		var msgs []string
		for _, m := range result.Errors {
			loc := ""
			if m.Location != nil {
				loc = fmt.Sprintf(" (%s:%d)", m.Location.File, m.Location.Line)
			}
			msgs = append(msgs, m.Text+loc)
		}
		fail("bundle: %s", strings.Join(msgs, "; "))
	}

	if err := os.MkdirAll(filepath.Dir(*out), 0o755); err != nil {
		fail("bundle: %v", err)
	}
	var code []byte
	if len(result.OutputFiles) != 1 {
		fail("bundle: expected one output file, got %d", len(result.OutputFiles))
	}
	code = result.OutputFiles[0].Contents
	if err := os.WriteFile(*out, code, 0o644); err != nil {
		fail("bundle: %v", err)
	}
	fmt.Printf("bundle: %s (%d bytes)\n", *out, len(code))
}

func fail(format string, args ...any) {
	fmt.Fprintf(os.Stderr, format+"\n", args...)
	os.Exit(1)
}
