// IPC 封装（DESIGN.md §12.2 前端侧）：命令走 invoke，事件走 listen。
// 命令只做发起，内容全部从 `agent://` 事件来。
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AgentEvent,
  ApprovalRequest,
  BatchResult,
  ChatUpdate,
  ContextUpdate,
  InstalledPlugin,
  MemoryRow,
  PluginUsageView,
  LlmUpdate,
  MarketplaceItem,
  MemoryUpdate,
  PluginStateChange,
  SessionMeta,
  SettingsView,
  AppearanceUpdate,
  DeveloperUpdate,
  ShellUpdate,
  ToolsUpdate,
  ExposureDebug,
  EmbeddingUpdate,
  PluginsUpdate,
  ExperienceRow,
  SessionSearchHit,
  NotificationItem,
} from "../types";

/** 发起一轮对话；返回实际 step 数。内容经事件到达，不经返回值。 */
export function chat(sessionId: string, message: string): Promise<number> {
  return invoke("agent_chat", { sessionId, message });
}

/** 撤销最近一次上下文压缩（⑨）：其后的轮次一并回退（前缀游标语义）。
 * 会话 id 不变。返回回退点。 */
export function contextUndoCompact(sessionId: string): Promise<number> {
  return invoke("context_undo_compact", { sessionId });
}

/** 回填审批决策（§12.1 / §3.17）。scope="session" 且放行 = 允许本次对话
 * （豁免集记在后端 ApprovalHub，本会话后续同工具不再问）。
 * 与已处理/未知的请求返回 reject（含超时已被后端自动拒绝的请求）。 */
export function approve(
  requestId: string,
  allowed: boolean,
  scope?: "session",
): Promise<void> {
  return invoke("agent_approve", { requestId, allowed, scope: scope ?? null });
}

/** 未决审批补拉（前端刷新/重连时，事件可能已错过）。 */
export function listPendingApprovals(): Promise<ApprovalRequest[]> {
  return invoke("list_pending_approvals");
}

const CHANNELS = [
  "agent://chunk",
  "agent://tool-call",
  "agent://tool-result",
  "agent://turn-done",
  "agent://context-usage",
  "agent://context-compacted",
  // ⑭ 定时触发（搭车修复：chat store 有处理分支但本表漏列的死分支）
  "agent://schedule-fired",
  // ㉕·对话区①：审批结果投影（决策记录条 + 摘除待决卡）
  "agent://approval-resolved",
] as const;

export function subscribeAgentEvents(
  onEvent: (e: AgentEvent) => void,
): Promise<UnlistenFn[]> {
  return Promise.all(
    CHANNELS.map((ch) => listen<AgentEvent>(ch, (e) => onEvent(e.payload))),
  );
}

/** 审批结果订阅（㉕·对话区①）：approvals store 专用——待决卡摘除；
 * 决策记录条由 chat store 的全量订阅消费同一通道。 */
export function subscribeApprovalResolved(
  onResolved: (e: Extract<AgentEvent, { kind: "approval_resolved" }>) => void,
): Promise<UnlistenFn[]> {
  return listen<AgentEvent>("agent://approval-resolved", (e) =>
    onResolved(e.payload as Extract<AgentEvent, { kind: "approval_resolved" }>),
  ).then((un) => [un]);
}

export function subscribeApprovals(
  onRequest: (req: ApprovalRequest) => void,
): Promise<UnlistenFn[]> {
  return listen<ApprovalRequest>("agent://approval-request", (e) =>
    onRequest(e.payload),
  ).then((un) => [un]);
}

// ---- 插件管理（DESIGN.md §11.1 / §13.3 / §13.4） ----

/** 已安装列表（lockfile 投影 + 注册表运行态合并）。 */
export function pluginList(): Promise<InstalledPlugin[]> {
  return invoke("plugin_list");
}

/** 带使用统计的已安装列表（⑩⑤）：闲置分级后端裁决，按 id join。 */
export function pluginListWithUsage(): Promise<PluginUsageView[]> {
  return invoke("plugin_list_with_usage");
}

/** 批量启用/停用（⑩⑥）：逐条成败，部分失败不回滚。 */
export function pluginBatchSetEnabled(
  ids: string[],
  enabled: boolean,
): Promise<BatchResult> {
  return invoke("plugin_batch_set_enabled", { ids, enabled });
}

