//! `fiber` 模块单测：六态状态机的合法/非法迁移与失败载荷分离。

use super::{FailureKind, Fiber, FiberId, FiberState};
use crate::context::Context;
use crate::plugin::{Plugin, PluginError};

struct Noop;

#[async_trait::async_trait]
impl Plugin for Noop {
    async fn apply(&self, _ctx: &Context) -> Result<(), PluginError> {
        Ok(())
    }

    async fn dispose(&self, _ctx: &Context) -> Result<(), PluginError> {
        Ok(())
    }
}

fn fiber() -> Fiber {
    Fiber::new(FiberId(42), Box::new(Noop))
}

#[test]
fn happy_path_walks_the_state_diagram() {
    let f = fiber();
    assert_eq!(f.state(), FiberState::Pending);
    f.transition(FiberState::Loading).unwrap();
    f.transition(FiberState::Active).unwrap();
    f.transition(FiberState::Unloading).unwrap();
    f.transition(FiberState::Unloaded).unwrap();
    // 依赖恢复：Unloaded → Pending（§3.3 状态图回边）
    f.transition(FiberState::Pending).unwrap();
    assert_eq!(f.state(), FiberState::Pending);
}

#[test]
fn illegal_transitions_are_rejected() {
    let f = fiber();
    // Pending 只能进 Loading
    assert!(f.transition(FiberState::Active).is_err());
    // Active 不能直接重进 Loading
    f.transition(FiberState::Loading).unwrap();
    f.transition(FiberState::Active).unwrap();
    assert!(f.transition(FiberState::Loading).is_err());
    // Unloaded 不能跳进 Active（必须经 Pending → Loading）
    f.transition(FiberState::Unloading).unwrap();
    f.transition(FiberState::Unloaded).unwrap();
    assert!(f.transition(FiberState::Active).is_err());
}

#[test]
fn failed_is_terminal_and_carries_payload_separately() {
    let f = fiber();
    f.transition(FiberState::Loading).unwrap();
    f.set_failure("boom".into());
    f.transition(FiberState::Failed(FailureKind::ApplyRejected))
        .unwrap();
    assert_eq!(f.state(), FiberState::Failed(FailureKind::ApplyRejected));
    assert_eq!(f.failure().as_deref(), Some("boom"));
    // Failed 是终态，无出边
    assert!(f.transition(FiberState::Pending).is_err());
}

#[test]
fn state_enums_stay_copy() {
    fn assert_copy<T: Copy>() {}
    assert_copy::<FiberState>();
    assert_copy::<FailureKind>();
    // FiberId / 状态快照零成本
    let f = fiber();
    let a = f.state();
    let b = f.state();
    assert_eq!(a, b);
    assert_eq!(f.id(), FiberId(42));
}
