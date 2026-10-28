package main

import (
	"github.com/egoist/mygo/ui"

	"github.com/qingbird1983/locowrk/desktop"
	"github.com/qingbird1983/locowrk/internal/eventbus"
)

// smokeCrashPlugin panics on every build. It is registered only for
// -smoke-scenario plugin-crash, to prove the host isolates a plugin failure
// without shipping a faulty plugin in the product registry.
type smokeCrashPlugin struct{}

func (smokeCrashPlugin) Name() string  { return "smoke-crash" }
func (smokeCrashPlugin) Title() string { return "崩溃探针" }
func (smokeCrashPlugin) Slot() desktop.Slot {
	return desktop.SlotSessionSidebar
}
func (smokeCrashPlugin) InterestedEvents() []eventbus.Kind           { return nil }
func (smokeCrashPlugin) Dispatch(eventbus.Event, desktop.PluginHost) {}

func (smokeCrashPlugin) Build(_ *ui.Context, _ desktop.PluginHost) {
	panic("smoke: plugin build panic")
}
