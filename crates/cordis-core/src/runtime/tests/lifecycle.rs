//! 激活门、撤销链与生命周期事件（§3.3 / §3.2）。

use std::any::TypeId;
use std::sync::Arc;

use super::helpers::{Counter, EventRecorder, Greeter, TestPlugin, new_log};
use crate::context::Context;
use crate::error::CordisError;
use crate::fiber::{FailureKind, FiberId, FiberState};
use crate::plugin::PluginError;
use crate::runtime::Runtime;

#[tokio::test]
async fn activate_links_provider_and_dependent() {
    let ctx = Context::new();
    let rt = Runtime::new();

    let p1 = rt
        .activate(
            &ctx,
            Box::new(TestPlugin::new("p1", new_log()).provides_counter(7)),
        )
        .await
        .unwrap();
    let p2 = rt
        .activate(
            &ctx,
            Box::new(
                TestPlugin::new("p2", new_log())
                    .depends_on_counter()
                    .provides_greeter(),
            ),
        )
        .await
        .unwrap();

    assert_eq!(rt.provider_of(TypeId::of::<Counter>()), Some(p1));
    assert_eq!(rt.provider_of(TypeId::of::<dyn Greeter>()), Some(p2));
    assert_eq!(rt.dependents_of(p1), vec![p2]);
    assert_eq!(rt.state_of(p1), Some(FiberState::Active));
    assert_eq!(rt.state_of(p2), Some(FiberState::Active));
    assert_eq!(ctx.use_service::<Counter>().unwrap().0, 7);
}

#[tokio::test]
async fn missing_dependency_blocks_activation_without_residue() {
    let ctx = Context::new();
    let rt = Runtime::new();

    let err = rt
        .activate(
            &ctx,
            Box::new(TestPlugin::new("p2", new_log()).depends_on_counter()),
        )
        .await
        .unwrap_err();
    assert!(matches!(err, CordisError::DependencyMissing(_)));
    // 拒绝激活不留任何登记（§3.3 状态图：Pending 等待，事件驱动重试归编排层）
    assert!(ctx.use_service::<Counter>().is_err());
}

#[tokio::test]
async fn apply_failure_revokes_effects_then_marks_failed() {
    let ctx = Context::new();
    let rt = Runtime::new();

    rt.activate(
        &ctx,
        Box::new(
            TestPlugin::new("bad", new_log())
                .provides_counter(3)
                .fails_apply(),
        ),
    )
    .await
    .unwrap_err();

    // apply 先登记了 Counter 再失败：Effect 已被撤销，服务不可见（§3.2）
    assert!(ctx.use_service::<Counter>().is_err());
    // Failed Fiber 保留在表里（UI 投影可见，§11.6），载荷在独立字段
    assert_eq!(
        rt.state_of(FiberId(1)),
        Some(FiberState::Failed(FailureKind::ApplyRejected))
    );
    assert!(rt.failure_of(FiberId(1)).unwrap().contains("boom"));
}

#[tokio::test]
async fn deactivate_unloads_downstream_first_and_revokes() {
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

    rt.deactivate(&ctx, p1).await.unwrap();

    // 深 → 浅：先卸最深的消费者，最后卸 Provider 自己（§3.3 流程 1-2）
    assert_eq!(
        *lg.lock().expect("log lock poisoned"),
        vec![
            "p1:apply",
            "p2:apply",
            "p3:apply", //
            "p3:dispose",
            "p2:dispose",
            "p1:dispose",
        ]
    );
    // 撤销链已把三个 Fiber 登记的服务全部回收（§3.2）
    assert!(ctx.use_service::<Counter>().is_err());
    assert!(ctx.use_dyn::<dyn Greeter>().is_err());
    assert_eq!(rt.state_of(p1), None);
    assert_eq!(rt.state_of(p2), None);
    assert_eq!(rt.state_of(p3), None);
    assert_eq!(rt.provider_of(TypeId::of::<Counter>()), None);
    assert!(rt.dependents_of(p1).is_empty());
    assert!(rt.dependents_of(p2).is_empty());
}

#[tokio::test]
async fn dispose_failure_marks_failed_and_stops_unload() {
    let ctx = Context::new();
    let rt = Runtime::new();

    let p1 = rt
        .activate(
            &ctx,
            Box::new(
                TestPlugin::new("p1", new_log())
                    .provides_counter(1)
                    .fails_dispose(),
            ),
        )
        .await
        .unwrap();

    let err = rt.deactivate(&ctx, p1).await.unwrap_err();
    assert!(matches!(err, CordisError::Plugin(PluginError::Dispose(_))));
    assert_eq!(
        rt.state_of(p1),
        Some(FiberState::Failed(FailureKind::DisposeRejected))
    );
    assert_eq!(
        rt.failure_of(p1).as_deref(),
        Some("dispose failed: cleanup broke")
    );
    // 撤销链已执行（先于 dispose），服务不可见；Fiber 停在 Failed 不再下探
    assert!(ctx.use_service::<Counter>().is_err());
}

#[tokio::test]
async fn lifecycle_events_are_emitted_per_transition() {
    let ctx = Context::new();
    let rt = Runtime::new();
    let seen = new_log();

    ctx.events
        .on("fiber.loading", Arc::new(EventRecorder(seen.clone())));
    ctx.events
        .on("fiber.active", Arc::new(EventRecorder(seen.clone())));

    rt.activate(
        &ctx,
        Box::new(TestPlugin::new("p", new_log()).provides_counter(1)),
    )
    .await
    .unwrap();

    // 每次状态迁移一条事件（§3.3：事件驱动而非轮询）
    assert_eq!(
        *seen.lock().expect("log lock poisoned"),
        vec!["fiber.loading", "fiber.active"]
    );
}
