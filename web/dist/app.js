// node_modules/mygo-runtime/dist/index.js
function runtime() {
  const rt = globalThis.mygo;
  if (!rt)
    throw new Error("mygo: runtime not found; is this page running inside a MyGo window?");
  return rt;
}
function call(method, ...args) {
  try {
    return runtime().call(method, ...args);
  } catch (err) {
    return Promise.reject(err);
  }
}
function on(name, listener) {
  return runtime().on(name, listener);
}
function once(name, listener) {
  return runtime().once(name, listener);
}
function event(name) {
  return {
    name,
    on: (listener) => on(name, listener),
    once: (listener) => once(name, listener)
  };
}

// src/mygo.ts
var Desktop = {
  /** History returns the transcript as page DTOs. */
  history(sessionID) {
    return call("Desktop.History", sessionID);
  },
  /** Info returns the defaults shown at startup. */
  info() {
    return call("Desktop.Info");
  },
  /** Models lists the models offered in the picker. */
  models() {
    return call("Desktop.Models");
  },
  /** NewSession creates a session and selects it. */
  newSession(cwd, model, thinking) {
    return call("Desktop.NewSession", cwd, model, thinking);
  },
  /** OpenSession resumes a stored session and selects it. */
  openSession(path, model, thinking) {
    return call("Desktop.OpenSession", path, model, thinking);
  },
  /** Send runs one turn and blocks until it finishes. */
  send(sessionID, text) {
    return call("Desktop.Send", sessionID, text);
  },
  /** Sessions lists stored sessions for cwd. */
  sessions(cwd) {
    return call("Desktop.Sessions", cwd);
  },
  /** SetModel switches the session's model. */
  setModel(sessionID, model) {
    return call("Desktop.SetModel", sessionID, model);
  },
  /** SetThinking switches the session's thinking level. */
  setThinking(sessionID, level) {
    return call("Desktop.SetThinking", sessionID, level);
  },
  /** Stop cancels the session's active turn. */
  stop(sessionID) {
    return call("Desktop.Stop", sessionID);
  }
};
var events = {
  /** Event carries one project event to the page. */
  harnessEvent: event("harness-event")
};

