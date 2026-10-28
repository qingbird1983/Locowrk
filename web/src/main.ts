import {
  Desktop,
  events,
  type Envelope,
  type Info,
  type Message,
  type SessionRow,
} from "./mygo";

const transcript = document.getElementById("transcript") as HTMLDivElement;
const sessionsList = document.getElementById("sessions") as HTMLUListElement;
const promptBox = document.getElementById("prompt") as HTMLTextAreaElement;
const composer = document.getElementById("composer") as HTMLFormElement;
const sendButton = document.getElementById("send") as HTMLButtonElement;
const stopButton = document.getElementById("stop") as HTMLButtonElement;
const newButton = document.getElementById("new-session") as HTMLButtonElement;
const modelSelect = document.getElementById("model") as HTMLSelectElement;
const thinkingSelect = document.getElementById("thinking") as HTMLSelectElement;
const statusLine = document.getElementById("status") as HTMLDivElement;
const scrollBottom = document.getElementById("scroll-bottom") as HTMLButtonElement;

let info: Info | null = null;
let currentSession = "";
let busy = false;
let follow = true;
let rows: SessionRow[] = [];

const messageNodes = new Map<string, { root: HTMLDivElement; body: HTMLDivElement; thinking: HTMLDivElement }>();
const toolNodes = new Map<string, HTMLDetailsElement>();

function setBusy(next: boolean) {
  busy = next;
  sendButton.disabled = next;
  stopButton.disabled = !next;
  promptBox.disabled = next;
  setStatus(next ? "正在生成…" : "");
}

function setStatus(text: string, isError = false) {
  statusLine.textContent = text;
  statusLine.classList.toggle("error", isError);
}

function nearBottom(): boolean {
  return transcript.scrollHeight - transcript.scrollTop - transcript.clientHeight < 80;
}

function scrollToBottom(force = false) {
  if (force || follow) {
    transcript.scrollTop = transcript.scrollHeight;
  }
}

function appendMessage(role: string, text = ""): { root: HTMLDivElement; body: HTMLDivElement; thinking: HTMLDivElement } {
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

function messageNode(id: string, role: string) {
  let node = messageNodes.get(id);
  if (!node) {
    node = appendMessage(role);
    messageNodes.set(id, node);
  }
  return node;
}

function renderMessages(messages: Message[]) {
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
      node.body.textContent = `${m.text}\n${m.error}`.trim();
    }
    for (const tc of m.toolCalls ?? []) {
      toolCard(tc.id, tc.name, tc.arguments);
    }
  }
  scrollToBottom(true);
}

function toolCard(id: string, name: string, args: unknown): HTMLDetailsElement {
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
  state.textContent = "运行中…";
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

function finishTool(id: string, text: string, isError: boolean, durationMs?: number) {
  const node = toolNodes.get(id);
  if (!node) return;
  const state = node.querySelector("summary span:last-child") as HTMLSpanElement | null;
  if (state) state.textContent = isError ? "失败" : "完成";
  node.classList.toggle("failed", isError);
  const result = node.querySelector(".result") as HTMLPreElement | null;
  if (result) {
    const suffix = durationMs ? `\n(${durationMs} ms)` : "";
    result.textContent = `${text}${suffix}`.trim();
  }
  scrollToBottom();
}

function handleEvent(env: Envelope) {
  if (env.sessionId && currentSession && env.sessionId !== currentSession) return;
  const payload = env.payload as Record<string, unknown> | null;
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
        node.body.textContent = `${node.body.textContent}\n${payload.error}`.trim();
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
        const result = node?.querySelector(".result") as HTMLPreElement | null;
        if (result) result.textContent = `${result.textContent ?? ""}${payload.text ?? ""}`;
      }
      break;
    case "tool_execution_end":
      if (payload) {
        finishTool(String(payload.toolCallId), String(payload.text ?? ""), Boolean(payload.isError), payload.durationMs as number | undefined);
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
    meta.textContent = `${row.timestamp || "unknown"} · ${row.messages} 条`;
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
    button.classList.toggle("active", (button as HTMLButtonElement).dataset.id === currentSession);
  }
}

async function openSession(path: string) {
  if (!info) return;
  try {
    setStatus("打开会话…");
    const id = await Desktop.openSession(path, modelSelect.value, thinkingSelect.value);
    currentSession = id;
    renderMessages(await Desktop.history(id));
    markActive();
    setStatus("");
  } catch (err) {
    setStatus(`打开失败：${errorText(err)}`, true);
  }
}

async function newSession() {
  if (!info) return;
  try {
    setStatus("新建会话…");
    const id = await Desktop.newSession(info.cwd, modelSelect.value, thinkingSelect.value);
    currentSession = id;
    renderMessages([]);
    await refreshSessions();
    markActive();
    setStatus("");
    promptBox.focus();
  } catch (err) {
    setStatus(`新建失败：${errorText(err)}`, true);
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
    setStatus(`发送失败：${errorText(err)}`, true);
  } finally {
    setBusy(false);
    await refreshSessions();
    markActive();
  }
}

function errorText(err: unknown): string {
  if (err instanceof Error) return err.message;
  return String(err);
}

composer.addEventListener("submit", (event) => {
  event.preventDefault();
  void send();
});

promptBox.addEventListener("keydown", (event) => {
  if (event.key === "Enter" && !event.shiftKey) {
    event.preventDefault();
    void send();
  }
});

stopButton.addEventListener("click", () => {
  if (!currentSession) return;
  setStatus("正在停止…");
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
  setStatus(`工作目录：${info.cwd}`);

  await refreshSessions();
  const first = rows[0];
  if (first) {
    await openSession(first.path);
  } else {
    promptBox.focus();
  }
}

function maybeRegisterSmoke() {
  const target = window as unknown as { __locowrkSmoke?: () => Promise<string> };
  target.__locowrkSmoke = async () => {
    try {
      if (!info) info = await Desktop.info();
      const current = info as Info;
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

init().catch((err) => setStatus(`启动失败：${errorText(err)}`, true));
