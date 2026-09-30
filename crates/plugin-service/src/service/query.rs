//! PluginService 的查询投影面（读路径）：状态订阅、市场目录、已安装列表。
//! 写路径编排（install / update / uninstall / rollback / set_enabled）在
//! `super`；拆分只因单文件 400 行纪律，语义上同属一个编排层。

use std::collections::HashMap;
use std::path::PathBuf;

use plugin_host::PluginStateSummary;
use semver::Version;
use tokio::sync::broadcast;

use crate::builtin;
use crate::display::{PluginDisplay, read_display};
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

    /// 已安装列表（§11.4 lockfile 投影 + §5.7 registry 运行态合并 +
    /// packaging.md §5.3 展示投影）。
    ///
    /// - 注册表里有、lockfile 无条目的插件 = 开发态/内置装载（desktop 从
    ///   仓库 `plugins/` 目录直载，不经市场），以 `Builtin` 来源补进投影——
    ///   否则工具实际可用，"插件"页却看不见（阶段八③用户反馈）
    /// - 展示元数据从版本目录的 manifest.json 读出：lockfile 行优先用
    ///   registry 里已加载版本的目录，否则安装目录；内置行用 registry
    ///   记录的装载目录。目录缺失/manifest 损坏都降级为缺省投影，
    ///   不崩列表（packaging.md §7 Fallback 矩阵）
    pub fn list_installed(&self) -> Vec<InstalledPlugin> {
        let states: HashMap<String, PluginStateSummary> = self
            .registry
            .list_states()
            .into_iter()
            .map(|s| (s.id.clone(), s))
            .collect();

        // 先在锁内取齐核心字段与展示目录，锁外再做磁盘 IO（manifest+图标）
        let mut rows: Vec<(InstalledPlugin, Option<PathBuf>)> = Vec::new();
        {
            let lf = self.lockfile.lock().expect("lockfile poisoned");
            for e in lf.plugins.values() {
                let s = states.get(&e.id);
                let dir = self
                    .registry
                    .plugin_dir(&e.id, &e.version)
                    .or_else(|| self.display_dir_of(&e.id, &e.version, e.source));
                rows.push((
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
                        display: PluginDisplay::default(),
                    },
                    dir,
                ));
            }
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
                    rows.push((
                        InstalledPlugin {
                            id: id.clone(),
                            version,
                            enabled: s.active.is_some(),
                            installed_at: 0,
                            source: InstallSource::Builtin,
                            active_version: s.active.as_ref().map(Version::to_string),
                            loaded_versions: s.loaded.iter().map(Version::to_string).collect(),
                            display: PluginDisplay::default(),
                        },
                        s.dir.clone(),
                    ));
                }
            }
        }
        let mut out: Vec<InstalledPlugin> = rows
            .into_iter()
            .map(|(mut p, dir)| {
                if let Some(dir) = dir {
                    p.display = read_display(&dir, &self.icons);
                }
                p
            })
            .collect();
        out.sort_by(|a, b| a.id.cmp(&b.id));
        out
    }

    /// lockfile 条目的展示目录（manifest/图标读取用）：注册表优先，其后
    /// 按来源回落——市场条目走安装根；内置条目走内置根（停用的内置插件
    /// 不在注册表里，也必须读得出名字和图标；条目版本目录缺失时取内置
    /// 根最新版本，插件独立升级会迁移版本目录）。
    fn display_dir_of(
        &self,
        id: &str,
        version: &Version,
        source: InstallSource,
    ) -> Option<PathBuf> {
        match source {
            InstallSource::Builtin => {
                let root = self.builtin_root.as_ref()?;
                let at_version = root.join(id).join(version.to_string());
                if at_version.is_dir() {
                    return Some(at_version);
                }
                builtin::resolve_latest(&root.join(id)).map(|(_, dir)| dir)
            }
            _ => Some(self.install_root().join(id).join(version.to_string())),
        }
    }
}
