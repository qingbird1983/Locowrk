//! 逐模型连通测试（§8.3）：最小请求 —— 单条 user "ping"，非流式、无工具、无 max_tokens，
//! 带总超时打一次；成功返回耗时 ms，失败按五分类 + unknown 如实归类（message 面向用户）。
//! 本片是 `client.rs` 主环的**子模块**（薄切出来的那一条旁路）：它吃主环两个 `pub(super)`
//! 装配函数与那枚私有超时常量 —— 子模块经 `super::` 够得到，所以超时**不在这里重抄第二个 15**，
//! 定义仍只在 `client.rs` 一处。

use std::time::Instant;

use crate::llm::models::apply_proxy;
use crate::settings::LlmSettings;

use super::{CompatLlm, TEST_TIMEOUT};

/// 逐模型连通测试失败（§8.3 五分类 + unknown；message 面向用户如实）。
#[derive(Debug, Clone)]
pub enum TestFailure {
    Auth(String),
    ModelNotFound(String),
    RateLimit(String),
    Network(String),
    Server(String),
    Unknown(String),
}

impl TestFailure {
    /// 分类标识（IPC `category` 字段，前端如实渲染徽章）。
    pub fn category(&self) -> &'static str {
        match self {
            TestFailure::Auth(_) => "auth",
            TestFailure::ModelNotFound(_) => "model_not_found",
            TestFailure::RateLimit(_) => "rate_limit",
            TestFailure::Network(_) => "network",
            TestFailure::Server(_) => "server",
            TestFailure::Unknown(_) => "unknown",
        }
    }

    pub fn message(&self) -> String {
        match self {
            TestFailure::Auth(m)
            | TestFailure::ModelNotFound(m)
            | TestFailure::RateLimit(m)
            | TestFailure::Network(m)
            | TestFailure::Server(m)
            | TestFailure::Unknown(m) => m.clone(),
        }
    }
}

/// 报文截断（200 字符，探测同款）。
fn snippet(body: &str) -> String {
    let s: String = body.chars().take(200).collect();
    s
}

/// 逐模型连通测试（§8.3）：最小请求（单条 user "ping"，非流式、无工具、
/// 无 max_tokens）+ 15s 总超时。成功返回耗时 ms；失败按五分类如实归类。
pub async fn test_model(cfg: &LlmSettings, model: &str) -> Result<u64, TestFailure> {
    let started = Instant::now();
    let client = apply_proxy(
        reqwest::Client::builder().timeout(TEST_TIMEOUT),
        cfg.network_proxy.as_ref(),
    )
    .build()
    .map_err(|e| TestFailure::Network(format!("HTTP 客户端构建失败: {e}")))?;
    let payload = serde_json::json!({
        "model": model,
        "messages": [{ "role": "user", "content": "ping" }],
        "stream": false,
    });
    let key = cfg.api_key.clone().unwrap_or_default();
    let resp = client
        .post(CompatLlm::endpoint(cfg))
        .headers(CompatLlm::build_headers(cfg, &key))
        .json(&payload)
        .send()
        .await;
    let resp = match resp {
        Ok(r) => r,
        Err(e) => {
            // 超时 / DNS / 连接拒绝统一 network（不细分——前端只渲染徽章）
            return Err(TestFailure::Network(format!("连接失败: {e}")));
        }
    };
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    let detail = snippet(&body);
    match status.as_u16() {
        200..=299 => Ok(started.elapsed().as_millis() as u64),
        401 | 403 => Err(TestFailure::Auth(format!(
            "鉴权失败（HTTP {status}）: {detail}"
        ))),
        404 => Err(TestFailure::ModelNotFound(format!(
            "模型 {model} 不存在（HTTP 404）: {detail}"
        ))),
        429 => Err(TestFailure::RateLimit(format!(
            "触发限流（HTTP 429）: {detail}"
        ))),
        s if (500..=599).contains(&s) => Err(TestFailure::Server(format!(
            "服务端错误（HTTP {status}）: {detail}"
        ))),
        s => Err(TestFailure::Unknown(format!("HTTP {s}: {detail}"))),
    }
}
