//! 会话事件日志：唯一真相源（DESIGN.md §4、§6）。
//!
//! 阶段一模块：`event`（SessionEvent 全集）、`session`（追加/投影/回退/fork）、
//! `snapshot`（schema_version + upcaster 演进）、`compensation`（补偿计划 +
//! 幂等登记）；⑲ 增 `persist`（JSONL 落盘 / 恢复 / undo 截断重写）。
//!
//! 纪律：事件只追加不删除，回退 = 移动游标 + replay；
//! 补偿工具必须幂等，用 `call_id` 做去重键。

mod compensation;
mod content;
mod error;
mod event;
mod persist;
mod session;
mod snapshot;
mod uncertain;

pub use compensation::{CompensatedLedger, Compensation, plan};
pub use content::{AssetRef, ContentPart, fold_parts};
pub use error::StoreError;
pub use event::{SessionEvent, TurnId, TurnOutcome, UncertainDecision};
pub use persist::{
    SessionMeta, SessionMetaFile, SessionSearchHit, append_event, delete_session, list_sessions,
    load_session, read_meta, rewrite_log, search_sessions, session_file, write_model_override,
    write_organizer, write_permission_mode, write_thinking_level, write_title, write_workspace,
};
pub use session::{Message, SNAPSHOT_EVERY, Session, SessionState, ToolCallMsg};
pub use snapshot::{SNAPSHOT_SCHEMA_VERSION, Snapshot, Upcaster, Upcasters, V1ToV2Upcaster};
pub use uncertain::{UncertainCall, UncertainScan, closure, scan_uncertain};
