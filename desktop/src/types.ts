/**
 * `agent://` 事件的 TS 镜像类型。
 * 与 desktop/src-tauri/src/events.rs 的 AgentEvent 保持一致（snake_case 字段）。
 */

/** 资产引用（批 D 多模态，session-store AssetRef）：内容寻址 id + mime +
 * 原文件名（展示用）。字节在 `<数据根>/assets/`，消息只存引用。 */
export interface AssetRefDto {
  id: string;
  mime: string;
  name: string;
}

/** 消息附件部件（批 D，session-store ContentPart）：text = 正文段（用户
 * 消息正文在 content 字段，此变体当前不产生）；image/file 携带 AssetRef。 */
export type ContentPartDto =
  | { type: "text"; text: string }
  | { type: "image"; asset: AssetRefDto }
  | { type: "file"; asset: AssetRefDto };

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
  | UncertainResolvedEvent;

/** 崩溃恢复 uncertain（G2，agent://tool-uncertain——只在会话重放产出，
 * 实时轮不产生）：悬空调用（无 ToolResult 事件）的标记。tool/args 可能
 * 缺（assistant 声明了但 ToolCall 事件未及落盘），按「细节缺失」渲染。 */
export interface ToolUncertainEvent {
  kind: "tool_uncertain";
  session_id: string;
  call_id: string;
  tool: string;
  args: unknown;
}

/** 用户裁决（G2，agent://uncertain-resolved，实时回推与重放同形）：
 * decision = completed / not_executed / inspected。 */
export interface UncertainResolvedEvent {
  kind: "uncertain_resolved";
  session_id: string;
  call_id: string;
  tool: string;
  decision: "completed" | "not_executed" | "inspected";
  note: string;
}

/** 传输重试（LLM.md v3.0，agent://llm-retry）：兼容客户端重试前通知——
 * 清空本步未完成的流式缓冲（重试轮 delta 从零重建同一气泡）+ 状态行。 */
export interface LlmRetryEvent {
  kind: "llm_retry";
  session_id: string;
  attempt: number;
  delay_ms: number;
}

/** 单轮步数到顶（LLM.md v3.0 同批，agent://turn-capped）：TurnDone 后
 * 如实告知（不再硬报错）。 */
export interface TurnCappedEvent {
  kind: "turn_capped";
  session_id: string;
  steps: number;
}

export interface ChunkEvent {
  kind: "chunk";
  session_id: string;
  delta: string;
}

export interface ToolCallEvent {
  kind: "tool_call";
  session_id: string;
  call_id: string;
  tool: string;
  args: unknown;
  side_effect: string;
  plugin_version: string;
}

export interface ToolResultEvent {
  kind: "tool_result";
  session_id: string;
  call_id: string;
  ok: boolean;
  output: unknown;
  error: string | null;
}

export interface TurnDoneEvent {
  kind: "turn_done";
  session_id: string;
  /** step 数。v2.101 起可选：TurnEnd 的重放投影不带（日志未记录 step
   * 数，宁缺勿假）；实时路径恒有。 */
  steps?: number;
  /** 收束时刻 Unix 毫秒（v2.101，轮界事件投影）；缺省 = 旧日志无（不虚构）。 */
  ts?: number;
}

/** 上下文用量（agent://context-usage，⑨）：与压缩判定同一估算口径。
 * toolsEstTokens（v2.58 §3.12）= 本轮实际携带的暴露目录估算（0 = 目录
 * 缺席），工具上下文行的"约 N tokens"。 */
export interface ContextUsageEvent {
  kind: "context_usage";
  session_id: string;
  used: number;
  threshold: number;
  /** 模型窗口；0 = 未设置。 */
  window_tokens: number;
  tools_est_tokens: number;
}

/** 单步 LLM 用量（v2.58 §3.12，agent://usage / 重放同形）：供应商真实
 * 计数 + 耗时，前端按轮聚合出「本轮合计 / 生成速度 / 模型用量」。供应商
 * 不报告的 step 不产生（宁缺勿假）。 */
