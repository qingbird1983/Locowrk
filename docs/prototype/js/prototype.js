/**
 * prototype.js — 原型状态机 + 本地 UI 交互。
 *
 * 职责边界（静态原型）：
 *  - 真实行为：14 个状态切换、明暗主题、侧栏/工作面板指针拖拽调宽（含键盘与持久化）、
 *    侧栏折叠/展开、面板开关/最大化/页签切换与关闭、三级折叠（过程组/思考行/工具行）、
 *    命令面板开合与键盘高亮、上下文与通知浮层、权限模式菜单、批量卸载确认弹窗、
 *    通知全部已读/清空/筛选、复制按钮写剪贴板、提交框自动增长。
 *  - 不实现：任何需要后端的行为（发消息、跑工具、审批写回、装插件、撤销压缩）。
 *    这些按钮保持语义与状态正确，但不伪造结果；原型未实现的启动器条目显式禁用。
 *
 * 状态 → 界面采用「附加覆盖」：data-state 决定结构性可见性（首页/转录/目的地页），
 * html.is-* 类与 --lw-* 变量由本文件维护（面板、折叠、浮层、宽度），两者不重复描述同一件事。
 */
(function () {
  "use strict";

  /* 14 个状态：id → 该状态的默认 UI 覆盖（相对 S2 的附加差异） */
  var STATES = [
    { id: "s1", label: "S1 首页空态", ui: {} },
    { id: "s2", label: "S2 对话中", ui: {} },
    { id: "s3", label: "S3 面板打开", ui: { panel: true, panelTab: "review" } },
    { id: "s4", label: "S4 面板空态", ui: { panel: true, panelTab: null } },
    { id: "s5", label: "S5 命令面板", ui: { palette: true } },
    { id: "s6", label: "S6 审批决策卡（内联）", ui: {}, scrollTo: "[data-permission]" },
    { id: "s7", label: "S7 上下文浮层", ui: { ctx: true } },
    { id: "s8", label: "S8 通知浮层", ui: { notify: true } },
    { id: "s9", label: "S9 扩展整页", ui: {} },
    { id: "s10", label: "S10 侧栏折叠", ui: { collapsed: true } },
    { id: "s11", label: "S11 工具行展开", ui: { expandTool: true } },
    { id: "s12", label: "S12 确认弹窗（批量卸载）", ui: { confirm: true } },
    { id: "s13", label: "S13 子 Agent 转录", ui: { panel: true, panelTab: "subagent" } },
    { id: "s14", label: "S14 权限模式菜单", ui: { perm: true } }
  ];

  var root = document.documentElement;
  var state = "s2";

  var ui = {
    panel: false,
    panelTab: null,        /* "review" | "subagent" | null（null = 启动器空态） */
    panelMax: false,
    collapsed: false,
    palette: false,
    confirm: false,
    ctx: false,
    notify: false,
    notifyFilter: "all",
    perm: false,
    permMode: "full",
    expandTool: false
  };

  var $ = function (sel) { return document.querySelector(sel); };
  var $$ = function (sel) { return Array.prototype.slice.call(document.querySelectorAll(sel)); };

  var refs = {
    scroller: $("#scroller"),
    panel: $(".workpanel"),
    panelToggle: $("#panelToggle"),
    panelMax: $("#panelMax"),
    panelNewTab: $(".workpanel__header .icon-btn:not(#panelMax)"),
    sidebarCollapse: $("#sidebarCollapse"),
    sidebarExpand: $("#sidebarExpand"),
    sidebarGrip: $("#sidebarGrip"),
    panelGrip: $("#panelGrip"),
    ctxBtn: $("#ctxBtn"),
    ctxPopover: $("#ctxPopover"),
    notifyBell: $("#notifyBell"),
    notifyPopover: $("#notifyPopover"),
    notifyList: $("#notifyList"),
    notifyReadAll: $("#notifyReadAll"),
    notifyClear: $("#notifyClear"),
    overlayPalette: $("#overlayPalette"),
    overlayConfirm: $("#overlayConfirm"),
    paletteInput: $("#paletteInput"),
    paletteResults: $("#paletteResults"),
    composerInput: $("#composerInput"),
    sendBtn: $("#sendBtn"),
    permChip: $("#permChip"),
    permChipLabel: $("#permChipLabel"),
    permInfo: $("#permInfo"),
    permMenu: $("#permMenu"),
    proto: $("#proto"),
    protoPill: $("#protoPill"),
    protoCurrent: $("#protoCurrent"),
    protoGrid: $("#protoGrid"),
    protoCount: $("#protoCount")
  };

  /* ------------------------------------------------------------------ *
   * 三级折叠：默认态 + 用户手动覆盖（当轮内不被默认态覆盖）
   * ------------------------------------------------------------------ */
  var collapsibles = $$("[data-collapsible], [data-tool]").map(function (btn) {
    return { btn: btn, open: btn.getAttribute("aria-expanded") === "true" };
  });
  var manualToggles = [];   /* 用户手动切过的单元 */

  function bodyOf(btn) {
    var host = btn.closest(".tool, .thinking, .activity, .turn");
    if (!host) return null;
    return host.querySelector(".tool__detail, .thinking__body, .activity__body, .turn__body");
  }

  function setOpen(btn, open) {
    btn.setAttribute("aria-expanded", open ? "true" : "false");
    var body = bodyOf(btn);
    if (body) body.hidden = !open;
  }

  function registerCollapsibles() {
    collapsibles.forEach(function (item) {
      item.btn.addEventListener("click", function () {
        /* 手动切过的单元当轮内不再被默认态覆盖；三个层级各自独立，互不连带 */
        if (manualToggles.indexOf(item.btn) === -1) manualToggles.push(item.btn);
        setOpen(item.btn, item.btn.getAttribute("aria-expanded") !== "true");
      });
    });
  }

  /* ------------------------------------------------------------------ *
   * 尺寸：侧栏 240–520（缺省 275）与工作面板 244–预算（缺省 360）
   * 预算 = 视口 − 侧栏 − 主区地板 450；顶到地板时先折叠侧栏
   * ------------------------------------------------------------------ */
  var SIDEBAR_MIN = 240, SIDEBAR_MAX = 520, SIDEBAR_DEFAULT = 275;
  var PANEL_MIN = 244, PANEL_DEFAULT = 360, MAIN_MIN = 450;
  var STORE_SIDEBAR = "lw.proto.sidebarW", STORE_PANEL = "lw.proto.panelW";

  var size = { sidebarW: SIDEBAR_DEFAULT, panelW: PANEL_DEFAULT };

  function readToken(name, fallback) {
    var v = getComputedStyle(root).getPropertyValue(name).trim();
    var n = parseFloat(v);
    return isNaN(n) ? fallback : n;
  }

  function loadTokens() {
    SIDEBAR_MIN = readToken("--lw-sidebar-min", SIDEBAR_MIN);
    SIDEBAR_MAX = readToken("--lw-sidebar-max", SIDEBAR_MAX);
    SIDEBAR_DEFAULT = readToken("--lw-sidebar-default", SIDEBAR_DEFAULT);
    PANEL_MIN = readToken("--lw-panel-min", PANEL_MIN);
    PANEL_DEFAULT = readToken("--lw-panel-default", PANEL_DEFAULT);
    MAIN_MIN = readToken("--lw-main-min", MAIN_MIN);
  }

  function store(key, value) {
    try { window.localStorage.setItem(key, String(Math.round(value))); } catch (e) { /* file:// 私有模式忽略 */ }
  }

  function restore() {
    try {
      var s = parseFloat(window.localStorage.getItem(STORE_SIDEBAR));
      var p = parseFloat(window.localStorage.getItem(STORE_PANEL));
      if (!isNaN(s)) size.sidebarW = s;
      if (!isNaN(p)) size.panelW = p;
    } catch (e) { /* 无 localStorage 就用缺省值 */ }
  }

  function sidebarMax() {
    /* §6.4：--lw-sidebar-w = clamp(240, 275, min(520px, 100vw - 320px)) */
    return Math.max(SIDEBAR_MIN, Math.min(SIDEBAR_MAX, window.innerWidth - 320));
  }

  function panelBudget() {
    var side = ui.collapsed ? 0 : size.sidebarW;
    return Math.max(PANEL_MIN, window.innerWidth - side - MAIN_MIN);
  }

  function panelBudgetNoSidebar() {
    return Math.max(PANEL_MIN, window.innerWidth - MAIN_MIN);
  }

  function applySizes() {
    root.style.setProperty("--lw-sidebar-w", size.sidebarW + "px");
    root.style.setProperty("--lw-panel-w", size.panelW + "px");
    if (refs.sidebarGrip) {
      refs.sidebarGrip.setAttribute("aria-valuenow", String(Math.round(size.sidebarW)));
      refs.sidebarGrip.setAttribute("aria-valuemax", String(Math.round(sidebarMax())));
      refs.sidebarGrip.setAttribute("aria-valuetext", "侧栏宽度 " + Math.round(size.sidebarW) + " 像素");
    }
    if (refs.panelGrip) {
      refs.panelGrip.setAttribute("aria-valuenow", String(Math.round(size.panelW)));
      refs.panelGrip.setAttribute("aria-valuemax", String(Math.round(panelBudget())));
      refs.panelGrip.setAttribute("aria-valuetext", "工作面板宽度 " + Math.round(size.panelW) + " 像素");
    }
  }

  function setSidebarWidth(next) {
    if (next < SIDEBAR_MIN) {
      /* 拖到小于最小值即折叠——把「拖没了」变成明确状态，记住宽度留着 */
      ui.collapsed = true;
    } else {
      ui.collapsed = false;
      size.sidebarW = Math.min(next, sidebarMax());
    }
    applySizes();
    render();
  }

  function setPanelWidth(next) {
    var budget = panelBudget();
    if (next <= budget) {
      size.panelW = Math.max(PANEL_MIN, next);
    } else {
      /* 主区地板 450 是硬的：侧栏先让步，再谈面板 */
      ui.collapsed = true;
      size.panelW = Math.max(PANEL_MIN, Math.min(next, panelBudgetNoSidebar()));
    }
    applySizes();
    render();
  }

  function setCollapsed(v) {
    ui.collapsed = v;
    if (!v) size.panelW = Math.min(size.panelW, panelBudget());
    applySizes();
    render();
  }

  function persistSizes() {
    store(STORE_SIDEBAR, size.sidebarW);
    store(STORE_PANEL, size.panelW);
  }

  function bindGrip(grip, axis) {
    if (!grip) return;
    var dragging = false, startX = 0, startW = 0, pointerId = null;
    var dataKey = axis === "sidebar" ? "sidebarResizing" : "workPanelResizing";

    var startWOf = function () { return axis === "sidebar" ? size.sidebarW : size.panelW; };
    var applyDelta = function (deltaX) {
      /* 侧栏向右变宽；工作面板柄在左缘，向左变宽 */
      if (axis === "sidebar") setSidebarWidth(startW + deltaX);
      else setPanelWidth(startW - deltaX);
    };

    /* 键盘步进必须相对「当前」宽度：用拖拽起点会跳回按下时那一帧的值 */
    var applyStep = function (deltaX) {
      if (axis === "sidebar") setSidebarWidth(size.sidebarW + deltaX);
      else setPanelWidth(size.panelW - deltaX);
      persistSizes();
    };

    grip.addEventListener("pointerdown", function (event) {
      if (event.button !== 0) return;
      dragging = true;
      startX = event.clientX;
      startW = startWOf();
      pointerId = event.pointerId;
      try { grip.setPointerCapture(event.pointerId); } catch (e) { /* 老浏览器忽略 */ }
      root.dataset[dataKey] = "true";
      event.preventDefault();
    });

    grip.addEventListener("pointermove", function (event) {
      if (!dragging) return;
      applyDelta(event.clientX - startX);
    });

    var stop = function () {
      if (!dragging) return;
      dragging = false;
      if (pointerId !== null) {
        try { if (grip.hasPointerCapture(pointerId)) grip.releasePointerCapture(pointerId); } catch (e) { /* 已释放 */ }
      }
      delete root.dataset[dataKey];
      persistSizes();
    };
    grip.addEventListener("pointerup", stop);
    grip.addEventListener("pointercancel", stop);
    grip.addEventListener("lostpointercapture", stop);

    grip.addEventListener("keydown", function (event) {
      var step = 16;
      var grow = axis === "sidebar" ? "ArrowRight" : "ArrowLeft";
      var shrink = axis === "sidebar" ? "ArrowLeft" : "ArrowRight";
      /* 面板柄在左缘：向左 = 变宽；侧栏柄在右缘：向右 = 变宽 */
      var stepDelta = function (key) {
        if (axis === "sidebar") return key === "ArrowRight" ? step : -step;
        return key === "ArrowLeft" ? -step : step;
      };
      if (event.key === grow || event.key === shrink) {
        event.preventDefault();
        applyStep(stepDelta(event.key));
      } else if (event.key === "Home") {
        event.preventDefault();
        if (axis === "sidebar") setSidebarWidth(SIDEBAR_MIN);
        else setPanelWidth(PANEL_MIN);
        persistSizes();
      } else if (event.key === "End") {
        event.preventDefault();
        if (axis === "sidebar") setSidebarWidth(sidebarMax());
        else setPanelWidth(panelBudget());
        persistSizes();
      }
    });

    grip.addEventListener("dblclick", function (event) {
      event.preventDefault();
      if (axis === "sidebar") {
        ui.collapsed = false;
        size.sidebarW = Math.min(SIDEBAR_DEFAULT, sidebarMax());
      } else {
        size.panelW = Math.max(PANEL_MIN, Math.min(PANEL_DEFAULT, panelBudget()));
      }
      applySizes();
      render();
      persistSizes();
    });
  }

  /* 视口变化：预算重算，侧栏/面板都 clamp 回合法区间（侧栏先让步） */
  function onWindowResize() {
    size.sidebarW = Math.max(SIDEBAR_MIN, Math.min(size.sidebarW, sidebarMax()));
    if (ui.panel) setPanelWidth(size.panelW);
    else { applySizes(); render(); }
    persistSizes();
  }

  /* ------------------------------------------------------------------ *
   * 渲染
   * ------------------------------------------------------------------ */
  var openTabs = { review: true, subagent: true };

  function render() {
    root.dataset.state = state;

    root.classList.toggle("is-sidebar-collapsed", ui.collapsed);
    root.classList.toggle("is-panel-open", ui.panel);
    root.classList.toggle("is-panel-empty", ui.panel && ui.panelTab === null);
    root.classList.toggle("is-panel-tab-review", ui.panel && ui.panelTab === "review");
    root.classList.toggle("is-panel-tab-subagent", ui.panel && ui.panelTab === "subagent");
    root.classList.toggle("is-panel-max", ui.panel && ui.panelMax);

    /* 页签条：关闭的页签从条上消失；选中态跟着 panelTab */
    $$(".workpanel .tab").forEach(function (tab) {
      var name = tab.dataset.tab;
      tab.hidden = openTabs[name] !== true;
      tab.setAttribute("aria-selected", String(ui.panelTab === name));
    });

    /* 顶栏折叠按钮：展开态下隐藏且不可聚焦（零宽槽保持标题不跳位） */
    refs.sidebarExpand.tabIndex = ui.collapsed ? 0 : -1;
    refs.sidebarExpand.setAttribute("aria-hidden", ui.collapsed ? "false" : "true");

    if (refs.panelToggle) refs.panelToggle.setAttribute("aria-pressed", String(ui.panel));
    if (refs.panelMax) refs.panelMax.setAttribute("aria-pressed", String(ui.panelMax));

    if (refs.overlayPalette) refs.overlayPalette.classList.toggle("is-open", ui.palette);
    if (refs.overlayConfirm) refs.overlayConfirm.classList.toggle("is-open", ui.confirm);
    if (refs.ctxPopover) refs.ctxPopover.classList.toggle("is-open", ui.ctx);
    if (refs.ctxBtn) refs.ctxBtn.setAttribute("aria-expanded", String(ui.ctx));

    if (refs.permMenu) refs.permMenu.classList.toggle("is-open", ui.perm);
    if (refs.permChip) refs.permChip.setAttribute("aria-expanded", String(ui.perm));
    syncPermChip();

    var notifyOpen = ui.notify && !ui.collapsed;
    if (refs.notifyPopover) refs.notifyPopover.classList.toggle("is-open", notifyOpen);
    if (refs.notifyBell) refs.notifyBell.setAttribute("aria-expanded", String(notifyOpen));

    /* 提交按钮：S1 = 空闲 + 禁用（送信上箭头），其余 = 流式中（停止方块） */
    var idle = state === "s1";
    if (refs.sendBtn) {
      refs.sendBtn.disabled = idle;
      refs.sendBtn.setAttribute("aria-label", idle ? "发送" : "停止生成");
    }

    /* 三级折叠：默认态只作用于「用户没手动切过」的单元 */
    collapsibles.forEach(function (item) {
      if (manualToggles.indexOf(item.btn) === -1) setOpen(item.btn, item.open);
    });
    if (ui.expandTool) {
      var target = $('[data-expand="s11"]');
      var toggle = target && target.querySelector("[data-tool]");
      if (toggle && manualToggles.indexOf(toggle) === -1) setOpen(toggle, true);
    }

    applyNotifyFilter();
    applySizes();
  }

  /* ------------------------------------------------------------------ *
   * 状态切换
   * ------------------------------------------------------------------ */
  function applyState(id) {
    var entry = null;
    for (var i = 0; i < STATES.length; i++) {
      if (STATES[i].id === id) entry = STATES[i];
    }
    if (!entry) return;
    state = id;
    ui.panel = false;
    ui.panelTab = null;
    ui.panelMax = false;
    ui.collapsed = false;
    ui.palette = false;
    ui.confirm = false;
    ui.ctx = false;
    ui.notify = false;
    ui.notifyFilter = "all";
    ui.perm = false;
    ui.expandTool = false;
    Object.keys(entry.ui).forEach(function (key) { ui[key] = entry.ui[key]; });

    /* 切状态 = 回到标记里的折叠默认态（手动覆盖只保证「当轮内」不被覆盖） */
    manualToggles = [];
    /* 决策卡回到标记默认态：切状态 = 重新面对这次决策 */
    $$(".permission").forEach(function (card) { card.hidden = false; });
    openTabs = { review: true, subagent: true };
    /* 面板宽是壳状态（跨会话保持），但换状态后仍要让出主区地板：
       侧栏已按状态复位，这里把面板 clamp 回「视口 − 侧栏 − 450」 */
    if (ui.panel) size.panelW = Math.max(PANEL_MIN, Math.min(size.panelW, panelBudget()));

    if (refs.protoCurrent) refs.protoCurrent.textContent = entry.label;
    $$("#protoGrid .proto__state").forEach(function (btn) {
      btn.setAttribute("aria-pressed", String(btn.dataset.state === id));
    });
    render();

    if (entry.scrollTo) {
      var el = document.querySelector(entry.scrollTo);
      if (el && el.scrollIntoView) el.scrollIntoView({ block: "center" });
    } else if (refs.scroller) {
      refs.scroller.scrollTop = 0;
    }
  }

  /* ------------------------------------------------------------------ *
   * 权限模式 chip + 菜单（§3.11）
   * ------------------------------------------------------------------ */
  var MODE_LABEL = { plan: "计划", confirm: "变更前确认", autoedit: "自动编辑", full: "全自动" };

  function syncPermChip() {
    if (refs.permChipLabel) refs.permChipLabel.textContent = MODE_LABEL[ui.permMode] || ui.permMode;
    if (refs.permChip) {
      refs.permChip.setAttribute("aria-label", "权限模式：" + (MODE_LABEL[ui.permMode] || ui.permMode));
    }
    /* 危险档位旁挂 ⓘ：不加色、不加动效 */
    if (refs.permInfo) refs.permInfo.hidden = ui.permMode !== "full";
    $$("#permMenu .perm-menu__row").forEach(function (row) {
      var on = row.dataset.mode === ui.permMode;
      row.setAttribute("aria-selected", String(on));
      row.setAttribute("aria-checked", String(on));
    });
  }

  function initPerm() {
    if (refs.permChip) {
      refs.permChip.addEventListener("click", function (event) {
        event.stopPropagation();
        ui.perm = !ui.perm;
        render();
      });
    }
    if (refs.permMenu) {
      refs.permMenu.addEventListener("click", function (event) { event.stopPropagation(); });
      $$("#permMenu .perm-menu__row").forEach(function (row) {
        row.addEventListener("click", function () {
          ui.permMode = row.dataset.mode;
          ui.perm = false;          /* 选完就收 */
          render();
        });
      });
    }
  }

  /* ------------------------------------------------------------------ *
   * 工作面板页签
   * ------------------------------------------------------------------ */
  function initPanelTabs() {
    $$("[data-tab-select]").forEach(function (btn) {
      btn.addEventListener("click", function () {
        ui.panel = true;
        ui.panelTab = btn.dataset.tabSelect;
        openTabs[btn.dataset.tabSelect] = true;
        render();
      });
    });
    $$("[data-tab-close]").forEach(function (btn) {
      btn.addEventListener("click", function (event) {
        event.stopPropagation();
        var name = btn.dataset.tabClose;
        openTabs[name] = false;
        if (ui.panelTab === name) {
          ui.panelTab = openTabs.review ? "review" : (openTabs.subagent ? "subagent" : null);
        }
        render();
      });
    });
    var openSubagent = $("[data-open-subagent]");
    if (openSubagent) {
      openSubagent.addEventListener("click", function () {
        /* 子 Agent 卡是入口：直接切到它的转录页签 */
        ui.panel = true;
        ui.panelTab = "subagent";
        openTabs.subagent = true;
        render();
      });
    }
    /* 启动器里已实现的条目能开页签；未实现的条目显式禁用（不做假处理） */
    var launcherReview = $('[data-launcher-open="review"]');
    if (launcherReview) {
      launcherReview.addEventListener("click", function () {
        ui.panelTab = "review";
        openTabs.review = true;
        render();
      });
    }
    var launcherSub = $('[data-launcher-open="subagent"]');
    if (launcherSub) {
      launcherSub.addEventListener("click", function () {
        ui.panelTab = "subagent";
        openTabs.subagent = true;
        render();
      });
    }
  }

  /* ------------------------------------------------------------------ *
   * 通知浮层：全部已读 / 清空 / 未读筛选
   * ------------------------------------------------------------------ */
  function applyNotifyFilter() {
    if (!refs.notifyList) return;
    var rows = $$("#notifyList .notify");
    var empty = refs.notifyList.querySelector(".notify-empty");
    var visible = 0;
    rows.forEach(function (row) {
      var unread = row.dataset.unread === "true";
      var show = ui.notifyFilter === "all" || unread;
      row.hidden = !show;
      if (show) visible++;
    });
    if (empty) empty.hidden = !(rows.length === 0 || visible === 0);
  }

  function initNotify() {
    if (refs.notifyReadAll) {
      refs.notifyReadAll.addEventListener("click", function () {
        $$("#notifyList .notify").forEach(function (row) { row.dataset.unread = "false"; });
        applyNotifyFilter();
      });
    }
    if (refs.notifyClear) {
      refs.notifyClear.addEventListener("click", function () {
        if (refs.notifyList) refs.notifyList.innerHTML = '<div class="notify-empty">暂无通知</div>';
        applyNotifyFilter();
      });
    }
    var seg = refs.notifyPopover && refs.notifyPopover.querySelector(".notify-popover__seg");
    if (seg) {
      $$(".segmented__item", seg).forEach(function (btn) {
        btn.addEventListener("click", function () {
          $$(".segmented__item", seg).forEach(function (b) { b.setAttribute("aria-selected", "false"); });
          btn.setAttribute("aria-selected", "true");
          ui.notifyFilter = btn.textContent.trim() === "未读" ? "unread" : "all";
          applyNotifyFilter();
        });
      });
    }
  }

  /* ------------------------------------------------------------------ *
   * 命令面板：键盘上下移动高亮、Esc 关闭
   * ------------------------------------------------------------------ */
  var paletteIndex = 0;

  function paletteItems() { return $$("#paletteResults [data-palette-item]"); }

  function highlightPalette(next) {
    var items = paletteItems();
    if (!items.length) return;
    paletteIndex = Math.max(0, Math.min(items.length - 1, next));
    items.forEach(function (item, i) { item.setAttribute("aria-selected", String(i === paletteIndex)); });
    var el = items[paletteIndex];
    if (el && el.scrollIntoView) el.scrollIntoView({ block: "nearest" });
  }

  function initPalette() {
    paletteItems().forEach(function (item, i) {
      item.addEventListener("mouseenter", function () { highlightPalette(i); });
    });
    highlightPalette(0);
    if (refs.paletteInput) {
      refs.paletteInput.addEventListener("input", function () {
        var q = refs.paletteInput.value.trim().toLowerCase();
        paletteItems().forEach(function (item) {
          item.hidden = q !== "" && item.textContent.toLowerCase().indexOf(q) === -1;
        });
        $$("#paletteResults .palette__group-label").forEach(function (label) {
          var kept = false;
          var node = label.nextElementSibling;
          while (node && !node.classList.contains("palette__group-label")) {
            if (node.matches("[data-palette-item]") && !node.hidden) kept = true;
            node = node.nextElementSibling;
          }
          label.hidden = !kept;
        });
        highlightPalette(0);
      });
    }
  }

  /* ------------------------------------------------------------------ *
   * 复制按钮：能取到源文本就写剪贴板（真实行为），取不到就不假装成功
   * ------------------------------------------------------------------ */
  function copyText(text) {
    if (navigator.clipboard && navigator.clipboard.writeText) {
      navigator.clipboard.writeText(text).catch(function () {});
      return;
    }
    var ta = document.createElement("textarea");
    ta.value = text;
    ta.setAttribute("readonly", "");
    ta.style.position = "fixed";
    ta.style.opacity = "0";
    document.body.appendChild(ta);
    ta.select();
    try { document.execCommand("copy"); } catch (e) { /* file:// 下不可用则保持无操作 */ }
    document.body.removeChild(ta);
  }

  function initCopyButtons() {
    $$(".copy-btn").forEach(function (btn) {
      btn.addEventListener("click", function () {
        var host = btn.closest(".code-block, .tool, .msg, .permission");
        var source = host && host.querySelector("pre code, .tool__detail, .permission__well");
        copyText(source ? source.textContent.trim() : "");
      });
    });
  }

  /* ------------------------------------------------------------------ *
   * 提交框自动增长（1 行起，最高 180px）
   * ------------------------------------------------------------------ */
  function initComposer() {
    var el = refs.composerInput;
    if (!el) return;
    var grow = function () {
      el.style.height = "auto";
      el.style.height = Math.min(el.scrollHeight, 180) + "px";
    };
    el.addEventListener("input", grow);
    grow();
  }

  /* ------------------------------------------------------------------ *
   * 明暗主题
   * ------------------------------------------------------------------ */
  function setTheme(theme) {
    root.dataset.theme = theme;
    $$("[data-theme]").forEach(function (btn) {
      if (btn.classList.contains("segmented__item")) {
        btn.setAttribute("aria-selected", String(btn.dataset.theme === theme));
      }
    });
  }

  /* ------------------------------------------------------------------ *
   * 原型切换器
   * ------------------------------------------------------------------ */
  function initProto() {
    if (refs.protoGrid) {
      STATES.forEach(function (s) {
        var btn = document.createElement("button");
        btn.type = "button";
        btn.className = "proto__state";
        btn.dataset.state = s.id;
        btn.textContent = s.label;
        btn.setAttribute("aria-pressed", String(s.id === state));
        btn.addEventListener("click", function () { applyState(s.id); });
        refs.protoGrid.appendChild(btn);
      });
      if (refs.protoCount) refs.protoCount.textContent = STATES.length + " 个状态";
    }
    if (refs.protoPill) {
      refs.protoPill.addEventListener("click", function () {
        var open = refs.proto.classList.toggle("is-open");
        refs.protoPill.setAttribute("aria-expanded", String(open));
      });
    }
    $$("#proto [data-theme]").forEach(function (btn) {
      btn.addEventListener("click", function () { setTheme(btn.dataset.theme); });
    });
  }

  /* ------------------------------------------------------------------ *
   * 外壳交互
   * ------------------------------------------------------------------ */
  function closeTransient() {
    var changed = false;
    ["palette", "confirm", "ctx", "notify", "perm"].forEach(function (key) {
      if (ui[key]) { ui[key] = false; changed = true; }
    });
    if (changed) render();
  }

  function initShell() {
    if (refs.sidebarCollapse) {
      refs.sidebarCollapse.addEventListener("click", function () { setCollapsed(true); persistSizes(); });
    }
    if (refs.sidebarExpand) {
      refs.sidebarExpand.addEventListener("click", function () { setCollapsed(false); persistSizes(); });
    }
    if (refs.panelToggle) {
      refs.panelToggle.addEventListener("click", function () {
        ui.panel = !ui.panel;
        if (ui.panel && ui.panelTab === null && (openTabs.review || openTabs.subagent)) {
          ui.panelTab = openTabs.review ? "review" : "subagent";
        }
        ui.panelMax = ui.panelMax && ui.panel;
        render();
      });
    }
    if (refs.panelMax) {
      refs.panelMax.addEventListener("click", function () {
        ui.panelMax = !ui.panelMax;
        render();
      });
    }
    /* 「+」：有页签 → 回到启动器空态；没有页签 → 重开审阅 */
    var plus = refs.panel ? refs.panel.querySelector(".workpanel__header .icon-btn[title='新建标签页']") : null;
    if (plus) {
      plus.addEventListener("click", function () {
        ui.panel = true;
        if (ui.panelTab !== null) {
          openTabs = { review: false, subagent: false };
          ui.panelTab = null;
        } else {
          openTabs.review = true;
          ui.panelTab = "review";
        }
        render();
      });
    }
    if (refs.ctxBtn) {
      refs.ctxBtn.addEventListener("click", function (event) {
        event.stopPropagation();
        ui.ctx = !ui.ctx;
        if (ui.ctx) { ui.notify = false; ui.perm = false; }
        render();
      });
    }
    if (refs.notifyBell) {
      refs.notifyBell.addEventListener("click", function (event) {
        event.stopPropagation();
        ui.notify = !ui.notify;
        if (ui.notify) { ui.ctx = false; ui.perm = false; }
        render();
      });
    }
    if (refs.ctxPopover) refs.ctxPopover.addEventListener("click", function (event) { event.stopPropagation(); });
    if (refs.notifyPopover) refs.notifyPopover.addEventListener("click", function (event) { event.stopPropagation(); });

    if (refs.overlayPalette) {
      refs.overlayPalette.addEventListener("click", function (event) {
        if (event.target === refs.overlayPalette) { ui.palette = false; render(); }
      });
    }
    if (refs.overlayConfirm) {
      /* 取消 / 确认卸载都只关弹窗：原型不伪造卸载结果 */
      $$("#overlayConfirm [data-confirm]").forEach(function (btn) {
        btn.addEventListener("click", function () { ui.confirm = false; render(); });
      });
      refs.overlayConfirm.addEventListener("click", function (event) {
        if (event.target === refs.overlayConfirm) { ui.confirm = false; render(); }
      });
    }

    /* 决策卡三键：只收起卡片，不伪造审批写回 */
    $$("[data-decision]").forEach(function (btn) {
      btn.addEventListener("click", function () {
        var card = btn.closest(".permission");
        if (card) card.hidden = true;
      });
    });

    /* 原型切换器自身的点击不算「点外部」，否则切到 S5/S6/S7/S8 时会被立刻关掉 */
    document.addEventListener("click", function (event) {
      if (event.target.closest("#proto")) return;
      closeTransient();
    });

    document.addEventListener("keydown", function (event) {
      var mod = event.ctrlKey || event.metaKey;
      if (event.key === "Escape") {
        if (refs.proto.classList.contains("is-open")) {
          refs.proto.classList.remove("is-open");
          refs.protoPill.setAttribute("aria-expanded", "false");
          return;
        }
        closeTransient();
        return;
      }
      if (mod && event.key.toLowerCase() === "k") {
        event.preventDefault();
        ui.palette = !ui.palette;
        render();
        if (ui.palette && refs.paletteInput) refs.paletteInput.focus();
        return;
      }
      if (ui.palette && (event.key === "ArrowDown" || event.key === "ArrowUp")) {
        event.preventDefault();
        highlightPalette(paletteIndex + (event.key === "ArrowDown" ? 1 : -1));
      }
    });

    window.addEventListener("resize", onWindowResize);
  }

  /* ------------------------------------------------------------------ *
   * 滚动条：滚动中显示（tokens.css 第 8 节的三态之一）
   * ------------------------------------------------------------------ */
  function initScrollbars() {
    $$(".scroll-y").forEach(function (el) {
      var timer = null;
      el.addEventListener("scroll", function () {
        el.dataset.scrolling = "true";
        window.clearTimeout(timer);
        timer = window.setTimeout(function () { el.dataset.scrolling = "false"; }, 700);
      });
    });
  }

  /* ------------------------------------------------------------------ *
   * 启动
   * ------------------------------------------------------------------ */
  function boot() {
    loadTokens();
    restore();
    size.sidebarW = Math.max(SIDEBAR_MIN, Math.min(size.sidebarW, sidebarMax()));
    size.panelW = Math.max(PANEL_MIN, Math.min(size.panelW, panelBudgetNoSidebar()));
    /* 恢复的面板宽也必须过三栏预算：装不下就让侧栏先折叠，主区地板是硬的 */
    if (size.panelW > panelBudget()) {
      ui.collapsed = true;
      size.panelW = Math.max(PANEL_MIN, Math.min(size.panelW, panelBudgetNoSidebar()));
    }
    bindGrip(refs.sidebarGrip, "sidebar");
    bindGrip(refs.panelGrip, "panel");
    registerCollapsibles();
    initNotify();
    initPalette();
    initPerm();
    initPanelTabs();
    initCopyButtons();
    initComposer();
    initProto();
    initShell();
    initScrollbars();
    setTheme(root.dataset.theme || "light");
    applyState(state);
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", boot);
  } else {
    boot();
  }
})();
