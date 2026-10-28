package adapter

import "context"

// HostUI answers extension dialogs. A nil UI degrades select/confirm/input/
// editor to an unsupported diagnostic instead of failing the extension.
type HostUI interface {
	Select(ctx context.Context, title string, options []string) (value string, ok bool)
	Confirm(ctx context.Context, title, message string) (yes bool, ok bool)
	Input(ctx context.Context, title, placeholder string) (value string, ok bool)
	Editor(ctx context.Context, title, prefill string) (value string, ok bool)
}
