//! PluginService 的查询投影面（读路径）：状态订阅、市场目录、已安装列表。
//! 写路径编排（install / update / uninstall / rollback / set_enabled）在
//! `super`；拆分只因单文件 400 行纪律，语义上同属一个编排层。

use std::collections::HashMap;

use semver::Version;
use tokio::sync::broadcast;

use crate::lockfile::InstallSource;
use crate::market::MarketplaceItem;

use super::{InstalledPlugin, PluginService, PluginStateChange};

impl PluginService {
    /// UI 订阅状态变化（§13.4）。
    pub fn subscribe(&self) -> broadcast::Receiver<PluginStateChange> {
        self.state_tx.subscribe()
    }

    /// 市场目录（发现页）；目录缺失时由 Market 实现决定（空列表不是错误）。
    pub fn catalog(&self) -> Result<Vec<MarketplaceItem>, String> {
        self.market.catalog()
    }

    /// 已安装列表（§11.4 lockfile 投影 + §5.7 registry 运行态合并）。
    /// 注册表里有、lockfile 无条目的插件 = 开发态/内置装载（desktop 从
    /// 仓库 `plugins/` 目录直载，不经市场），以 `Builtin` 来源补进投影——
    /// 否则工具实际可用，"插件"页却看不见（阶段八③用户反馈）。
    pub fn list_installed(&self) -> Vec<InstalledPlugin> {
        let states: HashMap<String, plugin_host::PluginStateSummary> = self
            .registry
            .list_states()
            .into_iter()
            .map(|s| (s.id.clone(), s))
            .collect();
        let lf = self.lockfile.lock().expect("lockfile poisoned");
        let mut out: Vec<InstalledPlugin> = lf
            .plugins
            .values()
            .map(|e| {
                let s = states.get(&e.id);
                InstalledPlugin {
                    id: e.id.clone(),
                    version: e.version.to_string(),
                    enabled: e.enabled,
                    installed_at: e.installed_at,
                    source: e.source,
                    active_version: s.and_then(|s| s.active.as_ref().map(Version::to_string)),
                    loaded_versions: s
                        .map(|s| s.loaded.iter().map(Version::to_string).collect())
                        .unwrap_or_default(),
                }
            })
            .collect();
        // lockfile 之外：registry-only（开发态直载）。enabled 随 active 走；
        // installed_at 无意义（0 = 纪元），UI 不渲染该字段。
        for (id, s) in &states {
            if !lf.plugins.contains_key(id) {
                let version = s
                    .active
                    .as_ref()
                    .or_else(|| s.loaded.last())
                    .map(Version::to_string)
                    .unwrap_or_else(|| "0.0.0".into());
                out.push(InstalledPlugin {
                    id: id.clone(),
                    version,
                    enabled: s.active.is_some(),
                    installed_at: 0,
                    source: InstallSource::Builtin,
                    active_version: s.active.as_ref().map(Version::to_string),
                    loaded_versions: s.loaded.iter().map(Version::to_string).collect(),
                });
            }
        }
        out.sort_by(|a, b| a.id.cmp(&b.id));
        out
    }
}
