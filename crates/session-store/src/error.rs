//! session-store 统一错误类型。

/// 会话存储错误。
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("snapshot schema {found} cannot be upgraded to {target}: missing upcaster")]
    SchemaGap { found: u32, target: u32 },
    #[error("snapshot schema {found} is newer than supported {current} (cannot downgrade)")]
    TooNew { found: u32, current: u32 },
    #[error("invalid snapshot: {0}")]
    Invalid(String),
}
