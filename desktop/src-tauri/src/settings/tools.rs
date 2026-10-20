//! 工具暴露区类型（⑫，exposure 文档 §5.3/§6：N=20 可配 + 约束清单 §3
//! "每次请求最大工具数可配"——同一个旋钮）：设置原值 / IPC 投影 / 部分
//! 更新，附 [`kernel_agent::ToolExposureConfig`] 的换算（与其它分区
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
/// approval_timeout_secs 缺省 100（v2.49 可配置原则 §七-16①：审批守夜
/// 阈值不写死——原 APPROVAL_TIMEOUT 常量；0 = 一直等待，保存即热换入
/// ApprovalHub）。default_permission_mode 缺省 "ask"（㉘·B：新会话
/// 权限档缺省，会话级可覆盖并存旁车）。
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
    /// 单轮步数上限（runner 兜底：到顶自动收束本轮并如实告知，LLM.md v3.0）。
    pub max_turn_steps: u32,
    /// 审批等待时长（秒，0 = 不限）：超时按拒绝收束（fail-closed）。
    pub approval_timeout_secs: u64,
    /// ㉘·B 新会话权限档缺省（"plan" / "ask" / "acceptEdits" / "auto"）；
    /// 会话级覆盖存旁车（PermissionModeStore）。
    pub default_permission_mode: String,
    /// 工具集会话钉死（KV v2 §4.2，缺省开）：暴露集会话首次请求确定并
    /// 缓存，tools 块逐字节稳定保前缀缓存；关 = 恢复每轮 Top-N 旧行为。
    pub pin_session: bool,
    /// v4.11 §3.39① 选择卡守夜（秒，0 = 不限）：`ask_user` 超时按跳过
    /// 收束——轮次不永久挂死；卡内首次交互可续期（snooze）。
    pub ask_timeout_secs: u64,
}

impl Default for ToolsSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            max_per_request: 20,
            call_timeout_secs: 30,
            fs_roots: Vec::new(),
            max_turn_steps: runner::DEFAULT_MAX_STEPS,
            approval_timeout_secs: 100,
            default_permission_mode: "ask".into(),
            pin_session: true,
            ask_timeout_secs: 300,
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
    pub approval_timeout_secs: u64,
    pub default_permission_mode: String,
    pub pin_session: bool,
    pub ask_timeout_secs: u64,
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
    pub approval_timeout_secs: Option<u64>,
    pub default_permission_mode: Option<String>,
    pub pin_session: Option<bool>,
    pub ask_timeout_secs: Option<u64>,
}

/// call_timeout_secs 合法区间（秒）：下限避开 0（等同关超时），上限覆盖
/// 最慢的 MCP 调用；模型侧 tool-shell 的 arg 上限 120s 在此之下不受影响。
pub const CALL_TIMEOUT_RANGE: std::ops::RangeInclusive<u32> = 5..=3600;

/// ㉕·微调③ 单轮步数上限合法区间：下限 1（0 等于拒绝任何工具轮），
/// 上限 100 防手滑打开失控口子；子代理预算（subagent max_turns 上限 32）
/// 是另一套旋钮，与此互不影响。
pub const MAX_TURN_STEPS_RANGE: std::ops::RangeInclusive<u32> = 1..=100;

/// v2.49 审批等待时长合法区间（秒）：0 = 不限（可配置原则——不为用户
/// 做死限定，决策卡不渲染倒计时）；1–86400 = 一天封顶。
pub const APPROVAL_TIMEOUT_RANGE: std::ops::RangeInclusive<u64> = 0..=86_400;

/// v4.11 选择卡守夜合法区间（秒）：0 = 不限（同审批口径）；1–86400。
pub const ASK_TIMEOUT_RANGE: std::ops::RangeInclusive<u64> = 0..=86_400;

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

