//! 记忆注入区类型（⑪，Context 文档 §6.2 MemorySettings 的落地子集——
//! 开关 + 注入条数 + ⑬ 隐式提取开关）：设置原值 / IPC 投影 / 部分更新。
//! 校验与应用逻辑在 settings.rs 的 `update` / `memory_inject_config`。
//! M2（Memory-KV Roadmap）起追加记忆树整理三键：整理节奏 / 增量块 X /
//! 会话记录员模型（《Memory Tree Design v2》§4.3 / §5.2，§15 可调项）。

use serde::{Deserialize, Serialize};

/// 整理节奏缺省（§4.3 批量档：记录员看得全、切分质量好）。
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

/// F1 记忆文件化总开关缺省（《Memory File-ization Design》§7 已裁定：
/// 关 = 回纯 sqlite，md 不再读写；缺省开——md 是人类层，写失败不破契约）。
pub const FILES_DEFAULT: bool = true;

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

/// 记忆注入设置。top_k 缺省 5（约束清单 §3"注入条数可配"）；
/// auto_extract（⑬ 隐式记忆提取）缺省关——"易记垃圾"，文档明说放最后
/// 且默认不自动记，开启后回合结束异步提取事实/经历。
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct MemorySettings {
    pub enabled: bool,
    pub top_k: u32,
    pub auto_extract: bool,
    /// 记忆树整理节奏（M2：batch 批量 / incremental 增量）。
    pub organize_pacing: String,
    /// 增量块 X（token，增量档触发线）。
    pub organize_chunk_tokens: u64,
    /// 记录员模型（§5.2：None/空 = 同主模型）；与 recorder_provider
    /// 成对（v2.58 档位制下模型归属服务档）。
    pub recorder_model: Option<String>,
    pub recorder_provider: Option<String>,
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
}

impl Default for MemorySettings {
    fn default() -> Self {
        Self {
            enabled: true,
            top_k: 5,
            auto_extract: false,
            organize_pacing: ORGANIZE_PACING_DEFAULT.into(),
            organize_chunk_tokens: ORGANIZE_CHUNK_TOKENS_DEFAULT,
            recorder_model: None,
            recorder_provider: None,
            tail_strategy: TAIL_STRATEGY_DEFAULT.into(),
            tail_window_turns: TAIL_WINDOW_TURNS_DEFAULT,
            collapse_enabled: false,
            collapse_threshold_pct: COLLAPSE_THRESHOLD_PCT_DEFAULT,
            files: FILES_DEFAULT,
        }
    }
}

/// 记忆注入区投影（结构即设置原值，无生效换算）。
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MemorySettingsView {
    pub enabled: bool,
    pub top_k: u32,
    pub auto_extract: bool,
    pub organize_pacing: String,
    pub organize_chunk_tokens: u64,
    pub recorder_model: Option<String>,
    pub recorder_provider: Option<String>,
    pub tail_strategy: String,
    pub tail_window_turns: u32,
    pub collapse_enabled: bool,
    pub collapse_threshold_pct: u8,
    pub files: bool,
}

/// 记忆注入区部分更新（settings_set 入参）：字段 None = 保持不变。
/// recorder_model / recorder_provider 传**空串 = 清除**（回落同主模型）
/// ——与 base_url/api_key 同约定（双层 Option 的 null 在 serde_json 里
/// 与"字段缺失"不可区分，无法表达清除）。
#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct MemoryUpdate {
    pub enabled: Option<bool>,
    pub top_k: Option<u32>,
    pub auto_extract: Option<bool>,
    pub organize_pacing: Option<String>,
    pub organize_chunk_tokens: Option<u64>,
    pub recorder_model: Option<String>,
    pub recorder_provider: Option<String>,
    pub tail_strategy: Option<String>,
    pub tail_window_turns: Option<u32>,
    pub collapse_enabled: Option<bool>,
    pub collapse_threshold_pct: Option<u8>,
    pub files: Option<bool>,
}
