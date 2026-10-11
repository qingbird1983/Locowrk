//! llm 模块测试（全部无网络）：线上协议装配与解析（wire）、SSE 分帧
//! （sse）、错误分类与退避（retry）、设置分发与缺 Key 快速失败。

//! 拆分说明（v2.159 批 F-9）：本模块只留**共享夹具**（`temp_root` / `user_session` /
//! `asset_output` / `tool_msg`）与顶层 `use`，18 条用例按被测面分派到 4 个子文件——
//! `wire`（线上协议装配）· `transport`（SSE 分帧 + 错误分类与退避）·
//! `dispatch`（端点与设置分发）· `assets`（工具结果图片投影）。
//! 形状照 `settings/tests/` 与 `organize/tests/` 先例：夹具是 tests 的私有项，
//! 子簇经 `use super::*;` 取用 ⇒ **可见性零改动**；用例本体逐字搬运，断言一条未增删。

mod assets;
mod dispatch;
mod payload;
mod transport;

use std::sync::Arc;

use kernel_agent::{LlmError, LlmRequest, LlmService};

use plugin_api::{CallId, SideEffect, ToolSpec};

use serde_json::json;

use session_store::{Message, ToolCallMsg};

use super::app_llm::AppLlm;

use super::client::CompatLlm;

use super::dialect::Dialect;

use super::retry::{self, Failure};

use super::sse::{SseEvent, SseParser};

use super::wire::{build_payload, parse_usage, StreamAcc, WireCaps};
// 子簇正文里的 `super::wire::…` 需要 tests 命名空间存在 `wire` 绑定 ⇒ 整模块 use。
use super::wire;

use crate::settings::SettingsStore;

fn temp_root(tag: &str) -> std::path::PathBuf {
    let dir = crate::testutil::work_dir(&format!("llm-{}", tag));
    dir
}

fn user_session() -> Vec<Message> {
    vec![
        Message::User {
            content: "查下天气".into(),
            parts: Vec::new(),
        },
        Message::Assistant {
            content: String::new(),
            tool_calls: vec![ToolCallMsg {
                id: CallId("call-9".into()),
                tool: "get_weather".into(),
                args: json!({ "city": "北京" }),
            }],
        },
        Message::ToolResult {
            call_id: CallId("call-9".into()),
            tool: "get_weather".into(),
            args: json!({ "city": "北京" }),
            output: json!({ "temp": "22C" }),
            error: None,
        },
    ]
}

fn asset_output() -> serde_json::Value {
    json!({
        "text": "screenshot 1920x1080 captured",
        "asset": { "id": "abc123", "mime": "image/png", "name": "shot.png" }
    })
}

fn tool_msg(output: serde_json::Value) -> Message {
    Message::ToolResult {
        call_id: CallId("call-1".into()),
        tool: "screenshot".into(),
        args: json!({}),
        output,
        error: None,
    }
}
