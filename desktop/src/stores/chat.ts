// 聊天状态（DESIGN.md §10.2：store 管状态，组件只渲染）。
// UI 事件是会话日志的投影（后端 runner.rs），本 store 只消费不生产。
import { defineStore } from "pinia";
import {
  chat,
  agentStop,
  sessionAssociate,
  sessionSetOrganizer,
  sessionDelete,
  sessionExport,
  sessionHistory,
  sessionList,
  sessionRename,
  sessionSetModel,
  sessionSetThinking,
  sessionSetPermissionMode,
  sessionTruncateAfter,
  pickSavePath,
  subscribeAgentEvents,
  subscribeHandoffCreated,
  subscribeSessionTitle,
  subscribeOrganizeDone,
  subscribeOrganizePaused,
  organizeNow,
} from "../lib/api";
import { compactNumber } from "../lib/format";
import { i18n } from "../i18n";
import { useAsksStore } from "./asks";
import { useSettingsStore } from "./settings";
import { useToastStore } from "./toast";
import type { AgentEvent, SessionMeta } from "../types";

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
  | {
      id: number;
      kind: "user";
      text: string;
      ts?: number;
      /** 本轮收束时刻（v2.101，turn_done 的轮界事件投影）：第 0 层计时行
       * 重放读数（「已工作 N 分 N 秒」）的终点；缺省 = 旧日志无（不虚构）。
       * 调度轮没有用户消息，计时行本就无从起算，不受影响。 */
      endTs?: number;
      /** 源事件在会话日志里的位次（§3.34 编辑重发的定位键）。重放消息有；
       * 实时发送的没有（后端按「最后一条用户消息」解析）。 */
      pos?: number;
    }
  | {
      id: number;
      kind: "assistant";
      text: string;
      ts?: number;
    }
  | ToolItem
  /** 思考行（§3.9，v4.11 记账时长）：实时 delta 起点记 ts、下一条过程项
   * 到达时收口 doneTs（`持续了 N 秒`）；历史重放整段无时戳 → 首句摘要。 */
  | {
      id: number;
      kind: "thinking";
      text: string;
      ts?: number;
      doneTs?: number;
    }
  | { id: number; kind: "error"; text: string }
  /** 审批决策记录行（§3.7 v4.11 裁定①）：无卡片底，折叠进过程组为
   * 平铺行（无组时回落兄弟节点）；已允许/已拒绝都是可审计事实。 */
  | { id: number; kind: "decision"; tool: string; allowed: boolean }
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

/** 本轮 LLM 用量聚合（v2.58 §3.12）：llm_usage 事件按轮累计——用户消息
 * / 发送重置，重放同序重建。speed = 最后一步 output/duration（tokens/s）。 */
export type TurnUsage = {
  input: number;
  output: number;
  cacheRead: number;
  cacheWrite: number;
  reasoning: number;
  total: number;
  speed: number | null;
  steps: number;
};

/** 会话累计用量（M0 观测，KV v2 §7）：llm_usage 事件全会话累计——与
 * turnUsage（轮级，user_message / send 重置）不同，只在 switchSession /
 * newSession 重置；实时与重放同源累计，切走切回重建。 */
