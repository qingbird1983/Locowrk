/**
 * 会话与运行态的读模型：会话元信息 / 检索命中 / 定时任务 / 整理与回收的载荷 / 通知。
 *
 * 由批 F-22（v2.175）从 `desktop/src/types.ts` 目录化拆出：**每个类型声明块逐字节原样搬移**，
 * 只改两处——①按引用面决定 `export` 与否（外部 import 过的与兄弟簇要用的留 `export`，
 * 其余降为模块内部）；②跨簇引用走 `import type`。等价判据见 scripts/../tmp/v180-equiv.mjs。
 */

/** 会话列表行（⑲ session_list；mtime 降序）。title = 首条用户消息
 * 截断（None = 还没有用户消息，选项回退显示 id）；workspace = §2.3
 * 工作区归属（v2.50，旁车 meta 透出，null = 未关联）。 */
export interface SessionMeta {
  id: string;
  updatedAt: number;
  events: number;
  title: string | null;
  workspace: string | null;
  /** ㉘·B 会话级权限模式（null = 取设置缺省档）。 */
  permissionMode: "plan" | "ask" | "acceptEdits" | "auto" | null;
  /** v2.58 §3.30 会话级模型覆盖（成对；null = 跟随设置缺省）。 */
  model: string | null;
  modelProvider: string | null;
  /** 批 B 会话级推理档位（null = 未设置，模型走自家缺省）。 */
  thinkingLevel: string | null;
}

/** 会话正文搜索命中（v2.49）：mtime 降序限量；snippet = 首处摘录。 */
export interface SessionSearchHit {
  id: string;
  title: string | null;
  snippet: string;
  matchCount: number;
  updatedAt: number;
}

/** 一条待触发调度（v2.57 §3.35，schedule_list 投影）。 */
export interface SessionSchedule {
  id: string;
  sessionId: string;
  kind: "once" | "interval";
  title: string;
  prompt: string;
  runAtMs: number;
  intervalSecs: number | null;
  /** 引擎忙顺延次数（如实呈现等待状态）。 */
  deferrals: number;
  createdAtMs: number;
}

/** 轮级总结（Organize Summary Design v1）：一次整理区间的浓缩叙事，
 * 该批章节的父节点。 */
export interface RunSummaryView {
  text: string;
  eventFrom: number;
  eventTo: number;
  createdAt: number;
  /** 项目视图带会话标注；会话视图内为空串。 */
  sessionId: string;
  sessionTitle: string;
  /** 本轮区间内已定稿章节 id（数值序，队列 Q14）。归组权威源是后端投影的
   * event_range ⇒ 前端**不要**再按区间反查章节（反查 = 第二个出处）；
   * 汇聚流的组键就是这一条的 `eventFrom:eventTo`，成员就是这一串 id。 */
  chapterIds: string[];
}

/** `/压缩` 的返回值（S2 刀⑦ · AC-S2-11）。它**只说"这一趟没动，因为"**：压成的那份
 * 负载（省了多少 / 摘要预览）走 `agent://context-compacted` 推送——AGENTS §3 那条
 * "流式输出不塞进 command 返回值"，而且这样手动与自动共用前端那一条断层条构造，
 * 界面上不可能长出两种"压缩完成"。
 * `reason` = 后端**判据名**（不是人话）：文案住语言包，前端按名取（同
 * `OrganizeDonePayload.reason` 那处分支的先例）。取值域与 Rust 侧同源，
 * 由 `check-single-source` 的 SS24 钉住。 */
export interface CompactNowView {
  compacted: boolean;
  reason: string | null;
}

/** 整理收束推送（lw://organize-done；手动/自动/恢复三路同源）。 */
export interface OrganizeDonePayload {
  sessionId: string;
  users: number;
  chapters: number;
  mode: "recorder" | "rule";
  status: "ok" | "degraded";
  reason: string;
}

/** 熔断翻转推送（lw://organize-paused；仅翻转瞬间发）。 */
export interface OrganizePausedPayload {
  sessionId: string;
  rebounds: number;
  k: number;
  m: number;
}

/** 浏览器页签开签推送（lw://browser-open；browser_open 工具 → 工作面板）。 */
export interface BrowserOpenPayload {
  tabId: string;
  kind: "file" | "url";
  /** 页签最终 URL（file = lwpreview 预览地址；url = 原样）。 */
  url: string;
  /** 原始输入（文件绝对路径或 http(s) 地址，URL 栏回显用）。 */
  target: string;
  title: string;
}

/** 前缀断裂诊断报告（M6.1，llm_cache_diagnose；breaks 空 = 健康纯追加）。 */
export interface CacheBreakReport {
  step: number;
  breaks: string[];
  prevMessages: number;
  messages: number;
  tsMs: number;
}

/** F2 reconcile 结果（memory_reconcile 命令返回；计数如实透出）。
 *  「影子 → 文件」方向的写动作随 S1 刀②c-2 退役，`healed`（愈合文件数）计数就此消失。 */
export interface ReconcileSummary {
  scanned: number;
  created: number;
  updated: number;
  deleted: number;
  warnings: number;
}

/** 应用内通知类别（§3.13，v2.51）：五类「用户可能错过的事」。 */
type NotificationKind =
  | "schedule_fired"
  | "subagent_done"
  | "subagent_failed"
  | "context_compacted"
  | "approval_timeout";

/** 单条应用内通知（§3.13，v2.51）：后端 notifications.json 镜像；label
 * 是类别语义的数据值（调度名 / 省下的 token / 错误摘要），标题模板由
 * 前端 i18n 渲染。 */
export interface NotificationItem {
  id: string;
  kind: NotificationKind;
  sessionId: string | null;
  label: string | null;
  createdAt: number;
  read: boolean;
}
