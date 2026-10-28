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
var follow = true;
var rows = [];
var messageNodes = /* @__PURE__ */ new Map();
var toolNodes = /* @__PURE__ */ new Map();
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
function nearBottom() {
  return transcript.scrollHeight - transcript.scrollTop - transcript.clientHeight < 80;
}
function scrollToBottom(force = false) {
  if (force || follow) {
    transcript.scrollTop = transcript.scrollHeight;
  }
}
function appendMessage(role, text = "") {
  const root = document.createElement("div");
  root.className = `msg ${role}`;
  const label = document.createElement("div");
  label.className = "role";
  label.textContent = role === "toolResult" ? "tool result" : role;
  const body = document.createElement("div");
  body.className = "body";
  body.textContent = text;
  const thinking = document.createElement("div");
  thinking.className = "thinking";
  root.append(label, body);
  if (role === "assistant") root.append(thinking);
  transcript.append(root);
  scrollToBottom();
  return { root, body, thinking };
}
function messageNode(id, role) {
  let node = messageNodes.get(id);
  if (!node) {
    node = appendMessage(role);
    messageNodes.set(id, node);
  }
  return node;
}
function renderMessages(messages) {
  transcript.replaceChildren();
  messageNodes.clear();
  toolNodes.clear();
  for (const m of messages) {
    if (m.role === "system") continue;
    if (m.role === "toolResult") {
      appendMessage("toolResult", m.text);
      continue;
    }
    const node = messageNode(m.id, m.role);
    node.body.textContent = m.text;
    node.thinking.textContent = m.thinking ?? "";
    if (m.error) {
      node.root.classList.add("error");
      node.body.textContent = `${m.text}
${m.error}`.trim();
    }
    for (const tc of m.toolCalls ?? []) {
      toolCard(tc.id, tc.name, tc.arguments);
    }
  }
  scrollToBottom(true);
}
function toolCard(id, name, args) {
  let node = toolNodes.get(id);
  if (node) return node;
  node = document.createElement("details");
  node.className = "tool";
  node.open = true;
  const summary = document.createElement("summary");
  const label = document.createElement("span");
  label.className = "tool-name";
  label.textContent = name;
  const state = document.createElement("span");
  state.textContent = "\u8FD0\u884C\u4E2D\u2026";
  summary.append(label, state);
  const argsNode = document.createElement("pre");
  argsNode.className = "args";
  argsNode.textContent = JSON.stringify(args ?? {}, null, 2);
  const result = document.createElement("pre");
  result.className = "result";
  node.append(summary, argsNode, result);
  node.dataset.state = state.textContent ?? "";
  transcript.append(node);
  toolNodes.set(id, node);
  scrollToBottom();
  return node;
}
function finishTool(id, text, isError, durationMs) {
  const node = toolNodes.get(id);
  if (!node) return;
  const state = node.querySelector("summary span:last-child");
  if (state) state.textContent = isError ? "\u5931\u8D25" : "\u5B8C\u6210";
  node.classList.toggle("failed", isError);
  const result = node.querySelector(".result");
  if (result) {
    const suffix = durationMs ? `
(${durationMs} ms)` : "";
    result.textContent = `${text}${suffix}`.trim();
  }
  scrollToBottom();
}
function handleEvent(env) {
  if (env.sessionId && currentSession && env.sessionId !== currentSession) return;
  const payload = env.payload;
  switch (env.kind) {
    case "agent_start":
      setBusy(true);
      break;
    case "agent_end":
      setBusy(false);
      break;
    case "message_start":
      if (payload) messageNode(String(payload.id), String(payload.role));
      break;
    case "message_update": {
      if (!payload) break;
      const id = String(payload.messageId ?? "");
      if (!id) break;
      const node = messageNode(id, id.startsWith("assistant") ? "assistant" : "assistant");
      if (payload.blockType === "thinking") {
        node.thinking.textContent += String(payload.delta ?? "");
      } else if (payload.blockType === "text") {
        node.body.textContent += String(payload.delta ?? "");
      }
      scrollToBottom();
      break;
    }
    case "message_end": {
      if (!payload) break;
      const id = String(payload.id ?? "");
      const node = messageNode(id, String(payload.role ?? "assistant"));
      node.body.textContent = String(payload.text ?? "");
      node.thinking.textContent = String(payload.thinking ?? "");
      if (payload.error) {
        node.root.classList.add("error");
        node.body.textContent = `${node.body.textContent}
${payload.error}`.trim();
      }
      scrollToBottom();
      break;
    }
    case "tool_execution_start":
      if (payload) toolCard(String(payload.toolCallId), String(payload.toolName), payload.arguments);
      break;
    case "tool_execution_update":
      if (payload) {
        const node = toolNodes.get(String(payload.toolCallId));
        const result = node?.querySelector(".result");
        if (result) result.textContent = `${result.textContent ?? ""}${payload.text ?? ""}`;
      }
      break;
    case "tool_execution_end":
      if (payload) {
        finishTool(String(payload.toolCallId), String(payload.text ?? ""), Boolean(payload.isError), payload.durationMs);
      }
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
    button.addEventListener("click", () => openSession(row.path));
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
    renderMessages(await Desktop.history(id));
    markActive();
    setStatus("");
  } catch (err) {
    setStatus(`\u6253\u5F00\u5931\u8D25\uFF1A${errorText(err)}`, true);
  }
}
async function newSession() {
  if (!info) return;
  try {
    setStatus("\u65B0\u5EFA\u4F1A\u8BDD\u2026");
    const id = await Desktop.newSession(info.cwd, modelSelect.value, thinkingSelect.value);
    currentSession = id;
    renderMessages([]);
    await refreshSessions();
    markActive();
    setStatus("");
    promptBox.focus();
  } catch (err) {
    setStatus(`\u65B0\u5EFA\u5931\u8D25\uFF1A${errorText(err)}`, true);
  }
}
async function send() {
  const text = promptBox.value.trim();
  if (!text || !currentSession || busy) return;
  promptBox.value = "";
  appendMessage("user", text);
  setBusy(true);
  try {
    await Desktop.send(currentSession, text);
    setStatus("");
  } catch (err) {
    setStatus(`\u53D1\u9001\u5931\u8D25\uFF1A${errorText(err)}`, true);
  } finally {
    setBusy(false);
    await refreshSessions();
    markActive();
  }
}
function errorText(err) {
  if (err instanceof Error) return err.message;
  return String(err);
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
newButton.addEventListener("click", () => void newSession());
scrollBottom.addEventListener("click", () => scrollToBottom(true));
transcript.addEventListener("scroll", () => {
  follow = nearBottom();
  scrollBottom.hidden = follow;
});
modelSelect.addEventListener("change", () => {
  if (currentSession) void Desktop.setModel(currentSession, modelSelect.value);
});
thinkingSelect.addEventListener("change", () => {
  if (currentSession) void Desktop.setThinking(currentSession, thinkingSelect.value);
});
maybeRegisterSmoke();
async function init() {
  info = await Desktop.info();
  const models = await Desktop.models();
  modelSelect.replaceChildren();
  for (const m of models) {
    const option = document.createElement("option");
    option.value = m;
    option.textContent = m;
    modelSelect.append(option);
  }
  modelSelect.value = info.model;
  thinkingSelect.value = info.thinking || "off";
  events.harnessEvent.on(handleEvent);
  setStatus(`\u5DE5\u4F5C\u76EE\u5F55\uFF1A${info.cwd}`);
  await refreshSessions();
  const first = rows[0];
  if (first) {
    await openSession(first.path);
  } else {
    promptBox.focus();
  }
}
function maybeRegisterSmoke() {
  const target = window;
  target.__locowrkSmoke = async () => {
    try {
      if (!info) info = await Desktop.info();
      const current = info;
      const id = await Desktop.newSession(current.cwd, current.model, "off");
      currentSession = id;
      await Desktop.send(id, "smoke");
      await new Promise((resolve) => setTimeout(resolve, 100));
      const text = transcript.textContent ?? "";
      const ok = text.includes("faux assistant reply");
      return JSON.stringify({ ok, text, session: id });
    } catch (err) {
      return JSON.stringify({ ok: false, error: errorText(err) });
    }
  };
}
init().catch((err) => setStatus(`\u542F\u52A8\u5931\u8D25\uFF1A${errorText(err)}`, true));
