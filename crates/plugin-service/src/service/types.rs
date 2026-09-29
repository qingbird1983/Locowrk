//! 插件管理的公开类型：UI 状态变化广播（§13.4）、安装态+运行态合并
//! 视图（§11.6）、安装 Effect 的逆操作。

use std::sync::Arc;

use cordis_core::Disposer;
use plugin_host::PluginRegistry;
use semver::Version;
use serde::Serialize;

use crate::lockfile::InstallSource;

/// UI 状态变化（§13.4）。`type` 标签随 JSON 序列化为
/// installed / enabled_changed / uninstalled / active_version_changed。
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PluginStateChange {
    Installed { id: String, version: Version },
    EnabledChanged { id: String, enabled: bool },
    Uninstalled { id: String },
    ActiveVersionChanged { id: String, version: Version },
}

/// 安装态 + 运行态的合并视图（§11.6 PluginUiState 的最小落地）。
#[derive(Debug, Clone, Serialize)]
pub struct InstalledPlugin {
    pub id: String,
    /// lockfile 记录的当前版本。
    pub version: String,
    pub enabled: bool,
    pub installed_at: i64,
    pub source: InstallSource,
    /// 本进程注册表里的 active 版本（None = 未激活）。
    pub active_version: Option<String>,
    /// 本进程已加载的全部版本。
    pub loaded_versions: Vec<String>,
}

/// 安装 Effect 的逆操作：逻辑卸载（active 摘除 + graveyard，永不 dlclose）。
/// 幂等：未激活时 `deactivate` 是 no-op（纪律 10 的宿主侧同款思路）。
pub(crate) struct LogicalUnload {
    pub(crate) id: String,
    pub(crate) registry: Arc<PluginRegistry>,
}

#[async_trait::async_trait]
impl Disposer for LogicalUnload {
    async fn dispose(self: Box<Self>) {
        self.registry.deactivate(&self.id);
    }
}
