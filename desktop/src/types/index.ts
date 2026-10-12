/**
 * `desktop/src/types.ts` 的目录化后继（批 F-22 / v2.175）。
 *
 * 这里只做一件事：**把外部真正 import 过的类型名原样转发出去**，让 40 处
 * `from "../types"` 的引用点一行都不用改（`moduleResolution: bundler` 会把
 * `../types` 解析到本文件）。
 *
 * 名单 = 实测 import 具名集合 56 条，不多不少：多一条 = 又把"谁在用"糊回去，
 * 少一条 = vue-tsc 当场报找不到名字。另有 4 条只在簇与簇之间流转（例如
 * `MemorySettingsView` 被 settings.ts 引、外面没人引）⇒ 它们在簇文件里是 `export`
 * 但**故意不进 barrel**：barrel 的职责是"对外表面"，不是"内部布线表"。
 *
 * `settings.ts ⇄ llm.ts` 有一对簇级互相引用：type-only import 在编译期擦除，
 * 不产生运行时环，登记在此说明而不是藏着。
 */

export type { AgentEvent, ApprovalRequest, AskUserEvent, SessionTitlePayload } from "./events-agent";

export type { BrowserOpenPayload, CacheBreakReport, NotificationItem, OrganizeDonePayload, OrganizePausedPayload, ReconcileSummary, RunSummaryView, SessionMeta, SessionSchedule, SessionSearchHit } from "./session";

export type { AppearanceUpdate, BatchResult, ChatUpdate, ContextUpdate, DeveloperUpdate, ExposureDebug, SettingsView, ToolsUpdate } from "./settings";

export type { EmbeddingUpdate, MemoryRow, MemoryUpdate, ProjectChapterView, ProjectTreeView, TreeChapterView, TreeChaptersView, TreeDebug, TreeOriginalItem, TreeSummaryView } from "./memory";

export type { CatalogModelInfo, CatalogStatus, ImportApplyReport, ImportCandidate, ImportScanOutcome, LlmSettingsView, LlmUpdate, ModelTestResult, ProbeOutcome, ProviderInput, ProviderModelPricing, ProviderModelSources, ProviderModelView, ProviderModels, ProviderView, SubagentPin } from "./llm";

export type { AssetRefDto, ContentPartDto, IdleInfo, InstalledPlugin, MarketplaceItem, PluginStateChange, PluginUsageView, PluginsUpdate } from "./plugins";
