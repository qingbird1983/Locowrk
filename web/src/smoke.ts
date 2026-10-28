import type { Message } from "./mygo";
import type { TranscriptView } from "./transcript";

// The smoke hook drives the real page through an acceptance scenario. It is
// registered in every build but only runs when the shell opens the page with
// ?smoke=<scenario>; see docs/design/phase-3/frontend.md §7.

export interface SmokeHost {
  view: TranscriptView;
  newSession(): Promise<string>;
  openFirst(): Promise<string>;
  send(text: string): Promise<void>;
  stop(): Promise<void>;
  text(): string;
}

interface SmokeResult {
  ok: boolean;
  scenario: string;
  session?: string;
  text?: string;
  error?: string;
  ms?: number;
  rows?: number;
  mounted?: number;
  tools?: number;
}

const SCENARIOS = ["faux", "real", "tool", "cancel", "long", "restore", "plugin", "plugin-crash"] as const;

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function nextFrame(): Promise<void> {
  // Hidden web views pause animation frames, so a timer stands in for them.
  return new Promise((resolve) => {
    let done = false;
    const finish = (): void => {
      if (done) return;
      done = true;
      resolve();
    };
    requestAnimationFrame(finish);
    setTimeout(finish, 120);
  });
}

async function waitFor(check: () => boolean, timeoutMs: number, label: string): Promise<void> {
  const deadline = performance.now() + timeoutMs;
  while (performance.now() < deadline) {
    if (check()) return;
    await sleep(40);
  }
  throw new Error(`超时等待：${label}`);
}

function longMessages(count: number): Message[] {
  const messages: Message[] = [];
  for (let i = 0; i < count; i++) {
    const assistant = i % 2 === 1;
    messages.push({
      id: `long-${i}`,
      role: assistant ? "assistant" : "user",
      text: assistant
        ? `第 ${i} 条自动生成的长会话回答，用来验证窗口化渲染不会挂载整段消息。`
        : `第 ${i} 条自动生成的问题。`,
      stopReason: assistant ? "stop" : undefined,
    });
  }
  return messages;
}

async function scenarioFaux(host: SmokeHost, expect: string): Promise<SmokeResult> {
  const id = await host.newSession();
  const started = performance.now();
  await host.send("smoke");
  const wanted = expect || "faux assistant reply";
  await waitFor(() => host.text().includes(wanted), 10000, wanted);
  await nextFrame();
  const stats = host.view.stats();
  return {
    ok: true,
    scenario: "faux",
    session: id,
    text: host.text().slice(-240),
    ms: Math.round(performance.now() - started),
    rows: stats.rows,
    mounted: stats.mounted,
  };
}

async function scenarioReal(host: SmokeHost): Promise<SmokeResult> {
  const id = await host.newSession();
  const started = performance.now();
  await host.send("请只回复一个词：pong");
  await waitFor(() => host.view.lastAssistantText().length > 0, 20000, "真实模型的回答");
  const text = host.view.lastAssistantText();
  if (!text.trim()) throw new Error("回答为空");
  return {
    ok: true,
    scenario: "real",
    session: id,
    text: text.slice(0, 240),
    ms: Math.round(performance.now() - started),
    rows: host.view.stats().rows,
    mounted: host.view.stats().mounted,
  };
}

async function scenarioTool(host: SmokeHost): Promise<SmokeResult> {
  const id = await host.newSession();
  await host.send("读取 smoke-note.txt 并总结");
  await waitFor(() => host.view.lastAssistantText().length > 0, 20000, "工具后的回答");
  await waitFor(() => document.querySelectorAll(".tool").length > 0, 5000, "工具卡片");
  const cards = Array.from(document.querySelectorAll<HTMLElement>(".tool"));
  if (cards.length === 0) throw new Error("没有工具卡片");
  const states = cards.map((card) => card.querySelector(".tool-state")?.textContent ?? "");
  if (states.some((state) => state.startsWith("运行中"))) throw new Error(`工具仍在运行：${states.join(",")}`);
  const first = cards[0];
  if (!first) throw new Error("没有工具卡片");
  const result = first.querySelector(".result")?.textContent ?? "";
  if (!result.trim()) throw new Error("工具卡片没有结果");
  await nextFrame();
  const stats = host.view.stats();
  return {
    ok: true,
    scenario: "tool",
    session: id,
    text: result.slice(0, 240),
    tools: cards.length,
    rows: stats.rows,
    mounted: stats.mounted,
  };
}