impl ToolsSettings {
    /// 校验 + 应用（台账 T15 从 `settings/update.rs` **机械搬移**：两条变换 =
    /// `next.tools.` → `self.`、缩进减一层，其余逐字节不动；`super::tools::` 与
    /// `crate::permission_mode::` 都是绝对/自身路径，原样保留。签名吃消费型值，
    /// 理由与等价性证据同 `plugins.rs`）。
    pub(crate) fn apply_update(&mut self, u: ToolsUpdate) -> Result<(), String> {
        if let Some(e) = u.enabled {
            self.enabled = e;
        }
        if let Some(n) = u.max_per_request {
            if !(1..=200).contains(&n) {
                return Err(format!("每次请求最大工具数 {n} 超出范围（1–200）"));
            }
            self.max_per_request = n;
        }
        if let Some(secs) = u.call_timeout_secs {
            if !super::tools::CALL_TIMEOUT_RANGE.contains(&secs) {
                return Err(format!(
                    "单次工具调用超时 {secs}s 超出范围（{}–{} 秒）",
                    super::tools::CALL_TIMEOUT_RANGE.start(),
                    super::tools::CALL_TIMEOUT_RANGE.end()
                ));
            }
            self.call_timeout_secs = secs;
        }
        if let Some(steps) = u.max_turn_steps {
            if !super::tools::MAX_TURN_STEPS_RANGE.contains(&steps) {
                return Err(format!(
                    "单轮步数上限 {steps} 非法（{}–{}）",
                    super::tools::MAX_TURN_STEPS_RANGE.start(),
                    super::tools::MAX_TURN_STEPS_RANGE.end()
                ));
            }
            self.max_turn_steps = steps;
        }
        if let Some(roots) = u.fs_roots {
            self.fs_roots = super::tools::normalize_fs_roots(&roots)?;
        }
        if let Some(secs) = u.approval_timeout_secs {
            // v2.49 可配置原则：审批守夜阈值可调（0 = 不限等待）
            if !super::tools::APPROVAL_TIMEOUT_RANGE.contains(&secs) {
                return Err(format!(
                    "审批等待时长 {secs}s 非法（0 = 不限，或 {}–{} 秒）",
                    super::tools::APPROVAL_TIMEOUT_RANGE.start(),
                    super::tools::APPROVAL_TIMEOUT_RANGE.end()
                ));
            }
            self.approval_timeout_secs = secs;
        }
        if let Some(secs) = u.ask_timeout_secs {
            // v4.11 §3.39①：ask_user 守夜时长可调（0 = 不限等待）
            if !super::tools::ASK_TIMEOUT_RANGE.contains(&secs) {
                return Err(format!(
                    "选择等待时长 {secs}s 非法（0 = 不限，或 {}–{} 秒）",
                    super::tools::ASK_TIMEOUT_RANGE.start(),
                    super::tools::ASK_TIMEOUT_RANGE.end()
                ));
            }
            self.ask_timeout_secs = secs;
        }
        if let Some(mode) = u.default_permission_mode {
            // ㉘·B 新会话权限档缺省：只收四档合法值（会话级覆盖存旁车）
            if crate::permission_mode::PermissionMode::parse(&mode).is_none() {
                return Err(format!(
                    "权限档缺省 {mode:?} 非法（plan / ask / acceptEdits / auto）"
                ));
            }
            self.default_permission_mode = mode;
        }
        if let Some(pin) = u.pin_session {
            // KV v2 §4.2 工具集会话钉死（无范围校验，布尔开关）
            self.pin_session = pin;
        }
        Ok(())
    }
}

impl crate::settings::SettingsStore {
    /// 后端专用：生效截断配置（ToolExposure 共享锁的内容物，⑫）。
    pub(crate) fn exposure_config(&self) -> ToolExposureConfig {
        let t = self.inner.lock().expect("settings poisoned").tools.clone();
        ToolExposureConfig {
            enabled: t.enabled,
            max_tools: t.max_per_request as usize,
            pin_session: t.pin_session,
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

    /// 后端专用：审批守夜超时秒数（v2.49，ApprovalHub 热换入；0 = 不限）。
    pub fn approval_timeout_secs(&self) -> u64 {
        self.inner
            .lock()
            .expect("settings poisoned")
            .tools
            .approval_timeout_secs
    }

    /// 后端专用：选择卡守夜超时秒数（v4.11 §3.39①，AskHub 热换入；0 = 不限）。
    pub fn ask_timeout_secs(&self) -> u64 {
        self.inner
            .lock()
            .expect("settings poisoned")
            .tools
            .ask_timeout_secs
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

    /// 后端专用：新会话权限档缺省（㉘·B，PermissionModeStore 热换入）。
    /// 未知值回落 Ask（手改 settings.json 不致命）。
    pub fn default_permission_mode(&self) -> crate::permission_mode::PermissionMode {
        let s = self
            .inner
            .lock()
            .expect("settings poisoned")
            .tools
            .default_permission_mode
            .clone();
        crate::permission_mode::PermissionMode::parse(&s)
            .unwrap_or(crate::permission_mode::PermissionMode::Ask)
    }
}
