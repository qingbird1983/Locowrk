//! `context` 模块单测：服务注册双轨、撤销恢复旧值、交错 Effect 独立性。
//!
//! 注意：`Service` 是 blanket impl（`impl<T: ?Sized + Send + Sync + 'static>
//! Service for T {}`，§3.1），任何 Send+Sync+'static 类型自动满足，
//! 测试类型不需要也不能手写 `impl Service`。

use std::any::TypeId;
use std::sync::Arc;

use super::Context;
use crate::effect::EffectId;
use crate::error::CordisError;
use crate::fiber::{FiberId, FiberScope};
use crate::scope::ScopeKey;

#[derive(Debug)]
struct Counter(u32);

trait Greeter: Send + Sync + 'static {
    fn greet(&self) -> String;
}

struct En;
impl Greeter for En {
    fn greet(&self) -> String {
        "hello".into()
    }
}

#[tokio::test]
async fn provide_and_use_service_roundtrip() {
    let ctx = Context::new();
    ctx.provide(Counter(7));
    assert_eq!(ctx.use_service::<Counter>().unwrap().0, 7);
}

#[tokio::test]
async fn missing_service_is_err() {
    let ctx = Context::new();
    assert!(matches!(
        ctx.use_service::<Counter>(),
        Err(CordisError::ServiceMissing { .. })
    ));
}

#[tokio::test]
async fn trait_object_track_roundtrip() {
    let ctx = Context::new();
    // 跨插件解耦走这对 API：插件 provide_dyn，内核 use_dyn（§3.1）
    ctx.provide_dyn::<dyn Greeter>(Arc::new(En));
    assert_eq!(ctx.use_dyn::<dyn Greeter>().unwrap().greet(), "hello");
    // 两轨不可混用是编译期事实：use_service 要求 S: Sized，
    // `use_service::<dyn Greeter>()` 根本无法通过类型检查（§3.1）
}

#[tokio::test]
async fn revoke_restores_previous_value() {
    let ctx = Context::new();
    let first = ctx.provide(Counter(1));
    let second = ctx.provide(Counter(2)); // 覆盖注册
    assert_eq!(ctx.use_service::<Counter>().unwrap().0, 2);

    // 撤销第二次注册：恢复第一次的旧值，而不是误删（§3.1 关键设计点）
    ctx.revoke(second).await.unwrap();
    assert_eq!(ctx.use_service::<Counter>().unwrap().0, 1);

    // 再撤销第一次：服务消失
    ctx.revoke(first).await.unwrap();
    assert!(ctx.use_service::<Counter>().is_err());
}

#[tokio::test]
async fn revoke_missing_is_not_found() {
    let ctx = Context::new();
    assert!(matches!(
        ctx.revoke(EffectId(999)).await,
        Err(CordisError::EffectNotFound { id: 999 })
    ));
}

#[tokio::test]
async fn revoke_by_fiber_isolates_interleaved_effects() {
    let ctx = Context::new();
    ctx.provide(Counter(0)); // HOST 的服务

    let tagged = {
        let _scope = FiberScope::enter(FiberId(7));
        ctx.provide(Counter(7));
        ctx.provide_dyn::<dyn Greeter>(Arc::new(En));
        2 // 该 Fiber 登记的 Effect 数
    };

    // 只卸载 Fiber 7 的修改；此后 HOST 注册的服务不受影响（§9.2 交错独立性）
    assert_eq!(ctx.revoke_by_fiber(FiberId(7)).await, tagged);
    // 撤销覆盖注册 = 恢复 HOST 之前的 Counter(0)（恢复旧值而非删除，§3.1）
    assert_eq!(ctx.use_service::<Counter>().unwrap().0, 0);
    assert!(ctx.use_dyn::<dyn Greeter>().is_err());

    ctx.provide(Counter(42));
    assert_eq!(ctx.use_service::<Counter>().unwrap().0, 42);
}

#[tokio::test]
async fn provided_keys_of_reflects_fiber_registrations() {
    let ctx = Context::new();
    ctx.provide(Counter(0)); // HOST

    let keys = {
        let _scope = FiberScope::enter(FiberId(9));
        ctx.provide(Counter(1));
        ctx.provide_dyn::<dyn Greeter>(Arc::new(En));
        ctx.provided_keys_of(FiberId(9))
    };

    assert_eq!(keys.len(), 2);
    assert!(keys.contains(&TypeId::of::<Counter>()));
    assert!(keys.contains(&TypeId::of::<dyn Greeter>()));
    // HOST 的登记独立（§9.2 交错独立性）
    assert_eq!(
        ctx.provided_keys_of(FiberId::HOST),
        vec![TypeId::of::<Counter>()]
    );

    // 撤销后账目随账本清空（账本是唯一记录处，§3.1）
    ctx.revoke_by_fiber(FiberId(9)).await;
    assert!(ctx.provided_keys_of(FiberId(9)).is_empty());
}

