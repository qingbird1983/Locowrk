//! 传输层已知的失败事实（**结构化**）：档位随错误一起往上走。
//!
//! 为什么不用文本认（specs/13 刀⑦b · 挂账 ①）：压缩层原来从**错误文本**里猜
//! 「撞输出上限 / 被内容过滤 / 字节太大」——供应商换个措辞就归错档，界面那句
//! 「为什么没压成」直接说错。事实住在**知道它的地方**（HTTP 状态码、
//! `finish_reason`），随错误**结构化**交给上层；消费方按档认，不猜文本。
//!
//! **边界（写死，别越界）**：这条通道是**宿主本地**的（`desktop/src-tauri`）。
//! 内核契约那条路（`kernel_agent::LlmError` 只有 `Backend(String)`）原样不动
//! ——主对话轮的「超窗 ⇒ 触发压缩」仍按文本认（`llm::is_overflow_message`）。
//! 压缩那条路的发送口 `CompactionLlm` 是宿主本地 trait ⇒ 它的错误类型可以换。
//!
//! **枚举只有一个定义**（本文件）；两处"事实怎么归档"的判法各住在**知道那份
//! 事实的地方**：HTTP 家族在 `llm/retry.rs::classify_status`，`finish_reason`
//! 家族在 `llm/client.rs`（流式累积器已经收着它）。

use kernel_agent::LlmError;

/// 失败档位（接入面判、消费方认）。
///
/// ⚠ **只列真正会浮到调用方面前的那几档**：限流 / 瞬时故障在传输层**自己重试**
/// （"重试不设上限"那条裁定），浮不出来——预算掐断时浮出来的是 `Budget`。
/// 列一堆没人产出的档位 = 让人以为"限流也会以这一档浮出来"。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FailureKind {
    /// 上下文超窗（token 那一档）：请求太长 ⇒ 可收缩。
    ContextOverflow,
    /// 请求体字节太大（HTTP 413 那一族）：多半是内联媒体 ⇒ 改纯文本就能发。
    PayloadTooLarge,
    /// 撞输出上限（`finish_reason=length`）。
    OutputLimit,
    /// 被内容过滤（`finish_reason=content_filter` 或报文点名）。
    ContentFilter,
    /// 这条调用自己的**总时长预算**用完（specs/12 刀①）。
    Budget,
    /// 其它致命错误（鉴权 / 请求错误 / 未知）。
    Other,
}

/// 宿主侧的一次调用失败：**档位 + 文案**。
///
/// 文案只给人看（界面那句「为什么没压成」、日志那一行）；**判据只看档位**
/// ——所以把文案换成任意串也不该影响归类（判据：
/// `tests/compaction_opencode/failure_kind.rs`）。
#[derive(Debug, Clone)]
pub(crate) struct LlmCallError {
    pub kind: FailureKind,
    pub message: String,
}

impl LlmCallError {
    /// 造一枚失败（档位 + 文案）。
    pub(crate) fn new(kind: FailureKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    /// 退回内核那枚通用错误（**丢掉档位**）：内核契约那条路没有结构化通道。
    pub(crate) fn into_backend(self) -> LlmError {
        LlmError::Backend(self.message)
    }
}

impl std::fmt::Display for LlmCallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for LlmCallError {}
