//! 记忆注入区类型（⑪，Context 文档 §6.2 MemorySettings 的落地子集——
//! 开关 + 注入条数 + ⑬ 隐式提取开关）：设置原值 / IPC 投影 / 部分更新。
//! 校验与应用逻辑在 settings.rs 的 `update` / `memory_inject_config`。
//! M2（Memory-KV Roadmap）起追加记忆树整理三键：整理节奏 / 增量块 X /
//! 记忆管理器模型（《Memory Tree Design v2》§4.3 / §5.2，§15 可调项）。

use serde::{Deserialize, Serialize};

/// 整理节奏缺省（§4.3 批量档：记忆管理器看得全、切分质量好）。
pub const ORGANIZE_PACING_DEFAULT: &str = "batch";
/// 增量块 X 缺省与合法区间（§15：10K–200K 缺省 50K）。
pub const ORGANIZE_CHUNK_TOKENS_DEFAULT: u64 = 50_000;
pub const ORGANIZE_CHUNK_TOKENS_RANGE: std::ops::RangeInclusive<u64> = 10_000..=200_000;
/// 合法节奏值（未知值保存时拒绝，读取时回落 batch——trigger 侧兜底）。
pub const ORGANIZE_PACING_VALUES: [&str; 2] = ["batch", "incremental"];

// ---- M4 尾巴策略 + collapse（树 v2 §7.2/§8.3/§15；UI shell §3.38②④）----

/// 尾巴策略缺省 = A 边界结算（§7.2：阈值滑条成为唯一旋钮，与 KV v2 经济学
/// 一致）；B 固定窗口是显式实验档。
pub const TAIL_STRATEGY_DEFAULT: &str = "boundary";
/// 合法尾巴策略值（boundary = A 边界结算 / window = B 固定窗口）。
pub const TAIL_STRATEGY_VALUES: [&str; 2] = ["boundary", "window"];
/// 固定窗口 N 缺省与合法区间（§15：1–20 缺省 5 轮）。
pub const TAIL_WINDOW_TURNS_DEFAULT: u32 = 5;
pub const TAIL_WINDOW_TURNS_RANGE: std::ops::RangeInclusive<u32> = 1..=20;
/// memory_collapse 自动收起阈值缺省与合法区间（§15：50–95% 缺省 80）。
pub const COLLAPSE_THRESHOLD_PCT_DEFAULT: u8 = 80;
pub const COLLAPSE_THRESHOLD_PCT_RANGE: std::ops::RangeInclusive<u8> = 50..=95;
/// 整理事实卡单次上限缺省与合法区间（MR 稿 §5 提案 A + §6 预算表「≤3 张/次整理」；
/// 队列 §1 裁定按稿内建议 N=3 且做成可调项）。
pub const FACT_CARD_MAX_DEFAULT: u32 = 3;
pub const FACT_CARD_MAX_RANGE: std::ops::RangeInclusive<u32> = 1..=10;
/// 指令文件链预算（v2.188 · 队列 Q4）：全局链与项目链**各一档**可调（单位 KiB）。
/// ⚠ 缺省 100 不是随手取的——它等于改前那颗写死的 `MAX_FILE_BYTES = 100 * 1024`，
/// 所以「项目里只有一份 `AGENTS.md`」这个存量主形态在改前改后**逐字节相同**
/// （等价性由既有判据 `oversize_file_truncated_with_marker` 的 `x` 计数钉住，不是口头结论）。
/// 分两档而不是合一份：全局链装了用户的跨项目偏好、项目链装了仓库自己的约定，
/// 两者被同一额度挤压时互相挤掉的内容不同（参照实现 PI-Desktop 也是两档）。
pub const INSTRUCTION_GLOBAL_KIB_DEFAULT: u32 = 100;
pub const INSTRUCTION_PROJECT_KIB_DEFAULT: u32 = 100;
/// 合法区间 8–512 KiB。下限 8 = 再小就装不下一份有内容的约定文件（宁给小值也别
/// 让"关不掉"）；上限 512 给"整仓大 AGENTS.md 也能全量进"。参照稿的 32 KiB 在
/// 本仓偏紧（本仓自己的 `AGENTS.md` 实测 24 KiB，加上全局链就顶到额度）。
pub const INSTRUCTION_KIB_RANGE: std::ops::RangeInclusive<u32> = 8..=512;

