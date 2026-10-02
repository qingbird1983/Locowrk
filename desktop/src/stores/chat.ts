// 聊天状态（DESIGN.md §10.2：store 管状态，组件只渲染）。
// UI 事件是会话日志的投影（后端 runner.rs），本 store 只消费不生产。
import { defineStore } from "pinia";
import {
  chat,
  sessionAssociate,
  sessionDelete,
  sessionExport,
  sessionHistory,
  sessionList,
  sessionRename,
  pickSavePath,
  subscribeAgentEvents,
} from "../lib/api";
import { clip, compactNumber } from "../lib/format";
import { toolAction } from "../lib/tool-summary";
import { i18n } from "../i18n";
import type { AgentEvent, SessionMeta } from "../types";

/** §3.18 动作名（审批记录条用）；表外工具回落工具 id。 */
function actionLabel(tool: string): string {
  const a = toolAction(tool);
  return a ? i18n.global.t(a.key, a.params ?? {}) : tool;
}

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
  /** 实时到达的时刻（Date.now()）；历史重放无此项（§3.7 时长只在实时
   * 轮次显示，重放不虚构时序）。 */
  ts?: number;
  doneTs?: number;
};

export type ChatItem =
  | { id: number; kind: "user"; text: string; ts?: number }
  | { id: number; kind: "assistant"; text: string; ts?: number }
  | ToolItem
  /** 思考行（§3.9 模式先行；数据源 = 推理流事件，后端待接入 §8.2——
   * 本批不发明占位事件，等通道有了这里就是消费端）。 */
  | { id: number; kind: "thinking"; text: string }
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
    /** 会话列表（⑲，session_list mtime 降序）：切换器数据源。 */
    sessions: [] as SessionMeta[],
    /** 上下文用量（进度条投影；agent://context-usage 驱动）。windowTokens
     * 0 = 窗口未设置（比例段隐藏）。 */
    contextUsage: null as { used: number; threshold: number; windowTokens: number } | null,
    /** 系统消息"查看详情"的内容（id → 文本，如压缩摘要预览）。 */
    noticeDetails: {} as Record<number, string>,
    /** 历史重放中（switchSession / init 恢复）：不记时间戳、工具行不吃
     * 「默认展开」设置（§3.8 历史重放默认折叠）。 */
    replaying: false,
  }),
  actions: {
    async init() {
      await subscribeAgentEvents((e) => this.onEvent(e));
      // ㉕·对话区①（用户裁定）：启动落首页空态——不再自动接最近会话，
      // 继续哪条由用户从侧栏点开；列表照常拉（侧栏要有内容）。
      await this.refreshSessions();
    },
    /** 拉会话列表（⑲）：init 与每轮收束后刷新。 */
    async refreshSessions() {
      try {
        this.sessions = await sessionList();
      } catch {
        /* 列表拉取失败不打断聊天 */
      }
    },
    /** 切换会话（⑲）：清视图 → 全量投影重放（恢复气泡/工具卡/系统
     * 通知）。运行中不允许切（会话锁内轮次未收束，投影会半截）。 */
    async switchSession(id: string) {
      if (id === this.sessionId || this.running) return;
      this.sessionId = id;
      this.items = [];
      this.steps = null;
      this.contextUsage = null;
      this.noticeDetails = {};
      const events = await sessionHistory(id).catch(() => [] as AgentEvent[]);
      this.replaying = true;
      try {
        for (const e of events) this.onEvent(e);
      } finally {
        this.replaying = false;
      }
    },
    /** 重命名会话（v2.49 §3.6）：后端旁车标题（trim 后空 = 回落自动
     * 标题），列表层生效。 */
    async renameSession(id: string, title: string) {
      await sessionRename(id, title);
      await this.refreshSessions();
    },
    /** 工作区关联（v2.50 §2.3）：workspace = fsRoots 根目录，null =
     * 移出；后端校验必须是其一根，列表层生效。 */
    async associateSession(id: string, workspace: string | null) {
      await sessionAssociate(id, workspace);
      await this.refreshSessions();
    },
    /** 删除会话（v2.49）：后端忙锁守卫（运行中默认拒绝，开发者开关可
     * 强制）。删的是当前会话 = 回首页空态（newSession 语义，首条消息
     * 前不产生后端状态）。 */
    async deleteSession(id: string) {
      await sessionDelete(id);
      if (id === this.sessionId) this.newSession();
      await this.refreshSessions();
    },
    /** 导出会话（v2.49）：原生存储对话框 → Markdown 转录；取消 = null。 */
    async exportSession(id: string): Promise<string | null> {
      const meta = this.sessions.find((s) => s.id === id);
      const base =
        (meta?.title ?? id)
          .replace(/["\/:*?<>|]/g, "_")
          .trim()
          .slice(0, 40) || id;
      const path = await pickSavePath(`${base}.md`);
      if (!path) return null;
      return sessionExport(id, path);
    },
    /** 新会话（⑲）：前端生成 id 清空视图；首条消息发出前不创建任何
     * 后端状态（空会话不落盘不进列表，§3.6）。 */
    newSession() {
      if (this.running) return;
      this.sessionId = `desktop-${Date.now()}`;
      this.items = [];
      this.steps = null;
      this.contextUsage = null;
      this.noticeDetails = {};
    },
    onEvent(e: AgentEvent) {
      switch (e.kind) {
        case "user_message":
          // ㉕·fix 重放恢复用户气泡（此前投影漏 UserMessage，重启后历史
          // 会话只剩模型回复）。实时轮发送即本地回显，本事件不实时到达。
          this.items.push({
            id: nextId++,
            kind: "user",
            text: e.content,
            ...(this.replaying ? {} : { ts: Date.now() }),
          });
          break;
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
            ...(this.replaying ? {} : { ts: Date.now() }),
          });
          if (e.tool === "subagent_run") {
            // ⑮ 派生告知（SystemNotice）：子 Agent 内部活动不投影（黑盒），
            // 其审批请求走 approval-request 通道并带标识
            const prompt =
              typeof (e.args as { prompt?: unknown })?.prompt === "string"
                ? ((e.args as { prompt: string }).prompt)
                : "";
            this.pushNotice("notice.subagentStarted", { prompt: clip(prompt, 80) });
          }
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
            if (!this.replaying) item.doneTs = Date.now();
            if (item.tool === "subagent_run") {
              // ⑮ 收束告知：摘要回父会话；预算耗尽/超时/失败在
              // status/summary 里如实呈现（status 详见工具卡结果 JSON）
              const out = (e.output ?? {}) as { summary?: unknown };
              const summary =
                e.error ?? (typeof out.summary === "string" ? out.summary : "");
              if (summary) {
                this.pushNotice("notice.subagentDone", { summary: clip(summary, 120) });
              }
            }
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
        case "approval_resolved":
          // ㉕·对话区① §3.17 决策记录条：投影唯一来源（实时增量与历史
          // 重放同源，无双条）；"做过什么决定"是可审计事实。被拒调用的
          // 失败工具行由 tool_result 分支自理，这里只补审计陈述。
          this.pushNotice(
            e.allowed ? "notice.approvalAllowed" : "notice.approvalDenied",
            { action: actionLabel(e.tool) },
          );
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
        this.items.push({
          id: nextId++,
          kind: "assistant",
          text: delta,
          ...(this.replaying ? {} : { ts: Date.now() }),
        });
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
      this.items.push({ id: nextId++, kind: "user", text: t, ts: Date.now() });
      this.running = true;
      this.steps = null;
      try {
        await chat(this.sessionId, t);
      } catch (err) {
        // 失败轮次不发 turn_done（后端约定），在这里收尾
        this.items.push({ id: nextId++, kind: "error", text: String(err) });
      } finally {
        this.running = false;
        // ⑲ 新会话首聊后进列表（列表更新不阻塞输入）
        void this.refreshSessions();
      }
    },
  },
});
