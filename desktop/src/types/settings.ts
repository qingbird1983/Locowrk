/**
 * 设置页的视图与补丁（`*View` = 读面，`*Update` = 写面），以及开发者分区与曝光调试。
 *
 * 由批 F-22（v2.175）从 `desktop/src/types.ts` 目录化拆出：**每个类型声明块逐字节原样搬移**，
 * 只改两处——①按引用面决定 `export` 与否（外部 import 过的与兄弟簇要用的留 `export`，
 * 其余降为模块内部）；②跨簇引用走 `import type`。等价判据见 scripts/../tmp/v180-equiv.mjs。
 */

import type { LlmSettingsView } from "./llm";
import type { EmbeddingView, MemorySettingsView } from "./memory";
import type { PluginsView } from "./plugins";

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
  plugins: PluginsView;
  /** 对话行为区（UI shell 文档 §3.2 ChatSection / §2.7 列宽偏好）。 */
  chat: ChatSettingsView;
  /** 开发者选项区（v2.49，默认隐藏分区）。 */
  developer: DeveloperView;
  /** 思考语言区（docs/design/NEW/language of thought.md）：reasoning 用的
   * 语言；会话首请求锁死，中途修改只影响新会话。 */
  thinking: ThinkingView;
}

/** 对话行为区投影：结构即设置原值。contentMaxWidth null = 未设置（前端按
 * 缺省 760 渲染）。 */
interface ChatSettingsView {
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

/** 工具暴露区投影（⑫）：结构即设置原值。S2 起含宿主单次调用超时；
 * ⑱ 起含 fsRoots 工作区目录（空 = 工具文件面仅数据目录）；
 * ㉕·微调③ 起含单轮步数上限。 */
interface ToolsSettingsView {
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

/** 外观区投影（⑰）：明暗主题 + 字号基座。生效点分别在前端 lib/theme.ts（写
 * `<html data-theme>`）与 lib/ui-font.ts（写 `--lw-ui-font-size`），后端只存取校验。 */
interface AppearanceView {
  theme: string;
  /** 相对字号六档的共同基座（px）。缺省 14 = 本可调项出现前 style.css 里写死的那颗；
   *  值域 12–18 与后端 `settings/appearance.rs` 的 `UI_FONT_{MIN,MAX}` 同一把尺，
   *  由 tmp/q14d-shape.mjs 三面（Rust 常量 / TS 常量 / CSS 字面）对账钉住。 */
  uiFontSize: number;
}

/** 外观区部分更新：字段缺省 = 保持不变。 */
export interface AppearanceUpdate {
  theme?: string;
  uiFontSize?: number;
}

/** 上下文管理区投影（⑨；v2.137 整理阈值 = 绝对 token——「上下文达到
 * 多少 K 就整理」，百分比/策略/稳定期三键随 ctx-compact 桌面链退役。
 * Q8 加第三键工具结果预算：KiB 原样传，**前端不做 KiB→字节的第二套算术**，
 * 0 = 不限制由文案承担显示）。 */
interface ContextSettingsView {
  /** 整理阈值（绝对 token，100K–800K 缺省 100K）。 */
  thresholdTokens: number;
  /** 阈值用量进度条显隐（v4.18，缺省开）。 */
  usageBar: boolean;
  /** 工具结果预算（KiB，0–512 缺省 24；0 = 不限制）。 */
  toolResultBudgetKib: number;
}

/** 上下文管理区部分更新（窗口走 LlmUpdate）。 */
export interface ContextUpdate {
  thresholdTokens?: number;
  usageBar?: boolean;
  toolResultBudgetKib?: number;
}

/** 开发者选项区投影（v2.49）：unlocked = 分区可见（安卓式连点版本号
 * 解锁）；其余是危险开关，缺省全关。 */
interface DeveloperView {
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
  /** 在途收束后允许连续补跑几次（队列 Q6，0–10 缺省 3；0 = 关掉补跑）。 */
  organizeMaxRerun: number;
  /** 一轮之内至多泄压几次（队列 Q8b，0–5 缺省 2；0 = 关掉轮内泄压）。 */
  organizeMaxRelease: number;
  /** 轮内泄压的水位：占会话生效窗口的百分比（队列 Q8b，50–99 缺省 90）。 */
  organizeReleaseWatermarkPct: number;
  /** 一次摘要补写至多处理几章（队列 Q8c，0–20 缺省 6；0 = 关掉补写）。 */
  organizeSummaryChapters: number;
  /** 同一章至多补写尝试几次（队列 Q8c，0–5 缺省 2；跨轮计数落投影）。 */
  organizeSummaryAttempts: number;
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
  organizeMaxRerun?: number;
  organizeMaxRelease?: number;
  organizeReleaseWatermarkPct?: number;
  organizeSummaryChapters?: number;
  organizeSummaryAttempts?: number;
}

/** 网络代理档（§8.5）：system / direct / custom（custom 必带 url）。 */
export interface NetworkProxySettings {
  mode: "system" | "direct" | "custom";
  url: string | null;
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

/** 批量操作结果（⑩⑥）：逐条成败，部分失败不回滚。 */
export interface BatchResult {
  ok: string[];
  failed: { id: string; error: string }[];
}

/** 思考语言区投影。 */
interface ThinkingView {
  /** follow_ui（缺省，跟随界面）| zh-CN | en-US | none。 */
  language: string;
}
