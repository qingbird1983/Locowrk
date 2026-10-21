//! specs/13 刀⑦b（**AC-28**）：**接入面分档**——传输层在**知道事实的地方**
//! 把失败分好档，档位随错误往上走。
//!
//! 为什么必须**真发一次**本地假端点，而不是只测分类纯函数：纯函数只能证
//! "分类表算出什么档"，证不了"传输层真的把档位挂到它交出去的那枚错误上"
//! （挂丢 ⇒ 压缩层收不到档 ⇒ 又得回文本匹配那条老路）。
//!
//! 四条事实家族（每条一个用例）：
//! · `finish_reason=length` ⇒ 撞输出上限（`OutputLimit`）
//! · `finish_reason=content_filter` ⇒ 被内容过滤（`ContentFilter`）
//! · HTTP 413 ⇒ 请求体字节太大（`PayloadTooLarge`）
//! · HTTP 400 + 超窗措辞 ⇒ 上下文超窗（`ContextOverflow`）
//! 另加：报文点名内容策略 ⇒ `ContentFilter`（别家供应商那一族）。

use super::*;

use std::time::Duration;

use crate::llm::failure::{FailureKind, LlmCallError};

/// 截断那一条流（`finish_reason=length`）。
const SSE_LENGTH: &str = "data: {\"choices\":[{\"delta\":{\"content\":\"摘要正文\"}}]}\n\n\
data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"length\"}]}\n\n\
data: [DONE]\n\n";

/// 被内容过滤那一条流（`finish_reason=content_filter`）。
const SSE_CONTENT_FILTER: &str = "data: {\"choices\":[{\"delta\":{\"content\":\"\"}}]}\n\n\
data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"content_filter\"}]}\n\n\
data: [DONE]\n\n";

/// 真发一趟，拿回那一枚错误（带档位）。
async fn call(base: &str) -> Result<kernel_agent::LlmResponse, LlmCallError> {
    let cfg = cfg_to(base);
    let llm = CompatLlm::new();
    tokio::time::timeout(
        Duration::from_secs(20),
        llm.complete_with(
            &cfg,
            WireCaps::default(),
            one_user_req(),
            None,
            None,
            Some(Duration::from_secs(10)),
        ),
    )
    .await
    .expect("预算内就该回来（没回来 = 这条会挂死整个测试进程）")
}

/// 状态码那一族：起一个回固定状态码的假端点。
async fn status_endpoint(status_line: &'static str, body: &'static str) -> String {
    fake_endpoint(Some((status_line.to_string(), body.to_string()))).await
}

/// 截断（`finish_reason=length`）⇒ 撞输出上限那一档。
///
/// 改前：传输层把它翻成一句中文报错，压缩层再从那句话里认"最大输出上限"那几个
/// 字 ⇒ 供应商换个措辞就归错档（界面那句"为什么没压成"直接说错）。
#[tokio::test]
async fn a_truncated_reply_is_an_output_limit() {
    let base = fake_endpoint(Some((SSE_HEAD.to_string(), SSE_LENGTH.to_string()))).await;
    let err = call(&base)
        .await
        .expect_err("截断（finish_reason=length）必须如实报错，不许当完整收束");
    assert_eq!(
        err.kind,
        FailureKind::OutputLimit,
        "截断 ⇒ 撞输出上限那一档：{err}"
    );
}

/// 被内容过滤（`finish_reason=content_filter`）⇒ 被内容过滤那一档。
///
/// 改前它**根本不算错误**（传输层把任何 `finish_reason` 都当正常收束）⇒ 主对话轮
/// 落一个空气泡、压缩层看到空回复（归 `no-summary`，还白跑一趟提醒重发）。
#[tokio::test]
async fn a_filtered_reply_is_a_content_filter() {
    let base = fake_endpoint(Some((SSE_HEAD.to_string(), SSE_CONTENT_FILTER.to_string()))).await;
    let err = call(&base)
        .await
        .expect_err("被内容过滤必须如实报错，不许当空收束");
    assert_eq!(
        err.kind,
        FailureKind::ContentFilter,
        "被内容过滤 ⇒ 内容过滤那一档：{err}"
    );
}

/// HTTP 413 ⇒ 请求体字节太大那一档（**结构化**：靠状态码，不靠报文里那几个词）。
#[tokio::test]
async fn an_http_413_is_a_payload_too_large() {
    let base = status_endpoint(
        "HTTP/1.1 413 Payload Too Large\r\nConnection: close\r\n\r\n",
        "一句与判据无关的报文",
    )
    .await;
    let err = call(&base).await.expect_err("413 必须报错");
    assert_eq!(
        err.kind,
        FailureKind::PayloadTooLarge,
        "413 ⇒ 字节太大那一档（多半是内联媒体 ⇒ 改纯文本再发）：{err}"
    );
}

/// HTTP 400 + 超窗措辞 ⇒ 上下文超窗那一档（可收缩 ⇒ 走收缩矩阵）。
#[tokio::test]
async fn an_overflow_400_is_a_context_overflow() {
    let base = status_endpoint(
        "HTTP/1.1 400 Bad Request\r\nConnection: close\r\n\r\n",
        "maximum context length exceeded",
    )
    .await;
    let err = call(&base).await.expect_err("超窗必须报错");
    assert_eq!(
        err.kind,
        FailureKind::ContextOverflow,
        "400 + 超窗措辞 ⇒ 超窗那一档：{err}"
    );
}

/// 报文点名内容策略（别家供应商那一族）⇒ 被内容过滤那一档。
#[tokio::test]
async fn a_content_policy_body_is_a_content_filter() {
    let base = status_endpoint(
        "HTTP/1.1 400 Bad Request\r\nConnection: close\r\n\r\n",
        "your prompt was blocked by our content policy",
    )
    .await;
    let err = call(&base).await.expect_err("内容策略拒收必须报错");
    assert_eq!(
        err.kind,
        FailureKind::ContentFilter,
        "报文点名内容策略 ⇒ 内容过滤那一档：{err}"
    );
}

/// 反向：普通请求错误**不许**被认成上面任何一档（否则界面会归错因）。
#[tokio::test]
async fn an_ordinary_request_error_is_not_one_of_those_kinds() {
    let base = status_endpoint(
        "HTTP/1.1 400 Bad Request\r\nConnection: close\r\n\r\n",
        "invalid field 'foo'",
    )
    .await;
    let err = call(&base).await.expect_err("400 必须报错");
    assert_eq!(
        err.kind,
        FailureKind::Other,
        "普通请求错误 ⇒ 其它那一档（不归超窗 / 不归内容过滤）：{err}"
    );
}
