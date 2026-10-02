//! 工具暴露区类型（⑫，exposure 文档 §5.3/§6：N=20 可配 + 约束清单 §3
//! "每次请求最大工具数可配"——同一个旋钮）：设置原值 / IPC 投影 / 部分
//! 更新，附 [`kernel_agent::ToolExposureConfig`] 的换算（与 memory 区
//! 不同，这里连"生效配置"构造一起同居本文件——settings.rs 的 400 行
//! 纪律；校验与应用逻辑仍在 settings.rs 的 `update`）。

use std::time::Duration;

use serde::{Deserialize, Serialize};

use kernel_agent::ToolExposureConfig;

use crate::runner;

/// 工具暴露设置。max_per_request 缺省 20（exposure 文档 §5.3 Top-N）。
/// call_timeout_secs 缺省 30（S2，约束最小化清单 §3"单个插件最长执行
/// 时间可配"——宿主 Timeout 中间件热换入，原硬编码 30s）。fs_roots 缺省
/// 空（⑱：工具文件面只放行数据根，与 fsutil 缺省语义一致；旧 settings.json
/// 无此字段零迁移）。max_turn_steps 缺省 8（㉕·微调③：桌面 runner 单轮
/// step 兜底阈值可配，原硬编码；每轮开跑时取值，保存即对下一轮生效）。
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ToolsSettings {
    pub enabled: bool,
    pub max_per_request: u32,
    pub call_timeout_secs: u32,
    /// ⑱ 工作区目录（fsutil `LOCOWRK_FS_ROOTS` 的 UI 面）：tool-fs /
    /// tool-edit / tool-git / tool-shell 的允许根。存的是用户选择的原始
    /// 路径（canonicalize 归 fsutil 使用时做）。
    pub fs_roots: Vec<String>,
    /// 单轮步数上限（runner 兜底：超过强制收束本轮并报错）。
    pub max_turn_steps: u32,
}

impl Default for ToolsSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            max_per_request: 20,
            call_timeout_secs: 30,
            fs_roots: Vec::new(),
            max_turn_steps: runner::DEFAULT_MAX_STEPS,
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
    pub fs_roots: Vec<String>,
    pub max_turn_steps: u32,
}

/// 工具暴露区部分更新（settings_set 入参）：字段 None = 保持不变。
#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ToolsUpdate {
    pub enabled: Option<bool>,
    pub max_per_request: Option<u32>,
    pub call_timeout_secs: Option<u32>,
    pub fs_roots: Option<Vec<String>>,
    pub max_turn_steps: Option<u32>,
}

/// call_timeout_secs 合法区间（秒）：下限避开 0（等同关超时），上限覆盖
/// 最慢的 MCP 调用；模型侧 tool-shell 的 arg 上限 120s 在此之下不受影响。
pub const CALL_TIMEOUT_RANGE: std::ops::RangeInclusive<u32> = 5..=3600;

/// ㉕·微调③ 单轮步数上限合法区间：下限 1（0 等于拒绝任何工具轮），
/// 上限 100 防手滑打开失控口子；子代理预算（subagent max_turns 上限 32）
/// 是另一套旋钮，与此互不影响。
pub const MAX_TURN_STEPS_RANGE: std::ops::RangeInclusive<u32> = 1..=100;

/// ⑱ fs_roots 归一化 + 存在性校验：去首尾空白、丢空段、按首次出现去重；
/// 任何目录不存在即拒绝（与 fsutil `allowed_roots()` 的 fail-closed 同
/// 契约——目录被移动/删除后 fsutil 会在调用时报错，这里在保存时报）。
pub fn normalize_fs_roots(roots: &[String]) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::new();
    for r in roots {
        let t = r.trim();
        if !t.is_empty() && !out.iter().any(|e| e == t) {
            out.push(t.to_string());
        }
    }
    for p in &out {
        if !std::path::Path::new(p).is_dir() {
            return Err(format!(
                "工作区目录不存在或不是目录: {p}（被移动/删除后请移除并重新选择）"
            ));
        }
    }
    Ok(out)
}

impl crate::settings::SettingsStore {
    /// 后端专用：生效截断配置（ToolExposure 共享锁的内容物，⑫）。
    pub(crate) fn exposure_config(&self) -> ToolExposureConfig {
        let t = self.inner.lock().expect("settings poisoned").tools.clone();
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

    /// 后端专用：单轮步数上限（㉕·微调③，runner 每轮开跑时取一次）。
    pub fn max_turn_steps(&self) -> u32 {
        self.inner
            .lock()
            .expect("settings poisoned")
            .tools
            .max_turn_steps
    }

    /// 后端专用：工作区目录快照（⑱ env 注入 / 重注入用）。
    pub fn fs_roots(&self) -> Vec<String> {
        self.inner
            .lock()
            .expect("settings poisoned")
            .tools
            .fs_roots
            .clone()
    }
}
