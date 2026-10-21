// stores/chat/state.ts —— chat store 的状态面（T22 刀二）。
// 这里是 chat 状态**唯一的类型声明处**：`initialChatState()` 的返回类型标注就是它，
// 所以"字段清单"只有一份（各功能片一律 `s: ChatState` 取状态，不再各自声明第二份形状）。
// 原实现里每个字段的类型是靠 `[] as X[]` / `null as X | null` 现场断言的，搬进 interface 后
// 断言消失、语义不变（返回类型标注提供的上下文类型与那些断言是同一个类型）。
import type { SessionMeta } from "../../types";
import type {
  AttachmentItem,
  ChatItem,
  SessionUsage,
  TurnUsage,
} from "./types";

export interface ChatState {
  sessionId: string;
  items: ChatItem[];
  running: boolean;
  steps: number | null;
  /** 会话列表（⑲，session_list mtime 降序）：切换器数据源。 */
  sessions: SessionMeta[];
  /** 上下文用量（阈值用量进度条投影；agent://context-usage 驱动）。
   * threshold = 有效阈值（滑杆与"窗口 − 输出预留"取小，绝对 token，分母）；
   * windowTokens = 会话生效窗口（v2.137 四层链自动识别，0 = 未识别，比例段隐藏）。 */
  contextUsage: {
    used: number;
    threshold: number;
    windowTokens: number;
    toolsEstTokens: number;
  } | null;
  /** 系统消息"查看详情"的内容（id → 文本，如压缩摘要预览）。 */
  noticeDetails: Record<number, string>;
  /** 压缩摘要正文（`agent://compact-status` 的 `delta` 相流式累积）。
   *  压成后作为「查看详情」的**全文**（后端事件只带 200 字预览，那截断版是
   *  给「没点开详情」时兜底的）。切会话即清（会话级 UI 态）。 */
  compactSummary: string;
  /** 历史重放中（switchSession / init 恢复）：不记时间戳、工具行不吃
   * 「默认展开」设置（§3.8 历史重放默认折叠）。 */
  replaying: boolean;
  /** ㉘·B 当前会话权限模式（null = 未显式设置，显示取设置缺省档）。 */
  permissionMode: "plan" | "ask" | "acceptEdits" | "auto" | null;
  /** v2.58 §3.30 当前会话模型覆盖（null = 跟随设置缺省）。 */
  modelOverride: { providerId: string; model: string } | null;
  /** 批 B 会话级推理档位（null = 未设置，模型走自家缺省）。 */
  thinkingLevel: string | null;
  /** v2.58 §3.30 本轮用量聚合（null = 本轮尚无供应商报告的用量）。 */
  turnUsage: TurnUsage | null;
  /** M0 观测（KV v2 §7）：会话累计用量——只随会话切换/新建重置，
   * 实时与重放同源累计。 */
  sessionUsage: SessionUsage | null;
  /** M0 观测告警已触发标记（每会话至多一次，随会话重置）。 */
  cacheAlertFired: boolean;
  /** §3.6 v3.9 项目下新建任务：挂起的归属目标，首聊建会话后消费。 */
  pendingWorkspace: string | null;
  /** v4.11 §3.39④ 本轮已请求停止（过程组头行显示「已停止」）；轮次
   * 收束复位。前端记忆，不进会话日志。 */
  stopRequested: boolean;
  /** v4.11 §3.39② 运行中排队（前端态，不入会话日志）：autoDrain =
   * 当前轮收束后自动派发；手动停止/轮次出错置 false，用户恢复。 */
  queue: { id: number; text: string }[];
  queueDraining: boolean;
  /** v4.11 §3.39② 正在派发队首（防重入；收束回调里消费）。 */
  dispatching: boolean;
  /** §3.39② 「立即发送」的打断目标：当前轮被停止收束后立即发送。 */
  preempt: string | null;
  /** 批 D §9.2 附件预览条（三条入口同进：按钮选择/粘贴/拖拽）。 */
  attachments: AttachmentItem[];
}

export const initialChatState = (): ChatState => ({
  sessionId: `desktop-${Date.now()}`,
  items: [],
  running: false,
  steps: null,
  sessions: [],
  contextUsage: null,
  noticeDetails: {},
  compactSummary: "",
  replaying: false,
  permissionMode: null,
  modelOverride: null,
  thinkingLevel: null,
  turnUsage: null,
  sessionUsage: null,
  cacheAlertFired: false,
  pendingWorkspace: null,
  stopRequested: false,
  queue: [],
  queueDraining: true,
  dispatching: false,
  preempt: null,
  attachments: [],
});
