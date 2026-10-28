package desktop

import (
	"context"

	"github.com/egoist/mygo"
)

// DialogUI maps extension dialogs to native message boxes. Input and editor
// have no native single-line equivalent yet; they degrade to a cancel.
type DialogUI struct {
	host *Host
}

// NewDialogUI returns the A-track dialog adapter. The host may not be attached
// yet; calls before the window exists cancel.
func NewDialogUI(h *Host) *DialogUI { return &DialogUI{host: h} }

func (d *DialogUI) window() *mygo.Window {
	d.host.mu.Lock()
	defer d.host.mu.Unlock()
	return d.host.win
}

// Confirm shows a native question box.
func (d *DialogUI) Confirm(ctx context.Context, title, message string) (bool, bool) {
	win := d.window()
	if win == nil {
		return false, false
	}
	res, err := mygo.Dialog.Message(mygo.MessageOptions{
		Parent: win, Type: mygo.MessageQuestion, Title: title, Message: message,
		Buttons: []string{"取消", "确定"}, DefaultButton: 1, CancelButton: 0,
	})
	if err != nil {
		return false, false
	}
	return res.Button == 1, true
}

// Select shows one button per option plus cancel.
func (d *DialogUI) Select(ctx context.Context, title string, options []string) (string, bool) {
	win := d.window()
	if win == nil || len(options) == 0 {
		return "", false
	}
	buttons := append(append([]string{}, options...), "取消")
	res, err := mygo.Dialog.Message(mygo.MessageOptions{
		Parent: win, Type: mygo.MessageQuestion, Title: title,
		Buttons: buttons, CancelButton: len(options),
	})
	if err != nil || res.Button < 0 || res.Button >= len(options) {
		return "", false
	}
	return options[res.Button], true
}

// Input is not implemented natively in Phase 6: cancel rather than block.
func (d *DialogUI) Input(context.Context, string, string) (string, bool) {
	return "", false
}

// Editor is not implemented natively in Phase 6: cancel rather than block.
func (d *DialogUI) Editor(context.Context, string, string) (string, bool) {
	return "", false
}
