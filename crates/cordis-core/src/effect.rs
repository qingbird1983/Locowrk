//! Effect：每次修改携带逆操作（DESIGN.md §3.2、§9.1）。
//!
//! 两段式撤销（§3.1/§3.2/参考笔记 ADR 0010）：锁内只 remove 领取、锁外执行——
//! Disposer 可能回调 `Context`，持锁 await 会死锁（std::sync::Mutex 不可重入）。
//! 领取即所有权转移（at-most-once）：重复 revoke 得到 `EffectNotFound`，天然幂等。

use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::CordisError;
use crate::fiber::FiberId;

/// 一次可逆修改的标识。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EffectId(pub u64);

/// 逆操作。`dispose` 必须在锁外执行（§9.1 方案 A：账本用 std::sync::Mutex，
/// 临界区只做 push/remove，从不 await）。
#[async_trait::async_trait]
pub trait Disposer: Send + Sync + 'static {
    async fn dispose(self: Box<Self>);
}

/// 账本条目。`fiber_id` 用于交错 Effect 的独立撤销（§9.2：FiberId 标签 +
/// partition + 逆序执行，只卸载一个组件时不动其他组件的修改）。
pub(crate) struct EffectEntry {
    pub(crate) id: EffectId,
    pub(crate) fiber_id: FiberId,
    pub(crate) disposer: Box<dyn Disposer>,
}

/// Effect 账本（§3.1：唯一存于 Context，Fiber 不再维护副本）。
#[derive(Default)]
pub(crate) struct EffectLedger {
    inner: Mutex<Vec<EffectEntry>>,
    next: AtomicU64,
}

impl EffectLedger {
    /// 登记一次可逆修改，返回 EffectId。
    pub(crate) fn push(&self, fiber_id: FiberId, disposer: Box<dyn Disposer>) -> EffectId {
        let id = EffectId(self.next.fetch_add(1, Ordering::Relaxed));
        self.inner
            .lock()
            .expect("effect ledger lock poisoned")
            .push(EffectEntry {
                id,
                fiber_id,
                disposer,
            });
        id
    }

    /// 领取一条 Effect 的逆操作（消费即所有权转移）。
    /// 领取不到 = 已撤销，返回 `EffectNotFound`。
    pub(crate) fn take(&self, id: EffectId) -> Result<Box<dyn Disposer>, CordisError> {
        let mut entries = self.inner.lock().expect("effect ledger lock poisoned");
        let pos = entries
            .iter()
            .position(|e| e.id == id)
            .ok_or(CordisError::EffectNotFound { id: id.0 })?;
        Ok(entries.remove(pos).disposer)
    }

    /// 领取某 Fiber 登记的全部 Effect（保持登记顺序；调用方逆序 dispose）。
    /// 只摘走该 Fiber 的条目，其余原样保留——交错 Effect 的独立性（§9.2）。
    pub(crate) fn take_by_fiber(&self, fiber_id: FiberId) -> Vec<Box<dyn Disposer>> {
        let mut entries = self.inner.lock().expect("effect ledger lock poisoned");
        let (mine, rest): (Vec<_>, Vec<_>) =
            entries.drain(..).partition(|e| e.fiber_id == fiber_id);
        *entries = rest;
        mine.into_iter().map(|e| e.disposer).collect()
    }
}
