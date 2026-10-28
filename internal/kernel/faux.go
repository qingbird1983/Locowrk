package kernel

import (
	"strings"

	"github.com/qingbird1983/locowrk/pi/ai"
	"github.com/qingbird1983/locowrk/pi/ai/providers"
)

// WithFauxModel registers a scripted provider that answers a prompt with text
// (single turn) and wires it into opts. It exists for the smoke CLI and
// development runs that must not call a real provider; tests that need tool
// calls or streaming use providers.RegisterFauxProvider directly inside this
// package.
//
// The returned cleanup unregisters the provider.
func WithFauxModel(opts Options, text string) (Options, ModelRef, func()) {
	return withFaux(opts, providers.RegisterFauxProviderOptions{}, []providers.FauxResponseStep{
		providers.FauxStatic(providers.FauxAssistantMessage(ai.ContentList{
			providers.FauxText(text),
		}, ai.StopStop)),
	})
}

// WithFauxScenario is WithFauxModel for the Phase 3 acceptance smoke: each
// scenario scripts the turns the page drives (tool card, cancellation, long
// output). It returns the same cleanup contract.
func WithFauxScenario(opts Options, scenario, notePath string) (Options, ModelRef, func()) {
	regOpts := providers.RegisterFauxProviderOptions{}
	var steps []providers.FauxResponseStep
	switch scenario {
	case "tool", "plugin":
		steps = []providers.FauxResponseStep{
			providers.FauxStatic(providers.FauxAssistantMessage(ai.ContentList{
				providers.FauxToolCall("read", map[string]any{"path": notePath}),
			}, ai.StopToolUse)),
			providers.FauxStatic(providers.FauxAssistantMessage(ai.ContentList{
				providers.FauxText("工具读取完成：locowrk smoke note"),
			}, ai.StopStop)),
		}
	case "cancel":
		regOpts.TokensPerSecond = 40
		steps = []providers.FauxResponseStep{
			providers.FauxStatic(providers.FauxAssistantMessage(ai.ContentList{
				providers.FauxText(strings.Repeat("streaming output for the cancel scenario ", 120)),
			}, ai.StopStop)),
			providers.FauxStatic(providers.FauxAssistantMessage(ai.ContentList{
				providers.FauxText("second answer"),
			}, ai.StopStop)),
		}
	default:
		steps = []providers.FauxResponseStep{
			providers.FauxStatic(providers.FauxAssistantMessage(ai.ContentList{
				providers.FauxText("faux assistant reply"),
			}, ai.StopStop)),
		}
	}
	return withFaux(opts, regOpts, steps)
}

func withFaux(opts Options, regOpts providers.RegisterFauxProviderOptions, steps []providers.FauxResponseStep) (Options, ModelRef, func()) {
	if regOpts.Models == nil {
		regOpts.Models = []providers.FauxModelDefinition{{ID: "faux-1", ContextWindow: 200000, MaxTokens: 8192}}
	}
	reg := providers.RegisterFauxProvider(regOpts)
	reg.SetResponses(steps)
	model := reg.GetModel()
	opts.LookupModel = func(ModelRef) (*ai.Model, error) { return model, nil }
	return opts, ModelRef{Provider: string(model.Provider), ID: model.ID}, reg.Unregister
}
