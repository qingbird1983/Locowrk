//! 反应式重载：拓扑序四步流程与失败路径（§3.3）。

use std::any::TypeId;

use super::helpers::{Counter, Greeter, TestPlugin, new_log};
use crate::context::Context;
use crate::error::CordisError;
use crate::fiber::FiberState;
use crate::runtime::Runtime;

#[tokio::test]
async fn reload_provider_reactivates_downstream_in_topo_order() {
    let ctx = Context::new();
    let rt = Runtime::new();
    let lg = new_log();

    let p1 = rt
        .activate(
            &ctx,
            Box::new(TestPlugin::new("p1", lg.clone()).provides_counter(7)),
        )
        .await
        .unwrap();
    let p2 = rt
        .activate(
            &ctx,
            Box::new(
                TestPlugin::new("p2", lg.clone())
                    .depends_on_counter()
                    .provides_greeter(),
            ),
        )
        .await
        .unwrap();
    let p3 = rt
        .activate(
            &ctx,
            Box::new(TestPlugin::new("p3", lg.clone()).depends_on_greeter()),
        )
        .await
        .unwrap();

    let new_p1 = rt
        .reload_provider(
            &ctx,
            p1,
            Box::new(TestPlugin::new("p1'", lg.clone()).provides_counter(9)),
        )
        .await
        .unwrap();

    // 四步（§3.3）：深→浅卸载 → 卸旧 → 装新 → 浅→深重激活
    assert_eq!(
        *lg.lock().expect("log lock poisoned"),
        vec![
            "p1:apply",
            "p2:apply",
            "p3:apply", //
            "p3:dispose",
            "p2:dispose",
            "p1:dispose",
            "p1':apply",
            "p2:apply",
            "p3:apply",
        ]
    );
    assert_ne!(new_p1, p1);
    assert_eq!(rt.provider_of(TypeId::of::<Counter>()), Some(new_p1));
    assert_eq!(rt.dependents_of(new_p1), vec![p2]);
    assert_eq!(rt.dependents_of(p2), vec![p3]);
    // 旧 Fiber 保留为 Unloaded（逻辑卸载 + 物理保留，§5.6 纪律一）
    assert_eq!(rt.state_of(p1), Some(FiberState::Unloaded));
    assert_eq!(rt.state_of(p2), Some(FiberState::Active));
    assert_eq!(rt.state_of(p3), Some(FiberState::Active));
    assert_eq!(ctx.use_service::<Counter>().unwrap().0, 9);
    assert_eq!(ctx.use_dyn::<dyn Greeter>().unwrap().greet(), "hello");
}

#[tokio::test]
async fn failed_replacement_leaves_downstream_unloaded() {
    let ctx = Context::new();
    let rt = Runtime::new();
    let lg = new_log();

    let p1 = rt
        .activate(
            &ctx,
            Box::new(TestPlugin::new("p1", lg.clone()).provides_counter(7)),
        )
        .await
        .unwrap();
    let p2 = rt
        .activate(
            &ctx,
            Box::new(TestPlugin::new("p2", lg.clone()).depends_on_counter()),
        )
        .await
        .unwrap();

    // 替换者声明了缺失的依赖 → 激活门拒绝（§15.4：报错，不静默降级）
    let err = rt
        .reload_provider(
            &ctx,
            p1,
            Box::new(TestPlugin::new("bad", lg).depends_on_greeter()),
        )
        .await
        .unwrap_err();
    assert!(matches!(err, CordisError::DependencyMissing(_)));
    // 旧 Provider 与下游都保留为 Unloaded（逻辑卸载 + 物理保留）
    assert_eq!(rt.state_of(p1), Some(FiberState::Unloaded));
    assert_eq!(rt.state_of(p2), Some(FiberState::Unloaded));
    assert!(ctx.use_service::<Counter>().is_err());
}