export type SessionUsage = {
  input: number;
  output: number;
  cacheRead: number;
  cacheWrite: number;
  reasoning: number;
  total: number;
  steps: number;
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
    contextUsage: null as {
      used: number;
      threshold: number;
      windowTokens: number;
      toolsEstTokens: number;
    } | null,
    /** 系统消息"查看详情"的内容（id → 文本，如压缩摘要预览）。 */
    noticeDetails: {} as Record<number, string>,
    /** 历史重放中（switchSession / init 恢复）：不记时间戳、工具行不吃
     * 「默认展开」设置（§3.8 历史重放默认折叠）。 */
    replaying: false,
    /** ㉘·B 当前会话权限模式（null = 未显式设置，显示取设置缺省档）。 */
    permissionMode: null as "plan" | "ask" | "acceptEdits" | "auto" | null,
    /** v2.58 §3.30 当前会话模型覆盖（null = 跟随设置缺省）。 */
    modelOverride: null as { providerId: string; model: string } | null,
    /** 批 B 会话级推理档位（null = 未设置，模型走自家缺省）。 */
    thinkingLevel: null as string | null,
    /** v2.58 §3.30 本轮用量聚合（null = 本轮尚无供应商报告的用量）。 */
    turnUsage: null as TurnUsage | null,
    /** M0 观测（KV v2 §7）：会话累计用量——只随会话切换/新建重置，
     * 实时与重放同源累计。 */
    sessionUsage: null as SessionUsage | null,
    /** M0 观测告警已触发标记（每会话至多一次，随会话重置）。 */
    cacheAlertFired: false,
    /** §3.6 v3.9 项目下新建任务：挂起的归属目标，首聊建会话后消费。 */
    pendingWorkspace: null as string | null,
    /** v4.11 §3.39④ 本轮已请求停止（过程组头行显示「已停止」）；轮次
     * 收束复位。前端记忆，不进会话日志。 */
    stopRequested: false,
    /** v4.11 §3.39② 运行中排队（前端态，不入会话日志）：autoDrain =
     * 当前轮收束后自动派发；手动停止/轮次出错置 false，用户恢复。 */
    queue: [] as { id: number; text: string }[],
    queueDraining: true,
    /** v4.11 §3.39② 正在派发队首（防重入；收束回调里消费）。 */
    dispatching: false,
    /** §3.39② 「立即发送」的打断目标：当前轮被停止收束后立即发送。 */
    preempt: null as string | null,
  }),
  getters: {
    /** M0 观测（KV v2 §7）：会话累计缓存统计。命中率 = cacheRead /
     * input（**分母不含 cacheWrite**——纪律 7，加了会把命中率压低）；
     * 节省 = 命中 × (未命中价 − 命中价)，单价来自设置（元/百万 tokens）。
     * cacheRead 为 0 = 供应商未报告缓存字段或全 miss → null（行不渲染，
     * 宁缺勿假）。 */
    cacheStats(state): {
      hitRate: number;
      savings: number;
      input: number;
      cacheRead: number;
      cacheWrite: number;
      total: number;
      steps: number;
    } | null {
      const u = state.sessionUsage;
      if (!u || u.input <= 0 || u.cacheRead <= 0) return null;
      const chat = useSettingsStore().view?.chat ?? null;
      const delta = chat
        ? Math.max(0, chat.cacheMissPrice - chat.cacheHitPrice)
        : 0;
      return {
        hitRate: Math.round((u.cacheRead / u.input) * 100),
        savings: (u.cacheRead * delta) / 1_000_000,
        input: u.input,
        cacheRead: u.cacheRead,
        cacheWrite: u.cacheWrite,
        total: u.total,
        steps: u.steps,
      };
    },
    /** 当前归属——工作区 chip 与欢迎页问候的**唯一同源真源**（v2.61 用户
     * 裁定「把欢迎语项目名跟对话框下面的项目名关联」）：会话态 = 活动会话
     * 旁车 workspace；首页空态 = 挂起归属。此前问候另有「最近活动会话 /
     * 第一个 fsRoot」回落，chip 显示「选择文件夹」时问候仍挂旧项目名——
     * 回落退役，两边永远同变。null = 未选项目（chip「选择文件夹」+ 问候
     * 不带名）。 */
    currentWorkspace(state): string | null {
      const inSession =
        state.sessions.some((s) => s.id === state.sessionId) || state.running;
      if (inSession) {
        return (
          state.sessions.find((s) => s.id === state.sessionId)?.workspace ??
          null
        );
      }
      return state.pendingWorkspace;
    },
  },
  actions: {
    async init() {
      await subscribeAgentEvents((e) => this.onEvent(e));
      // v2.57 自动标题：旁车落盘后推送（侧栏 + 顶栏即时换名）
      void subscribeSessionTitle((p) => this.applySessionTitle(p.sessionId, p.title));
      // F3：交接文件首次创建 → toast 如实告知一次（已有文件的更新不发生）
      void subscribeHandoffCreated((p) => {
        useToastStore().push(
          "ok",
          i18n.global.t("toast.handoffCreatedTitle"),
          i18n.global.t("toast.handoffCreatedBody", { path: p.path }),
        );
      });
      // M3 §3.37③：整理收束 / 熔断翻转 → SystemNotice（非当前会话忽略，
      // 树文件本身是持久记录；「查看」动作跳记忆页，接线在 ChatTranscript）
      void subscribeOrganizeDone((p) => {
        if (p.sessionId !== this.sessionId) return;
        this.pushNotice(
          p.status === "degraded"
            ? "notice.organizeDoneDegraded"
            : "notice.organizeDone",
          { users: p.users, chapters: p.chapters },
          p.status === "degraded" ? undefined : ["view"],
        );
      });
      void subscribeOrganizePaused((p) => {
        if (p.sessionId !== this.sessionId) return;
        this.pushNotice("notice.organizePaused", {
          rebounds: p.rebounds,
          m: p.m,
        });
      });
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
      // §3.39②：队列是会话级 UI 态，切会话即清（诚实——预览不跨会话漂移）
      this.queue = [];
      this.preempt = null;
      this.stopRequested = false;
      this.dispatching = false;
      this.queueDraining = true;
      // ㉘·B：档位随会话走（无显式设置 = null，显示取设置缺省）
      this.permissionMode =
        this.sessions.find((s) => s.id === id)?.permissionMode ?? null;
      // v2.58：模型覆盖随会话走（成对字段，null = 跟随设置缺省）
      const meta = this.sessions.find((s) => s.id === id);
      this.modelOverride =
        meta?.model && meta?.modelProvider
          ? { providerId: meta.modelProvider, model: meta.model }
          : null;
      this.thinkingLevel = meta?.thinkingLevel ?? null;
      this.turnUsage = null;
      // M0 观测：会话累计随会话切换重置（重放同源重建），告警标记同理
      this.sessionUsage = null;
      this.cacheAlertFired = false;
      this.pendingWorkspace = null; // 挂起归属只属于触发它的那个新会话
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
    /** §3.11 v3.9 工作区 chip 首页语义：直接改挂起归属（首聊自动关联）。 */
    setPendingWorkspace(ws: string | null) {
      this.pendingWorkspace = ws;
    },
    /** 工作区关联（v2.50 §2.3）：workspace = fsRoots 根目录，null =
     * 移出；后端校验必须是其一根，列表层生效。 */
    async associateSession(id: string, workspace: string | null) {
      await sessionAssociate(id, workspace);
      await this.refreshSessions();
    },
    /** 会话整理模式（M4 §3.38①）：legacy | tree；"" = 清除显式档
     * （跟随开发者缺省档）。下一次请求按新档组装，列表刷新生效档。 */
    async setOrganizer(id: string, mode: "legacy" | "tree" | "") {
      await sessionSetOrganizer(id, mode);
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
     * 后端状态（空会话不落盘不进列表，§3.6）。`workspace` = §2.3 v3.9
     * 「在项目下新建任务」：记账挂起，首条消息建会话后自动关联（§3.6）。 */
    newSession(workspace?: string | null) {
      if (this.running) return;
      this.sessionId = `desktop-${Date.now()}`;
      this.items = [];
      this.steps = null;
      this.contextUsage = null;
      this.noticeDetails = {};
      this.queue = [];
      this.preempt = null;
      this.stopRequested = false;
      this.dispatching = false;
      this.queueDraining = true;
      this.permissionMode = null;
      this.modelOverride = null;
      this.thinkingLevel = null;
      this.turnUsage = null;
      this.sessionUsage = null;
      this.cacheAlertFired = false;
      this.pendingWorkspace = workspace ?? null;
    },
    /** v2.58 §3.30 选择模型：会话内 = 写会话级覆盖（设置不动）；首页
     * 空态 = 写设置缺省（切活动档 + 该档模型——"接下来聊什么模型"）。 */
    async setModel(providerId: string, model: string) {
      const inSession = this.sessions.some((s) => s.id === this.sessionId);
      if (!inSession) {
        await useSettingsStore().saveLlm({ active: providerId, model });
        return;
      }
      const prev = this.modelOverride;
      this.modelOverride = { providerId, model };
      try {
        await sessionSetModel(this.sessionId, providerId, model);
      } catch (err) {
        // 写穿失败回滚本地（后端仍按旧模型跑——本地显示必须如实）
        this.modelOverride = prev;
        throw err;
      }
    },
    /** v2.58 清除会话模型覆盖（选择器「跟随默认」行）。 */
    async clearModelOverride() {
      const prev = this.modelOverride;
      this.modelOverride = null;
      try {
        await sessionSetModel(this.sessionId, "", "");
      } catch (err) {
        this.modelOverride = prev;
        throw err;
      }
    },
    /** 批 B §5.2 设置会话推理档位（循环钮）：写旁车 + 缓存，下一请求即
     * 生效；level null = 清除（模型走自家缺省）。失败回滚本地。 */
    async setThinking(level: string | null) {
      const prev = this.thinkingLevel;
      this.thinkingLevel = level;
      try {
        await sessionSetThinking(this.sessionId, level ?? "");
      } catch (err) {
        this.thinkingLevel = prev;
        throw err;
      }
    },
    /** ㉘·B 设置会话权限档（§3.33 切换器）：写穿后端旁车 + 本地即时
     * 生效（下一笔工具调用即按新档裁决，运行中可切）。 */
    async setPermissionMode(mode: "plan" | "ask" | "acceptEdits" | "auto") {
      this.permissionMode = mode;
      try {
        await sessionSetPermissionMode(this.sessionId, mode);
      } catch (err) {
        // 写穿失败回滚本地（后端仍按旧档裁决——本地显示必须如实）
        await this.refreshSessions();
        this.permissionMode =
          this.sessions.find((s) => s.id === this.sessionId)?.permissionMode ?? null;
        throw err;
      }
    },
    onEvent(e: AgentEvent) {
      switch (e.kind) {
        case "user_message":
          // ㉕·fix 重放恢复用户气泡（此前投影漏 UserMessage，重启后历史
          // 会话只剩模型回复）。实时轮发送即本地回显，本事件不实时到达。
          // pos（v2.57）：编辑重发的定位键随行保存。ts（v2.101）：轮起点
          // 时刻随重放到达（旧日志无 → 不虚构，计时行诚实省略读数）。
          // v2.58 §3.12：新轮开始——用量聚合重置（重放同序重建）
          this.turnUsage = null;
          this.items.push({
            id: nextId++,
            kind: "user",
            text: e.content,
            pos: e.pos,
            ...(e.ts != null ? { ts: e.ts } : this.replaying ? {} : { ts: Date.now() }),
          });
          break;
        case "thinking":
          // v2.57 §3.9：推理行点亮（实时 delta 与重放整段同形，追加到
          // 末尾思考行或新起一行——连续思考行在 §3.7 过程组里本就成组）
          this.appendThinking(e.text);
          break;
        case "chunk":
          this.appendAssistant(e.delta);
          break;
        case "tool_call":
          this.settleThinking();
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
          // ⑮ 派生不再发「已派生子 Agent」告知条（v2.105 目检裁定：两条
          // 状态盒退役，简洁状态收进动作名后——TranscriptFlow L1 极简行）
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
            // ⑮ 收束不再发「子 Agent 收束」告知条（v2.105 目检裁定，同上
            // ——状态与摘要看工具行明细 / 子代理卡 / 转录页签）
          }
          break;
        }
        case "turn_done":
          this.running = false;
          // steps（v2.101 起可选）：TurnEnd 的重放投影不带，保留 null 如实
          this.steps = e.steps ?? this.steps;
          // v2.101：轮收束时刻挂在最后一条用户消息上（第 0 层计时行重放
          // 读数终点）。调度轮没有用户消息——找不到就跳过，不虚构；
          // endTs 已有（该轮已记过）不覆盖。
          if (e.ts != null) {
            for (let i = this.items.length - 1; i >= 0; i--) {
              const it = this.items[i]!;
              if (it.kind === "user") {
                if (it.endTs === undefined) it.endTs = e.ts;
                break;
              }
            }
          }
          // §3.39②：轮次收束 → 消费打断目标 / 自动派发队首
          void this.dispatchQueue();
          break;
        case "schedule_fired":
          // ⑭ 定时触发：SystemNotice 告知（无动作按钮；触发的助手回复
          // 以 chunk 跟进。窗口不在位时后端另发系统通知兜底）
          this.pushNotice("notice.scheduleFired", { title: e.title });
          break;
        case "approval_resolved":
          // §3.7 v4.11（裁定①）：决策记录 = 过程组内的平铺行（无卡片底、
          // 无独立气泡；被拒调用的失败工具行由 tool_result 分支自理）。
          // 投影唯一来源（实时增量与历史重放同源，无双条）。
          this.items.push({
            id: nextId++,
            kind: "decision",
            tool: e.tool,
            allowed: e.allowed,
          });
          break;
        case "ask_user":
          // §3.39①：选择卡走 composer 坞（asks store），对话流零气泡。
          // ask 事件不进会话日志（与审批请求同纪律），重放不会出现。
          useAsksStore().push(e);
          break;
        case "ask_resolved":
          // 四路同源摘卡（应答/跳过/超时/停止）
          useAsksStore().resolve(e.request_id);
          break;
        case "llm_retry":
          // LLM.md v3.0 传输重试：清掉本步未完成的流式缓冲（重试轮 delta
          // 从零重建同一气泡），平铺状态行如实告知（不吓人也不装没事）
          this.resetAssistant();
          if (!this.replaying) {
            this.pushNotice("notice.llmRetried", { attempt: e.attempt });
          }
          break;
        case "turn_capped":
          // 单轮步数到顶（不再硬报错）：如实告知 + 「继续」指引
          this.pushNotice("notice.turnCapped", { steps: e.steps });
          break;
        case "context_usage":
          this.contextUsage = { used: e.used, threshold: e.threshold, windowTokens: e.window_tokens ?? 0, toolsEstTokens: e.tools_est_tokens ?? 0 };
          break;
        case "llm_usage": {
          // v2.58 §3.12：按轮累计（speed = 最后一步生成速度）
          const u = this.turnUsage ?? {
            input: 0, output: 0, cacheRead: 0, cacheWrite: 0,
            reasoning: 0, total: 0, speed: null, steps: 0,
          };
          u.input += e.input;
          u.output += e.output;
          u.cacheRead += e.cache_read;
          u.cacheWrite += e.cache_write;
          u.reasoning += e.reasoning;
          u.total += e.total;
          u.steps += 1;
          if (e.duration_ms > 0) {
            u.speed = Math.round((e.output / e.duration_ms) * 1000);
          }
          this.turnUsage = u;
          // M0 观测（KV v2 §7）：会话累计同步累加——与轮级重置解耦，
          // 重放同源重建，供命中率/节省/告警消费
          const su = this.sessionUsage ?? {
            input: 0, output: 0, cacheRead: 0, cacheWrite: 0,
            reasoning: 0, total: 0, steps: 0,
          };
          su.input += e.input;
          su.output += e.output;
          su.cacheRead += e.cache_read;
          su.cacheWrite += e.cache_write;
          su.reasoning += e.reasoning;
          su.total += e.total;
          su.steps += 1;
          this.sessionUsage = su;
          this.checkCacheAlert(su);
          break;
        }
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
    /** M0 观测告警（KV v2 §7.4，开发者开关缺省关）：会话累计命中率跌破
     * 阈值时 toast 一次（每会话至多一次）。≥3 步才判——冷启动首请求无
     * 缓存可命不是断裂；供应商从未报告过缓存字段（cacheRead/cacheWrite
     * 全 0）不判——未报告 ≠ 命中率塌方，宁缺勿假。 */
    checkCacheAlert(su: SessionUsage) {
      if (this.cacheAlertFired || this.replaying) return;
      const dev = useSettingsStore().view?.developer;
      if (!dev?.cacheHitAlert) return;
      if (su.steps < 3 || su.input <= 0) return;
      if (su.cacheRead + su.cacheWrite <= 0) return;
      if (su.cacheRead / su.input >= dev.cacheHitAlertPercent / 100) return;
      this.cacheAlertFired = true;
      useToastStore().push(
        "warn",
        i18n.global.t("usage.cacheAlert", { pct: dev.cacheHitAlertPercent }),
      );
    },
    appendAssistant(delta: string) {
      this.settleThinking();
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
    /** 流式重置（LLM.md v3.0）：传输重试前清空末尾未完成的流式正文——
     * 重试轮的 delta 从零重建同一气泡。末项不是流式正文时无事可做。 */
    resetAssistant() {
      const last = this.items[this.items.length - 1];
      if (last && last.kind === "assistant") {
        last.text = "";
      }
    },
    /** 思考行收口（§3.9 v4.11）：下一条过程项到达时，给末尾仍在"思考中"
     * 的行补 doneTs（`持续了 N 秒`）。重放条目无 ts，天然空转。 */
    settleThinking() {
      if (this.replaying) return;
      const last = this.items[this.items.length - 1];
      if (last && last.kind === "thinking" && last.ts !== undefined && last.doneTs === undefined) {
        last.doneTs = Date.now();
      }
    },
    /** 思考行追加（v2.57 §3.9）：实时 delta / 重放整段共用 append-or-create
     * ——与 appendAssistant 同构；Thinking 事件在日志里从不相邻（每步后
     * 必有 AssistantMessage/ToolCall），重放不会误并两条。 */
    appendThinking(text: string) {
      if (!text) return;
      const last = this.items[this.items.length - 1];
      if (last && last.kind === "thinking") {
        last.text += text;
      } else {
        this.items.push({
          id: nextId++,
          kind: "thinking",
          text,
          ...(this.replaying ? {} : { ts: Date.now() }),
        });
      }
    },
    /** 消息编辑重发（v2.57 §3.34）：后端截断到该消息之前 → 本地从该气泡
     * 起丢条目 → 走既有 send 通道重发（审批/用量/提取零新链路）。
     * `pos = null` = 当前最后一条用户消息（实时发送无事件位的解析口）。 */
    async editResend(pos: number | null, text: string) {
      const t = text.trim();
      if (!t || this.running) return;
      await sessionTruncateAfter(this.sessionId, pos);
      let idx = -1;
      if (pos === null) {
        for (let i = this.items.length - 1; i >= 0; i--) {
          if (this.items[i]!.kind === "user") {
            idx = i;
            break;
          }
        }
      } else {
        idx = this.items.findIndex((i) => i.kind === "user" && i.pos === pos);
      }
      if (idx >= 0) this.items.splice(idx);
      await this.send(t);
    },
    /** 自动标题落地（v2.57）：只更新列表层标题（顶栏数据源同源）。 */
    applySessionTitle(id: string, title: string) {
      const row = this.sessions.find((s) => s.id === id);
      if (row) row.title = title;
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
      this.stopRequested = false;
      // §3.39②：手动发送恢复自动派发（停止/出错后的暂停态由用户解）
      this.queueDraining = true;
      this.items.push({ id: nextId++, kind: "user", text: t, ts: Date.now() });
      this.running = true;
      this.steps = null;
      this.turnUsage = null;
      // §3.6 v3.9 项目下新建任务：挂起归属随首轮请求带入（㉘·A·fix 归属
      // 先行）——后端建会话即写旁车，首轮注入/围栏/提取就在正确键位上；
      // 旧时序（轮次收束后才 sessionAssociate）有首查键位缝，已退役。
      const ws = this.pendingWorkspace;
      this.pendingWorkspace = null;
      try {
        await chat(this.sessionId, t, ws);
      } catch (err) {
        // 失败轮次不发 turn_done（后端约定），在这里收尾；§3.39② 队列
        // 转暂停（恢复键由用户按，失败后不打连发）
        this.items.push({ id: nextId++, kind: "error", text: String(err) });
        this.queueDraining = false;
      } finally {
        this.running = false;
        // ⑲ 新会话首聊后进列表（列表更新不阻塞输入）
        void this.refreshSessions();
        // §3.39②：轮次收束（成功/失败）→ 消费打断目标 / 自动派发
        void this.dispatchQueue();
      }
    },
    // ---- §3.39 停止与排队（v4.11）----

    /** 统一提交口（composer Enter/发送键）：斜杠命令拦截 → 空闲直发，
     * 运行中入队（§3.39②）。 */
    async submit(text: string) {
      const t = text.trim();
      if (!t) return;
      // M2 斜杠命令 /整理（UI shell §3.36①）：前端拦截——元操作不进对话
      // 流、不建用户气泡；手动整理不受反弹熔断限制且复位熔断态（后端）。
      if (t === "/整理" || t === "/organize") {
        this.pushNotice("notice.organizeRequested");
        try {
          await organizeNow(this.sessionId);
        } catch (err) {
          this.pushRawSystem(String(err));
        }
        return;
      }
      if (this.running) {
        this.queue.push({ id: nextId++, text: t });
        return;
      }
      await this.send(t);
    },
    /** 停止生成（§3.39④）：后端流式截断 + 待决卡 fail-closed；队列转
     * 暂停（draining=false），「已停止」头行由 stopRequested 驱动。 */
    async stop() {
      if (!this.running) return;
      this.stopRequested = true;
      this.queueDraining = false;
      try {
        await agentStop(this.sessionId);
      } catch (err) {
        // 命令失败如实回滚（轮次仍在跑，不停留在假停止态）
        this.stopRequested = false;
        if (this.queue.length > 0) this.queueDraining = true;
        this.pushRawSystem(String(err));
      }
    },
    /** 删除一条排队消息。 */
    removeQueued(id: number) {
      this.queue = this.queue.filter((q) => q.id !== id);
    },
    /** 排队卡头垃圾桶（§3.39② v4.11 参考图）：一键清空全部排队。 */
    clearQueue() {
      this.queue = [];
    },
    /** 「立即发送」= 打断通道（§3.39②）：移出队列 → 停止当前轮 →
     * 收束后立即发送该条（其余保持排队）。空闲时直接发。 */
    async sendQueuedNow(id: number) {
      const idx = this.queue.findIndex((q) => q.id === id);
      if (idx < 0) return;
      const [item] = this.queue.splice(idx, 1);
      if (this.running) {
        this.preempt = item.text;
        await this.stop();
      } else {
        await this.send(item.text);
      }
    },
    /** 恢复自动派发（暂停态的「继续」键）。 */
    resumeDrain() {
      this.queueDraining = true;
      void this.dispatchQueue();
    },
    /** 轮次收束后的派发口：先消费打断目标，再逐条自动派发队首（一条
     * 一轮——send 收束后再进下一轮）。 */
    async dispatchQueue() {
      if (this.dispatching) return;
      if (this.preempt && !this.running) {
        this.dispatching = true;
        const text = this.preempt;
        this.preempt = null;
        try {
          await this.send(text);
        } finally {
          this.dispatching = false;
        }
      }
      while (!this.running && this.queueDraining && this.queue.length > 0) {
        this.dispatching = true;
        const head = this.queue.shift()!;
        try {
          await this.send(head.text);
        } finally {
          this.dispatching = false;
        }
      }
    },
  },
});
