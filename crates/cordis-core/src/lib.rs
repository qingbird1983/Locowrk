//! Cordis 五支柱（DESIGN.md §3）。
//!
//! 阶段一模块：`context`（TypeId 服务载体）、`effect`（可逆修改 + LIFO 撤销链）、
//! `fiber`（身份 + 当前 Fiber 槽）、`event_bus`（emit 语义）。
//! 阶段二补：`fiber` 六态状态机、`runtime`（依赖图 + 反应式重载）。
//! 阶段四补：`scope`（单进程多 Agent 隔离）、emit 之外的
//! parallel / serial / waterfall 三种分发语义。
//!
//! 纪律：所有对 `Context` 的修改必须携带逆操作（Disposer）入 Effect 账本——
//! `provide` / `provide_dyn` 是登记入口；撤销锁内只领取、锁外执行（§9.1）。

mod context;
mod effect;
mod error;
mod event_bus;
mod fiber;

pub use context::{Context, Service};
pub use effect::{Disposer, EffectId};
pub use error::{CordisError, Result};
pub use event_bus::{EventBus, Listener, ListenerId};
pub use fiber::{FiberId, FiberScope, current_fiber_id};
