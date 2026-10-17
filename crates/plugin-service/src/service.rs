//! PluginService：插件管理编排（DESIGN.md §11.2 / §11.5）。
//!
//! 包装 `PluginRegistry` 而不替代它；桌面端 UI 只经这里操作插件。
//! 状态变化经 `broadcast` 推给 UI（§13.4 的 `plugin://state-changed`
//! 桥接由组装点完成，本 crate 不依赖 Tauri）。
//! 本文件是写路径编排；查询投影（list_installed / catalog / subscribe）
//! 在 `service/query.rs`（400 行纪律的职责拆分，非边界拆分）。
//!
//! 与文档的两处偏差（见 DESIGN.md §11.2 实现备注）：
//! - `install` 多一个可选 version 参数（市场卡片可选版本安装，
//!   默认 latest）；`check_updates` 不设独立命令——前端用目录 latest
//!   对比本地版本即可（§13.3 的更新检测逻辑）
//! - §5.7 的 `registry.subscribe()` 折叠为这里的单一广播源：注册表
//!   的变更在本架构下都经编排层发生，进程启动的开发态装载发生在
//!   UI 订阅之前，由 `list_installed` 拉取兜底

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use cordis_core::Context;
use plugin_host::{IconLoader, PluginRegistry};
use semver::Version;
use tokio::sync::broadcast;

mod enable;
mod query;
mod types;

pub use types::{InstalledPlugin, PluginStateChange};

use crate::installer;
use crate::lockfile::{InstallSource, LockFile};
use crate::market::{Market, artifact_file_name};
use types::LogicalUnload;

pub struct PluginService {
    registry: Arc<PluginRegistry>,
    ctx: Arc<Context>,
    /// 安装态数据根（lockfile.json 与 plugins/ 的父目录，§11.3）。
    data_dir: PathBuf,
    lockfile: Mutex<LockFile>,
    market: Box<dyn Market>,
    state_tx: broadcast::Sender<PluginStateChange>,
    /// 图标 data URL 缓存（list_installed 每次刷新都投影，不能每次读盘）。
    icons: IconLoader,
    /// 内置插件根（开发态 `plugins/` 目录，S2）。None = 无内置目录
    /// （纯安装态部署），Builtin 条目只保留登记不参与启停。
    builtin_root: Option<PathBuf>,
}

impl PluginService {
    /// 打开安装态目录并加载 lockfile（目录不存在会按需创建）。
    /// `builtin_root` 是内置插件根（桌面端传开发态 `plugins/` 目录）。
    pub fn open(
        data_dir: PathBuf,
        registry: Arc<PluginRegistry>,
        ctx: Arc<Context>,
        market: Box<dyn Market>,
        builtin_root: Option<PathBuf>,
    ) -> Result<Self, String> {
        std::fs::create_dir_all(data_dir.join("plugins"))
            .map_err(|e| format!("create_dir_all {}: {e}", data_dir.display()))?;
        let lockfile = LockFile::load(&Self::lockfile_path_of(&data_dir))?;
        let (state_tx, _) = broadcast::channel(32);
        Ok(Self {
            registry,
            ctx,
            data_dir,
            lockfile: Mutex::new(lockfile),
            market,
            state_tx,
            icons: IconLoader::new(),
            builtin_root,
        })
    }

    pub fn install_root(&self) -> PathBuf {
        self.data_dir.join("plugins")
    }

    pub fn lockfile_path(&self) -> PathBuf {
        Self::lockfile_path_of(&self.data_dir)
    }

    fn lockfile_path_of(data_dir: &Path) -> PathBuf {
        data_dir.join("lockfile.json")
    }

    /// 市场安装（§11.2）。`version` 缺省取市场 latest。
    /// 加载/激活失败时新目录按"加载即失败"处理：能删则删，删不掉 tombstone。
    pub async fn install(&self, id: &str, version: Option<&str>) -> Result<Version, String> {
        let ver = match version {
            Some(v) => Version::parse(v).map_err(|e| format!("bad version {v}: {e}"))?,
            None => self
                .latest_of(id)?
                .ok_or_else(|| format!("plugin {id} not in market"))?,
        };
        let src = self.market.package(id, &ver)?;
        let dst = installer::extract(&src, &self.install_root(), id, &ver)?;
        let dll = dst.join(artifact_file_name(id));
        self.registry
            .load(id, &ver, &dll)
            .and_then(|_| self.registry.activate(id, &ver))
            .map_err(|e| {
                self.tombstone_or_remove(&dst);
                e.to_string()
            })?;

        let effect = self.ctx.effect(Box::new(LogicalUnload {
            id: id.to_string(),
            registry: Arc::clone(&self.registry),
        }));
        {
            let mut lf = self.lockfile.lock().expect("lockfile poisoned");
            lf.add(id, ver.clone(), effect.0, InstallSource::Market);
            lf.save(&self.lockfile_path())?;
        }
        let _ = self.state_tx.send(PluginStateChange::Installed {
            id: id.to_string(),
            version: ver.clone(),
        });
        tracing::info!(plugin = id, version = %ver, "installed");
        Ok(ver)
    }

