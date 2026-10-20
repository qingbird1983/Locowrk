/**
 * 记忆面：记忆设置、记忆行与嵌入视图。
 *
 * 由批 F-22（v2.175）从 `desktop/src/types.ts` 目录化拆出：**每个类型声明块逐字节原样搬移**，
 * 只改两处——①按引用面决定 `export` 与否（外部 import 过的与兄弟簇要用的留 `export`，
 * 其余降为模块内部）；②跨簇引用走 `import type`。等价判据见 scripts/../tmp/v180-equiv.mjs。
 *
 * S4 刀③b：记忆树（`TreeDebug` / `TreeSummaryView` / 章节 / 项目视图 / 会话流）随
 * organize 宿主模块整面删除；其余记忆管理面类型留到刀③c 与后端同删。
 */

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
  /** 项目记忆层（缺省开）：关 = `<memory_index>` 里的 `[project]` 段整节缺席。 */
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
}

/** 记忆条目（memory_list 返回行；snake_case 与既有 IPC 投影一致）。
 * source = "explicit" | "extracted" | "organized"（前端 SourceBadge 按档位
 * 渲染人类文案；"organized" = v2.186 整理副产物，见 §3.4 来源族）；
 * workspace = 项目归属（㉘），"" = 未归属。
 * type = 分类，值域固定六个：`user` / `feedback` / `project` / `reference` 四类型 +
 * `fact`（= 未分类，读侧 `normalize_type` 把四类型之外的一切——旧文件的 `fact`、人乱写的、
 * 根本没这行——都收成它）。所以前端**不必**再判一次合法性，只需按六个键出文案。 */
export interface MemoryRow {
  key: string;
  value: string;
  tags: string;
  type: string;
  source: string;
  workspace: string;
  updated_at: number;
}

// ---- 层区与回收区（S1 刀④c · ADR-0007 §18「每层一行」）----

/** 一层的一行。`hint` 是层的身份（`''` = 全局层，其余是 `<可读名>-<哈希>` 目录名），
 * 「删除该层」「打开该层文件夹」都拿它原样传回——显示、清除、打开三件事指的是后端
 * 同一个 `LayerDir`，前端不参与算路径。 */
export interface LayerRow {
  hint: string;
  fact_count: number;
  /** 该层最新一条的 `metadata.updated`（秒）；一条都没有 = 0，界面显示"—"。 */
  newest_updated_at: number;
  /** 该层没能读出来的文件数（不许让 `fact_count` 冒充全量）。 */
  unreadable: number;
}

/** 层区载荷。两个"没能读出来"的数必须显示：少了它们，"整层读不动"就冒充"这层没记过"。 */
export interface LayersView {
  rows: LayerRow[];
  blocked_layers: number;
  unreadable_files: number;
}

/** 回收区一行（`obsolete/` 里的一份镜像）。`rel` 是相对回收区根的路径，恢复时原样传回。 */
export interface ObsoleteRow {
  rel: string;
  key: string;
  /** 原来在哪一层（同样是 hint）。 */
  workspace: string;
  /** 删除时刻（毫秒）。 */
  deleted_at_ms: number;
}

export interface ObsoleteView {
  rows: ObsoleteRow[];
  /** 回收区里没能列出来的条目数（同上，"空列表"与"读不动"必须是两句话）。 */
  unreadable: number;
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

/** memory_inject_view 命令载荷（管理页只读区块）。snake_case 与既有 IPC 投影一致。
 * ⚠ 这里原先还有一档 `explain`（`InjectExplain` + `InjectRow`：最近一次"真算过四信号"
 * 的回执、四档千分比占比、present/unreadable 两态）——它的生产者与 Rust 侧
 * `inject_view::InjectExplain` 同刀退役（S3 刀③b，裁定 = specs/10 开工三问②）。
 * 为什么整块删而不是留一个恒假的字段：面板拿到 `present: false` 只能演成"没得说"，
 * 而那件事现在**每个会话都成立**，界面就变成一格永远空着的框。
 * 将来若要恢复"这一轮注入了哪几条"，正解是给索引段埋点（specs/10 欠账 ⓠ）。 */
export interface InjectView {
  overview: InjectOverview;
}

