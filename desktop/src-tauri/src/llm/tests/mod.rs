//! llm 模块测试（全部无网络）：线上协议装配与解析（wire）、SSE 分帧
//! （sse）、错误分类与退避（retry）、**失败档位**（failure_kind）、设置分发与
//! 缺 Key 快速失败。

//! 拆分说明（v2.159 批 F-9；specs/13 刀⑦b 加第五片）：本模块只留**共享夹具**
//! （`temp_root` / `user_session` / `asset_output` / `tool_msg` / **本地假端点**
//! `fake_endpoint`）与顶层 `use`，用例按被测面分派到子文件——`wire`（线上协议
//! 装配）· `transport`（SSE 分帧 + 错误分类与退避）· `dispatch`（端点与设置分发）·
//! `assets`（工具结果图片投影）· `failure_kind`（**接入面分档**：状态码 /
//! `finish_reason` ⇒ 档位）。
//! 形状照 `settings/tests/` 先例：夹具是 tests 的私有项，
//! 子簇经 `use super::*;` 取用 ⇒ **可见性零改动**；用例本体逐字搬运，断言一条未增删。

mod assets;
mod budget;
mod dispatch;
mod failure_kind;
mod payload;
mod refusals;
mod spill;
mod tool_budget;
mod transport;

use std::sync::Arc;

use kernel_agent::{LlmError, LlmRequest, LlmService};

use plugin_api::{CallId, SideEffect, ToolSpec};

use serde_json::json;

use session_store::{Message, ToolCallMsg};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use super::app_llm::AppLlm;

use super::client::{CompatLlm, RetryHooks};

use super::dialect::Dialect;

use super::failure::FailureKind;

use super::retry::{self, Failure};

use super::sse::{SseEvent, SseParser};

use super::wire::{build_payload, parse_usage, StreamAcc, WireCaps};
// 子簇正文里的 `super::wire::…` 需要 tests 命名空间存在 `wire` 绑定 ⇒ 整模块 use。
use super::wire;

use crate::settings::{LlmSettings, NetworkProxy, SettingsStore};

pub(crate) fn temp_root(tag: &str) -> std::path::PathBuf {
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

// ---- 本地假端点（传输层的真实出口替身） ----------------------------------------

/// 指向本地假端点的配置。
///
/// ⚠ `network_proxy = direct` 是**必须**的：缺省是 system 档（吃环境变量），
/// 开发机上有 `HTTP_PROXY` 时请求会被代理接管 ⇒ 测的根本不是本地端点。
pub(crate) fn cfg_to(base: &str) -> LlmSettings {
    LlmSettings {
        provider: "openai".into(),
        model: "m".into(),
        base_url: Some(base.into()),
        api_key: Some("sk-test".into()),
        window_tokens: None,
        headers: Default::default(),
        network_proxy: Some(NetworkProxy {
            mode: "direct".into(),
            url: None,
        }),
    }
}

/// 一句最省的用户请求。
pub(crate) fn one_user_req() -> LlmRequest {
    LlmRequest {
        messages: vec![Message::User {
            content: "hi".into(),
            parts: Vec::new(),
        }],
        ..Default::default()
    }
}

/// 起一个本地 TCP 假端点，返回它的 base url（端点路径照客户端的归一走 `/v1/...`，
/// 假服务器不关心路径）。
///
/// `reply`：
/// - `Some((状态行, 响应体))` ⇒ 回这一份（状态行自带 CRLF 头）；写完即关连接
///   （客户端读到 EOF ⇒ 收束）；
/// - `None` ⇒ 接受连接后**一个字节都不写**（挂死那一档：客户端挂在读上，直到它
///   自己的看门狗或预算接管）。
///
/// 为什么住在这儿（而不是各子文件各起一个）：假服务器是这一族共享的夹具，
/// 两处各写一遍 = 两个形状（改一处另一处不跟）。
pub(crate) async fn fake_endpoint(reply: Option<(String, String)>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        while let Ok((mut sock, _)) = listener.accept().await {
            // 每次连接一份（应答体不是 Copy ⇒ 不能整轮共用同一份所有权）。
            let reply = reply.clone();
            tokio::spawn(async move {
                // 读掉请求（客户端在等响应；读不完也不影响它收到响应）
                let mut buf = vec![0u8; 16 * 1024];
                let _ = sock.read(&mut buf).await;
                match reply {
                    Some((head, body)) => {
                        let _ = sock.write_all(head.as_bytes()).await;
                        let _ = sock.write_all(body.as_bytes()).await;
                        let _ = sock.flush().await;
                    }
                    None => {
                        // 不写、不关：客户端挂在读上，直到它自己的看门狗或预算接管
                        std::future::pending::<()>().await;
                    }
                }
            });
        }
    });
    format!("http://{addr}")
}

/// 200 + SSE 的应答头（`Content-Type` 那一族）。
pub(crate) const SSE_HEAD: &str =
    "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n";
