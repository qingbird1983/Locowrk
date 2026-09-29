//! 多版本注册表（DESIGN.md §5.7）：多版本并存 + active 指针 + graveyard。
//!
//! 纪律：
//! - 内部可变性，方法全部 `&self`，外层 `Arc<PluginRegistry>` 共享
//! - 回退 = `activate` 旧版本（微秒级），旧版本永远保留
//! - 逻辑卸载 = 把 Arc 挪进 graveyard（物理保留，永不 dlclose）；
//!   在途调用持有各自的 Arc 克隆，结束后引用计数自然归一
//! - load 的幂等：同 (id, version) 重复加载返回既有实例，不重复 dlopen

use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::{Arc, Mutex, RwLock};

use semver::Version;

use crate::error::LoadError;
use crate::instance::PluginInstance;
use crate::loader::DylibLoader;

/// 单个插件的版本视图（UI 投影的最小形态；订阅广播在 plugin-service）。
#[derive(Debug, Clone)]
pub struct PluginStateSummary {
    pub id: String,
    /// 当前 active 版本（None = 已安装未激活/已卸载）。
    pub active: Option<Version>,
    /// 已加载（仍在 versions 表中）的版本，升序。
    pub loaded: Vec<Version>,
}

/// 多版本注册表（附录 A）。所有方法 `&self`。
pub struct PluginRegistry {
    /// id → (version → 实例)。
    versions: RwLock<HashMap<String, BTreeMap<Version, Arc<PluginInstance>>>>,
    /// id → active 版本指针。
    active: RwLock<HashMap<String, Version>>,
    /// 逻辑卸载但物理保留的实例（永不 dlclose，§5.6 纪律一）。
    graveyard: Mutex<Vec<Arc<PluginInstance>>>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            versions: RwLock::new(HashMap::new()),
            active: RwLock::new(HashMap::new()),
            graveyard: Mutex::new(Vec::new()),
        }
    }

    /// dlopen 加载并登记一个版本。`id` / `ver` 是登记键（开发态来自
    /// 目录名 plugins/{id}/{ver}/），必须与 manifest 自述一致。
    /// 同键重复加载返回既有实例（幂等，§5.7 load）。
    pub fn load(
        &self,
        id: &str,
        ver: &Version,
        path: &Path,
    ) -> Result<Arc<PluginInstance>, LoadError> {
        if let Some(inst) = self.instance(id, ver) {
            return Ok(inst);
        }
        let inst = Arc::new(DylibLoader::load(path)?);
        let manifest = inst.manifest();
        if manifest.id != id {
            return Err(LoadError::IdMismatch {
                expected: id.to_string(),
                manifest: manifest.id.clone(),
            });
        }
        if manifest.version != ver.to_string() {
            return Err(LoadError::VersionMismatch {
                expected: ver.to_string(),
                manifest: manifest.version.clone(),
            });
        }
        // 双检：并发 load 同键时后到者放弃自己的实例（宁可多 dlopen 一次
        // 也不能登记两份——放弃的实例走"加载即失败"豁免路径释放）
        let mut versions = self.versions.write().expect("versions lock poisoned");
        let slot = versions.entry(id.to_string()).or_default();
        if let Some(existing) = slot.get(ver) {
            return Ok(Arc::clone(existing));
        }
        slot.insert(ver.clone(), Arc::clone(&inst));
        Ok(inst)
    }

    /// 切换 active 指针（激活/升级/回退共用，§5.7 activate）。
    pub fn activate(&self, id: &str, ver: &Version) -> Result<(), LoadError> {
        if !self.has_version(id, ver) {
            return Err(LoadError::NotLoaded(format!("{id}@{ver}")));
        }
        self.active
            .write()
            .expect("active lock poisoned")
            .insert(id.to_string(), ver.clone());
        Ok(())
    }

    /// 回退到前一个版本（§6.1 第 1 层回退）。返回回退到的版本。
    pub fn rollback(&self, id: &str) -> Result<Version, LoadError> {
        let cur = self
            .active
            .read()
            .expect("active lock poisoned")
            .get(id)
            .cloned()
            .ok_or_else(|| LoadError::NotLoaded(id.to_string()))?;
        let prev = self
            .versions
            .read()
            .expect("versions lock poisoned")
            .get(id)
            .and_then(|m| m.range(..cur).next_back().map(|(v, _)| v.clone()))
            .ok_or_else(|| LoadError::NoPreviousVersion(id.to_string()))?;
        self.activate(id, &prev)?;
        Ok(prev)
    }

    /// 逻辑卸载：active 摘除 + versions 移除 + Arc 入 graveyard
    /// （物理保留，进程重启才真正释放，§5.6 纪律一）。
    pub fn deactivate(&self, id: &str) {
        let ver = self
            .active
            .write()
            .expect("active lock poisoned")
            .remove(id);
        let Some(ver) = ver else { return };
        let removed = self
            .versions
            .write()
            .expect("versions lock poisoned")
            .get_mut(id)
            .and_then(|m| m.remove(&ver));
        if let Some(inst) = removed {
            self.graveyard
                .lock()
                .expect("graveyard lock poisoned")
                .push(inst);
        }
    }

    /// 当前激活版本的实例（读路径：拿到 Arc 即放锁，§5.7）。
    pub fn current(&self, id: &str) -> Option<Arc<PluginInstance>> {
        let ver = self
            .active
            .read()
            .expect("active lock poisoned")
            .get(id)?
            .clone();
        self.instance(id, &ver)
    }

    pub fn has_version(&self, id: &str, ver: &Version) -> bool {
        self.instance(id, ver).is_some()
    }

    /// 活跃实例清单：active 指针指向的实例，按 id 排序（工具目录
    /// 查找顺序确定性的基础）。
    pub fn active_instances(&self) -> Vec<(String, Arc<PluginInstance>)> {
        let active = self.active.read().expect("active lock poisoned");
        let mut pairs: Vec<(String, Arc<PluginInstance>)> = active
            .iter()
            .filter_map(|(id, ver)| self.instance(id, ver).map(|i| (id.clone(), i)))
            .collect();
        pairs.sort_by(|a, b| a.0.cmp(&b.0));
        pairs
    }

    /// 全部插件的状态摘要（UI 列表用，§5.7 list_states）。
    pub fn list_states(&self) -> Vec<PluginStateSummary> {
        let versions = self.versions.read().expect("versions lock poisoned");
        let active = self.active.read().expect("active lock poisoned");
        let mut ids: Vec<_> = versions.keys().cloned().collect();
        ids.sort();
        ids.into_iter()
            .map(|id| PluginStateSummary {
                active: active.get(&id).cloned(),
                loaded: versions
                    .get(&id)
                    .map(|m| m.keys().cloned().collect())
                    .unwrap_or_default(),
                id,
            })
            .collect()
    }

    /// graveyard 长度（诊断/测试用：确认逻辑卸载只挪不移除）。
    pub fn graveyard_len(&self) -> usize {
        self.graveyard
            .lock()
            .expect("graveyard lock poisoned")
            .len()
    }

    fn instance(&self, id: &str, ver: &Version) -> Option<Arc<PluginInstance>> {
        self.versions
            .read()
            .expect("versions lock poisoned")
            .get(id)
            .and_then(|m| m.get(ver))
            .cloned()
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
