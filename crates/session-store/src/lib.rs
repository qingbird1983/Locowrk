//! 会话事件日志：唯一真相源（DESIGN.md §4、§6）。
//!
//! 阶段一模块：`event`（SessionEvent 全集）、`session`（追加/投影/回退/fork）、
//! `snapshot`（schema_version + upcaster 演进）、`compensation`（补偿计划 +
//! 幂等登记）。
//!
//! 纪律：事件只追加不删除，回退 = 移动游标 + replay；
//! 补偿工具必须幂等，用 `call_id` 做去重键。

mod compensation;
mod error;
mod event;
mod session;
mod snapshot;

pub use compensation::{CompensatedLedger, Compensation, plan};
pub use error::StoreError;
pub use event::{SessionEvent, TurnId, TurnOutcome};
pub use session::{Message, SNAPSHOT_EVERY, Session, SessionState, ToolCallMsg};
pub use snapshot::{SNAPSHOT_SCHEMA_VERSION, Snapshot, Upcaster, Upcasters};