// src/smoke.ts
var SCENARIOS = ["faux", "real", "tool", "cancel", "long", "restore"];
function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
function nextFrame() {
  return new Promise((resolve) => requestAnimationFrame(() => resolve()));
}
async function waitFor(check, timeoutMs, label) {
  const deadline = performance.now() + timeoutMs;
  while (performance.now() < deadline) {
    if (check()) return;
    await sleep(40);
  }
  throw new Error(`\u8D85\u65F6\u7B49\u5F85\uFF1A${label}`);
}
function longMessages(count) {
  const messages = [];
  for (let i = 0; i < count; i++) {
    const assistant = i % 2 === 1;
    messages.push({
      id: `long-${i}`,
      role: assistant ? "assistant" : "user",
      text: assistant ? `\u7B2C ${i} \u6761\u81EA\u52A8\u751F\u6210\u7684\u957F\u4F1A\u8BDD\u56DE\u7B54\uFF0C\u7528\u6765\u9A8C\u8BC1\u7A97\u53E3\u5316\u6E32\u67D3\u4E0D\u4F1A\u6302\u8F7D\u6574\u6BB5\u6D88\u606F\u3002` : `\u7B2C ${i} \u6761\u81EA\u52A8\u751F\u6210\u7684\u95EE\u9898\u3002`,
      stopReason: assistant ? "stop" : void 0
    });
  }
  return messages;
}
async function scenarioFaux(host, expect) {
  const id = await host.newSession();
  const started = performance.now();
  await host.send("smoke");
  const wanted = expect || "faux assistant reply";
  await waitFor(() => host.text().includes(wanted), 1e4, wanted);
  await nextFrame();
  const stats = host.view.stats();
  return {
    ok: true,
    scenario: "faux",
    session: id,
    text: host.text().slice(-240),
    ms: Math.round(performance.now() - started),
    rows: stats.rows,
    mounted: stats.mounted
  };
}
async function scenarioReal(host) {
  const id = await host.newSession();
  const started = performance.now();
  await host.send("\u8BF7\u53EA\u56DE\u590D\u4E00\u4E2A\u8BCD\uFF1Apong");
  await waitFor(() => host.view.lastAssistantText().length > 0, 2e4, "\u771F\u5B9E\u6A21\u578B\u7684\u56DE\u7B54");
  const text = host.view.lastAssistantText();
  if (!text.trim()) throw new Error("\u56DE\u7B54\u4E3A\u7A7A");
  return {
    ok: true,
    scenario: "real",
    session: id,
    text: text.slice(0, 240),
    ms: Math.round(performance.now() - started),
    rows: host.view.stats().rows,
    mounted: host.view.stats().mounted
  };
}
async function scenarioTool(host) {
  const id = await host.newSession();
  await host.send("\u8BFB\u53D6 smoke-note.txt \u5E76\u603B\u7ED3");
  await waitFor(() => host.view.lastAssistantText().length > 0, 2e4, "\u5DE5\u5177\u540E\u7684\u56DE\u7B54");
  const cards = Array.from(document.querySelectorAll(".tool"));
  if (cards.length === 0) throw new Error("\u6CA1\u6709\u5DE5\u5177\u5361\u7247");
  const states = cards.map((card) => card.querySelector(".tool-state")?.textContent ?? "");
  if (states.some((state) => state.startsWith("\u8FD0\u884C\u4E2D"))) throw new Error(`\u5DE5\u5177\u4ECD\u5728\u8FD0\u884C\uFF1A${states.join(",")}`);
  const first = cards[0];
  if (!first) throw new Error("\u6CA1\u6709\u5DE5\u5177\u5361\u7247");
  const result = first.querySelector(".result")?.textContent ?? "";
  if (!result.trim()) throw new Error("\u5DE5\u5177\u5361\u7247\u6CA1\u6709\u7ED3\u679C");
  await nextFrame();
  const stats = host.view.stats();
  return {
    ok: true,
    scenario: "tool",
    session: id,
    text: result.slice(0, 240),
    tools: cards.length,
    rows: stats.rows,
    mounted: stats.mounted
  };
}
async function scenarioCancel(host) {
  const id = await host.newSession();
  const first = host.send("\u5199\u4E00\u6BB5\u5F88\u957F\u7684\u8BF4\u660E");
  await waitFor(() => host.view.lastAssistantText().length > 0, 2e4, "\u53D6\u6D88\u524D\u7684\u90E8\u5206\u8F93\u51FA");
  const before = host.view.lastAssistantText();
  await host.stop();
  await first;
  await waitFor(() => document.querySelector("#stop")?.disabled === true, 3e3, "\u505C\u6B62\u5B8C\u6210");
  const kept = host.view.lastAssistantText();
  if (!kept) throw new Error("\u53D6\u6D88\u540E\u5DF2\u4EA7\u751F\u5185\u5BB9\u4E22\u5931");
  await host.send("\u518D\u6765\u4E00\u6761");
  await waitFor(() => host.view.lastAssistantText() !== kept, 2e4, "\u53D6\u6D88\u540E\u7684\u4E0B\u4E00\u6761\u56DE\u7B54");
  await nextFrame();
  const stats = host.view.stats();
  return {
    ok: true,
    scenario: "cancel",
    session: id,
    text: kept.slice(0, 120),
    rows: stats.rows,
    mounted: stats.mounted
  };
}
async function scenarioLong(host) {
  const messages = longMessages(5e3);
  const started = performance.now();
  host.view.setMessages(messages);
  await new Promise((resolve) => requestAnimationFrame(() => resolve()));
  const renderMs = Math.round(performance.now() - started);
  const loaded = host.view.stats();
  host.view.scrollToTop();
  await sleep(120);
  const top = host.view.stats();
  host.view.scrollToBottom();
  await sleep(120);
  const bottom = host.view.stats();
  if (loaded.rows !== 5e3) throw new Error(`\u6D88\u606F\u6761\u6570\u4E0D\u5BF9\uFF1A${loaded.rows}`);
  const mounted = Math.max(loaded.mounted, top.mounted, bottom.mounted);
  if (mounted > 200) throw new Error(`\u6302\u8F7D\u8282\u70B9\u8FC7\u591A\uFF1A${mounted}`);
  return {
    ok: true,
    scenario: "long",
    ms: renderMs,
    rows: loaded.rows,
    mounted,
    text: `render=${renderMs}ms rows=${loaded.rows} mounted=${mounted}`
  };
}
async function scenarioRestore(host, expect) {
  const started = performance.now();
  const id = await host.openFirst();
  if (expect) await waitFor(() => host.text().includes(expect), 1e4, `\u6062\u590D\u5185\u5BB9 ${expect}`);
  await nextFrame();
  const stats = host.view.stats();
  return {
    ok: true,
    scenario: "restore",
    session: id,
    text: host.text().slice(-240),
    ms: Math.round(performance.now() - started),
    rows: stats.rows,
    mounted: stats.mounted
  };
}
async function run(host) {
  const params = new URLSearchParams(location.search);
  const scenario = params.get("smoke") ?? "faux";
  const expect = params.get("expect") ?? "";
  switch (scenario) {
    case "real":
      return scenarioReal(host);
    case "tool":
      return scenarioTool(host);
    case "cancel":
      return scenarioCancel(host);
    case "long":
      return scenarioLong(host);
    case "restore":
      return scenarioRestore(host, expect);
    default:
      return scenarioFaux(host, expect);
  }
}
function registerSmoke(host) {
  const target = window;
  target.__locowrkSmoke = async () => {
    try {
      const result = await run(host);
      return JSON.stringify(result);
    } catch (err) {
      const error = err instanceof Error ? err.message : String(err);
      return JSON.stringify({ ok: false, scenario: "unknown", error });
    }
  };
  document.documentElement.dataset.smokeScenarios = SCENARIOS.join(",");
}