    /// 卸载（§11.2）：逻辑卸载 → 撤销 Effect → 物理删除（Windows 文件锁
    /// 删不掉则 tombstone）→ lockfile 移除 → 广播。
    pub async fn uninstall(&self, id: &str) -> Result<(), String> {
        let effect_ids = {
            let mut lf = self.lockfile.lock().expect("lockfile poisoned");
            // 内置插件随程序分发（S2）：删了下个版本又回来，只许停用。
            if lf
                .entry(id)
                .is_some_and(|e| e.source == InstallSource::Builtin)
            {
                return Err(format!("plugin {id} is built in and cannot be uninstalled"));
            }
            let entry = lf
                .remove(id)
                .ok_or_else(|| format!("plugin {id} not installed"))?;
            lf.save(&self.lockfile_path())?;
            entry.effect_ids
        };
        // 逻辑卸载（graveyard）；随后逐条撤销安装 Effect（宿主已卸则 no-op）
        self.registry.deactivate(id);
        for eid in effect_ids {
            let _ = self.ctx.revoke(cordis_core::EffectId(eid)).await;
        }
        let dir = self.install_root().join(id);
        if dir.exists() && std::fs::remove_dir_all(&dir).is_err() {
            let mut lf = self.lockfile.lock().expect("lockfile poisoned");
            lf.push_pending_delete(dir.clone());
            lf.save(&self.lockfile_path())?;
            tracing::warn!(plugin = id, dir = %dir.display(), "delete deferred to next startup");
        }
        let _ = self
            .state_tx
            .send(PluginStateChange::Uninstalled { id: id.to_string() });
        Ok(())
    }

    /// 回退到指定版本（§11.2 rollback）：本质是 activate 旧指针（必要时
    /// 先从磁盘 load）。旧版本永远保留，回退零下载。
    pub async fn rollback(&self, id: &str, version: &str) -> Result<(), String> {
        let ver = Version::parse(version).map_err(|e| format!("bad version {version}: {e}"))?;
        if !self.registry.has_version(id, &ver) {
            let dir = self.install_root().join(id).join(version);
            if !dir.is_dir() {
                return Err(format!("version {version} of {id} not installed"));
            }
            self.registry
                .load(id, &ver, &dir.join(artifact_file_name(id)))
                .map_err(|e| e.to_string())?;
        }
        self.registry
            .activate(id, &ver)
            .map_err(|e| e.to_string())?;
        self.ctx
            .events
            .emit(
                "plugin/swapped",
                &serde_json::json!({ "id": id, "to": ver.to_string() }),
            )
            .await;
        {
            let mut lf = self.lockfile.lock().expect("lockfile poisoned");
            lf.update_version(id, &ver)?;
            lf.set_enabled(id, true)?;
            lf.save(&self.lockfile_path())?;
        }
        let _ = self.state_tx.send(PluginStateChange::ActiveVersionChanged {
            id: id.to_string(),
            version: ver,
        });
        Ok(())
    }

