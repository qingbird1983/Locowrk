package desktop

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

// SetModel switches the session's model.
func (b *Bindings) SetModel(sessionID, model string) error {
	return b.app.SetModel(sessionID, model)
}

// SetThinking switches the session's thinking level.
func (b *Bindings) SetThinking(sessionID, level string) error {
	return b.app.SetThinking(sessionID, level)
}
