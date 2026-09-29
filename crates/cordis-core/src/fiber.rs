//! Fiber：插件生命周期容器（DESIGN.md §3.3）。
//!
//! 六态状态机：Pending ─[依赖满足]→ Loading ─[apply 成功]→ Active
//! ─[依赖消失]→ Unloading ─[撤销链完成]→ Unloaded ─[依赖恢复]→ Pending；
//! apply/dispose 失败落 `Failed(FailureKind)`（终态）。**状态枚举保持
//! Copy**（rutis 笔记纪律）：失败详情拆独立字段 `failure`，不塞进枚举载荷。
//!
//! 阶段一已有身份与"当前 Fiber"槽：`Context::provide` 用 `current_fiber_id()`
//! 给 Effect 打标签（§3.1）；runtime 在 apply/dispose 前进入 `FiberScope`。

use std::any::TypeId;
use std::cell::Cell;
use std::marker::PhantomData;
use std::sync::{Arc, RwLock};

use crate::error::{CordisError, Result};
use crate::plugin::Plugin;

/// Fiber 身份。`HOST`（0）表示宿主直接注册（非插件路径）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FiberId(pub u64);

impl FiberId {
    pub const HOST: FiberId = FiberId(0);
}

thread_local! {
    static CURRENT_FIBER: Cell<FiberId> = const { Cell::new(FiberId::HOST) };
}

/// 当前 Fiber。`Context::provide` 用它给 Effect 打标签（§3.1 的 `current_fiber_id()`）。
pub fn current_fiber_id() -> FiberId {
    CURRENT_FIBER.with(|c| c.get())
}

/// RAII 守卫：作用域内把当前 Fiber 设为 `fid`，drop 恢复。
/// 非 Send——守卫不可跨线程迁移；runtime 在 apply/dispose 前进入，
/// 保证登记落到正确的 Fiber（§3.1）。
pub struct FiberScope {
    prev: FiberId,
    _not_send: PhantomData<*const ()>,
}

impl FiberScope {
    pub fn enter(fiber_id: FiberId) -> Self {
        let prev = current_fiber_id();
        CURRENT_FIBER.with(|c| c.set(fiber_id));
        Self {
            prev,
            _not_send: PhantomData,
        }
    }
}

impl Drop for FiberScope {
    fn drop(&mut self) {
        CURRENT_FIBER.with(|c| c.set(self.prev));
    }
}

/// 失败类别（无载荷，保持 `FiberState: Copy`；详情在 `Fiber::failure`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureKind {
    /// apply 返回错误（已登记的 Effect 由 runtime 先撤销再落此态，§3.2）
    ApplyRejected,
    /// dispose 返回错误
    DisposeRejected,
}

/// 六态状态机（§3.3）。`Copy` 是刻意的：状态快照零成本，
/// 锁争用由 `Arc<RwLock<_>>` 承担。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FiberState {
    /// 等待依赖满足
    Pending,
    /// 正在执行 apply
    Loading,
    /// 正常运行
    Active,
    /// 正在执行撤销链
    Unloading,
    /// 已卸载，可重新激活
    Unloaded,
    /// 失败（类别见 `FailureKind`，详情见 `Fiber::failure`）
    Failed(FailureKind),
}

/// 插件运行实例（附录 A）。Effect 账本唯一存于 Context（按 FiberId 打标签），
/// Fiber 不维护副本（§3.3 注释）。
pub struct Fiber {
    id: FiberId,
    plugin: Box<dyn Plugin>,
    state: Arc<RwLock<FiberState>>,
    deps: Vec<TypeId>,
    /// 失败详情独立字段——状态枚举保持 Copy 的代价分离
    failure: RwLock<Option<String>>,
}

impl Fiber {
    pub(crate) fn new(id: FiberId, plugin: Box<dyn Plugin>) -> Self {
        let deps = plugin.inject();
        Self {
            id,
            plugin,
            state: Arc::new(RwLock::new(FiberState::Pending)),
            deps,
            failure: RwLock::new(None),
        }
    }

    pub fn id(&self) -> FiberId {
        self.id
    }

    /// 声明的依赖服务键（`inject`）。
    pub fn deps(&self) -> &[TypeId] {
        &self.deps
    }

    pub fn state(&self) -> FiberState {
        *self.state.read().expect("fiber state lock poisoned")
    }

    /// 失败详情（`Failed` 态才有；独立于状态枚举的载荷）。
    pub fn failure(&self) -> Option<String> {
        self.failure
            .read()
            .expect("fiber failure lock poisoned")
            .clone()
    }

    pub(crate) fn plugin(&self) -> &dyn Plugin {
        self.plugin.as_ref()
    }

    pub(crate) fn set_failure(&self, message: String) {
        *self.failure.write().expect("fiber failure lock poisoned") = Some(message);
    }

    /// 状态机迁移（§3.3 状态图；§15.4：非法迁移报错，绝不静默降级）。
    /// 检查与写入非原子：runtime 是唯一写者且按 Fiber 串行（阶段二约定），
    /// 并发编排经 §5.7 的事件缝合。
    pub(crate) fn transition(&self, to: FiberState) -> Result<()> {
        let from = self.state();
        let legal = matches!(
            (from, to),
            (FiberState::Pending, FiberState::Loading)
                | (FiberState::Loading, FiberState::Active)
                | (FiberState::Loading, FiberState::Failed(_))
                | (FiberState::Active, FiberState::Unloading)
                | (FiberState::Active, FiberState::Failed(_))
                | (FiberState::Unloading, FiberState::Unloaded)
                | (FiberState::Unloading, FiberState::Failed(_))
                | (FiberState::Unloaded, FiberState::Pending)
        );
        if !legal {
            return Err(CordisError::IllegalTransition {
                fiber: self.id.0,
                from,
                to,
            });
        }
        *self.state.write().expect("fiber state lock poisoned") = to;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
