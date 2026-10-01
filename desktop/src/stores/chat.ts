// 聊天状态（DESIGN.md §10.2：store 管状态，组件只渲染）。
// UI 事件是会话日志的投影（后端 runner.rs），本 store 只消费不生产。
import { defineStore } from "pinia";
import { chat, subscribeAgentEvents } from "../lib/api";
import { compactNumber } from "../lib/format";
import type { AgentEvent } from "../types";

export type ToolItem = {
  id: number;
  kind: "tool";
  callId: string;
  tool: string;
  args: unknown;
  sideEffect: string;
  pluginVersion: string;
  pending: boolean;
  ok: boolean | null;
  output?: unknown;
  error?: string | null;
};

export type ChatItem =
  | { id: number; kind: "user"; text: string }
  | { id: number; kind: "assistant"; text: string }
  | ToolItem
  | { id: number; kind: "error"; text: string }
  /** 会话内系统消息（SystemNotice 模式）：压缩通知 / 已记住 / 审批结果。 */
  | SystemItem;

/**
 * 系统消息是 UI 陈述而非会话数据：存 i18n key，渲染期翻译以跟随语言
 * 切换。key 落在 locales 的 notice.* 区；actions 是动作 id 列表（文案
 * notice.action.*），实际行为由对应批次接线（⑨ 撤销/详情）。
 */
export type SystemItem = {
  id: number;
  kind: "system";
  key: string;
  params?: Record<string, unknown>;
  actions?: string[];
};

let nextId = 1;

export const useChatStore = defineStore("chat", {
  state: () => ({
    sessionId: `desktop-${Date.now()}`,
    items: [] as ChatItem[],
    running: false,
    steps: null as number | null,
    /** 上下文用量（进度条投影；agent://context-usage 驱动）。windowTokens
     * 0 = 窗口未设置（比例段隐藏）。 */
    contextUsage: null as { used: number; threshold: number; windowTokens: number } | null,
    /** 系统消息"查看详情"的内容（id → 文本，如压缩摘要预览）。 */
    noticeDetails: {} as Record<number, string>,
  }),
  actions: {
    async init() {
      await subscribeAgentEvents((e) => this.onEvent(e));
    },
    onEvent(e: AgentEvent) {
      switch (e.kind) {
        case "chunk":
          this.appendAssistant(e.delta);
          break;
        case "tool_call":
          this.items.push({
            id: nextId++,
            kind: "tool",
            callId: e.call_id,
            tool: e.tool,
            args: e.args,
            sideEffect: e.side_effect,
            pluginVersion: e.plugin_version,
            pending: true,
            ok: null,
          });
          break;
        case "tool_result": {
          const item = this.items.find(
            (i): i is ToolItem => i.kind === "tool" && i.callId === e.call_id,
          );
          if (item) {
            item.pending = false;
            item.ok = e.ok;
            item.output = e.output;
            item.error = e.error;
          }
          break;
        }
        case "turn_done":
          this.running = false;
          this.steps = e.steps;
          break;
        case "schedule_fired":
          // ⑭ 定时触发：SystemNotice 告知（无动作按钮；触发的助手回复
          // 以 chunk 跟进。窗口不在位时后端另发系统通知兜底）
          this.pushNotice("notice.scheduleFired", { title: e.title });
          break;
        case "context_usage":
          this.contextUsage = { used: e.used, threshold: e.threshold, windowTokens: e.window_tokens ?? 0 };
          break;
        case "context_compacted": {
          // 压缩系统消息（Context 文档 §3.8.3）：[查看详情][撤销]，
          // 撤销经 context_undo_compact（rewind_to，ChatMessages 接线）
          const id = nextId++;
          const saved = Math.max(0, e.from_tokens - e.to_tokens);
          this.items.push({
            id,
            kind: "system",
            key: "notice.contextCompacted",
            params: { saved: compactNumber(saved), strategy: e.strategy },
            actions: ["details", "undo"],
          });
          if (e.summary_preview) this.noticeDetails[id] = e.summary_preview;
          break;
        }
      }
    },
    appendAssistant(delta: string) {
      const last = this.items[this.items.length - 1];
      if (last && last.kind === "assistant") {
        last.text += delta;
      } else {
        this.items.push({ id: nextId++, kind: "assistant", text: delta });
      }
    },
    /** 系统消息入列（SystemNotice 模式的唯一生产入口；首个生产者 ⑨
     * 压缩通知，随后 ⑪ "已记住"）。 */
    pushNotice(key: string, params?: Record<string, unknown>, actions?: string[]) {
      this.items.push({ id: nextId++, kind: "system", key, params, actions });
    },
    /** 原文系统条（key = "__raw__"，ChatMessages 按 params.text 渲染）：
     * 用于"查看详情"展开压缩摘要等内容型陈述。 */
    pushRawSystem(text: string) {
      this.items.push({
        id: nextId++,
        kind: "system",
        key: "__raw__",
        params: { text },
      });
    },
    async send(text: string) {
      const t = text.trim();
      if (!t || this.running) return;
      this.items.push({ id: nextId++, kind: "user", text: t });
      this.running = true;
      this.steps = null;
      try {
        await chat(this.sessionId, t);
      } catch (err) {
        // 失败轮次不发 turn_done（后端约定），在这里收尾
        this.items.push({ id: nextId++, kind: "error", text: String(err) });
      } finally {
        this.running = false;
      }
    },
  },
});