// src/transcript.ts
var OVERSCAN = 800;
var MESSAGE_ESTIMATE = 78;
var TOOL_ESTIMATE = 160;
function messageKey(id) {
  return `m:${id}`;
}
function toolKey(id) {
  return `t:${id}`;
}
function element(tag, className) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  return node;
}
var TranscriptView = class {
  constructor(scroller, onFollowChange) {
    this.items = [];
    this.positions = /* @__PURE__ */ new Map();
    this.heights = [];
    this.mounted = /* @__PURE__ */ new Map();
    this.dirty = /* @__PURE__ */ new Set();
    this.start = 0;
    this.end = 0;
    this.follow = true;
    this.frame = 0;
    this.scroller = scroller;
    this.onFollowChange = onFollowChange;
    this.topSpacer = element("div", "vspace");
    this.list = element("div", "vlist");
    this.bottomSpacer = element("div", "vspace");
    scroller.replaceChildren(this.topSpacer, this.list, this.bottomSpacer);
    scroller.addEventListener("scroll", () => this.handleScroll(), { passive: true });
    new ResizeObserver(() => this.schedule()).observe(scroller);
  }
  setMessages(messages) {
    this.items = [];
    this.positions.clear();
    this.heights = [];
    this.mounted.clear();
    this.dirty.clear();
    this.list.replaceChildren();
    this.start = this.end = 0;
    for (const message of messages) {
      if (message.role === "system") continue;
      if (message.role === "toolResult") {
        this.attachToolResult(message.text, Boolean(message.error));
        continue;
      }
      this.appendItem(this.messageItem(message));
      for (const call2 of message.toolCalls ?? []) {
        this.appendItem({
          kind: "tool",
          key: toolKey(call2.id),
          id: call2.id,
          name: call2.name,
          args: call2.arguments,
          result: "",
          state: "done"
        });
      }
    }
    this.follow = true;
    this.notifyFollow();
    this.flush();
  }
  startMessage(id, role, text = "") {
    if (role === "system" || role === "toolResult") return;
    const existing = this.find(messageKey(id));
    if (existing) {
      if (text) this.setMessageText(existing, text);
      return;
    }
    this.appendItem({
      kind: "message",
      key: messageKey(id),
      id,
      role,
      text,
      thinking: "",
      error: ""
    });
    this.schedule();
  }
  appendDelta(id, block, delta) {
    if (!delta) return;
    let item = this.find(messageKey(id));
    if (!item) {
      this.startMessage(id, "assistant");
      item = this.find(messageKey(id));
    }
    if (!item) return;
    if (block === "thinking") item.thinking += delta;
    else item.text += delta;
    this.markDirty(item);
    this.schedule();
  }
  endMessage(id, role, text, thinking, error) {
    if (role === "system" || role === "toolResult") return;
    let item = this.find(messageKey(id));
    if (!item) {
      this.startMessage(id, role, text);
      item = this.find(messageKey(id));
      if (!item) return;
    }
    item.text = text;
    item.thinking = thinking;
    item.error = error;
    this.markDirty(item);
    this.schedule();
  }
  startTool(id, name, args) {
    const existing = this.find(toolKey(id));
    if (existing) {
      existing.name = name;
      existing.args = args;
      existing.state = "running";
      this.markDirty(existing);
      this.schedule();
      return;
    }
    this.appendItem({
      kind: "tool",
      key: toolKey(id),
      id,
      name,
      args,
      result: "",
      state: "running"
    });
    this.schedule();
  }
  appendTool(id, text) {
    const item = this.find(toolKey(id));
    if (!item || !text) return;
    item.result += text;
    this.markDirty(item);
    this.schedule();
  }
  endTool(id, text, isError, durationMs) {
    const item = this.find(toolKey(id));
    if (!item) return;
    item.result = text;
    item.state = isError ? "failed" : "done";
    item.durationMs = durationMs;
    this.markDirty(item);
    this.schedule();
  }
  scrollToBottom() {
    this.follow = true;
    this.notifyFollow();
    this.scroller.scrollTop = this.scroller.scrollHeight;
    this.schedule();
  }
  scrollToTop() {
    this.follow = false;
    this.notifyFollow();
    this.scroller.scrollTop = 0;
    this.schedule();
  }
  lastAssistantText() {
    for (let i = this.items.length - 1; i >= 0; i--) {
      const item = this.items[i];
      if (!item) continue;
      if (item.kind === "message" && item.role === "assistant" && item.text) return item.text;
    }
    return "";
  }
  stats() {
    return { rows: this.items.length, mounted: this.mounted.size, totalHeight: this.totalHeight() };
  }
  attachToolResult(text, failed) {
    for (const item of this.items) {
      if (item.kind !== "tool" || item.result !== "") continue;
      item.result = text;
      item.state = failed ? "failed" : "done";
      this.markDirty(item);
      return;
    }
  }
  messageItem(message) {
    return {
      kind: "message",
      key: messageKey(message.id),
      id: message.id,
      role: message.role,
      text: message.text,
      thinking: message.thinking ?? "",
      error: message.error ?? ""
    };
  }
  appendItem(item) {
    this.positions.set(item.key, this.items.length);
    this.items.push(item);
    this.heights.push(item.kind === "tool" ? TOOL_ESTIMATE : MESSAGE_ESTIMATE);
  }
  find(key) {
    const index = this.positions.get(key);
    return index === void 0 ? void 0 : this.items[index];
  }
  setMessageText(item, text) {
    item.text = text;
    this.markDirty(item);
    this.schedule();
  }
  markDirty(item) {
    this.dirty.add(item.key);
  }
  handleScroll() {
    const nearBottom = this.distanceFromBottom() < 80;
    if (nearBottom !== this.follow) {
      this.follow = nearBottom;
      this.notifyFollow();
    }
    this.schedule();
  }
  notifyFollow() {
    this.onFollowChange?.(this.follow);
  }
  distanceFromBottom() {
    return this.scroller.scrollHeight - this.scroller.scrollTop - this.scroller.clientHeight;
  }
  schedule() {
    if (this.frame) return;
    this.frame = requestAnimationFrame(() => {
      this.frame = 0;
      this.flush();
    });
  }
  flush() {
    const total = this.totalHeight();
    if (this.items.length === 0) {
      this.list.replaceChildren();
      this.mounted.clear();
      this.start = this.end = 0;
      this.topSpacer.style.height = "0px";
      this.bottomSpacer.style.height = "0px";
      return;
    }
    const viewport = this.scroller.clientHeight || 600;
    const anchor = this.follow ? Math.max(0, total - viewport) : this.scroller.scrollTop;
    const start = this.indexAt(anchor - OVERSCAN);
    const end = Math.min(this.items.length, this.indexAt(anchor + viewport + OVERSCAN) + 1);
    if (start !== this.start || end !== this.end) {
      this.renderWindow(start, end);
    } else {
      this.refreshMounted();
      this.measure();
    }
    this.updateSpacers();
    if (this.follow) this.stickToBottom();
  }
  stickToBottom() {
    const target = this.scroller.scrollHeight - this.scroller.clientHeight;
    if (target < 0) return;
    if (Math.abs(this.scroller.scrollTop - target) > 1) this.scroller.scrollTop = target;
  }
  renderWindow(start, end) {
    const beforeTop = this.topSpacer.offsetHeight;
    const keep = this.scroller.scrollTop;
    for (const [key, row] of this.mounted) {
      const index = this.positions.get(key);
      if (index === void 0 || index < start || index >= end) {
        row.el.remove();
        this.mounted.delete(key);
      }
    }
    for (let i = start; i < end; i++) {
      const item = this.items[i];
      if (!item) continue;
      let row = this.mounted.get(item.key);
      if (!row) {
        row = this.createRow(item);
        this.mounted.set(item.key, row);
      }
      this.updateRow(row, item);
      this.list.append(row.el);
    }
    this.start = start;
    this.end = end;
    this.dirty.clear();
    this.measure();
    this.updateSpacers();
    const shift = this.topSpacer.offsetHeight - beforeTop;
    if (!this.follow && shift !== 0) this.scroller.scrollTop = keep + shift;
  }
  refreshMounted() {
    if (this.dirty.size === 0) return;
    for (const key of this.dirty) {
      const index = this.positions.get(key);
      const row = this.mounted.get(key);
      const item = index === void 0 ? void 0 : this.items[index];
      if (!row || !item) continue;
      this.updateRow(row, item);
    }
    this.dirty.clear();
  }
  measure() {
    for (let i = this.start; i < this.end; i++) {
      const item = this.items[i];
      if (!item) continue;
      const row = this.mounted.get(item.key);
      if (!row) continue;
      const height = row.el.offsetHeight;
      if (height > 0 && Math.abs((this.heights[i] ?? 0) - height) > 0.5) this.heights[i] = height;
    }
  }
  updateSpacers() {
    let top = 0;
    for (let i = 0; i < this.start; i++) top += this.heights[i] ?? 0;
    let rest = 0;
    for (let i = this.end; i < this.items.length; i++) rest += this.heights[i] ?? 0;
    this.topSpacer.style.height = `${Math.round(top)}px`;
    this.bottomSpacer.style.height = `${Math.round(rest)}px`;
  }
  totalHeight() {
    let total = 0;
    for (const height of this.heights) total += height;
    return total;
  }
  indexAt(y) {
    if (this.items.length === 0) return 0;
    let acc = 0;
    for (let i = 0; i < this.items.length; i++) {
      acc += this.heights[i] ?? 0;
      if (y < acc) return i;
    }
    return this.items.length - 1;
  }
  createRow(item) {
    const el = element("div", "trow");
    if (item.kind === "tool") {
      const details = element("details", "tool");
      details.open = true;
      const summary = element("summary");
      const name = element("span", "tool-name");
      const state = element("span", "tool-state");
      summary.append(name, state);
      const args = element("pre", "args");
      const result = element("pre", "result");
      details.append(summary, args, result);
      el.append(details);
      return { el, result, state };
    }
    const root = element("div", `msg ${item.role}`);
    const role = element("div", "role");
    role.textContent = item.role === "toolResult" ? "tool result" : item.role;
    const body = element("div", "body");
    const thinking = element("div", "thinking");
    root.append(role, body);
    if (item.role === "assistant") root.append(thinking);
    el.append(root);
    return { el, body, thinking };
  }
  updateRow(row, item) {
    if (item.kind === "tool") {
      const name = row.el.querySelector(".tool-name");
      if (name) name.textContent = item.name;
      if (row.state) {
        const suffix = item.durationMs === void 0 ? "" : ` (${item.durationMs} ms)`;
        row.state.textContent = item.state === "running" ? "\u8FD0\u884C\u4E2D\u2026" : item.state === "failed" ? `\u5931\u8D25${suffix}` : `\u5B8C\u6210${suffix}`;
      }
      const args = row.el.querySelector(".args");
      if (args) args.textContent = JSON.stringify(item.args ?? {}, null, 2);
      if (row.result) row.result.textContent = item.result;
      row.el.classList.toggle("failed", item.state === "failed");
      return;
    }
    if (row.body) {
      row.body.textContent = item.error ? `${item.text}
${item.error}`.trim() : item.text;
    }
    if (row.thinking) row.thinking.textContent = item.thinking;
    row.el.classList.toggle("error", Boolean(item.error));
  }
};