/// 两档链预算的出界校验（v2.188 · 队列 Q4）：**区间端点只有 `INSTRUCTION_KIB_RANGE`
/// 一个出处**，文案里的范围数字由它现算；`label` 由各调用点传入，因为出界时必须
/// 说清是哪一档（两句都写成通用文案，用户在界面上就会看到「项目链超范围」却改的是
/// 全局链，排障方向反掉——判据 `each_out_of_range_dial_names_itself_in_the_error`）。
///
/// ⚠ 为什么是函数而不是在 `settings/update.rs` 里就地写两遍 `if`：那个文件本批已到
/// **393 行**，就地两遍把区间端点 + 文案复制成两份、行数顶到 407 = 新增超限件
/// （`check_length` 基线 22 判红）。按纪律不抬基线、不压注释，改成一处出处。
pub fn check_instruction_kib(label: &str, n: u32) -> Result<(), String> {
    if INSTRUCTION_KIB_RANGE.contains(&n) {
        return Ok(());
    }
    Err(format!(
        "{label}指令链预算 {n} 超出范围（{}–{} KiB）",
        INSTRUCTION_KIB_RANGE.start(),
        INSTRUCTION_KIB_RANGE.end()
    ))
}

/// F1 记忆文件化总开关缺省（《Memory File-ization Design》§7 已裁定：
/// 关 = 回纯 sqlite，md 不再读写；缺省开——md 是人类层，写失败不破契约）。
pub const FILES_DEFAULT: bool = true;
/// F2 交接提醒缺省（v2.136 项目侧全提醒制：开）——归属会话注入
/// `<repo>/.Locowrk/` 静态提醒段（节点钩子）+ 首次引导建目录。
pub const REPO_CONTEXT_DEFAULT: bool = true;
/// 指令文件层缺省（提示词分层设计 §4.3：开——有文件就注入，无文件零
/// 负担；关 = 用户显式不要 AGENTS.md 面）。
pub const INSTRUCTION_FILES_DEFAULT: bool = true;
/// 整理事实卡缺省（MR 稿 §5 提案 A，用户 2026-10-12 裁定走 A）：**开**。
/// 缺省开的理由不是热情而是算术——这条写路径与整理同频（不是每轮），
/// 成本 = 同一次记录员调用多输出 ≤3 张小 JSON；关掉它的代价 = 事实层
/// 重新回到「只有模型自觉写」那条实测一次都没走过的老路（`memories` 表
/// 0 行就是它的结果）。关 = 提示词逐字节回到无卡段形状 + 一张都不落库。
pub const FACT_CARDS_DEFAULT: bool = true;

/// 尾巴策略枚举（settings 字符串 → 组装判定）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TailStrategy {
    /// A 边界结算：尾巴 = 自上次整理以来全部原文（缺省）。
    Boundary,
    /// B 固定窗口：尾巴 = 最近 N 轮（实验档）。
    Window,
}

impl TailStrategy {
    pub fn parse(s: &str) -> TailStrategy {
        match s {
            "window" => TailStrategy::Window,
            _ => TailStrategy::Boundary,
        }
    }
}

/// 记忆注入设置。top_k 缺省 5（约束清单 §3"注入条数可配"）。
/// v2.136 裁定退役三键：`autoExtract`（自动提取，⑬）/ `handoffUpdate`
/// （F3 收束后台改写）/ `experiences`（向量经历层）——项目记忆改由主
/// 模型自维护 + 提醒钩子；v2.137 退役 `defaultOrganizer`（M5 缺省整理
/// 方式：legacy/标准整理整面退役，一切会话皆树）。settings.json 里的
/// 历史键均为**幻影键**（serde 未知字段忽略，读取不破，下次保存自然消失）。
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct MemorySettings {
    pub enabled: bool,
    pub top_k: u32,
    /// 记忆树整理节奏（M2：batch 批量 / incremental 增量）。
    pub organize_pacing: String,
    /// 增量块 X（token，增量档触发线）。
    pub organize_chunk_tokens: u64,
    /// 记忆管理器模型（§5.2：None/空 = 同主模型）；与 memory_manager_provider
    /// 成对（v2.58 档位制下模型归属服务档）。**持久化键沿用历史名
    /// recorderModel/recorderProvider**（设置文件兼容，术语注见设计文档）。
    #[serde(rename = "recorderModel")]
    pub memory_manager_model: Option<String>,
    #[serde(rename = "recorderProvider")]
    pub memory_manager_provider: Option<String>,
    /// 尾巴策略（M4 §3.38②：boundary = A 边界结算缺省 / window = B 固定
    /// 窗口实验档；只作用于树模式会话）。
    pub tail_strategy: String,
    /// 固定窗口 N 轮（B 档生效，1–20 缺省 5）。
    pub tail_window_turns: u32,
    /// memory_collapse 暴露开关（§8.3 缺省关——收起是空间急救不是省钱）。
    pub collapse_enabled: bool,
    /// 自动收起占用阈值（collapse 开启时生效，50–95% 缺省 80）。
    pub collapse_threshold_pct: u8,
    /// 记忆文件化总开关（F1 §7 缺省开）：关 = 回纯 sqlite，md 不再读写。
    pub files: bool,
    /// 交接提醒（F2 §7 缺省开；v2.136 提醒制）：归属会话注入
    /// `<repo>/.Locowrk/` 静态提醒段 + 首次引导建目录。
    pub repo_context: bool,
    /// 指令文件层（提示词分层设计 §4.3，缺省开）：全局 `<数据根>/AGENTS.md`
    /// 与项目层指令文件以合成 user 消息注入消息层头部（会话首查快照锁字节）。
    /// 项目层 v2.188 起认**三族**（`AGENTS.md` / `CLAUDE.md` / `AGENTS.override.md`，
    /// 族内二选一先到先得、族间串联，override 排在最后 = 追加不替换）。
    pub instruction_files: bool,
    /// 全局指令链预算（KiB，开关开启时生效，8–512 缺省 100）：这一条链的总字节
    /// 上限，额度用尽后面的文件**不读并如实标注**（不静默丢弃）。
    pub instruction_global_kib: u32,
    /// 项目指令链预算（KiB，同上）：三族串联合并后的总上限。
    pub instruction_project_kib: u32,
    /// 整理事实卡（MR §5 提案 A · 队列 Q3，缺省开）：整理收束时记录员在章节 +
    /// 轮级总结之外顺带产出 ≤`fact_card_max` 张事实卡并 upsert 进记忆层，
    /// `source = "organized"`。关 = 提示词与落库两面都回到现役形状。
    pub fact_cards: bool,
    /// 单次整理的卡片上限（开关开启时生效，1–10 缺省 3）。
    pub fact_card_max: u32,
}

