//! `session` 模块单测：两遍投影回填、游标回退、fork 快照裁剪、快照折算、
//! parts 穿透（批 D）。
//! 批 F-23 目录化（原 `tests.rs` 536 行按面分五簇，**用例本体逐字未改**）：
//! 共享事件夹具留本文件，子簇 `use super::*` 取用——
//! · `projection`   投影回填 / 尾巴投影 / 整理段落总结
//! · `cursor`       游标回退 / fork 前缀克隆 / rewind_before / 快照折算
//! · `compaction`   ContextCompacted 折叠（Context and Memory.md §3.7，⑨）
//! · `parts`        附件 parts 穿透（批 D）与 serde 向后兼容
//! · `pending_slot` 批 F-23（审计 M-24b）「命中即停」的等价证明
//!
//! 为什么这一刀落在性能批里：M-24b 的等价证明本身要 177 行（参考实现 + 可达
//! 形状序列 + 阳性对照 + 不可达形状的分岔钉子），直接把 359 行的 `tests.rs`
//! 推过 400 线 ⇒ 与其让 `check_length` 多一件超限，不如按 F-8/F-16 的既有
//! 形状目录化（超限件 22 持平）。

mod compaction;
mod cursor;
mod parts;
mod pending_slot;
mod projection;

use semver::Version;
use serde_json::json;

use plugin_api::{CallId, SideEffect};

use super::{SNAPSHOT_EVERY, Session};
use crate::content::{AssetRef, ContentPart};
use crate::event::SessionEvent;

fn user(content: impl Into<String>) -> SessionEvent {
    SessionEvent::UserMessage {
        content: content.into(),
        parts: Vec::new(),
    }
}

fn assistant(content: impl Into<String>) -> SessionEvent {
    SessionEvent::AssistantMessage {
        content: content.into(),
        tool_calls: vec![],
    }
}

fn tool_call(seq: u32) -> SessionEvent {
    SessionEvent::ToolCall {
        call_id: CallId(format!("call-{seq}")),
        tool: "echo".into(),
        args: json!({ "text": "hi" }),
        plugin_id: None,
        plugin_version: Version::new(0, 1, 0),
        side_effect: SideEffect::Pure,
    }
}

fn img(name: &str) -> ContentPart {
    ContentPart::Image {
        asset: AssetRef {
            id: format!("sha-{name}"),
            mime: "image/png".into(),
            name: name.into(),
        },
    }
}

// ---- ContextCompacted 折叠（Context and Memory.md §3.7，⑨）----

fn compacted(kept: usize, summary: &str) -> SessionEvent {
    SessionEvent::ContextCompacted {
        kept_messages: kept,
        summary: summary.into(),
        from_tokens: 1000,
        to_tokens: 400,
        strategy: "layered".into(),
        ts: 0,
    }
}