/** 批量卸载（⑩⑥）：调用方负责二次确认并列出插件名（§9.5）。 */
export function pluginBatchUninstall(ids: string[]): Promise<BatchResult> {
  return invoke("plugin_batch_uninstall", { ids });
}

/** 市场目录（发现页）。 */
export function pluginRegistryList(): Promise<MarketplaceItem[]> {
  return invoke("plugin_registry_list");
}

/** 从市场安装；version 缺省取 latest。返回实际安装的版本。 */
export function pluginInstall(id: string, version?: string): Promise<string> {
  return invoke("plugin_install", { id, version: version ?? null });
}

/** 启用 / 停用（热插拔）。 */
export function pluginSetEnabled(id: string, enabled: boolean): Promise<void> {
  return invoke("plugin_set_enabled", { id, enabled });
}

/** 卸载（Windows 文件锁下删目录会延迟到下次启动清理）。 */
export function pluginUninstall(id: string): Promise<void> {
  return invoke("plugin_uninstall", { id });
}

/** 回退到指定版本。返回回退到的版本。 */
export function pluginRollback(id: string, version: string): Promise<string> {
  return invoke("plugin_rollback", { id, version });
}

/** 更新到市场 latest（失败自动回滚）。返回新版本。 */
export function pluginUpdate(id: string): Promise<string> {
  return invoke("plugin_update", { id });
}

/** 订阅插件状态实时同步（§13.4）。 */
export function subscribePluginState(
  onChange: (change: PluginStateChange) => void,
): Promise<UnlistenFn[]> {
  return listen<PluginStateChange>("plugin://state-changed", (e) =>
    onChange(e.payload),
  ).then((un) => [un]);
}

// ---- 设置（DESIGN.md §13.1 / §13.5） ----

/** 读设置投影（永不含 API Key 原值）。 */
export function getSettings(): Promise<SettingsView> {
  return invoke("settings_get");
}

/** 部分更新设置，返回最新投影。 */
export function setSettings(patch: {
  locale?: string;
  appearance?: AppearanceUpdate;
  shortcut?: string;
  llm?: LlmUpdate;
  embedding?: EmbeddingUpdate;
  context?: ContextUpdate;
  memory?: MemoryUpdate;
  tools?: ToolsUpdate;
  shell?: ShellUpdate;
  plugins?: PluginsUpdate;
  chat?: ChatUpdate;
  developer?: DeveloperUpdate;
}): Promise<SettingsView> {
  return invoke("settings_set", {
    locale: patch.locale ?? null,
    appearance: patch.appearance ?? null,
    shortcut: patch.shortcut ?? null,
    llm: patch.llm ?? null,
    embedding: patch.embedding ?? null,
    context: patch.context ?? null,
    memory: patch.memory ?? null,
    tools: patch.tools ?? null,
    shell: patch.shell ?? null,
    plugins: patch.plugins ?? null,
    chat: patch.chat ?? null,
    developer: patch.developer ?? null,
  });
}

/** 会话列表（⑲）：sessions/ 目录 mtime 降序；空会话无文件不出现。 */
export function sessionList(): Promise<SessionMeta[]> {
  return invoke("session_list");
}

/** 会话全量投影（⑲）：整个日志按 runner 同一映射转 UI 事件序列，
 * 前端重放恢复；历史不走 agent:// 事件通道。 */
export function sessionHistory(sessionId: string): Promise<AgentEvent[]> {
  return invoke("session_history", { sessionId });
}

/** 会话重命名（v2.49 §3.6）：旁车 meta.json 标题；空串 = 回落自动标题。 */
export function sessionRename(sessionId: string, title: string): Promise<void> {
  return invoke("session_rename", { sessionId, title });
}

/** 会话↔工作区关联（v2.50 §2.3）：workspace = fsRoots 根目录，null =
 * 移出；后端校验必须是其一根。 */
export function sessionAssociate(
  sessionId: string,
  workspace: string | null,
): Promise<void> {
  return invoke("session_associate", { sessionId, workspace });
}

/** 会话删除（v2.49）：忙锁守卫在后端（运行中默认拒绝，开发者开关可
 * 强制）。幂等。 */
export function sessionDelete(sessionId: string): Promise<void> {
  return invoke("session_delete", { sessionId });
}

/** 会话正文搜索（v2.49）：大小写不敏感子串，mtime 降序限量。 */
export function sessionSearch(query: string, limit?: number): Promise<SessionSearchHit[]> {
  return invoke("session_search", { query, limit: limit ?? null });
}

