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
use crate::scope::{ScopeKey, ScopedLayers};

/// 服务标记 trait：Send + Sync + 'static 即可，`TypeId` 就是天然服务键，
/// 不需要 `const KEY`（§3.1）。服务 trait 必须声明 `Send + Sync` 父 trait，
/// `dyn T` 才满足本 blanket impl。
pub trait Service: Send + Sync + 'static {}
impl<T: ?Sized + Send + Sync + 'static> Service for T {}

type ServiceMap = DashMap<TypeId, Arc<dyn Any + Send + Sync>>;

/// 撤销一次服务注册的逆操作：恢复旧值而非删除——否则"覆盖注册后撤销"
/// 会误删第一次注册的服务（§3.1 关键设计点）。`scope` 记录注册落点：
/// `None` = global 层，`Some` = 某作用域层（§3.4），撤销只动原层。
struct Unprovide {
    key: TypeId,
    previous: Option<Arc<dyn Any + Send + Sync>>,
    layers: Arc<ScopedLayers<ServiceMap>>,
    scope: Option<ScopeKey>,
}

#[async_trait::async_trait]
impl Disposer for Unprovide {
    async fn dispose(self: Box<Self>) {
        match &self.scope {
            None => match self.previous {
                Some(prev) => {
                    self.layers.global().insert(self.key, prev);
                }
                None => {
                    self.layers.global().remove(&self.key);
                }
            },
            Some(scope) => {
                // DashMap 的 insert/remove 都是 &self：RefMut 只守分片存活
                let layer = self.layers.layer_mut(scope);
                match self.previous {
                    Some(prev) => {
                        layer.insert(self.key, prev);
                    }
                    None => {
                        layer.remove(&self.key);
                    }
                }
            }
        }
    }
}

/// 服务载体 + Effect 账本 + 事件总线（附录 A）。所有能力挂在同一个
/// `Context` 上，内核没有特权模块。
pub struct Context {
    /// 服务表载体：global 层 + 按作用域惰性创建的 scoped 层（§3.4）。
    layers: Arc<ScopedLayers<ServiceMap>>,
    effects: EffectLedger,
    /// 事件总线：内核经 `ctx.events` 访问（§7.2）。
    pub events: EventBus,
}

impl Context {
    pub fn new() -> Self {
        Self {
            layers: Arc::new(ScopedLayers::new()),
            effects: EffectLedger::default(),
            events: EventBus::new(),
        }
    }

    /// 按具体类型注册服务（global 层），返回 EffectId 供撤销（§3.1）。
    pub fn provide<S: Service>(&self, svc: S) -> EffectId {
        let key = TypeId::of::<S>();
        // 捕获旧值：撤销时恢复旧注册而不是直接删除
        let previous = self.layers.global().insert(key, Arc::new(svc));
        self.register_effect(key, previous, None)
    }

    /// 按 trait 对象注册服务（global 层）：键 = `TypeId::of::<T>()`，
    /// T 是 trait 本身。存储具体类型 `Arc<dyn T>`（Sized，可被 downcast），
    /// 外层包 `Arc<dyn Any>` 进表（§3.1）。
    pub fn provide_dyn<T: ?Sized + Service>(&self, svc: Arc<T>) -> EffectId {
        let key = TypeId::of::<T>();
        let previous = self
            .layers
            .global()
            .insert(key, Arc::new(svc) as Arc<dyn Any + Send + Sync>);
        self.register_effect(key, previous, None)
    }

    /// 按具体类型注册到某作用域层（§3.4）：只遮蔽该作用域，其他 Agent
    /// 与 global 查询不可见。同样走 Effect 账本，可撤销。
    pub fn provide_scoped<S: Service>(&self, scope: &ScopeKey, svc: S) -> EffectId {
        let key = TypeId::of::<S>();
        let previous = self.layers.layer_mut(scope).insert(key, Arc::new(svc));
        self.register_effect(key, previous, Some(scope.clone()))
    }

