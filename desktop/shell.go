package desktop

import (
	"github.com/egoist/mygo"
	"github.com/egoist/mygo/ui"
)

// pluginView is the shell's copy of one registry row.
type pluginView struct {
	name    string
	title   string
	enabled bool
	failed  bool
	diag    string
}

// frameState is what the shell draws, taken under one lock per frame.
type frameState struct {
	web      *mygo.WebView
	railOpen bool
	plugins  []pluginView
	diag     string
}

func (h *Host) frameState() frameState {
	h.mu.Lock()
	defer h.mu.Unlock()
	st := frameState{web: h.web, railOpen: h.settings.Rail != "closed"}
	for _, e := range h.entries {
		pv := pluginView{
			name:    e.plugin.Name(),
			title:   e.plugin.Title(),
			enabled: e.enabled,
			failed:  e.failed,
		}
		if d := h.diags[pv.name]; d != nil {
			pv.diag = d.Message
		}
		st.plugins = append(st.plugins, pv)
	}
	if d := h.diags[""]; d != nil {
		st.diag = d.Message
	}
	return st
}

// View draws the native shell: a plugin strip and rail around the embedded
// page. The page keeps the transcript, sessions and composer.
func (h *Host) View(c *ui.Context) {
	st := h.frameState()
	h.mu.Lock()
	h.frames++
	h.mu.Unlock()
	if st.web == nil {
		// The window draws before its web view exists; Attach repaints.
		return
	}
	ui.Column(c).Fill().Children(func() {
		ui.Row(c).Grow(1).AlignItems(ui.Stretch).Children(func() {
			ui.Column(c).Grow(1).MinWidth(0).Children(func() {
				h.buildStrip(c)
				ui.Divider(c)
				ui.WebView(c, st.web).Grow(1)
			})
			if st.railOpen {
				ui.Divider(c)
				h.buildRail(c)
			}
		})
	})
}

// buildStrip draws session.timeline.above and the plugin menu.
func (h *Host) buildStrip(c *ui.Context) {
	t := c.Theme()
	ui.Row(c).Height(38).Padding(6, 10).Gap(6).AlignItems(ui.Center).Children(func() {
		ui.Row(c).Grow(1).MinWidth(0).Children(func() {
			if len(h.slotEntries(SlotTimelineAbove)) == 0 {
				ui.Text(c, "插件").FontSize(11).TextColor(t.TextMuted)
			}
			for _, e := range h.slotEntries(SlotTimelineAbove) {
				h.buildPlugin(c, e)
			}
		})
		menu := ui.Button(c, "插件 ▾")
		if menu.Clicked() {
			h.menuOpen = !h.menuOpen
		}
		ui.Popover(c, menu, &h.menuOpen, func() { h.buildMenu(c) })
	})
}

// buildRail draws the native plugin rail.
func (h *Host) buildRail(c *ui.Context) {
	t := c.Theme()
	ui.Column(c).Width(260).Children(func() {
		ui.Row(c).Padding(6, 8).Gap(6).AlignItems(ui.Center).Children(func() {
			ui.Text(c, "插件栏").FontSize(11).TextColor(t.TextMuted)
			ui.Box(c).Grow(1)
			if ui.Button(c, "›").Tooltip("收起插件栏").Clicked() {
				h.setRail(false)
			}
		})
		ui.Divider(c)
		h.buildSection(c, SlotSessionSidebar)
		ui.Divider(c)
		h.buildSection(c, SlotToolDetail)
	})
}

// buildSection draws one rail section: a title row and a scrolling body.
func (h *Host) buildSection(c *ui.Context, slot Slot) {
	entries := h.slotEntries(slot)
	if len(entries) == 0 {
		return
	}
	t := c.Theme()
	ui.Column(c).Grow(1).MinHeight(0).Children(func() {
		ui.Row(c).Padding(6, 8).Gap(4).AlignItems(ui.Center).Children(func() {
			for i, e := range entries {
				if i > 0 {
					ui.Text(c, "·").FontSize(11).TextColor(t.TextMuted)
				}
				ui.Text(c, e.plugin.Title()).Bold().FontSize(12)
			}
		})
		ui.Scroll(c).Grow(1).Padding(2, 8, 8, 8).Children(func() {
			for _, e := range entries {
				h.buildPlugin(c, e)
			}
		})
	})
}

// buildMenu lists the plugins, their state and the diagnostics.
func (h *Host) buildMenu(c *ui.Context) {
	st := h.frameState()
	t := c.Theme()
	ui.Column(c).Width(260).Padding(8).Gap(6).Children(func() {
		ui.Text(c, "插件").Bold().FontSize(12)
		for _, p := range st.plugins {
			on := p.enabled
			if ui.Checkbox(c, &on, p.title).Changed() {
				h.setEnabled(p.name, on)
			}
			if p.failed && p.diag != "" {
				ui.Text(c, p.diag).TextColor(t.Danger).FontSize(10)
			}
		}
		ui.Divider(c)
		rail := st.railOpen
		if ui.Checkbox(c, &rail, "显示插件栏").Changed() {
			h.setRail(rail)
		}
		if st.diag != "" {
			ui.Text(c, st.diag).TextColor(t.Danger).FontSize(10)
		}
	})
}
