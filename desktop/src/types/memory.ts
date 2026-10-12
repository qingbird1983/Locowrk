/**
 * 记忆面：记忆设置、记忆行、记忆树（章节捆 / 整理记录 / 项目树）与嵌入视图。
 *
 * 由批 F-22（v2.175）从 `desktop/src/types.ts` 目录化拆出：**每个类型声明块逐字节原样搬移**，
 * 只改两处——①按引用面决定 `export` 与否（外部 import 过的与兄弟簇要用的留 `export`，
 * 其余降为模块内部）；②跨簇引用走 `import type`。等价判据见 scripts/../tmp/v180-equiv.mjs。
 */

import type { RunSummaryView } from "./session";

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
  /** 指令文件层（缺省开）：全局 `<数据根>/AGENTS.md` + 项目层三族候选
   * （AGENTS.md / CLAUDE.md / AGENTS.override.md，族内二选一、族间串联）
   * 注入消息层头部。 */
  instructionFiles: boolean;
  /** 全局指令链预算（KiB，8–512 缺省 100；v2.188 队列 Q4）。 */
  instructionGlobalKib: number;
  /** 项目指令链预算（KiB，同上）：三族串联后的总上限，额度用尽后面的族不读。 */
  instructionProjectKib: number;
  /** M2 整理事实卡（缺省开，v2.186）：整理收束时记录员顺带产出 ≤上限 张卡落记忆层。 */
  factCards: boolean;
  /** 每次整理最多几张卡（1–10 缺省 3，factCards 开启时生效）。 */
  factCardMax: number;
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
  instructionFiles?: boolean;
  /** 全局指令链预算（KiB，出界 = 后端拒绝并点名档位）。 */
  instructionGlobalKib?: number;
  /** 项目指令链预算（KiB，同上）。 */
  instructionProjectKib?: number;
  /** M2 整理事实卡总开关（v2.186，缺省开）。 */
  factCards?: boolean;
  /** 单次整理最多产几张卡（1–10 缺省 3，仅在 factCards 开时生效）。 */
  factCardMax?: number;
}

/** 记忆条目（memory_list 返回行；snake_case 与既有 IPC 投影一致）。
 * source = "explicit" | "extracted" | "organized"（前端 SourceBadge 按档位
 * 渲染人类文案；"organized" = v2.186 整理副产物，见 §3.4 来源族）；
 * workspace = 项目归属（㉘），"" = 未归属。 */
export interface MemoryRow {
  key: string;
  value: string;
  tags: string;
  source: string;
  workspace: string;
  updated_at: number;
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

interface TreeOrganizeRecord {
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
  /** 轮级总结链（v2.141）：按整理收束序，渲染在对应章节组上方。 */
  summaries: RunSummaryView[];
}

/** 项目联合视图（v2.141 扩展）：跨会话捆平铺 + 跨会话总结链（时间正序
 * = 项目进展叙事锚点）。 */
export interface ProjectTreeView {
  chapters: ProjectChapterView[];
  summaries: RunSummaryView[];
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
type TreeOriginalKind = "user" | "assistant" | "tool_call" | "tool_result" | "summary";

export interface TreeOriginalItem {
  index: number;
  kind: TreeOriginalKind;
  text: string;
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