    /// 按 trait 对象注册到某作用域层（§3.4）——两 Agent 工具视图隔离
    /// 的入口（§14 阶段四验收）。
    pub fn provide_dyn_scoped<T: ?Sized + Service>(
        &self,
        scope: &ScopeKey,
        svc: Arc<T>,
    ) -> EffectId {
        let key = TypeId::of::<T>();
        let previous = self
            .layers
            .layer_mut(scope)
            .insert(key, Arc::new(svc) as Arc<dyn Any + Send + Sync>);
        self.register_effect(key, previous, Some(scope.clone()))
    }

    /// 登记一次与"提供/撤下服务"无关的可逆修改（§3.2 原语，provide* 是
    /// 它的特例）。归宿主 Fiber（`FiberId::HOST`）名下，不关联服务键；
    /// `revoke(id)` 触发逆操作。典型调用方：PluginService 把"安装插件"
    /// 记成一条以"逻辑卸载"为逆操作的 Effect（§11.2）。
    pub fn effect(&self, disposer: Box<dyn Disposer>) -> EffectId {
        self.effects
            .push(FiberId::HOST, TypeId::of::<()>(), disposer)
    }

    /// 按具体类型查询服务（global 层），返回 Arc 快照——调用方只在这一
    /// 瞬间持有分片读锁，后续长耗时操作不持锁（§3.1）。
    pub fn use_service<S: Service>(&self) -> Result<Arc<S>> {
        self.layers
            .global()
            .get(&TypeId::of::<S>())
            .and_then(|e| e.value().clone().downcast::<S>().ok())
            .ok_or_else(CordisError::service_missing::<S>)
    }

    /// 按 trait 对象查询服务（global 层，§3.1）：内层存储是 `Arc<dyn T>`，
    /// downcast 成 `Arc<Arc<dyn T>>` 后拆壳克隆。
    pub fn use_dyn<T: ?Sized + Service>(&self) -> Result<Arc<T>> {
        self.layers
            .global()
            .get(&TypeId::of::<T>())
            .and_then(|e| e.value().clone().downcast::<Arc<T>>().ok())
            .map(|outer| (*outer).clone())
            .ok_or_else(CordisError::service_missing::<T>)
    }

    /// 作用域内查询（§3.4 use_scoped）：scoped 层命中优先，未命中回退
    /// global。没有任何 scoped 层时直接走 global，零额外开销（§9.3）。
    pub fn use_service_scoped<S: Service>(&self, scope: &ScopeKey) -> Result<Arc<S>> {
        if !self.layers.scoped_is_empty() {
            let key = TypeId::of::<S>();
            let hit = self.layers.with_scoped(scope, |l| {
                l.get(&key)
                    .and_then(|e| e.value().clone().downcast::<S>().ok())
            });
            if let Some(svc) = hit {
                return Ok(svc);
            }
        }
        self.use_service::<S>()
    }

    /// 作用域内 trait 对象查询（§3.4）：scoped 命中优先，否则回退 global。
    pub fn use_dyn_scoped<T: ?Sized + Service>(&self, scope: &ScopeKey) -> Result<Arc<T>> {
        if !self.layers.scoped_is_empty() {
            let key = TypeId::of::<T>();
            let hit = self.layers.with_scoped(scope, |l| {
                l.get(&key)
                    .and_then(|e| e.value().clone().downcast::<Arc<T>>().ok())
                    .map(|outer| (*outer).clone())
            });
            if let Some(svc) = hit {
                return Ok(svc);
            }
        }
        self.use_dyn::<T>()
    }

    /// 依赖探针：服务键是否已有提供者（runtime 激活门的依据，§3.3）。
    /// 只看 global 层：插件依赖声明是进程级的，作用域层不参与激活门。
    pub fn has_service(&self, key: TypeId) -> bool {
        self.layers.global().contains_key(&key)
    }

    /// 某 Fiber 当前登记提供的服务键（runtime 建 service_providers 表的
    /// 来源，§3.3）。账本是唯一记录处（§3.1：Fiber 不维护副本）。
    pub(crate) fn provided_keys_of(&self, fiber_id: FiberId) -> Vec<TypeId> {
        self.effects.keys_by_fiber(fiber_id)
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
        scope: Option<ScopeKey>,
    ) -> EffectId {
        self.effects.push(
            current_fiber_id(),
            key,
            Box::new(Unprovide {
                key,
                previous,
                layers: Arc::clone(&self.layers),
                scope,
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
