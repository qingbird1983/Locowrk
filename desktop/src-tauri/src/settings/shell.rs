//! shell 插件治理设置（S1：《Setting and Constrained Optimization.md》
//! §2.7/§3 约束最小化——高危命令"默认拒绝、可解锁走审批"）。
//! 类型独立成文件遵守 400 行纪律（memory/tools 同款）。

use serde::{Deserialize, Serialize};

/// `shell` 插件区（settings.json）。`unlockRisky` 缺省 false：rm / sudo /
/// chmod / dd 等用户可判断后果的高危命令直接拒绝；开启后由 state.rs 在
/// **插件加载前**写入 `LOCOWRK_SHELL_UNLOCK_RISK`，tool-shell init 读到
/// 后放行进审批（shell 工具 Irreversible，放行即审批）——改动重启生效。
/// 硬锁类（解释器/包装/网络/执行面）不受本开关影响，永不放行。
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ShellSettings {
    pub unlock_risky: bool,
}

/// IPC 投影（无秘钥，全量展示）。
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ShellSettingsView {
    pub unlock_risky: bool,
}

/// 部分更新（settings_set 入参）。字段 None = 保持不变。
#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ShellUpdate {
    pub unlock_risky: Option<bool>,
}

impl crate::settings::SettingsStore {
    /// 后端专用：高危解锁开关（state.rs 插件加载前写 env 用）。
    pub(crate) fn shell_unlock_risky(&self) -> bool {
        self.inner.lock().expect("settings poisoned").shell.unlock_risky
    }
}
