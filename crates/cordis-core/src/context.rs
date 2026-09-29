//! Context：服务的唯一载体（DESIGN.md §3.1）。
//!
//! 服务键双轨：具体类型键（`provide` / `use_service`）与 trait 对象键
//! （`provide_dyn` / `use_dyn`，键 = `TypeId::of::<dyn T>()`）。跨插件解耦
//! 一律走 trait 对象键——`use_service::<dyn T>()` 永远查不到 `provide_dyn`
//! 注册的服务，两者键不相等（§3.1 注释）。

use std::any::{Any, TypeId};
use std::sync::Arc;

use dashmap::DashMap;

use crate::effect::{Disposer, EffectId, EffectLedger};
use crate::error::{CordisError, Result};
use crate::event_bus::EventBus;
use crate::fiber::{FiberId, current_fiber_id};

/// 服务标记 trait：Send + Sync + 'static 即可，`TypeId` 就是天然服务键，
/// 不需要 `const KEY`（§3.1）。服务 trait 必须声明 `Send + Sync` 父 trait，
/// `dyn T` 才满足本 blanket impl。
pub trait Service: Send + Sync + 'static {}
impl<T: ?Sized + Send + Sync + 'static> Service for T {}

type ServiceMap = DashMap<TypeId, Arc<dyn Any + Send + Sync>>;

/// 撤销一次服务注册的逆操作：恢复旧值而非删除——否则"覆盖注册后撤销"
/// 会误删第一次注册的服务（§3.1 关键设计点）。
struct Unprovide {
    key: TypeId,
    previous: Option<Arc<dyn Any + Send + Sync>>,
    services: Arc<ServiceMap>,
}

#[async_trait::async_trait]
impl Disposer for Unprovide {
    async fn dispose(self: Box<Self>) {
        match self.previous {
            Some(prev) => {
                self.services.insert(self.key, prev);
            }
            None => {
                self.services.remove(&self.key);
            }
        }
    }
}

/// 服务载体 + Effect 账本 + 事件总线（附录 A）。所有能力挂在同一个
/// `Context` 上，内核没有特权模块。
pub struct Context {
    services: Arc<ServiceMap>,
    effects: EffectLedger,
    /// 事件总线：内核经 `ctx.events` 访问（§7.2）。
    pub events: EventBus,
}

impl Context {
    pub fn new() -> Self {
        Self {
            services: Arc::new(DashMap::new()),
            effects: EffectLedger::default(),
            events: EventBus::new(),
        }
    }

    /// 按具体类型注册服务，返回 EffectId 供撤销（§3.1）。
    pub fn provide<S: Service>(&self, svc: S) -> EffectId {
        let key = TypeId::of::<S>();
        // 捕获旧值：撤销时恢复旧注册而不是直接删除
        let previous = self.services.insert(key, Arc::new(svc));
        self.register_effect(key, previous)
    }

    /// 按 trait 对象注册服务：键 = `TypeId::of::<T>()`，T 是 trait 本身。
    /// 存储具体类型 `Arc<dyn T>`（Sized，可被 downcast），外层包 `Arc<dyn Any>`
    /// 进表（§3.1）。
    pub fn provide_dyn<T: ?Sized + Service>(&self, svc: Arc<T>) -> EffectId {
        let key = TypeId::of::<T>();
        let previous = self
            .services
            .insert(key, Arc::new(svc) as Arc<dyn Any + Send + Sync>);
        self.register_effect(key, previous)
    }

    /// 按具体类型查询服务，返回 Arc 快照——调用方只在这一瞬间持有分片
    /// 读锁，后续长耗时操作不持锁（§3.1）。
    pub fn use_service<S: Service>(&self) -> Result<Arc<S>> {
        self.services
            .get(&TypeId::of::<S>())
            .and_then(|e| e.value().clone().downcast::<S>().ok())
            .ok_or_else(CordisError::service_missing::<S>)
    }

    /// 按 trait 对象查询服务（§3.1）：内层存储是 `Arc<dyn T>`，
    /// downcast 成 `Arc<Arc<dyn T>>` 后拆壳克隆。
    pub fn use_dyn<T: ?Sized + Service>(&self) -> Result<Arc<T>> {
        self.services
            .get(&TypeId::of::<T>())
            .and_then(|e| e.value().clone().downcast::<Arc<T>>().ok())
            .map(|outer| (*outer).clone())
            .ok_or_else(CordisError::service_missing::<T>)
    }

    /// 显式撤销某个 Effect（§3.1/§3.2）：先出账、放锁、再执行逆操作。
    /// 重复撤销返回 `EffectNotFound`（两段式消费，幂等）。
    pub async fn revoke(&self, id: EffectId) -> Result<()> {
        let disposer = self.effects.take(id)?;
        disposer.dispose().await;
        Ok(())
    }

    /// 撤销某 Fiber 登记的全部 Effect：只摘走该 Fiber 的条目并逆序（LIFO）
    /// 执行（§3.2），其他 Fiber 的修改不受影响。
    pub async fn revoke_by_fiber(&self, fiber_id: FiberId) -> usize {
        let disposers = self.effects.take_by_fiber(fiber_id);
        let count = disposers.len();
        for disposer in disposers.into_iter().rev() {
            disposer.dispose().await;
        }
        count
    }

    fn register_effect(
        &self,
        key: TypeId,
        previous: Option<Arc<dyn Any + Send + Sync>>,
    ) -> EffectId {
        self.effects.push(
            current_fiber_id(),
            Box::new(Unprovide {
                key,
                previous,
                services: Arc::clone(&self.services),
            }),
        )
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
