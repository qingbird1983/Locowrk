// Command locowrk-smoke proves the Phase 1 kernel end to end: it creates or
// resumes a session, streams every project event to stdout as JSONL and prints
// the final assistant text to stderr.
//
// It is a temporary shell; Phase 3 replaces it with the desktop host.
package main

import (
	"context"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"os"
	"os/signal"
	"strings"
	"syscall"

	"github.com/sky-valley/pi/ai"
	"github.com/sky-valley/pi/ai/providers"

	"github.com/qingbird1983/locowrk/internal/eventbus"
	"github.com/qingbird1983/locowrk/internal/kernel"
)

func main() {
	if err := run(); err != nil {
		if errors.Is(err, kernel.ErrCanceled) {
			fmt.Fprintln(os.Stderr, "canceled")
			return
		}
		fmt.Fprintln(os.Stderr, "error:", err)
		os.Exit(1)
	}
}

func run() error {
	var (
		prompt  = flag.String("prompt", "", "prompt to send (required)")
		model   = flag.String("model", "anthropic/claude-sonnet-4-5", "model as provider/id")
		cwd     = flag.String("cwd", "", "working directory (default: current directory)")
		tools   = flag.String("tools", "read,write,edit,bash", "comma-separated tool allowlist, empty for defaults")
		think   = flag.String("think", "", "thinking level: off|minimal|low|medium|high|xhigh")
		resume  = flag.String("resume", "", "session JSONL file to resume")
		faux    = flag.Bool("faux", false, "use the built-in faux provider (no API key needed)")
		verbose = flag.Bool("verbose", false, "print the session file and event count to stderr")
	)
	flag.Parse()

	if *prompt == "" {
		flag.Usage()
		return errors.New("-prompt is required")
	}
	workDir := *cwd
	if workDir == "" {
		var err error
		workDir, err = os.Getwd()
		if err != nil {
			return err
		}
	}

	lookup := func(ref kernel.ModelRef) (*ai.Model, error) {
		if m := ai.GetModel(ref.Provider, ref.ID); m != nil {
			return m, nil
		}
		return nil, fmt.Errorf("unknown model %s", ref)
	}
	ref, err := kernel.ParseModelRef(*model)
	if err != nil {
		return err
	}

	if *faux {
		reg := providers.RegisterFauxProvider(providers.RegisterFauxProviderOptions{
			Models: []providers.FauxModelDefinition{{ID: "faux-1", ContextWindow: 200000, MaxTokens: 8192}},
		})
		defer reg.Unregister()
		fauxModel := reg.GetModel()
		reg.SetResponses([]providers.FauxResponseStep{
			providers.FauxStatic(providers.FauxAssistantMessage(ai.ContentList{
				providers.FauxText("faux provider: received " + *prompt),
			}, ai.StopStop)),
		})
		ref = kernel.ModelRef{Provider: string(fauxModel.Provider), ID: fauxModel.ID}
		lookup = func(kernel.ModelRef) (*ai.Model, error) { return fauxModel, nil }
	}

	svc, err := kernel.New(kernel.Options{LookupModel: lookup})
	if err != nil {
		return err
	}
	defer svc.Close(context.Background())

	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer stop()

	var toolsList []string
	if strings.TrimSpace(*tools) != "" {
		for _, name := range strings.Split(*tools, ",") {
			if name = strings.TrimSpace(name); name != "" {
				toolsList = append(toolsList, name)
			}
		}
	}

	var id kernel.SessionID
	if *resume != "" {
		id, err = svc.OpenSession(ctx, kernel.OpenOptions{
			Path: *resume, Model: ref, Thinking: *think, Tools: toolsList,
		})
	} else {
		id, err = svc.CreateSession(ctx, kernel.SessionOptions{
			Cwd: workDir, Model: ref, Thinking: *think, Tools: toolsList,
		})
	}
	if err != nil {
		return err
	}
	if *verbose {
		if path, ok := svc.SessionPath(id); ok {
			fmt.Fprintf(os.Stderr, "session %s -> %s\n", id, path)
		}
	}

	sub := svc.Subscribe(eventbus.SubscribeOptions{SessionID: string(id)})
	defer sub.Close()

	enc := json.NewEncoder(os.Stdout)
	ended := make(chan struct{})
	go func() {
		var count int
		for ev := range sub.C() {
			if err := enc.Encode(ev); err != nil {
				fmt.Fprintln(os.Stderr, "write event:", err)
			}
			count++
			if ev.Kind == eventbus.KindAgentEnd {
				if *verbose {
					fmt.Fprintf(os.Stderr, "events: %d\n", count)
				}
				close(ended)
				return
			}
		}
	}()

	if err := svc.Prompt(ctx, id, *prompt); err != nil {
		return err
	}
	select {
	case <-ended:
	case <-ctx.Done():
	}

	history, err := svc.History(ctx, id)
	if err != nil {
		return err
	}
	if len(history) > 0 {
		fmt.Fprintln(os.Stderr, history[len(history)-1].Text)
	}
	return nil
}
