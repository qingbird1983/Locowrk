//! Cordis 五支柱（DESIGN.md §3）。
//!
//! 阶段一模块：`context`（TypeId 服务载体）、`effect`（可逆修改 + LIFO 撤销链）、
//! `event_bus`（emit 语义）。
//! 阶段二已补：`fiber`（六态状态机 + 身份/当前 Fiber 槽）、`plugin`（生命周期
//! 接口）、`runtime`（依赖图 + 反应式重载）。
//! 阶段四已补：`scope`（单进程多 Agent 隔离）、event_bus 的
//! parallel / serial / waterfall 三种分发语义与决策类型（§3.4 / §3.5）。
//!
//! 纪律：所有对 `Context` 的修改必须携带逆操作（Disposer）入 Effect 账本——
//! `provide` / `provide_dyn`（含 scoped 变体）是登记入口；撤销锁内只领取、
//! 锁外执行（§9.1）。

mod context;
mod effect;
mod error;
mod event_bus;
mod fiber;
mod plugin;
mod runtime;
mod scope;

pub use context::{Context, Service};
pub use effect::{Disposer, EffectId};
pub use error::{CordisError, Result};
pub use event_bus::{
    EventBus, Listener, ListenerId, RejectReason, SerialListener, SerialOutcome, WaterfallDecision,
    WaterfallListener, WaterfallOutcome,
};
pub use fiber::{FailureKind, Fiber, FiberId, FiberScope, FiberState, current_fiber_id};
pub use plugin::{Plugin, PluginError};
pub use runtime::{DependencyGraph, Runtime};
pub use scope::{ScopeKey, ScopedLayers};
