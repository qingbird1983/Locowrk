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
  | ScheduleFiredEvent;

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

/** 上下文用量（agent://context-usage，⑨）：与压缩判定同一估算口径。 */
export interface ContextUsageEvent {
  kind: "context_usage";
  session_id: string;
  used: number;
  threshold: number;
  /** 模型窗口；0 = 未设置。 */
  window_tokens: number;
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

/** 定时触发（agent://schedule-fired，⑭）：调度引擎注入轮之前告知。 */
export interface ScheduleFiredEvent {
  kind: "schedule_fired";
  session_id: string;
  schedule_id: string;
  title: string;
}

/** 审批请求载荷（agent://approval-request）。与 events.rs 的
 * ApprovalRequestPayload 对齐（snake_case 字段）。session_id（⑮）=
 * 发起调用的会话；子 Agent 派生的调用带 `{parent}-sub-{n}`，
 * ApprovalModal 据此打标识徽章。 */
export interface ApprovalRequest {
  request_id: string;
  tool: string;
  args: unknown;
  side_effect: string;
  session_id?: string | null;
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

/** 工具暴露区投影（⑫）：结构即设置原值。S2 起含宿主单次调用超时。 */
export interface ToolsSettingsView {
  enabled: boolean;
  maxPerRequest: number;
  callTimeoutSecs: number;
}

/** 工具暴露区部分更新：字段缺省 = 保持不变。 */
export interface ToolsUpdate {
  enabled?: boolean;
  maxPerRequest?: number;
  callTimeoutSecs?: number;
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

/** 经历记忆条目（⑬ experience_list 返回行；camelCase）。 */
export interface ExperienceRow {
  id: number;
  content: string;
  sessionId: string;
  createdAt: number;
}

/** 记忆条目（memory_list 返回行；snake_case 与既有 IPC 投影一致）。
 * source = "explicit" | "extracted"（前端按档位渲染人类文案）。 */
export interface MemoryRow {
  key: string;
  value: string;
  tags: string;
  source: string;
  updated_at: number;
}

/** 上下文管理区投影（⑨）：设置原值 + 生效值（阈值绝对数 / 建议百分比）
 * 后端一并算好，前端不做第二套换算。 */
export interface ContextSettingsView {
  /** 窗口不在本区（S2 迁 llm 区）；窗口未知时后端按 32k 保守保护。 */
  thresholdPct: number;
  /** "sliding_window" | "layered" | "summary_only"。 */
  strategy: string;
  thresholdTokens: number;
  /** 建议阈值百分比；null = 窗口未知，UI 隐藏建议行。 */
  suggestedPct: number | null;
}

/** 上下文管理区部分更新（S2 起只含阈值与策略；窗口走 LlmUpdate）。 */
export interface ContextUpdate {
  thresholdPct?: number;
  strategy?: string;
}

export interface LlmSettingsView {
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
  provider?: string;
  model?: string;
  baseUrl?: string;
  apiKey?: string;
  windowTokens?: number | null;
}
