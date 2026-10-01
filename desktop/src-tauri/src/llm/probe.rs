//! Ollama /api/show 上下文窗口探测（S2，清单 §5 自动提取来源）：设置面板
//! "自动检测"按钮的数据源。一次 HTTP 调用、5 秒超时；失败返回 Err 由 UI
//! 提示手动选择（读不到手动兜底，不做静默猜测）。

use std::time::Duration;

use serde_json::{json, Value};

const PROBE_TIMEOUT: Duration = Duration::from_secs(5);
const DEFAULT_OLLAMA_BASE: &str = "http://localhost:11434";

/// 探测模型上下文窗口（token）。`base_url` 空 = Ollama 默认端点。
pub async fn probe_context_window(base_url: Option<&str>, model: &str) -> Result<u64, String> {
    let base = base_url
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_OLLAMA_BASE);
    let url = format!("{}/api/show", base.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(PROBE_TIMEOUT)
        .build()
        .map_err(|e| format!("HTTP 客户端构建失败: {e}"))?;
    let resp: Value = client
        .post(&url)
        .json(&json!({ "model": model }))
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("Ollama /api/show 请求失败: {e}"))?
        .json()
        .await
        .map_err(|e| format!("Ollama /api/show 响应解析失败: {e}"))?;
    parse_context_length(&resp).ok_or_else(|| "Ollama 响应里没有 context_length".to_string())
}

/// /api/show 的窗口字段两代形态：顶层 `context_length`（新）或
/// `model_info` 里的 `<arch>.context_length`（旧，键前缀随架构变化）。
fn parse_context_length(resp: &Value) -> Option<u64> {
    if let Some(n) = resp.get("context_length").and_then(Value::as_u64) {
        return Some(n);
    }
    resp.get("model_info")?
        .as_object()?
        .iter()
        .find(|(k, _)| k.ends_with(".context_length"))
        .and_then(|(_, v)| v.as_u64())
}

#[cfg(test)]
mod tests;