// src/main.ts
var transcript = document.getElementById("transcript");
var sessionsList = document.getElementById("sessions");
var promptBox = document.getElementById("prompt");
var composer = document.getElementById("composer");
var sendButton = document.getElementById("send");
var stopButton = document.getElementById("stop");
var newButton = document.getElementById("new-session");
var modelSelect = document.getElementById("model");
var thinkingSelect = document.getElementById("thinking");
var statusLine = document.getElementById("status");
var scrollBottom = document.getElementById("scroll-bottom");
var info = null;
var currentSession = "";
var busy = false;
var rows = [];
var view = new TranscriptView(transcript, (follow) => {
  scrollBottom.hidden = follow;
});
function setBusy(next) {
  busy = next;
  sendButton.disabled = next;
  stopButton.disabled = !next;
  promptBox.disabled = next;
  setStatus(next ? "\u6B63\u5728\u751F\u6210\u2026" : "");
}
function setStatus(text, isError = false) {
  statusLine.textContent = text;
  statusLine.classList.toggle("error", isError);
}
function errorText(err) {
  if (err instanceof Error) return err.message;
  return String(err);
}
function handleEvent(env) {
  if (env.sessionId && currentSession && env.sessionId !== currentSession) return;
  const payload = env.payload ?? {};
  switch (env.kind) {
    case "agent_start":
      setBusy(true);
      break;
    case "agent_end":
      setBusy(false);
      break;
    case "message_start":
      view.startMessage(String(payload.id ?? ""), String(payload.role ?? ""), String(payload.text ?? ""));
      break;
    case "message_update": {
      const id = String(payload.messageId ?? "");
      if (!id) break;
      const block = payload.blockType === "thinking" ? "thinking" : "text";
      view.appendDelta(id, block, String(payload.delta ?? ""));
      break;
    }
    case "message_end":
      view.endMessage(
        String(payload.id ?? ""),
        String(payload.role ?? "assistant"),
        String(payload.text ?? ""),
        String(payload.thinking ?? ""),
        String(payload.error ?? "")
      );
      break;
    case "tool_execution_start":
      view.startTool(String(payload.toolCallId ?? ""), String(payload.toolName ?? ""), payload.arguments);
      break;
    case "tool_execution_update":
      view.appendTool(String(payload.toolCallId ?? ""), String(payload.text ?? ""));
      break;
    case "tool_execution_end":
      view.endTool(
        String(payload.toolCallId ?? ""),
        String(payload.text ?? ""),
        Boolean(payload.isError),
        typeof payload.durationMs === "number" ? payload.durationMs : void 0
      );
      break;
    default:
      break;
  }
}
async function refreshSessions() {
  if (!info) return;
  rows = await Desktop.sessions(info.cwd);
  sessionsList.replaceChildren();
  for (const row of rows) {
    const li = document.createElement("li");
    const button = document.createElement("button");
    button.type = "button";
    const title = document.createElement("span");
    title.textContent = row.id.slice(0, 8);
    const meta = document.createElement("span");
    meta.className = "meta";
    meta.textContent = `${row.timestamp || "unknown"} \xB7 ${row.messages} \u6761`;
    button.append(title, meta);
    button.addEventListener("click", () => void openSession(row.path));
    button.dataset.id = row.id;
    button.classList.toggle("active", row.id === currentSession);
    li.append(button);
    sessionsList.append(li);
  }
}
function markActive() {
  for (const button of sessionsList.querySelectorAll("button")) {
    button.classList.toggle("active", button.dataset.id === currentSession);
  }
}
async function openSession(path) {
  if (!info) return;
  try {
    setStatus("\u6253\u5F00\u4F1A\u8BDD\u2026");
    const id = await Desktop.openSession(path, modelSelect.value, thinkingSelect.value);
    currentSession = id;
    view.setMessages(await Desktop.history(id));
    markActive();
    setStatus("");
  } catch (err) {
    setStatus(`\u6253\u5F00\u5931\u8D25\uFF1A${errorText(err)}`, true);
  }
}
async function newSession() {
  if (!info) throw new Error("\u754C\u9762\u5C1A\u672A\u5C31\u7EEA");
  const id = await Desktop.newSession(info.cwd, modelSelect.value, thinkingSelect.value);
  currentSession = id;
  view.setMessages([]);
  await refreshSessions();
  markActive();
  promptBox.focus();
  return id;
}
async function sendText(text) {
  if (!currentSession) throw new Error("\u6CA1\u6709\u9009\u4E2D\u7684\u4F1A\u8BDD");
  setBusy(true);
  try {
    await Desktop.send(currentSession, text);
    setStatus("");
  } finally {
    setBusy(false);
    await refreshSessions();
    markActive();
  }
}
async function send() {
  const text = promptBox.value.trim();
  if (!text || !currentSession || busy) return;
  promptBox.value = "";
  try {
    await sendText(text);
  } catch (err) {
    setStatus(`\u53D1\u9001\u5931\u8D25\uFF1A${errorText(err)}`, true);
    if (!promptBox.value) promptBox.value = text;
  }
}
composer.addEventListener("submit", (event2) => {
  event2.preventDefault();
  void send();
});
promptBox.addEventListener("keydown", (event2) => {
  if (event2.key === "Enter" && !event2.shiftKey) {
    event2.preventDefault();
    void send();
  }
});
stopButton.addEventListener("click", () => {
  if (!currentSession) return;
  setStatus("\u6B63\u5728\u505C\u6B62\u2026");
  Desktop.stop(currentSession).catch((err) => setStatus(errorText(err), true));
});
newButton.addEventListener("click", () => {
  newSession().catch((err) => setStatus(`\u65B0\u5EFA\u5931\u8D25\uFF1A${errorText(err)}`, true));
});
scrollBottom.addEventListener("click", () => view.scrollToBottom());
modelSelect.addEventListener("change", () => {
  if (currentSession) void Desktop.setModel(currentSession, modelSelect.value);
});
thinkingSelect.addEventListener("change", () => {
  if (currentSession) void Desktop.setThinking(currentSession, thinkingSelect.value);
});
async function init() {
  info = await Desktop.info();
  const models = await Desktop.models();
  modelSelect.replaceChildren();
  for (const model of models) {
    const option = document.createElement("option");
    option.value = model;
    option.textContent = model;
    modelSelect.append(option);
  }
  modelSelect.value = info.model;
  thinkingSelect.value = info.thinking || "off";
  events.harnessEvent.on(handleEvent);
  const credential = info.credentials ? "\u5DF2\u914D\u7F6E" : "\u672A\u914D\u7F6E";
  setStatus(`\u5DE5\u4F5C\u76EE\u5F55\uFF1A${info.cwd} \xB7 \u51ED\u8BC1\uFF1A${credential}`);
  const shell = {
    view,
    newSession,
    openFirst: async () => {
      await refreshSessions();
      const first2 = rows[0];
      if (!first2) throw new Error("\u6CA1\u6709\u5DF2\u4FDD\u5B58\u7684\u4F1A\u8BDD");
      await openSession(first2.path);
      if (!currentSession) throw new Error("\u4F1A\u8BDD\u672A\u80FD\u6253\u5F00");
      return currentSession;
    },
    send: sendText,
    stop: async () => {
      if (currentSession) await Desktop.stop(currentSession);
    },
    text: () => transcript.textContent ?? ""
  };
  await refreshSessions();
  const first = rows[0];
  if (first) await openSession(first.path);
  else promptBox.focus();
  registerSmoke(shell);
}
init().catch((err) => setStatus(`\u542F\u52A8\u5931\u8D25\uFF1A${errorText(err)}`, true));