impl Default for MemorySettings {
    fn default() -> Self {
        Self {
            enabled: true,
            top_k: 5,
            organize_pacing: ORGANIZE_PACING_DEFAULT.into(),
            organize_chunk_tokens: ORGANIZE_CHUNK_TOKENS_DEFAULT,
            memory_manager_model: None,
            memory_manager_provider: None,
            tail_strategy: TAIL_STRATEGY_DEFAULT.into(),
            tail_window_turns: TAIL_WINDOW_TURNS_DEFAULT,
            collapse_enabled: false,
            collapse_threshold_pct: COLLAPSE_THRESHOLD_PCT_DEFAULT,
            files: FILES_DEFAULT,
            repo_context: REPO_CONTEXT_DEFAULT,
            instruction_files: INSTRUCTION_FILES_DEFAULT,
            instruction_global_kib: INSTRUCTION_GLOBAL_KIB_DEFAULT,
            instruction_project_kib: INSTRUCTION_PROJECT_KIB_DEFAULT,
            fact_cards: FACT_CARDS_DEFAULT,
            fact_card_max: FACT_CARD_MAX_DEFAULT,
        }
    }
}

/// 记忆注入区投影（结构即设置原值，无生效换算）。
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MemorySettingsView {
    pub enabled: bool,
    pub top_k: u32,
    pub organize_pacing: String,
    pub organize_chunk_tokens: u64,
    #[serde(rename = "recorderModel")]
    pub memory_manager_model: Option<String>,
    #[serde(rename = "recorderProvider")]
    pub memory_manager_provider: Option<String>,
    pub tail_strategy: String,
    pub tail_window_turns: u32,
    pub collapse_enabled: bool,
    pub collapse_threshold_pct: u8,
    pub files: bool,
    pub repo_context: bool,
    pub instruction_files: bool,
    pub instruction_global_kib: u32,
    pub instruction_project_kib: u32,
    pub fact_cards: bool,
    pub fact_card_max: u32,
}

/// 记忆注入区部分更新（settings_set 入参）：字段 None = 保持不变。
/// memory_manager_model / memory_manager_provider 传**空串 = 清除**（回落同主模型）
/// ——与 base_url/api_key 同约定（双层 Option 的 null 在 serde_json 里
/// 与"字段缺失"不可区分，无法表达清除）。
#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct MemoryUpdate {
    pub enabled: Option<bool>,
    pub top_k: Option<u32>,
    pub organize_pacing: Option<String>,
    pub organize_chunk_tokens: Option<u64>,
    #[serde(rename = "recorderModel")]
    pub memory_manager_model: Option<String>,
    #[serde(rename = "recorderProvider")]
    pub memory_manager_provider: Option<String>,
    pub tail_strategy: Option<String>,
    pub tail_window_turns: Option<u32>,
    pub collapse_enabled: Option<bool>,
    pub collapse_threshold_pct: Option<u8>,
    pub files: Option<bool>,
    pub repo_context: Option<bool>,
    pub instruction_files: Option<bool>,
    /// 全局指令链预算（KiB，出界 = `settings_set` 拒绝，8–512 缺省 100）。
    pub instruction_global_kib: Option<u32>,
    /// 项目指令链预算（KiB，出界同上）。
    pub instruction_project_kib: Option<u32>,
    /// 整理事实卡开关（`Some(false)` = 关掉：提示词不再要卡、落库一张不写）。
    pub fact_cards: Option<bool>,
    /// 单次整理卡片上限（出界 = `settings_set` 拒绝，1–10 缺省 3）。
    pub fact_card_max: Option<u32>,
}
