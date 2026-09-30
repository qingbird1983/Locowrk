//! 启用/停用与内置插件登记（S2，清单 §1；DESIGN.md §11.2 set_enabled）。
//! 从 service.rs 拆出只因 400 行纪律，语义上同属写路径编排。

use std::path::PathBuf;

use semver::Version;

use super::{PluginService, PluginStateChange};
use crate::builtin;
use crate::lockfile::InstallSource;
use crate::market::artifact_file_name;

impl PluginService {
    /// 启用 / 停用（§11.2 set_enabled）。启用 = load（若未加载）+ activate；
    /// 停用 = active 摘除（graveyard，永不 dlclose）。Builtin 来源从内置根
    /// 解析最新版本（插件独立升级，条目版本可能滞后），市场来源照旧走
    /// 安装根的条目版本。
    pub async fn set_enabled(&self, id: &str, enabled: bool) -> Result<(), String> {
        let (ver, source) = {
            let lf = self.lockfile.lock().expect("lockfile poisoned");
            let e = lf
                .entry(id)
                .ok_or_else(|| format!("plugin {id} not installed"))?;
            (e.version.clone(), e.source)
        };
        if enabled {
            let (load_ver, dll) = match source {
                InstallSource::Builtin => self.builtin_latest(id)?,
                _ => (
                    ver.clone(),
                    self.install_root()
                        .join(id)
                        .join(ver.to_string())
                        .join(artifact_file_name(id)),
                ),
            };
            self.registry
                .load(id, &load_ver, &dll)
                .and_then(|_| self.registry.activate(id, &load_ver))
                .map_err(|e| e.to_string())?;
            if load_ver != ver {
                // 版本漂移同步回条目（内置根升级后重启，登记版本跟上）
                let mut lf = self.lockfile.lock().expect("lockfile poisoned");
                lf.update_version(id, &load_ver)?;
                lf.save(&self.lockfile_path())?;
            }
        } else {
            self.registry.deactivate(id);
        }
        {
            let mut lf = self.lockfile.lock().expect("lockfile poisoned");
            lf.set_enabled(id, enabled)?;
            lf.save(&self.lockfile_path())?;
        }
        let _ = self.state_tx.send(PluginStateChange::EnabledChanged {
            id: id.to_string(),
            enabled,
        });
        Ok(())
    }

    /// 内置插件登记（组装点开发态扫描每成功加载一个调一次）：lockfile
    /// 无条目或版本不同才写，已有条目保留 enabled（重启不复活停用项）。
    /// 已被市场安装接管的条目不动（安装态赢过开发态）。
    pub fn register_builtin(&self, id: &str, version: &Version) {
        let mut lf = self.lockfile.lock().expect("lockfile poisoned");
        if lf.register_builtin(id, version) {
            if let Err(e) = lf.save(&self.lockfile_path()) {
                tracing::warn!(plugin = id, "builtin register save failed: {e}");
            }
        }
    }

    /// 开发态扫描的跳过集（S2）：lockfile 里的非 Builtin 条目（安装态
    /// 接管，避免同 id 双版本互踩）+ 已停用的 Builtin 条目（用户主动
    /// 停用，重启不复活）。
    pub fn dev_scan_skip_ids(&self) -> Vec<String> {
        let lf = self.lockfile.lock().expect("lockfile poisoned");
        lf.plugins
            .values()
            .filter(|e| e.source != InstallSource::Builtin || !e.enabled)
            .map(|e| e.id.clone())
            .collect()
    }

    /// 内置根下该插件的最新版本与加载工件；无内置根 / 无版本目录 /
    /// 无工件都显式报错（不静默猜测）。
    fn builtin_latest(&self, id: &str) -> Result<(Version, PathBuf), String> {
        let root = self
            .builtin_root
            .as_ref()
            .ok_or_else(|| format!("plugin {id}: builtin root unavailable"))?;
        let (ver, dir) = builtin::resolve_latest(&root.join(id))
            .ok_or_else(|| format!("plugin {id}: no version dir under builtin root"))?;
        let dll = builtin::find_artifact(&dir, id)
            .ok_or_else(|| format!("plugin {id}: no loadable artifact in {}", dir.display()))?;
        Ok((ver, dll))
    }
}