/** 会话导出 Markdown 转录（v2.49）：path 由 pickSavePath 选定。返回路径。 */
export function sessionExport(sessionId: string, path: string): Promise<string> {
  return invoke("session_export", { sessionId, path });
}

/** 原生目录选择对话框（⑱，UI shell 文档 §7 规则 8）：Rust 侧
 * tauri-plugin-dialog 封装；取消/关窗返回 null。 */
export function pickDirectory(): Promise<string | null> {
  return invoke("pick_directory");
}

/** 原生存储对话框（v2.49 会话导出用）：defaultName 建议文件名；
 * 取消/关窗返回 null。 */
export function pickSavePath(defaultName: string): Promise<string | null> {
  return invoke("pick_save_path", { defaultName });
}

// ---- 应用内通知（v2.51 §3.13）：读 / 全读 / 单读 / 清空 + 推送订阅 ----

export function notificationList(): Promise<NotificationItem[]> {
  return invoke("notification_list");
}

export function notificationReadAll(): Promise<number> {
  return invoke("notification_read_all");
}

export function notificationRead(id: string): Promise<boolean> {
  return invoke("notification_read", { id });
}

export function notificationClear(): Promise<void> {
  return invoke("notification_clear");
}

/** 新通知推送（后端 TauriSink 入账后 emit）。 */
export function subscribeNotificationAdded(
  onAdded: (item: NotificationItem) => void,
): Promise<UnlistenFn> {
  return listen<NotificationItem>("lw://notification", (e) => onAdded(e.payload));
}

/** 子 Agent 转录（v2.52 §3.16）：sub-transcripts/<childId> 全量投影，
 * 与 session_history 同一映射；文件缺席 = reject（页签如实报不可用）。 */
export function subagentTranscript(childId: string): Promise<AgentEvent[]> {
  return invoke("subagent_transcript", { childId });
}

// ---- 模型窗口探测（S2，清单 §5）：Ollama /api/show 自动提取 ----
/** 探测 Ollama 模型上下文窗口（token）；失败 reject（UI 提示手动选择）。 */
export function llmProbeWindow(baseUrl: string, model: string): Promise<number> {
  return invoke("llm_probe_window", { baseUrl: baseUrl || null, model });
}

// ---- embedding 探测（⑬）：嵌一条测试文本返回向量维度 ----

/** 探测 embedding 端点（不落盘）；失败 reject（UI 提示）。 */
export function llmProbeEmbedding(
  baseUrl: string | null,
  model: string,
  apiKey?: string,
): Promise<number> {
  return invoke("llm_probe_embedding", {
    baseUrl: baseUrl || null,
    model,
    apiKey: apiKey || null,
  });
}

// ---- 记忆管理（⑪，Context and Memory.md §7.2） ----

/** 列举记忆：query 空 = 全量（新→旧），否则子串过滤。 */
export function memoryList(query?: string, limit?: number): Promise<MemoryRow[]> {
  return invoke("memory_list", { query: query ?? null, limit: limit ?? null });
}

/** 编辑记忆值（只改 value；按 (workspace, key) 键位定位，键不存在后端报错）。 */
export function memoryUpdate(workspace: string, key: string, value: string): Promise<void> {
  return invoke("memory_update", { workspace, key, value });
}

/** 删除单条记忆（按 (workspace, key) 键位；不存在后端报错）。 */
export function memoryDelete(workspace: string, key: string): Promise<void> {
  return invoke("memory_delete", { workspace, key });
}

/** 清空全部记忆。返回删除条数。 */
export function memoryClearAll(): Promise<number> {
  return invoke("memory_clear_all");
}

/** 最近一次工具暴露截断快照（⑫ 开发者面板；打开分区时拉取）。 */
export function toolExposureDebug(): Promise<ExposureDebug> {
  return invoke("tool_exposure_debug");
}

// ---- 经历记忆（⑬，Context and Memory.md §4.3 宿主侧） ----

/** 列举经历（新→旧）。 */
export function experienceList(limit?: number): Promise<ExperienceRow[]> {
  return invoke("experience_list", { limit: limit ?? null });
}

/** 删除一条经历（不存在后端报错）。 */
export function experienceDelete(id: number): Promise<void> {
  return invoke("experience_delete", { id });
}

/** 清空全部经历。返回删除条数。 */
export function experienceClearAll(): Promise<number> {
  return invoke("experience_clear_all");
}
