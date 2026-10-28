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
	tabs     []frameTab
	active   TabID
}

func (h *Host) frameState() frameState {
	h.mu.Lock()
	defer h.mu.Unlock()
	st := frameState{web: h.web, railOpen: h.settings.Rail != "closed", active: TabSession}
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
	for _, e := range h.tabs {
		st.tabs = append(st.tabs, frameTab{info: e.info, web: e.web})
		if e.info.ID == h.active {
			st.active = h.active
		}
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
				if len(st.tabs) > 0 {
					h.buildTabBar(c, st)
					ui.Divider(c)
				}
				active := st.web
				if st.active != TabSession {
					for _, tab := range st.tabs {
						if tab.info.ID == st.active {
							active = tab.web
							break
						}
					}
				}
				if active != nil {
					ui.WebView(c, active).Grow(1)
				}
			})
			if st.railOpen {
				ui.Divider(c)
				h.buildRail(c)
			}
		})
	})
}

// buildTabBar draws the transcript tab and the open web tabs. Its popover is
// built before the page and still paints over it: MyGo places popovers above
// web views (see research/03 §11).
func (h *Host) buildTabBar(c *ui.Context, st frameState) {
	ui.Row(c).Height(30).Padding(2, 6).Gap(4).AlignItems(ui.Center).Children(func() {
		label := "会话"
		if st.active == TabSession {
			label = "● " + label
		}
		if ui.Button(c, label).FontSize(12).Clicked() {
			h.ActivateTab(TabSession)
		}
		for _, tab := range st.tabs {
			title := tab.info.Title
			if st.active == tab.info.ID {
				title = "● " + title
			}
			if ui.Button(c, title).FontSize(12).Clicked() {
				h.ActivateTab(tab.info.ID)
			}
			if ui.Button(c, "×").FontSize(11).Tooltip("关闭标签页").Clicked() {
				h.CloseTab(tab.info.ID)
			}
		}
		ui.Box(c).Grow(1)
		if st.active == TabSession {
			return
		}
		more := ui.Button(c, "⋯")
		if more.Clicked() {
			h.tabMenu = !h.tabMenu
		}
		ui.Popover(c, more, &h.tabMenu, func() {
			h.buildTabMenu(c, st.active)
		})
	})
}

// buildTabMenu offers actions for the active web tab.
func (h *Host) buildTabMenu(c *ui.Context, active TabID) {
	ui.Column(c).Width(200).Padding(6).Gap(4).Children(func() {
		if ui.Button(c, "刷新").Clicked() {
			h.reloadTab(active)
			h.tabMenu = false
		}
		if ui.Button(c, "复制路径").Clicked() {
			h.copyTabPath(active)
			h.tabMenu = false
		}
		if ui.Button(c, "关闭标签页").Clicked() {
			h.CloseTab(active)
			h.tabMenu = false
		}
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
