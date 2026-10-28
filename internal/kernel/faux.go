package kernel

import (
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
	reg := providers.RegisterFauxProvider(providers.RegisterFauxProviderOptions{
		Models: []providers.FauxModelDefinition{{ID: "faux-1", ContextWindow: 200000, MaxTokens: 8192}},
	})
	reg.SetResponses([]providers.FauxResponseStep{
		providers.FauxStatic(providers.FauxAssistantMessage(ai.ContentList{
			providers.FauxText(text),
		}, ai.StopStop)),
	})
	model := reg.GetModel()
	opts.LookupModel = func(ModelRef) (*ai.Model, error) { return model, nil }
	return opts, ModelRef{Provider: string(model.Provider), ID: model.ID}, reg.Unregister
}
