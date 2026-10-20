/**
 * `agent://` 事件的 TS 镜像（与 desktop/src-tauri/src/events.rs 的 AgentEvent 一一对应，snake_case 字段）。
 *
 * 由批 F-22（v2.175）从 `desktop/src/types.ts` 目录化拆出：**每个类型声明块逐字节原样搬移**，
 * 只改两处——①按引用面决定 `export` 与否（外部 import 过的与兄弟簇要用的留 `export`，
 * 其余降为模块内部）；②跨簇引用走 `import type`。等价判据见 scripts/../tmp/v180-equiv.mjs。
 */

import type { ContentPartDto } from "./plugins";

export type AgentEvent =
  | ChunkEvent
  | ToolCallEvent
  | ToolResultEvent
  | TurnDoneEvent
  | ContextUsageEvent
  | ContextCompactedEvent
  | ScheduleFiredEvent
  | UserMessageEvent
  | ApprovalResolvedEvent
  | ThinkingEvent
  | LlmUsageEvent
  | AskUserEvent
  | AskResolvedEvent
  | LlmRetryEvent
  | TurnCappedEvent
  | ToolUncertainEvent
  | UncertainResolvedEvent
  | ToolStatusEvent
  | FileChangesEvent
  | TurnHeaderEvent;

interface ChunkEvent {
  kind: "chunk";
  session_id: string;
  delta: string;
}

interface ToolCallEvent {
  kind: "tool_call";
  session_id: string;
  call_id: string;
  tool: string;
  args: unknown;
  side_effect: string;
  plugin_version: string;
}

interface ToolResultEvent {
  kind: "tool_result";
  session_id: string;
  call_id: string;
  ok: boolean;
  output: unknown;
  error: string | null;
}

interface TurnDoneEvent {
  kind: "turn_done";
  session_id: string;
  /** step 数。v2.101 起可选：TurnEnd 的重放投影不带（日志未记录 step
   * 数，宁缺勿假）；实时路径恒有。 */
  steps?: number;
  /** 收束时刻 Unix 毫秒（v2.101，轮界事件投影）；缺省 = 旧日志无（不虚构）。 */
  ts?: number;
}

/** 单轮步数到顶（LLM.md v3.0 同批，agent://turn-capped）：TurnDone 后
 * 如实告知（不再硬报错）。 */
interface TurnCappedEvent {
  kind: "turn_capped";
  session_id: string;
  steps: number;
}

/** 上下文用量（agent://context-usage，⑨）：与压缩判定同一估算口径。
 * toolsEstTokens（v2.58 §3.12）= 本轮实际携带的暴露目录估算（0 = 目录
 * 缺席），工具上下文行的"约 N tokens"。 */
interface ContextUsageEvent {
  kind: "context_usage";
  session_id: string;
  used: number;
  threshold: number;
  /** 模型窗口；0 = 未设置。 */
  window_tokens: number;
  tools_est_tokens: number;
}

/** 压缩完成（agent://context-compacted，⑨）：summary 只有 200 字符预览
 * （全文在会话日志），strategy = sliding_window | layered | summary_only。 */
interface ContextCompactedEvent {
  kind: "context_compacted";
  session_id: string;
  from_tokens: number;
  to_tokens: number;
  strategy: string;
  summary_preview: string;
}

/** 推理内容（v2.57 §3.9，agent://thinking / 重放同形）：实时 = 增量
 * （StreamChunkSink 直推），重放 = 整段（SessionEvent::Thinking 投影）。
 * 前端 append-or-create，语义与正文 chunk 一致。 */
interface ThinkingEvent {
  kind: "thinking";
  session_id: string;
  text: string;
}

/** 单步 LLM 用量（v2.58 §3.12，agent://usage / 重放同形）：供应商真实
 * 计数 + 耗时，前端按轮聚合出「本轮合计 / 生成速度 / 模型用量」。供应商
 * 不报告的 step 不产生（宁缺勿假）。 */
interface LlmUsageEvent {
  kind: "llm_usage";
  session_id: string;
  input: number;
  output: number;
  cache_read: number;
  cache_write: number;
  reasoning: number;
  total: number;
  duration_ms: number;
}

