/**
 * `agent://` 事件的 TS 镜像类型。
 * 与 desktop/src-tauri/src/events.rs 的 AgentEvent 保持一致（snake_case 字段）。
 */
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
  | LlmUsageEvent;

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
  steps: number;
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
  /** ㉘·B：外部路径门命中——卡上不渲染「允许本次对话」。 */
  external?: boolean;
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
}

/** 开发者选项区投影（v2.49）：unlocked = 分区可见（安卓式连点版本号
 * 解锁）；其余是危险开关，缺省全关。 */
export interface DeveloperView {
  unlocked: boolean;
  allowDeleteRunningSession: boolean;
  allowSwitchRunningSession: boolean;
  /** 缓存命中率告警（M0 观测，缺省关）。 */
  cacheHitAlert: boolean;
  /** 告警阈值（%）。 */
  cacheHitAlertPercent: number;
}

/** 开发者选项区部分更新：字段缺省 = 保持不变。 */
export interface DeveloperUpdate {
  unlocked?: boolean;
  allowDeleteRunningSession?: boolean;
  allowSwitchRunningSession?: boolean;
  cacheHitAlert?: boolean;
  cacheHitAlertPercent?: number;
}

/** 对话行为区投影：结构即设置原值。contentMaxWidth null = 未设置（前端按
 * 缺省 760 渲染）。 */
export interface ChatSettingsView {
  contentMaxWidth: number | null;
  expandToolProcess: boolean;
  enterToSend: boolean;
  usageReadout: string;
  /** 自动会话标题（v2.57，缺省开）。 */
  autoTitle: boolean;
  /** 缓存命中率/节省显示（M0 观测，缺省开）。 */
  cacheStats: boolean;
  /** 缓存命中单价（元/百万 tokens）。 */
  cacheHitPrice: number;
  /** 缓存未命中单价（元/百万 tokens）。 */
  cacheMissPrice: number;
}

/** 对话行为区部分更新：字段缺省 = 保持不变。 */
export interface ChatUpdate {
  contentMaxWidth?: number;
  expandToolProcess?: boolean;
  enterToSend?: boolean;
  usageReadout?: string;
  autoTitle?: boolean;
  cacheStats?: boolean;
  cacheHitPrice?: number;
  cacheMissPrice?: number;
}

/** 记忆注入区投影（⑪）：结构即设置原值，无生效换算。autoExtract =
 * ⑬ 隐式提取开关（缺省关）。 */
export interface MemorySettingsView {
  enabled: boolean;
  topK: number;
  autoExtract: boolean;
}

/** 记忆注入区部分更新：字段缺省 = 保持不变。 */
export interface MemoryUpdate {
  enabled?: boolean;
  topK?: number;
  autoExtract?: boolean;
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

/** 经历记忆条目（⑬ experience_list 返回行；camelCase）。
 * workspace = 项目归属（㉘），"" = 未归属。 */
export interface ExperienceRow {
  id: number;
  content: string;
  sessionId: string;
  workspace: string;
  createdAt: number;
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

/** 上下文管理区投影（⑨）：设置原值 + 生效值（阈值绝对数 / 建议百分比）
 * 后端一并算好，前端不做第二套换算。 */
export interface ContextSettingsView {
  /** 窗口不在本区（S2 迁 llm 区）；窗口未知时后端按 32k 保守保护。 */
  thresholdPct: number;
  /** "sliding_window" | "layered" | "summary_only"。 */
  strategy: string;
  /** 稳定期轮数（KV v2 §6.4，1–10，缺省 3）。 */
  stabilityRounds: number;
  thresholdTokens: number;
  /** 建议阈值百分比；null = 窗口未知，UI 隐藏建议行。 */
  suggestedPct: number | null;
}

/** 上下文管理区部分更新（S2 起只含阈值与策略；窗口走 LlmUpdate）。 */
export interface ContextUpdate {
  thresholdPct?: number;
  strategy?: string;
  stabilityRounds?: number;
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
  /** 该档已配置模型清单（模型配置页右栏真源）。 */
  models: ProviderModelView[];
}

/** 档位内一个已配置模型（㉚；contextWindow null = 跟随静态档案）。 */
export interface ProviderModelView {
  id: string;
  alias: string | null;
  contextWindow: number | null;
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
}

/** 档位增改入参（llm_provider_save；id 缺省 = 新建）。apiKey undefined =
 * 保持已存值（更新时），空串 = 清除；models undefined = 保持已存清单，
 * 传数组 = 整表替换（原 model 不在清单时后端改指首项）。 */
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
}

/** 单档模型清单（llm_list_models 返回组；error = 该档探测失败）。 */
export interface ProviderModels {
  id: string;
  label: string;
  models: string[];
  error: string | null;
}