    /// 更新（§11.5）：下载解压到新版本目录 → load + activate；失败时旧
    /// active 指针从未离开，"回滚"只是确认旧指针 + 清理新目录。
    pub async fn update(&self, id: &str) -> Result<Version, String> {
        {
            let lf = self.lockfile.lock().expect("lockfile poisoned");
            // 内置插件随程序分发、版本独立升级（S2 定稿，packaging.md §9.4）：
            // 不走市场更新；同 id 需要市场版时在发现页显式安装（条目转 Market）。
            if lf
                .entry(id)
                .is_some_and(|e| e.source == InstallSource::Builtin)
            {
                return Err(format!(
                    "plugin {id} is built in; updates ship with the app"
                ));
            }
        }
        let snapshot_ver = {
            let lf = self.lockfile.lock().expect("lockfile poisoned");
            lf.entry(id)
                .ok_or_else(|| format!("plugin {id} not installed"))?
                .version
                .clone()
        };
        let latest = self
            .latest_of(id)?
            .ok_or_else(|| format!("plugin {id} not in market"))?;
        if latest <= snapshot_ver {
            return Err(format!(
                "no newer version: installed {snapshot_ver}, market latest {latest}"
            ));
        }
        // 回退之后再更新时，目标版本**本来就在盘上**（多版本并存是 §6.1 第 1 层回退的
        // 基础），而且大概率还在内存里（回退只挪 active 指针）。这时候再拷一遍同一份
        // 产物没有任何意义，却会撞上平台差异：Windows 上产物正被装载 ⇒ 覆盖被文件锁拒绝
        // （本机实测 `os error 32 另一个程序正在使用此文件`），Linux 上没有这把锁 ⇒ 覆盖
        // 成功并把同一份映像再 dlopen 一次。两侧都不该发生，所以这里先认已安装的产物。
        let installed_dir = self.install_root().join(id).join(latest.to_string());
        let artifact = installed_dir.join(artifact_file_name(id));
        let freshly_extracted = !artifact.is_file();
        let final_path = if freshly_extracted {
            let src = self.market.package(id, &latest)?;
            installer::extract(&src, &self.install_root(), id, &latest)?
        } else {
            installed_dir
        };
        match self
            .registry
            .load(id, &latest, &final_path.join(artifact_file_name(id)))
            .and_then(|_| self.registry.activate(id, &latest))
        {
            Ok(()) => {
                {
                    let mut lf = self.lockfile.lock().expect("lockfile poisoned");
                    lf.update_version(id, &latest)?;
                    lf.save(&self.lockfile_path())?;
                }
                self.ctx
                    .events
                    .emit(
                        "plugin/swapped",
                        &serde_json::json!({ "id": id, "to": latest.to_string() }),
                    )
                    .await;
                let _ = self.state_tx.send(PluginStateChange::ActiveVersionChanged {
                    id: id.to_string(),
                    version: latest.clone(),
                });
                tracing::info!(plugin = id, from = %snapshot_ver, to = %latest, "updated");
                Ok(latest)
            }
            Err(e) => {
                let _ = self.registry.activate(id, &snapshot_ver);
                // 只清理**本次新建**的目录：走"已在盘上"那条路时 final_path 里可能是
                // 仍在使用的安装，删它等于把回退目标的字节也带走（破坏性，绝不允许）。
                if freshly_extracted && std::fs::remove_dir_all(&final_path).is_err() {
                    let mut lf = self.lockfile.lock().expect("lockfile poisoned");
                    lf.push_pending_delete(final_path.clone());
                    lf.save(&self.lockfile_path())?;
                }
                Err(format!("update failed, rolled back: {e}"))
            }
        }
    }

    /// 启动对账（§11.2 关键设计）：清 tombstone → 条目与磁盘对账 →
    /// 恢复 enabled 插件。调用方必须在加载任何插件之前调用
    /// （此时无文件锁，tombstone 才删得掉）。
    ///
    /// Builtin 条目（S2）不参与本函数的磁盘对账与恢复：它们住在内置根
    /// （开发态目录）而非安装根，装载由组装点的开发态扫描负责（扫描
    /// 尊重 enabled:false——停用的内置插件重启保持停用）。
    pub fn startup_reconcile(&self) -> Result<(), String> {
        let mut lf = self.lockfile.lock().expect("lockfile poisoned");
        lf.pending_delete
            .retain(|dir| match std::fs::remove_dir_all(dir) {
                Ok(()) => {
                    tracing::info!(dir = %dir.display(), "tombstone cleaned");
                    false
                }
                Err(e) => {
                    tracing::warn!(dir = %dir.display(), "tombstone still locked: {e}");
                    true
                }
            });
        let missing: Vec<String> = lf
            .plugins
            .values()
            .filter(|e| {
                e.source != InstallSource::Builtin
                    && !self
                        .install_root()
                        .join(&e.id)
                        .join(e.version.to_string())
                        .is_dir()
            })
            .map(|e| e.id.clone())
            .collect();
        for id in missing {
            tracing::warn!(plugin = %id, "reconcile: install dir missing, dropping entry");
            lf.remove(&id);
        }
        let restore: Vec<(String, Version)> = lf
            .plugins
            .values()
            .filter(|e| e.enabled && e.source != InstallSource::Builtin)
            .map(|e| (e.id.clone(), e.version.clone()))
            .collect();
        for (id, ver) in restore {
            let dll = self
                .install_root()
                .join(&id)
                .join(ver.to_string())
                .join(artifact_file_name(&id));
            match self
                .registry
                .load(&id, &ver, &dll)
                .and_then(|_| self.registry.activate(&id, &ver))
            {
                Ok(_) => tracing::info!(plugin = %id, version = %ver, "restored from lockfile"),
                Err(e) => tracing::warn!(plugin = %id, "restore failed: {e}"),
            }
        }
        lf.save(&self.lockfile_path())
    }

    /// 市场 latest；无此插件返回 None。
    fn latest_of(&self, id: &str) -> Result<Option<Version>, String> {
        Ok(self
            .market
            .catalog()?
            .into_iter()
            .find(|item| item.id == id)
            .and_then(|item| item.latest().cloned()))
    }

    /// 删目录，删不掉（文件锁）则 tombstone。用于安装/更新失败清理。
    fn tombstone_or_remove(&self, dir: &Path) {
        if std::fs::remove_dir_all(dir).is_err() {
            let mut lf = self.lockfile.lock().expect("lockfile poisoned");
            lf.push_pending_delete(dir.to_path_buf());
            let _ = lf.save(&self.lockfile_path());
        }
    }
}

#[cfg(test)]
mod tests;
