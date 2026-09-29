//! `context` 模块单测：服务注册双轨、撤销恢复旧值、交错 Effect 独立性。
//!
//! 注意：`Service` 是 blanket impl（`impl<T: ?Sized + Send + Sync + 'static>
//! Service for T {}`，§3.1），任何 Send+Sync+'static 类型自动满足，
//! 测试类型不需要也不能手写 `impl Service`。

use std::sync::Arc;

use super::Context;
use crate::effect::EffectId;
use crate::error::CordisError;
use crate::fiber::{FiberId, FiberScope};

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
