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
}