/** 传输重试（LLM.md v3.0，agent://llm-retry）：兼容客户端重试前通知——
 * 清空本步未完成的流式缓冲（重试轮 delta 从零重建同一气泡）+ 状态行。 */
interface LlmRetryEvent {
  kind: "llm_retry";
  session_id: string;
  attempt: number;
  delay_ms: number;
}

/** 用户消息（㉕·fix）：只在 `session_history` 全量重放出现，重放恢复
 * 用户气泡；实时轮发送即本地回显、后端不投影该段，故不在 api.ts
 * CHANNELS 订阅表。`pos`（v2.57 §3.34）= 源事件在日志里的位次，消息
 * 编辑重发（session_truncate_after）的定位键。 */
interface UserMessageEvent {
  kind: "user_message";
  session_id: string;
  content: string;
  pos: number;
  /** 轮起点时刻（v2.101，TurnStart 配对投影）；缺省 = 旧日志无（不虚构）。 */
  ts?: number;
  /** 附件部件（批 D）；缺省 = 纯文本消息（旧日志渲染零变化）。 */
  parts?: ContentPartDto[];
}

export interface AskUserEvent {
  kind: "ask_user";
  session_id?: string | null;
  request_id: string;
  question: string;
  options: AskOption[];
  multi_select: boolean;
  timeout_secs: number;
}

/** ask_user 选择卡请求（v4.11 §3.39①，agent://ask-user）：模型反向提问，
 * composer 坞内渲染选择卡；应答经 ask_respond 回填。session_id（⑮）=
 * 发起调用的会话（子 Agent 派生带 `{parent}-sub-{n}`，卡头打徽章）。
 * timeout_secs = 守夜秒数（0 = 不限，卡不渲染倒计时）。 */
interface AskOption {
  label: string;
  description?: string | null;
}

/** 选择收束（v4.11 §3.39①，agent://ask-resolved）：应答/跳过/超时/停止
 * 四路同源摘卡信号。 */
interface AskResolvedEvent {
  kind: "ask_resolved";
  session_id?: string | null;
  request_id: string;
}

/** 审批结果（㉕·对话区① §3.17）：ApprovalDecided 的投影（agent://
 * approval-resolved）——决策记录条的唯一来源，实时增量与历史重放同源。
 * request_id == call_id（同一 CallId），前端据此摘除待决卡。 */
interface ApprovalResolvedEvent {
  kind: "approval_resolved";
  session_id: string;
  call_id: string;
  tool: string;
  allowed: boolean;
  reason?: string | null;
}

/** 审批请求载荷（agent://approval-request）。与 events.rs 的
 * ApprovalRequestPayload 对齐（snake_case 字段）。session_id（⑮）=
 * 发起调用的会话；子 Agent 派生的调用带 `{parent}-sub-{n}`，
 * PermissionCard 据此打标识徽章。description（㉕·对话区①）= 工具自述，
 * 决策卡理由行数据源。 */
export interface ApprovalRequest {
  request_id: string;
  tool: string;
  args: unknown;
  side_effect: string;
  session_id?: string | null;
  description?: string | null;
  /** 守夜超时秒数（v2.49）：倒计时数据源；0 = 不限（不渲染倒计时）。 */
  timeout_secs?: number | null;
  /** ㉘·B：会话真实归属——决策卡环境行数据源（null = 前端回落 fsRoots[0]/临时会话）。 */
  workspace?: string | null;
  /** ㉘·B 越界门 / ㉘·C 隐私门命中——硬门卡不渲染「允许本次对话」。 */
  external?: boolean;
  /** ㉘·C 隐私门类别键（privacy.rs 清单；i18n `approval.sensitive.<key>`）。 */
  sensitive?: string | null;
}

/** 定时触发（agent://schedule-fired，⑭）：调度引擎注入轮之前告知。 */
interface ScheduleFiredEvent {
  kind: "schedule_fired";
  session_id: string;
  schedule_id: string;
  title: string;
}

