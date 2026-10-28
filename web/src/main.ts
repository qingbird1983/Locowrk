import {
  Desktop,
  events,
  type Envelope,
  type Info,
  type SessionRow,
} from "./mygo";
import { registerSmoke, type SmokeHost } from "./smoke";
import { TranscriptView } from "./transcript";

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
let rows: SessionRow[] = [];

const view = new TranscriptView(transcript, (follow) => {
  scrollBottom.hidden = follow;
});

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

function errorText(err: unknown): string {
  if (err instanceof Error) return err.message;
  return String(err);
}

function handleEvent(env: Envelope): void {
  if (env.sessionId && currentSession && env.sessionId !== currentSession) return;
  const payload = (env.payload ?? {}) as Record<string, unknown>;
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
        String(payload.error ?? ""),
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
        typeof payload.durationMs === "number" ? payload.durationMs : undefined,
      );
      break;
    default:
      break;
  }
}

async function refreshSessions(): Promise<void> {
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
    button.addEventListener("click", () => void openSession(row.path));
    button.dataset.id = row.id;
    button.classList.toggle("active", row.id === currentSession);
    li.append(button);
    sessionsList.append(li);
  }
}

function markActive(): void {
  for (const button of sessionsList.querySelectorAll("button")) {
    button.classList.toggle("active", (button as HTMLButtonElement).dataset.id === currentSession);
  }
}

async function openSession(path: string): Promise<void> {
  if (!info) return;
  try {
    setStatus("打开会话…");
    const id = await Desktop.openSession(path, modelSelect.value, thinkingSelect.value);
    currentSession = id;
    view.setMessages(await Desktop.history(id));
    markActive();
    setStatus("");
  } catch (err) {
    setStatus(`打开失败：${errorText(err)}`, true);
  }
}

async function newSession(): Promise<string> {
  if (!info) throw new Error("界面尚未就绪");
  const id = await Desktop.newSession(info.cwd, modelSelect.value, thinkingSelect.value);
  currentSession = id;
  view.setMessages([]);
  await refreshSessions();
  markActive();
  promptBox.focus();
  return id;
}

async function sendText(text: string): Promise<void> {
  if (!currentSession) throw new Error("没有选中的会话");
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

async function send(): Promise<void> {
  const text = promptBox.value.trim();
  if (!text || !currentSession || busy) return;
  promptBox.value = "";
  try {
    await sendText(text);
  } catch (err) {
    setStatus(`发送失败：${errorText(err)}`, true);
    if (!promptBox.value) promptBox.value = text;
  }
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

newButton.addEventListener("click", () => {
  newSession().catch((err) => setStatus(`新建失败：${errorText(err)}`, true));
});
scrollBottom.addEventListener("click", () => view.scrollToBottom());

modelSelect.addEventListener("change", () => {
  if (currentSession) void Desktop.setModel(currentSession, modelSelect.value);
});
thinkingSelect.addEventListener("change", () => {
  if (currentSession) void Desktop.setThinking(currentSession, thinkingSelect.value);
});

async function init(): Promise<void> {
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
  const credential = info.credentials ? "已配置" : "未配置";
  setStatus(`工作目录：${info.cwd} · 凭证：${credential}`);

  const shell: SmokeHost = {
    view,
    newSession,
    openFirst: async () => {
      await refreshSessions();
      const first = rows[0];
      if (!first) throw new Error("没有已保存的会话");
      await openSession(first.path);
      if (!currentSession) throw new Error("会话未能打开");
      return currentSession;
    },
    send: sendText,
    stop: async () => {
      if (currentSession) await Desktop.stop(currentSession);
    },
    text: () => transcript.textContent ?? "",
  };

  await refreshSessions();
  const first = rows[0];
  if (first) await openSession(first.path);
  else promptBox.focus();
  registerSmoke(shell);
}

init().catch((err) => setStatus(`启动失败：${errorText(err)}`, true));
