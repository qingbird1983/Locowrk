//! LockFile：已安装插件契约文件（DESIGN.md §11.4）。安装态的磁盘真相。
//!
//! 崩溃恢复不依赖 EffectId——Disposer 是内存闭包，进程重启后不可还原；
//! `effect_ids` 只做审计线索，实际收尾靠 `pending_delete` tombstone 与
//! 目录对账（[`crate::service::PluginService::startup_reconcile`]）。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use semver::Version;
use serde::{Deserialize, Serialize};

pub const LOCKFILE_SCHEMA_VERSION: u32 = 1;

/// 安装来源（§11.4）。当前只有 Market 落地；Local / Builtin 预留。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallSource {
    Market,
    Local,
    Builtin,
}

/// 单个已安装插件的条目。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledEntry {
    pub id: String,
    pub version: Version,
    /// 该插件登记的全部 Effect（审计线索；重启后不用于恢复）。
    pub effect_ids: Vec<u64>,
    /// Unix 秒。
    pub installed_at: i64,
    pub enabled: bool,
    pub source: InstallSource,
}

/// lockfile 文档结构（JSON 落盘）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LockFile {
    pub schema_version: u32,
    pub plugins: BTreeMap<String, InstalledEntry>,
    /// 物理删除 tombstone：本进程删不掉（Windows 文件锁）的目录，
    /// 下次启动在加载任何插件之前清理（§11.2）。
    pub pending_delete: Vec<PathBuf>,
}

impl LockFile {
    /// 从磁盘加载；文件不存在视为空 lockfile（首次启动）。
    pub fn load(path: &Path) -> Result<LockFile, String> {
        let raw = match std::fs::read_to_string(path) {
            Ok(raw) => raw,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(LockFile {
                    schema_version: LOCKFILE_SCHEMA_VERSION,
                    ..LockFile::default()
                });
            }
            Err(e) => return Err(format!("read {}: {e}", path.display())),
        };
        let lf: LockFile =
            serde_json::from_str(&raw).map_err(|e| format!("parse {}: {e}", path.display()))?;
        Ok(lf)
    }

    /// 原子性要求不高（对账兜底），直接整文件覆写。
    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("create_dir_all {}: {e}", parent.display()))?;
        }
        let raw = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(path, raw).map_err(|e| format!("write {}: {e}", path.display()))
    }

    pub fn entry(&self, id: &str) -> Option<&InstalledEntry> {
        self.plugins.get(id)
    }

    /// 登记/重装（同 id 已有条目则整体替换，effect_ids 重开计数——旧
    /// Effect 留在账本里仅作审计，不再被 uninstall 领取）。
    pub fn add(&mut self, id: &str, version: Version, effect_id: u64, source: InstallSource) {
        self.plugins.insert(
            id.to_string(),
            InstalledEntry {
                id: id.to_string(),
                version,
                effect_ids: vec![effect_id],
                installed_at: now_unix(),
                enabled: true,
                source,
            },
        );
    }

    pub fn remove(&mut self, id: &str) -> Option<InstalledEntry> {
        self.plugins.remove(id)
    }

    pub fn update_version(&mut self, id: &str, version: &Version) -> Result<(), String> {
        let entry = self
            .plugins
            .get_mut(id)
            .ok_or_else(|| format!("plugin {id} not in lockfile"))?;
        entry.version = version.clone();
        Ok(())
    }

    /// 内置插件登记（S2，清单 §1）：无条目或版本不同才写（版本随内置根
    /// 独立升级，每次启动同步）；已有条目保留 `enabled`——重启不得复活
    /// 被用户停用的插件。返回是否实际改动。
    pub fn register_builtin(&mut self, id: &str, version: &Version) -> bool {
        match self.plugins.get_mut(id) {
            Some(e) if e.source == InstallSource::Builtin && e.version == *version => false,
            Some(e) if e.source == InstallSource::Builtin => {
                e.version = version.clone();
                true
            }
            // 无条目（首次启动登记）。
            None => {
                self.plugins.insert(
                    id.to_string(),
                    InstalledEntry {
                        id: id.to_string(),
                        version: version.clone(),
                        effect_ids: Vec::new(),
                        installed_at: now_unix(),
                        enabled: true,
                        source: InstallSource::Builtin,
                    },
                );
                true
            }
            // 已被市场安装接管的条目保持不动（安装态赢过开发态）。
            Some(_) => false,
        }
    }

    pub fn set_enabled(&mut self, id: &str, enabled: bool) -> Result<(), String> {
        let entry = self
            .plugins
            .get_mut(id)
            .ok_or_else(|| format!("plugin {id} not in lockfile"))?;
        entry.enabled = enabled;
        Ok(())
    }

    /// 追加 tombstone（去重）。
    ///
    /// 清理路径只有一条：`service.rs::startup_reconcile`（重启时逐条
    /// `remove_dir_all` 成功才摘）。曾另有一个 `clear_pending_delete(dir)`
    /// 单元入口，全仓无生产调用点（只被自家测试用）= 与上面同语义的第二
    /// 份实现，审计 M-22/D6 就此删掉——留两个入口迟早分叉。
    pub fn push_pending_delete(&mut self, dir: PathBuf) {
        if !self.pending_delete.contains(&dir) {
            self.pending_delete.push(dir);
        }
    }
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests;
