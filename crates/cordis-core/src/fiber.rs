//! Fiber：插件生命周期容器（DESIGN.md §3.3）。
//!
//! 阶段一只落地身份与"当前 Fiber"槽；六态状态机、依赖图与反应式重载
//! 在阶段二（`runtime.rs`）补齐。`Context::provide` 用当前 Fiber 给
//! Effect 打标签（§3.1），阶段二的 runtime 在 apply/dispose 前进入作用域。

use std::cell::Cell;
use std::marker::PhantomData;

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
/// 非 Send——守卫不可跨线程迁移；runtime（阶段二）在 apply/dispose 前
/// 进入，保证登记落到正确的 Fiber。
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
