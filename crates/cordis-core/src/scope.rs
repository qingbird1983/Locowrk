//! Scope：一进程多 Agent 隔离（DESIGN.md §3.4 / §9.3）。
//!
//! `ScopeKey` 是零开销身份键（`Arc<()>` 指针等价）；`ScopedLayers` 是
//! Context 服务表的载体——global 层 + 按 `ScopeKey` 惰性创建的 scoped 层。
//! 单 Agent 场景 scoped map 始终为空，查询走 `scoped_is_empty()` 快路径，
//! 开销只有一次 O(1) 的分片计数查询（§9.3）。

use std::sync::Arc;

use dashmap::DashMap;

/// 作用域身份键：同一 `ScopeKey` 克隆出的键相等，不同来源不相等。
/// 比较与哈希都基于 `Arc` 指针身份，不解引用内层。
#[derive(Clone, Debug)]
pub struct ScopeKey(Arc<()>);

impl ScopeKey {
    pub fn new() -> Self {
        Self(Arc::new(()))
    }
}

impl Default for ScopeKey {
    fn default() -> Self {
        Self::new()
    }
}

impl PartialEq for ScopeKey {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0) // 零开销身份比较（§3.4）
    }
}
impl Eq for ScopeKey {}

// 必须手写指针哈希，绝不能 derive(Hash)：Arc<()> 的自动 Hash 会对内层 ()
// 求值，所有 ScopeKey 哈希相同而 Eq 不同——违反 Eq/Hash 契约，还会把
// 所有作用域挤进同一分片（§3.4）。
impl std::hash::Hash for ScopeKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::ptr::hash(Arc::as_ptr(&self.0), state)
    }
}

/// 分层容器：global 层 + 每作用域一层（首次写入惰性 `or_default` 创建）。
/// 泛型参数 L 由 Context 取服务表类型；本类型只做容器与查找，不做 downcast。
pub struct ScopedLayers<L> {
    global: L,
    scoped: DashMap<ScopeKey, L>,
}

impl<L: Default> ScopedLayers<L> {
    pub fn new() -> Self {
        Self {
            global: L::default(),
            scoped: DashMap::new(),
        }
    }

    /// 取某作用域的层，不存在则惰性创建；返回分片写守卫，
    /// 调用方不得持守卫跨 await。
    pub fn layer_mut(&self, key: &ScopeKey) -> dashmap::mapref::one::RefMut<'_, ScopeKey, L> {
        self.scoped.entry(key.clone()).or_default()
    }
}

impl<L> ScopedLayers<L> {
    pub fn global(&self) -> &L {
        &self.global
    }

    /// 是否存在任何 scoped 层（§9.3 快路径的判据）。
    pub fn scoped_is_empty(&self) -> bool {
        self.scoped.is_empty()
    }

    /// 在某作用域的层上跑一次只读查询；层不存在返回 `None`
    /// （由调用方决定是否回退 global）。
    pub fn with_scoped<R>(&self, key: &ScopeKey, f: impl FnOnce(&L) -> Option<R>) -> Option<R> {
        self.scoped.get(key).and_then(|l| f(&l))
    }
}

impl<L: Default> Default for ScopedLayers<L> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
