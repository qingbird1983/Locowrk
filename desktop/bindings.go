package desktop

import "github.com/qingbird1983/locowrk/internal/bridge"

// Bindings is the surface exposed to the page. App keeps its Go-side methods
// (event pump, window, shutdown) out of the generated TypeScript client.
type Bindings struct {
	app *App
}

// NewBindings wraps an App for mygo.BindAs("Desktop", ...).
func NewBindings(a *App) *Bindings { return &Bindings{app: a} }

// Info returns the defaults shown at startup.
func (b *Bindings) Info() Info { return b.app.Info() }

// Models lists the models offered in the picker.
func (b *Bindings) Models() []string { return b.app.Models() }

// Sessions lists stored sessions for cwd.
func (b *Bindings) Sessions(cwd string) []SessionRow { return b.app.Sessions(cwd) }

// NewSession creates a session and selects it.
func (b *Bindings) NewSession(cwd, model, thinking string) (string, error) {
	return b.app.NewSession(cwd, model, thinking)
}

// OpenSession resumes a stored session and selects it.
func (b *Bindings) OpenSession(path, model, thinking string) (string, error) {
	return b.app.OpenSession(path, model, thinking)
}

// History returns the transcript as page DTOs.
func (b *Bindings) History(sessionID string) ([]Message, error) {
	return b.app.History(sessionID)
}

// Send runs one turn and blocks until it finishes.
func (b *Bindings) Send(sessionID, text string) error { return b.app.Send(sessionID, text) }

// Stop cancels the session's active turn.
func (b *Bindings) Stop(sessionID string) error { return b.app.Stop(sessionID) }

// Steer appends a user instruction to the running turn.
func (b *Bindings) Steer(sessionID, text string) error { return b.app.Steer(sessionID, text) }

// Skills returns skills and project trust for the workspace.
func (b *Bindings) Skills() bridge.SkillsView { return b.app.Skills() }

// Extensions returns A-track extension status rows.
func (b *Bindings) Extensions() []bridge.ExtensionRow { return b.app.Extensions() }

// TrustExtension records an extension trust decision.
func (b *Bindings) TrustExtension(name string, trusted bool) error {
	return b.app.TrustExtension(name, trusted)
}

// SetExtensionEnabled toggles one extension.
func (b *Bindings) SetExtensionEnabled(name string, enabled bool) error {
	return b.app.SetExtensionEnabled(name, enabled)
}

// Commands lists extension commands.
func (b *Bindings) Commands() []bridge.CommandRow { return b.app.Commands() }

// RunCommand executes one extension command.
func (b *Bindings) RunCommand(name, args string) error { return b.app.RunCommand(name, args) }

// SetProjectTrust trusts or untrusts the current workspace.
func (b *Bindings) SetProjectTrust(trusted bool) error { return b.app.SetProjectTrust(trusted) }

// SetModel switches the session's model.
func (b *Bindings) SetModel(sessionID, model string) error {
	return b.app.SetModel(sessionID, model)
}

// SetThinking switches the session's thinking level.
func (b *Bindings) SetThinking(sessionID, level string) error {
	return b.app.SetThinking(sessionID, level)
}

// SelectToolCall shows a tool card's details in the native inspector.
func (b *Bindings) SelectToolCall(sessionID, toolCallID string) error {
	return b.app.SelectToolCall(sessionID, toolCallID)
}

// OpenDiff opens the workspace diff for a tool card in a review tab.
func (b *Bindings) OpenDiff(sessionID, toolCallID string) error {
	return b.app.OpenDiff(sessionID, toolCallID)
}
