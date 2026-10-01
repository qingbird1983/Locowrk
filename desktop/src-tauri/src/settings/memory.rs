//! 记忆注入区类型（⑪，Context 文档 §6.2 MemorySettings 的落地子集——
//! 开关 + 注入条数 + ⑬ 隐式提取开关）：设置原值 / IPC 投影 / 部分更新。
//! 校验与应用逻辑在 settings.rs 的 `update` / `memory_inject_config`。

use serde::{Deserialize, Serialize};

/// 记忆注入设置。top_k 缺省 5（约束清单 §3"注入条数可配"）；
/// auto_extract（⑬ 隐式记忆提取）缺省关——"易记垃圾"，文档明说放最后
/// 且默认不自动记，开启后回合结束异步提取事实/经历。
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct MemorySettings {
    pub enabled: bool,
    pub top_k: u32,
    pub auto_extract: bool,
}

impl Default for MemorySettings {
    fn default() -> Self {
        Self {
            enabled: true,
            top_k: 5,
            auto_extract: false,
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
}

/// 记忆注入区部分更新（settings_set 入参）：字段 None = 保持不变。
#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct MemoryUpdate {
    pub enabled: Option<bool>,
    pub top_k: Option<u32>,
    pub auto_extract: Option<bool>,
}