// ---- Scope 隔离（§3.4 / §14 阶段四：两 Agent 服务视图隔离） ----

struct Zh;
impl Greeter for Zh {
    fn greet(&self) -> String {
        "你好".into()
    }
}

#[tokio::test]
async fn scoped_service_shadows_only_its_scope() {
    let ctx = Context::new();
    ctx.provide_dyn::<dyn Greeter>(Arc::new(En));

    let scope_b = ScopeKey::new();
    let scope_c = ScopeKey::new();
    ctx.provide_dyn_scoped::<dyn Greeter>(&scope_b, Arc::new(Zh));

    // 作用域 B 命中 scoped 层（遮蔽 global）
    assert_eq!(
        ctx.use_dyn_scoped::<dyn Greeter>(&scope_b).unwrap().greet(),
        "你好"
    );
    // 作用域 C 层不存在 → 回退 global（§3.4 use_scoped 语义）
    assert_eq!(
        ctx.use_dyn_scoped::<dyn Greeter>(&scope_c).unwrap().greet(),
        "hello"
    );
    // 无作用域查询与 global 查询不受影响
    assert_eq!(ctx.use_dyn::<dyn Greeter>().unwrap().greet(), "hello");
    // 具体类型轨同样成立
    ctx.provide_scoped(&scope_b, Counter(9));
    assert_eq!(ctx.use_service_scoped::<Counter>(&scope_b).unwrap().0, 9);
    assert!(ctx.use_service::<Counter>().is_err());
}

#[tokio::test]
async fn scoped_provide_revokes_back_to_global() {
    let ctx = Context::new();
    ctx.provide_dyn::<dyn Greeter>(Arc::new(En));
    let scope_b = ScopeKey::new();

    let id = ctx.provide_dyn_scoped::<dyn Greeter>(&scope_b, Arc::new(Zh));
    assert_eq!(
        ctx.use_dyn_scoped::<dyn Greeter>(&scope_b).unwrap().greet(),
        "你好"
    );

    // 撤销 scoped 注册：该层键消失，查询回退 global（Unprovide 只动原层）
    ctx.revoke(id).await.unwrap();
    assert_eq!(
        ctx.use_dyn_scoped::<dyn Greeter>(&scope_b).unwrap().greet(),
        "hello"
    );
}

#[tokio::test]
async fn scoped_overwrite_revoke_restores_previous_scoped_value() {
    let ctx = Context::new();
    let scope_b = ScopeKey::new();
    let first = ctx.provide_scoped(&scope_b, Counter(1));
    let second = ctx.provide_scoped(&scope_b, Counter(2));

    ctx.revoke(second).await.unwrap();
    assert_eq!(ctx.use_service_scoped::<Counter>(&scope_b).unwrap().0, 1);

    ctx.revoke(first).await.unwrap();
    // 层被清空但层容器保留；global 查询不受影响
    assert!(ctx.use_service_scoped::<Counter>(&scope_b).is_err());
    ctx.provide(Counter(5));
    assert_eq!(ctx.use_service_scoped::<Counter>(&scope_b).unwrap().0, 5);
}

// ---- Context::effect（§3.2 裸原语，宿主侧可逆修改） ----

/// 记录 dispose 次数的逆操作（测试观测用）。
struct Probe(std::sync::Arc<std::sync::atomic::AtomicUsize>);

#[async_trait::async_trait]
impl crate::effect::Disposer for Probe {
    async fn dispose(self: Box<Self>) {
        self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
}

#[tokio::test]
async fn bare_effect_revokes_once_and_is_host_tagged() {
    use std::sync::atomic::Ordering;
    let ctx = Context::new();
    let hits = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let id = ctx.effect(Box::new(Probe(Arc::clone(&hits))));

    // 归宿主 Fiber 名下：revoke_by_fiber(HOST) 能领走它
    assert_eq!(ctx.revoke_by_fiber(FiberId::HOST).await, 1);
    assert_eq!(hits.load(Ordering::Relaxed), 1);

    // 两段式消费：重复 revoke 是 EffectNotFound
    assert!(matches!(
        ctx.revoke(id).await,
        Err(CordisError::EffectNotFound { .. })
    ));
    assert_eq!(hits.load(Ordering::Relaxed), 1);
}
