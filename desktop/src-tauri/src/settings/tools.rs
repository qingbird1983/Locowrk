//! 工具暴露区类型（⑫，exposure 文档 §5.3/§6：N=20 可配 + 约束清单 §3
//! "每次请求最大工具数可配"——同一个旋钮）：设置原值 / IPC 投影 / 部分
//! 更新，附 [`kernel_agent::ToolExposureConfig`] 的换算（与 memory 区
//! 不同，这里连"生效配置"构造一起同居本文件——settings.rs 的 400 行
//! 纪律；校验与应用逻辑仍在 settings.rs 的 `update`）。

use std::time::Duration;

use serde::{Deserialize, Serialize};

use kernel_agent::ToolExposureConfig;

/// 工具暴露设置。max_per_request 缺省 20（exposure 文档 §5.3 Top-N）。
/// call_timeout_secs 缺省 30（S2，约束最小化清单 §3"单个插件最长执行
/// 时间可配"——宿主 Timeout 中间件热换入，原硬编码 30s）。
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ToolsSettings {
    pub enabled: bool,
    pub max_per_request: u32,
    pub call_timeout_secs: u32,
}

impl Default for ToolsSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            max_per_request: 20,
            call_timeout_secs: 30,
        }
    }
}

/// 工具暴露区投影（结构即设置原值，无生效换算）。
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ToolsSettingsView {
    pub enabled: bool,
    pub max_per_request: u32,
    pub call_timeout_secs: u32,
}

/// 工具暴露区部分更新（settings_set 入参）：字段 None = 保持不变。
#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ToolsUpdate {
    pub enabled: Option<bool>,
    pub max_per_request: Option<u32>,
    pub call_timeout_secs: Option<u32>,
}

/// call_timeout_secs 合法区间（秒）：下限避开 0（等同关超时），上限覆盖
/// 最慢的 MCP 调用；模型侧 tool-shell 的 arg 上限 120s 在此之下不受影响。
pub const CALL_TIMEOUT_RANGE: std::ops::RangeInclusive<u32> = 5..=3600;

impl crate::settings::SettingsStore {
    /// 后端专用：生效截断配置（ToolExposure 共享锁的内容物，⑫）。
    pub(crate) fn exposure_config(&self) -> ToolExposureConfig {
        let t = self
            .inner
            .lock()
            .expect("settings poisoned")
            .tools
            .clone();
        ToolExposureConfig {
            enabled: t.enabled,
            max_tools: t.max_per_request as usize,
        }
    }

    /// 后端专用：宿主 Timeout 中间件时长（S2 热换入共享锁的内容物）。
    pub fn call_timeout(&self) -> Duration {
        Duration::from_secs(
            self.inner
                .lock()
                .expect("settings poisoned")
                .tools
                .call_timeout_secs as u64,
        )
    }
}