async function scenarioPlugin(host: SmokeHost): Promise<SmokeResult> {
  const id = await host.newSession();
  await host.send("读取 smoke-note.txt 并总结");
  await waitFor(() => host.view.lastAssistantText().length > 0, 20000, "工具后的回答");
  await waitFor(() => document.querySelectorAll(".tool").length > 0, 5000, "工具卡片");
  const card = document.querySelector<HTMLElement>(".tool");
  if (!card) throw new Error("没有工具卡片");
  card.click();
  return {
    ok: true,
    scenario: "plugin",
    session: id,
    text: host.text().slice(-160),
    tools: document.querySelectorAll(".tool").length,
    rows: host.view.stats().rows,
    mounted: host.view.stats().mounted,
  };
}

async function scenarioCancel(host: SmokeHost): Promise<SmokeResult> {
  const id = await host.newSession();
  const first = host.send("写一段很长的说明");
  await waitFor(() => host.view.lastAssistantText().length > 0, 20000, "取消前的部分输出");
  const before = host.view.lastAssistantText();
  await host.stop();
  await first;
  await waitFor(() => document.querySelector<HTMLButtonElement>("#stop")?.disabled === true, 3000, "停止完成");
  const kept = host.view.lastAssistantText();
  if (!kept) throw new Error("取消后已产生内容丢失");
  await host.send("再来一条");
  await waitFor(() => host.view.lastAssistantText() !== kept, 20000, "取消后的下一条回答");
  await nextFrame();
  const stats = host.view.stats();
  return {
    ok: true,
    scenario: "cancel",
    session: id,
    text: kept.slice(0, 120),
    rows: stats.rows,
    mounted: stats.mounted,
  };
}

async function scenarioLong(host: SmokeHost): Promise<SmokeResult> {
  const messages = longMessages(5000);
  const started = performance.now();
  host.view.setMessages(messages);
  await nextFrame();
  const renderMs = Math.round(performance.now() - started);
  const loaded = host.view.stats();
  host.view.scrollToTop();
  await sleep(120);
  const top = host.view.stats();
  host.view.scrollToBottom();
  await sleep(120);
  const bottom = host.view.stats();
  if (loaded.rows !== 5000) throw new Error(`消息条数不对：${loaded.rows}`);
  const mounted = Math.max(loaded.mounted, top.mounted, bottom.mounted);
  if (mounted > 200) throw new Error(`挂载节点过多：${mounted}`);
  return {
    ok: true,
    scenario: "long",
    ms: renderMs,
    rows: loaded.rows,
    mounted,
    text: `render=${renderMs}ms rows=${loaded.rows} mounted=${mounted}`,
  };
}

async function scenarioRestore(host: SmokeHost, expect: string): Promise<SmokeResult> {
  const started = performance.now();
  const id = await host.openFirst();
  if (expect) await waitFor(() => host.text().includes(expect), 10000, `恢复内容 ${expect}`);
  await nextFrame();
  const stats = host.view.stats();
  return {
    ok: true,
    scenario: "restore",
    session: id,
    text: host.text().slice(-240),
    ms: Math.round(performance.now() - started),
    rows: stats.rows,
    mounted: stats.mounted,
  };
}

async function run(host: SmokeHost): Promise<SmokeResult> {
  const params = new URLSearchParams(location.search);
  const scenario = params.get("smoke") ?? "faux";
  const expect = params.get("expect") ?? "";
  switch (scenario) {
    case "real":
      return scenarioReal(host);
    case "tool":
      return scenarioTool(host);
    case "plugin":
      return scenarioPlugin(host);
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

export function registerSmoke(host: SmokeHost): void {
  const target = window as unknown as { __locowrkSmoke?: () => Promise<string> };
  target.__locowrkSmoke = async () => {
    try {
      const result = await run(host);
      return JSON.stringify(result);
    } catch (err) {
      const error = err instanceof Error ? err.message : String(err);
      return JSON.stringify({ ok: false, scenario: "unknown", error } satisfies SmokeResult);
    }
  };
  document.documentElement.dataset.smokeScenarios = SCENARIOS.join(",");
}