export interface LlmUsageEvent {
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

/** 压缩完成（agent://context-compacted，⑨）：summary 只有 200 字符预览
 * （全文在会话日志），strategy = sliding_window | layered | summary_only。 */
export interface ContextCompactedEvent {
  kind: "context_compacted";
  session_id: string;
  from_tokens: number;
  to_tokens: number;
  strategy: string;
  summary_preview: string;
}

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

/** 应用内通知类别（§3.13，v2.51）：五类「用户可能错过的事」。 */
export type NotificationKind =
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


/** 定时触发（agent://schedule-fired，⑭）：调度引擎注入轮之前告知。 */
export interface ScheduleFiredEvent {
  kind: "schedule_fired";
  session_id: string;
  schedule_id: string;
  title: string;
}

/** 用户消息（㉕·fix）：只在 `session_history` 全量重放出现，重放恢复
 * 用户气泡；实时轮发送即本地回显、后端不投影该段，故不在 api.ts
 * CHANNELS 订阅表。`pos`（v2.57 §3.34）= 源事件在日志里的位次，消息
 * 编辑重发（session_truncate_after）的定位键。 */
export interface UserMessageEvent {
  kind: "user_message";
  session_id: string;
  content: string;
  pos: number;
  /** 轮起点时刻（v2.101，TurnStart 配对投影）；缺省 = 旧日志无（不虚构）。 */
  ts?: number;
  /** 附件部件（批 D）；缺省 = 纯文本消息（旧日志渲染零变化）。 */
  parts?: ContentPartDto[];
}

/** 推理内容（v2.57 §3.9，agent://thinking / 重放同形）：实时 = 增量
 * （StreamChunkSink 直推），重放 = 整段（SessionEvent::Thinking 投影）。
 * 前端 append-or-create，语义与正文 chunk 一致。 */
export interface ThinkingEvent {
  kind: "thinking";
  session_id: string;
  text: string;
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

/** 自动会话标题推送（lw://session-title，v2.57）：旁车落盘后通知前端
 * 更新侧栏与顶栏标题。 */
export interface SessionTitlePayload {
  sessionId: string;
  title: string;
}

/** 审批结果（㉕·对话区① §3.17）：ApprovalDecided 的投影（agent://
 * approval-resolved）——决策记录条的唯一来源，实时增量与历史重放同源。
 * request_id == call_id（同一 CallId），前端据此摘除待决卡。 */
export interface ApprovalResolvedEvent {
  kind: "approval_resolved";
  session_id: string;
  call_id: string;
  tool: string;
  allowed: boolean;
  reason?: string | null;
}

/** ask_user 选择卡请求（v4.11 §3.39①，agent://ask-user）：模型反向提问，
 * composer 坞内渲染选择卡；应答经 ask_respond 回填。session_id（⑮）=
 * 发起调用的会话（子 Agent 派生带 `{parent}-sub-{n}`，卡头打徽章）。
 * timeout_secs = 守夜秒数（0 = 不限，卡不渲染倒计时）。 */
export interface AskOption {
  label: string;
  description?: string | null;
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

/** 选择收束（v4.11 §3.39①，agent://ask-resolved）：应答/跳过/超时/停止
 * 四路同源摘卡信号。 */
export interface AskResolvedEvent {
  kind: "ask_resolved";
  session_id?: string | null;
  request_id: string;
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

/** 市场目录条目（plugin_registry_list 返回项；versions 升序，末位 latest；
 * display 取自 latest 版本目录的 manifest.json，packaging.md §5.3）。 */
export interface MarketplaceItem {
  id: string;
  versions: string[];
  display: PluginDisplay;
}

/** 多语言文本：BCP-47 标签 → 文案。渲染经 pickLocalized 回退链。 */
export type LocalizedMap = Record<string, string>;

/** 插件展示元数据（packaging.md §5.3 投影；manifest.json 缺失时为
 * 缺省投影：origin = "community"，无图标——UI 首字母兜底）。 */
export interface PluginDisplay {
  display_name?: LocalizedMap;
  description?: LocalizedMap;
  /** data URL（svg/png/webp）；缺省 = 首字母方块。 */
  icon?: string;
  author?: { name: string; url?: string | null };
  homepage?: string;
  license?: string;
  /** "official" | "community"；只影响展示分组，不是信任边界。 */
  origin: string;
  /** 副作用去重清单（pure → compensable → irreversible 固定序）。 */
  side_effects: string[];
}

/** 已安装插件（lockfile 投影 + 注册表运行态合并 + 展示投影）。 */
export interface InstalledPlugin {
  id: string;
  version: string;
  enabled: boolean;
  installed_at: number;
  source: string;
  active_version: string | null;
  loaded_versions: string[];
  display: PluginDisplay;
}

/** ⑩ 闲置信息（后端 IdleInfo 契约，exposure 文档 §8.5）：后端只给结构化
 * 数据，前端按档位渲染"多久没用"人类文案（§11）——永不显示裸天数。 */
export interface IdleInfo {
  level: "fresh" | "normal" | "idle" | "stale" | "abandoned" | "never_used";
  value?: number | null;
  unit?:
    | "today"
    | "this_week"
    | "last_week"
    | "last_month"
    | "month"
    | "half_year"
    | "year"
    | "never_used"
    | "just_installed"
    | null;
}

/** plugin_list_with_usage 行（⑩⑤）：按 id join 到已安装列表的增量视图。 */
export interface PluginUsageView {
  id: string;
  enabled: boolean;
  active: boolean;
  installed_at: number;
  first_used_at: number | null;
  last_used_at: number | null;
  /** builtin 开发态插件不评估闲置（installed_at 无 lockfile 真源）。 */
  idle: IdleInfo | null;
  /** 筛选档位（后端裁决，嵌套：gt3m ⊂ gt1m ⊂ recent）。 */
  filter_key: "recent" | "gt1m" | "gt3m" | "gt6m" | null;
}

/** 批量操作结果（⑩⑥）：逐条成败，部分失败不回滚。 */
export interface BatchResult {
  ok: string[];
  failed: { id: string; error: string }[];
}

/** plugin://state-changed 载荷（plugin-service PluginStateChange 镜像，
 * type 标签）。 */
export type PluginStateChange =
  | { type: "installed"; id: string; version: string }
  | { type: "enabled_changed"; id: string; enabled: boolean }
  | { type: "uninstalled"; id: string }
  | { type: "active_version_changed"; id: string; version: string };

/** 设置投影（settings_get / settings_set 返回值；camelCase）。API Key
 * 原值永不出后端，只有 apiKeySet 布尔。 */
export interface SettingsView {
  /** "system" | "zh-CN" | "en"；system = 跟随系统语言。 */
  locale: string;
  /** 外观区（⑰）：明暗主题 "dark" | "light" | "system"。 */
  appearance: AppearanceView;
  /** 全局快捷键（切换主窗口可见性），如 "Ctrl+Shift+L"。 */
  shortcut: string;
  dataDir: string;
  llm: LlmSettingsView;
  embedding: EmbeddingView;
  context: ContextSettingsView;
  memory: MemorySettingsView;
  tools: ToolsSettingsView;
  shell: ShellSettingsView;
  plugins: PluginsView;
  /** 对话行为区（UI shell 文档 §3.2 ChatSection / §2.7 列宽偏好）。 */
  chat: ChatSettingsView;
  /** 开发者选项区（v2.49，默认隐藏分区）。 */
  developer: DeveloperView;
  /** 思考语言区（docs/design/NEW/language of thought.md）：reasoning 用的
   * 语言；会话首请求锁死，中途修改只影响新会话。 */
  thinking: ThinkingView;
}

/** 思考语言区投影。 */
export interface ThinkingView {
  /** follow_ui（缺省，跟随界面）| zh-CN | en-US | none。 */
  language: string;
}

/** 开发者选项区投影（v2.49）：unlocked = 分区可见（安卓式连点版本号
 * 解锁）；其余是危险开关，缺省全关。 */
export interface DeveloperView {
  unlocked: boolean;
  allowDeleteRunningSession: boolean;
  allowSwitchRunningSession: boolean;
  /** 缓存命中率告警（M0 观测，缺省关）。 */
  cacheHitAlert: boolean;
  /** 前缀断裂诊断（M6.1，缺省关）：开 = AppLlm 每 step 记分块 hash 链。 */
  cacheDiagnostics: boolean;
  /** 告警阈值（%）。 */
  cacheHitAlertPercent: number;
  /** 记忆树反弹熔断 k（M2，§5.5：整理后 k 个用户轮内再触发 = 反弹）。 */
  organizeReboundK: number;
  /** 反弹熔断 m（连续 m 次 → 暂停本会话自动整理）。 */
  organizeReboundM: number;
  /** 记忆管理器单次调用超时（秒）。 */
  recorderTimeoutSecs: number;
  /** 记忆管理器重试次数（仍失败 → 规则切分降级）。 */
  recorderMaxRetries: number;
  /** 规则切分块（token，5K–50K 缺省 20K）。 */
  ruleChunkTokens: number;
}

/** 开发者选项区部分更新：字段缺省 = 保持不变。 */
export interface DeveloperUpdate {
  unlocked?: boolean;
  allowDeleteRunningSession?: boolean;
  allowSwitchRunningSession?: boolean;
  cacheHitAlert?: boolean;
  cacheDiagnostics?: boolean;
  cacheHitAlertPercent?: number;
  organizeReboundK?: number;
  organizeReboundM?: number;
  recorderTimeoutSecs?: number;
  recorderMaxRetries?: number;
  ruleChunkTokens?: number;
}

/** 对话行为区投影：结构即设置原值。contentMaxWidth null = 未设置（前端按
 * 缺省 760 渲染）。 */
export interface ChatSettingsView {
  contentMaxWidth: number | null;
  /** 折叠工具执行过程（v4.20 语义反转，缺省关 = 过程组展开仅明细收起）。 */
  collapseToolProcess: boolean;
  enterToSend: boolean;
  usageReadout: string;
  /** 自动会话标题（v2.57，缺省开）。 */
  autoTitle: boolean;
  /** 缓存命中率显示（M0 观测，缺省开；v2.137 单价两键退役）。 */
  cacheStats: boolean;
  /** 长消息折叠（缺省开）：超过 foldLines 行收起 + 遮罩 + 展开钮。 */
  foldLongMessages: boolean;
  /** 长消息折叠行数（3–20，缺省 5）。 */
  foldLines: number;
}

/** 对话行为区部分更新：字段缺省 = 保持不变。 */
export interface ChatUpdate {
  contentMaxWidth?: number;
  collapseToolProcess?: boolean;
  enterToSend?: boolean;
  usageReadout?: string;
  autoTitle?: boolean;
  cacheStats?: boolean;
  foldLongMessages?: boolean;
  foldLines?: number;
}

/** 记忆注入区投影（⑪）：结构即设置原值，无生效换算。v2.136 起注入面
 * = 全局记忆 + 提醒段；autoExtract/handoffUpdate/experiences 三键退役。 */
export interface MemorySettingsView {
  enabled: boolean;
  topK: number;
  /** 记忆树整理节奏（M2：batch 批量 / incremental 增量，树 v2 §4.3）。 */
  organizePacing: string;
  /** 增量块 X（token，增量档触发线，10K–200K 缺省 50K）。 */
  organizeChunkTokens: number;
  /** 记忆管理器模型（树 v2 §5.2；null = 同主模型）。 */
  recorderModel: string | null;
  recorderProvider: string | null;
  /** M4 尾巴策略（boundary = A 边界结算缺省 / window = B 固定窗口）。 */
  tailStrategy: string;
  /** 固定窗口 N 轮（窗口档生效，1–20 缺省 5）。 */
  tailWindowTurns: number;
  /** memory_collapse 暴露开关（§8.3 缺省关）。 */
  collapseEnabled: boolean;
  /** 自动收起占用阈值（collapse 开启时生效，50–95% 缺省 80）。 */
  collapseThresholdPct: number;
  /** F1 记忆文件化总开关（缺省开；关 = 回纯 sqlite 不再读写 md）。 */
  files: boolean;
  /** F2 交接提醒（缺省开，v2.136 提醒制）：归属会话注入静态提醒段。 */
  repoContext: boolean;
}

/** 记忆注入区部分更新：字段缺省 = 保持不变。 */
export interface MemoryUpdate {
  enabled?: boolean;
  topK?: number;
  organizePacing?: string;
  organizeChunkTokens?: number;
  /** 空串 = 清除（回落同主模型）。 */
  recorderModel?: string;
  recorderProvider?: string;
  tailStrategy?: string;
  tailWindowTurns?: number;
  collapseEnabled?: boolean;
  collapseThresholdPct?: number;
  files?: boolean;
  repoContext?: boolean;
}

/** 前缀断裂诊断报告（M6.1，llm_cache_diagnose；breaks 空 = 健康纯追加）。 */
export interface CacheBreakReport {
  step: number;
  breaks: string[];
  prevMessages: number;
  messages: number;
  tsMs: number;
}

/** F2 reconcile 结果（memory_reconcile 命令返回；计数如实透出）。 */
export interface ReconcileSummary {
  scanned: number;
  created: number;
  updated: number;
  healed: number;
  deleted: number;
  warnings: number;
}

/** 工具暴露区投影（⑫）：结构即设置原值。S2 起含宿主单次调用超时；
 * ⑱ 起含 fsRoots 工作区目录（空 = 工具文件面仅数据目录）；
 * ㉕·微调③ 起含单轮步数上限。 */
export interface ToolsSettingsView {
  enabled: boolean;
  maxPerRequest: number;
  callTimeoutSecs: number;
  fsRoots: string[];
  maxTurnSteps: number;
  /** 审批等待时长（秒，0 = 不限；v2.49 可配置原则）。 */
  approvalTimeoutSecs: number;
  /** ask_user 选择等待时长（秒，0 = 不限；§3.39① 可配置原则）。 */
  askTimeoutSecs: number;
  /** ㉘·B 新会话权限档缺省。 */
  defaultPermissionMode: "plan" | "ask" | "acceptEdits" | "auto";
  /** 工具集会话钉死（KV v2 §4.2，缺省开）。 */
  pinSession: boolean;
}

/** 工具暴露区部分更新：字段缺省 = 保持不变。 */
export interface ToolsUpdate {
  enabled?: boolean;
  maxPerRequest?: number;
  callTimeoutSecs?: number;
  fsRoots?: string[];
  maxTurnSteps?: number;
  approvalTimeoutSecs?: number;
  askTimeoutSecs?: number;
  defaultPermissionMode?: "plan" | "ask" | "acceptEdits" | "auto";
  pinSession?: boolean;
}

/** embedding 模型区投影（⑬ 智能层共享基建）：无秘钥字段。 */
export interface EmbeddingView {
  /** 设置原值；null = 未填（后端按 provider 兜底：ollama → 本机 11434）。 */
  baseUrl: string | null;
  model: string | null;
  apiKeySet: boolean;
}

/** embedding 区部分更新：字段缺省 = 保持不变；传空串 = 清除。 */
export interface EmbeddingUpdate {
  baseUrl?: string;
  model?: string;
  apiKey?: string;
}

/** 插件管理区投影（⑬）：自动停用天数（0 = 关闭，缺省）。 */
export interface PluginsView {
  autoDisableIdleDays: number;
}

/** 插件管理区部分更新：字段缺省 = 保持不变。 */
export interface PluginsUpdate {
  autoDisableIdleDays?: number;
}

/** shell 治理区投影（S1）：高危命令解锁开关（硬锁类不受影响）。 */
export interface ShellSettingsView {
  unlockRisky: boolean;
}

/** shell 治理区部分更新：字段缺省 = 保持不变；改动重启后生效。 */
export interface ShellUpdate {
  unlockRisky?: boolean;
}

/** 外观区投影（⑰）：明暗主题。生效在前端 lib/theme.ts（§六）。 */
export interface AppearanceView {
  theme: string;
}

/** 外观区部分更新：字段缺省 = 保持不变。 */
export interface AppearanceUpdate {
  theme?: string;
}

/** 最近一次工具暴露截断快照（tool_exposure_debug；camelCase）。
 * savedTokensEstimate 为 chars/4 粗估，前端按 §3.5 档位渲染。 */
export interface ExposureDebug {
  enabled: boolean;
  total: number;
  exposed: number;
  always: string[];
  sessionHot: string[];
  globalHot: string[];
  /** 靠语义检索信号（0.5 × 相似度）入选的工具（⑬；embedding 未配置恒空）。 */
  retrieved: string[];
  savedTokensEstimate: number;
  /** 快照时刻（Unix 秒）；0 = 尚未有过请求。 */
  ts: number;
}


/** 记忆树影子运行观测（M2 §3.36③；memory_tree_view 命令载荷）。 */
export interface TreeDebug {
  exists: boolean;
  workspace: string;
  sessionId: string;
  chapteredUntil: number;
  chapterCount: number;
  tailEvents: number;
  tailTokensEst: number;
  anchorTokens: number;
  anchorSource: "llm_usage" | "estimate";
  /** 整理阈值（绝对 token，v2.137）。 */
  thresholdTokens: number;
  pacing: "batch" | "incremental";
  chunkTokens: number;
  pausedAuto: boolean;
  reboundCount: number;
  pending: { from: number; to: number; attempts: number } | null;
  history: TreeOrganizeRecord[];
}

export interface TreeOrganizeRecord {
  ts: number;
  reason: string;
  mode: string;
  status: string;
  from: number;
  to: number;
  chapters: number;
  /** wire = snake_case（OrganizeRecord 无 camelCase 改名，历史兼容）。 */
  manager_input: number;
  manager_output: number;
  durationMs: number;
}

// ---- 记忆树只读呈现（M3 §3.37，Memory-KV Roadmap） ----

/** 树投影清单行（memory_tree_sessions；会话切换器数据源）。 */
export interface TreeSummaryView {
  workspace: string;
  sessionId: string;
  chapterCount: number;
  chapteredUntil: number;
  pending: boolean;
  pausedAuto: boolean;
  updatedMs: number;
}

/** 一章节（L0 标题 / L1 摘要 / L2 事件区间；source 如实 recorder|rule）。
 * M5 起可编辑：*EditedByUser = 用户改过（自动层永不覆盖，UI 挂「手动」徽章）。 */
export interface TreeChapterView {
  chapterId: string;
  title: string;
  summary: string;
  source: string;
  pending: boolean;
  createdAt: number;
  eventFrom: number;
  eventTo: number;
  titleEditedByUser: boolean;
  summaryEditedByUser: boolean;
}

/** 选中会话的章节清单（memory_tree_chapters；投影缺/坏 = exists=false）。 */
export interface TreeChaptersView {
  exists: boolean;
  workspace: string;
  sessionId: string;
  chapteredUntil: number;
  pausedAuto: boolean;
  chapters: TreeChapterView[];
}

/** 项目联合视图行（M6.4，memory_tree_project；跨会话捆平铺 + 会话标注）。 */
export interface ProjectChapterView {
  sessionId: string;
  sessionTitle: string;
  chapterId: string;
  title: string;
  summary: string;
  source: string;
  titleEditedByUser: boolean;
  summaryEditedByUser: boolean;
  createdAt: number;
  eventFrom: number;
  eventTo: number;
}

/** 原文切片行（与记忆管理器材料同形；index = 会话日志事件位）。 */
export type TreeOriginalKind = "user" | "assistant" | "tool_call" | "tool_result";
export interface TreeOriginalItem {
  index: number;
  kind: TreeOriginalKind;
  text: string;
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


/** 记忆条目（memory_list 返回行；snake_case 与既有 IPC 投影一致）。
 * source = "explicit" | "extracted"（前端按档位渲染人类文案）；
 * workspace = 项目归属（㉘），"" = 未归属。 */
export interface MemoryRow {
  key: string;
  value: string;
  tags: string;
  source: string;
  workspace: string;
  updated_at: number;
}

/** 上下文管理区投影（⑨；v2.137 整理阈值 = 绝对 token——「上下文达到
 * 多少 K 就整理」，百分比/策略/稳定期三键随 ctx-compact 桌面链退役）。 */
export interface ContextSettingsView {
  /** 整理阈值（绝对 token，100K–800K 缺省 100K）。 */
  thresholdTokens: number;
  /** 阈值用量进度条显隐（v4.18，缺省开）。 */
  usageBar: boolean;
}

/** 上下文管理区部分更新（窗口走 LlmUpdate）。 */
export interface ContextUpdate {
  thresholdTokens?: number;
  usageBar?: boolean;
}

/** provider 档位投影（v2.58 档位制，DESIGN §13.1；无秘钥字段）。 */
export interface ProviderView {
  id: string;
  label: string;
  /** "openai" | "anthropic" | "ollama"。 */
  kind: string;
  baseUrl: string | null;
  apiKeySet: boolean;
  model: string;
  windowTokens: number | null;
  /** ㉚ 停用档：选择器与 active 回落跳过（配置保留）。 */
  enabled: boolean;
  /** 自定义请求头（批 C §8.1；保留键后端拒收，键大小写原样）。 */
  headers: Record<string, string>;
  /** 该档已配置模型清单（模型配置页右栏真源）。 */
  models: ProviderModelView[];
}

/** 每百万 token 单价（USD 原值如实；目录缺分量 = null 不冒充 0）。 */
export interface ProviderModelPricing {
  input: number | null;
  output: number | null;
  cacheRead: number | null;
}

/** 字段来源：`"user"` = 用户钉死（编辑即落 settings，刷新不覆盖）；
 * `"catalog"` = 跟随 models.dev；null = 未知（如实标注）。 */
export type ModelFieldSource = "user" | "catalog" | null;

/** 字段来源标记（批 A，供应商稿 §4.5 SourcesView）。 */
export interface ProviderModelSources {
  window: ModelFieldSource;
  maxOutput: ModelFieldSource;
  modalities: ModelFieldSource;
  reasoning: ModelFieldSource;
}

/** 档位内一个已配置模型（㉚；批 A 起为目录合成视图：钉死值 ∪ models.dev，
 * 目录部分只进内存——保存时 sources 为 catalog 的派生值由前端剥除）。 */
export interface ProviderModelView {
  id: string;
  alias: string | null;
  contextWindow: number | null;
  /** 用户钉死最大输出（token）；null = 跟随目录 / 不发。 */
  maxOutput: number | null;
  maxOutputSource: string | null;
  /** 用户钉死图像输入（tri-state：null = 随目录 / 纯文本缺省）。 */
  supportsImage: boolean | null;
  /** 可用于子代理（批 F：用户钉死；null = 不参与）。 */
  availableForSubagents: boolean | null;
  /** 展示用有效模态（未知 = ["text"]）。 */
  inputModalities: string[];
  /** 归一化推理梯子（空 = 隐藏档位选择器）。 */
  reasoningLadder: string[];
  toolCall: boolean | null;
  pricing: ProviderModelPricing | null;
  /** 后端投影恒带对象；null = 客户端新建行、待目录合并（ModelPanes 哨兵）。 */
  sources: ProviderModelSources | null;
}

/** 模型能力目录状态（llm_catalog_status；快照 bundled / 在线刷新 refreshed）。 */
export interface CatalogStatus {
  source: "bundled" | "refreshed";
  updatedMs: number;
  providers: number;
  models: number;
}

/** 单模型目录查询结果（llm_lookup_model 命中时）。 */
export interface CatalogModelInfo {
  providerKey: string;
  modelId: string;
  displayName: string | null;
  contextWindow: number | null;
  maxOutput: number | null;
  inputModalities: string[];
  reasoningOptions: string[];
  toolCall: boolean | null;
  pricing: ProviderModelPricing | null;
}

export interface LlmSettingsView {
  /** 活动档 id（空串 = 无档位）。 */
  active: string;
  providers: ProviderView[];
  /** 活动档平面速记（模型 chip / 上下文区 / 嵌入提示直读）。 */
  provider: string;
  model: string;
  baseUrl: string | null;
  apiKeySet: boolean;
  /** 模型上下文窗口（token，S2 迁入）；null = 未设置（回落保守保护，
   * Ollama 可经 /api/show 探测，云端查静态档案或手填）。 */
  windowTokens: number | null;
  /** 网络代理档（批 C §8.5）；null = system（吃环境变量）。 */
  networkProxy: NetworkProxySettings | null;
  /** 子代理主模型钉档（批 F）；null = 跟随会话（缺省）。 */
  subagent: SubagentPin | null;
}

/** 子代理主模型钉档（批 F）：thinkingLevel null = 跟随会话档位。 */
export interface SubagentPin {
  providerId: string;
  model: string;
  thinkingLevel: string | null;
}

/** 网络代理档（§8.5）：system / direct / custom（custom 必带 url）。 */
export interface NetworkProxySettings {
  mode: "system" | "direct" | "custom";
  url: string | null;
}

/** 表单探测结果（§8.2）：清单或错误 + 端点扫描自动补全的 base_url
 * （与传入不同时非空，表单字段未再编辑时自动采纳）。 */
export interface ProbeOutcome {
  models: string[];
  error: string | null;
  effectiveBaseUrl: string | null;
  /** 推荐三选（批 F：目录数据驱动 ≤3；空 = 前端回落旧全选行为）。 */
  recommended: string[];
}

/** 导入面板候选（批 F；密钥不出后端——hasKey 布尔代替）。 */
export interface ImportCandidate {
  source: "claude-code" | "codex" | "opencode" | "cc-switch";
  id: string;
  label: string;
  kind: string;
  baseUrl: string | null;
  model: string;
  models: ImportCandidateModel[];
  hasKey: boolean;
  notes: string[];
}

export interface ImportCandidateModel {
  id: string;
  contextWindow: number | null;
}

export interface ImportScanOutcome {
  candidates: ImportCandidate[];
  errors: string[];
}

export interface ImportApplyReport {
  imported: string[];
  skipped: string[];
}

/** 逐模型连通测试结果（§8.3；五分类 category + 如实报文）。 */
export interface ModelTestResult {
  ok: boolean;
  category:
    | "auth"
    | "model_not_found"
    | "rate_limit"
    | "network"
    | "server"
    | "unknown"
    | null;
  message: string | null;
  latencyMs: number | null;
}

/** 部分更新：字段缺省（undefined）= 保持不变；baseUrl / apiKey 传
 * 空串 = 清除；windowTokens 传 null = 清除窗口（回落保守保护）。 */
export interface LlmUpdate {
  /** 切活动档（v2.58）；id 必须已存在。 */
  active?: string;
  provider?: string;
  model?: string;
  baseUrl?: string;
  apiKey?: string;
  windowTokens?: number | null;
  /** 子代理钉档（批 F）；providerId/model 空串 = 清除（v2.65 约定）。 */
  subagent?: SubagentPin;
}

/** 档位增改入参（llm_provider_save；id 缺省 = 新建）。apiKey undefined =
 * 保持已存值（更新时），空串 = 清除；models undefined = 保持已存清单，
 * 传数组 = 整表替换（原 model 不在清单时后端改指首项）；headers
 * undefined = 保持已存值，传对象 = 整表替换（保留键后端拒收）。 */
export interface ProviderInput {
  id?: string;
  label: string;
  kind: string;
  baseUrl?: string | null;
  apiKey?: string;
  model: string;
  windowTokens?: number | null;
  enabled?: boolean;
  models?: ProviderModelView[];
  headers?: Record<string, string>;
}

/** 单档模型清单（llm_list_models 返回组；error = 该档探测失败）。 */
export interface ProviderModels {
  id: string;
  label: string;
  models: string[];
  error: string | null;
}