/** 崩溃恢复 uncertain（G2，agent://tool-uncertain——只在会话重放产出，
 * 实时轮不产生）：悬空调用（无 ToolResult 事件）的标记。tool/args 可能
 * 缺（assistant 声明了但 ToolCall 事件未及落盘），按「细节缺失」渲染。 */
interface ToolUncertainEvent {
  kind: "tool_uncertain";
  session_id: string;
  call_id: string;
  tool: string;
  args: unknown;
}

/** 用户裁决（G2，agent://uncertain-resolved，实时回推与重放同形）：
 * decision = completed / not_executed / inspected。 */
interface UncertainResolvedEvent {
  kind: "uncertain_resolved";
  session_id: string;
  call_id: string;
  tool: string;
  decision: "completed" | "not_executed" | "inspected";
  note: string;
}

/** 工具行相位（队列 Q12·C1，agent://tool-status）。**实时专属**：审批请求一
 * 发出就给那条在途调用点亮「等待审批」，不落会话日志 ⇒ 重放永不出现在这里
 * （重放时那次决策已落成 approval_resolved，行按事实闭合，不虚构等待态）。
 * 值集只有一个也是真的：行的成败由 tool_result 自带（`ok`），悬空由
 * tool_uncertain 自带 ⇒ 一份事实一个出处，本事件不许复制它们。 */
export type ToolRowStatus = "awaiting_approval" | "running" | "settled";

interface ToolStatusEvent {
  kind: "tool_status";
  session_id: string;
  call_id: string;
  status: "pending_approval";
}

/** 本轮文件改动（队列 Q12·C1，agent://file-changes）：投影层在轮末从本轮
 * 写类工具结果里的**插件上报行数**聚合而成，实时与重放同一个来源。
 * `additions`/`deletions` 为 null = 该工具算不出 diff（如 fs_write 覆盖已有
 * 文件），是如实读数不是 0 ⇒ UI 不渲染数字（也不渲染 +0 −0）。
 * `kind` 由宿主单点定形：created / modified / overwritten / deleted。 */
export interface FileChangeDto {
  path: string;
  kind: "created" | "modified" | "overwritten" | "deleted";
  additions?: number | null;
  deletions?: number | null;
}

interface FileChangesEvent {
  kind: "file_changes";
  session_id: string;
  turn_id?: number | null;
  files: FileChangeDto[];
}

/** 轮收束状态（队列 Q12·第二刀 v2.205，宿主 `TurnState` 的镜像）。
 * 三值 = 后端 `TurnEnd.outcome` 的一一对应。⚠ 设计稿 CF §2.1 写的是**四**态
 * （含 `running`），本实现刻意不收：running 的权威源是容器本地运行态（`running`
 * 布尔与 TurnProcess 的 phase 机），而 turn_header 只在**收束时**发结论——再带一份
 * running 就是给同一件事造第三个出处（§2.1 订正块记此改动，原话不擦）。
 * 后两态里 `interrupted` 是 v2.205 才分出来的：此前停止也记 completed ⇒ 旧会话
 * 读回来仍是 completed，前端不事后猜。 */
export type TurnState = "completed" | "interrupted" | "failed";

/** 轮头（agent://turn-header）：本轮的收束状态 + **真正干活的毫秒数**
 * （墙钟区间扣掉等待用户审批的那一段，等待源 = `ApprovalDecided` 的
 * `asked_at`/`decided_at`）。
 * `activeMs` 缺省 = 数据层读不出（旧日志无轮界时刻或无审批两刻）⇒ 前端回落到
 * 墙钟读数；**不许把它当 0**（0 = "这轮一秒没干活"，是完全不同的断言）。
 * 刻意不重复 `steps` / `endTs`：那两个读数已由 turn_done 提供（同类只许一个出处）。 */
interface TurnHeaderEvent {
  kind: "turn_header";
  session_id: string;
  turn_id?: number | null;
  state: TurnState;
  active_ms?: number | null;
}

/** 自动会话标题推送（lw://session-title，v2.57）：旁车落盘后通知前端
 * 更新侧栏与顶栏标题。 */
export interface SessionTitlePayload {
  sessionId: string;
  title: string;
}
