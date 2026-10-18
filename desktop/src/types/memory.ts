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
  /** F1 记忆文件化设置项。⚠ 它现在**没有作用面**：真源 = md，"关 = 回纯 sqlite"
   * 那一档随 S1 刀②c/③b 消失，字段只剩 reconcile 与首启导出的空转门（去处见 specs/05 刀④）。 */
  files: boolean;
  /** F2 交接提醒（缺省开，v2.136 提醒制）：归属会话注入静态提醒段。
   * 这一枚就是 ADR-0007 §18 三枚行为开关里的**交接层**那一枚。 */
  repoContext: boolean;
  /** 全局记忆层（S1 刀④b，缺省开）：关 = `''` 键位那批偏好不再自动进上下文。
   * 不管两件事：模型主动查仍读得到、管理页仍列全量（关的是注入，不是数据）。 */
  globalLayer: boolean;
  /** 项目记忆层（缺省开）：关 = `<project_memory_index>` 整节缺席。 */
  projectLayer: boolean;
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
  /** Q9 学习档总开关（缺省开）：关 = 注入排序逐字节回到现役，且命中写回一起停。 */
  learnEnabled: boolean;
  /** 四信号权重（千分比 0–1000，各自独立——四档之和不要求等于 1000）。 */
  relevanceWeight: number;
  strengthWeight: number;
  freshnessWeight: number;
  recencyWeight: number;
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
  /** 全局记忆层开关（`false` = 只停注入，不删文件、不动管理页）。 */
  globalLayer?: boolean;
  /** 项目记忆层开关（同上，管的是那一节索引）。 */
  projectLayer?: boolean;
  instructionFiles?: boolean;
  /** 全局指令链预算（KiB，出界 = 后端拒绝并点名档位）。 */
  instructionGlobalKib?: number;
  /** 项目指令链预算（KiB，同上）。 */
  instructionProjectKib?: number;
  /** M2 整理事实卡总开关（v2.186，缺省开）。 */
  factCards?: boolean;
  /** 单次整理最多产几张卡（1–10 缺省 3，仅在 factCards 开时生效）。 */
  factCardMax?: number;
  /** Q9 学习档开关（`false` = 排序与写回一起回到现役）。 */
  learnEnabled?: boolean;
  /** 四信号权重（千分比 0–1000；出界 = 后端拒绝并点名是哪一档）。 */
  relevanceWeight?: number;
  strengthWeight?: number;
  freshnessWeight?: number;
  recencyWeight?: number;
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

/** 队列 Q10（L4 / ML 稿 §3.1 原则 4）：一次注入里**一条交付**的四信号回执。
 * `share` 是千分比四档，**按位** = [相关性, 强度, 新鲜度, 时间]（与内核
 * `LearnWeights` 字段序同源，Rust 侧 `inject_view::ExplainRow`）。 */
export interface InjectRow {
  key: string;
  share: number[];
  total: number;
}

/** 最近一次"真算过四信号"的注入回执。`present=false` = 库里没有带回执的行
 * （学习档关着或一次注入都没发生）；`unreadable=true` = 有回执但那串读不成
 * 面板认得的形状（两种状态在界面上必须分开说，见 `inject_view.rs` 头注）。 */
export interface InjectExplain {
  present: boolean;
  unreadable: boolean;
  ts: number;
  session_key: string;
  scope_id: string;
  weights: number[];
  rows: InjectRow[];
}

/** 观测表的派生读数（L0 那两个比率 + 三个计数）。两个比率 **null 与 0 不是一回事**：
 * null = "没注入过 / 没键可算"，0 = "注入了但结果为空 / 一个没用上"。 */
export interface InjectOverview {
  injections: number;
  injected_keys: number;
  yielded_keys: number;
  store_empty_ratio: number | null;
  injection_yield: number | null;
}

/** memory_inject_view 命令载荷（管理页只读解释区块）。snake_case 与既有 IPC 投影一致。 */
export interface InjectView {
  explain: InjectExplain;
  overview: InjectOverview;
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
  /** 轮级总结条数（Q14 第三刀）：左栏会话行「N 总结 · N 章节」的数在清单侧一次给出。 */
  summaryCount: number;
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

/** 会话流小节（队列 Q14 · MB §2.3）：一个已定稿章节的原文切片。
 * rows 的 index 是会话日志**绝对**事件位，跨节连续 ⇒ 定位语义成立。
 * error 非空 = 本节切片读不出（区间越出日志），此时 rows 为空、其余小节照常。 */
export interface StreamSection {
  chapterId: string;
  title: string;
  source: string;
  /** 该章有人改过（标题或摘要任一手动标记；逐字段区分看 TreeChapterView）。 */
  edited: boolean;
  from: number;
  to: number;
  rows: TreeOriginalItem[];
  error: string | null;
}

/** 整组会话流（看板第 4 列一块面板；sections 按事件区间升序 = 事件序）。
 * sections 为空时 from/to 是 0 ⇒ 判空态看 sections.length，别看区间。 */
export interface StreamView {
  sections: StreamSection[];
  from: number;
  to: number;
  totalRows: number;
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
