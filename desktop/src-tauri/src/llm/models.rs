//! 模型清单探测（v2.58 §3.30 选择器数据源，DESIGN §13.1 档位制）：按
//! provider kind 探测可用模型——Ollama `GET {base}/api/tags`、OpenAI 兼容
//! `GET {base}/models`（Bearer Key）、Anthropic `GET {base}/models`
//! （x-api-key），`base` 经 `join_version` 归一（缺版本段补 `/v1`，已带
//! `/v<数字>` 原样——智谱 /v4、方舟 /v3 等预设端点由此可用）。5 秒超时；
//! 失败返回 Err 由调用方逐组如实报错（并行探测互不阻塞）。

use std::time::Duration;

use serde_json::Value;

/// 单档探测超时（composer 打开选择器要快，云端网关慢时宁可报错）。
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(5);
pub const DEFAULT_OLLAMA_BASE: &str = "http://localhost:11434";
pub const DEFAULT_OPENAI_BASE: &str = "https://api.openai.com/v1";
pub const DEFAULT_ANTHROPIC_BASE: &str = "https://api.anthropic.com/v1";

/// 端点版本段归一（㉚ 预设表解锁智谱 /v4、方舟 /v3 等非 /v1 端点）：
/// 末段已是 `/v<数字>` 原样返回，否则补 `/v1`——⑬「base_url 不含 /v1」
/// 的旧约定由此泛化为含不含都收。聊天（兼容客户端端点归一，openai /
/// anthropic 两 kind）与清单探测同吃此函数，两端约定不再漂移。
pub fn join_version(base: &str) -> String {
    let base = base.trim().trim_end_matches('/');
    let last = base.rsplit('/').next().unwrap_or("");
    let is_version =
        last.len() > 1 && last.starts_with('v') && last[1..].bytes().all(|b| b.is_ascii_digit());
    if is_version {
        base.to_string()
    } else {
        format!("{base}/v1")
    }
}

/// 按 kind 探测单档模型清单。`base_url` None = provider 缺省端点；
/// `api_key` 云端 kind 必需（缺 Key 直接报错，不给空结果装糊涂）。
pub async fn list_models(
    kind: &str,
    base_url: Option<&str>,
    api_key: Option<&str>,
) -> Result<Vec<String>, String> {
    match kind {
        "ollama" => list_ollama(base_url).await,
        "openai" => {
            let key = api_key.filter(|k| !k.is_empty()).ok_or_else(|| {
                "openai 兼容端点需要 API Key（设置里填写后可拉取模型清单）".to_string()
            })?;
            let base = base_url
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or(DEFAULT_OPENAI_BASE);
            list_openai_shape(base, key, |req| req.bearer_auth(key)).await
        }
        "anthropic" => {
            let key = api_key.filter(|k| !k.is_empty()).ok_or_else(|| {
                "anthropic 需要 API Key（设置里填写后可拉取模型清单）".to_string()
            })?;
            let base = base_url
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or(DEFAULT_ANTHROPIC_BASE);
            list_openai_shape(base, key, |req| {
                req.header("x-api-key", key)
                    .header("anthropic-version", "2023-06-01")
            })
            .await
        }
        other => Err(format!("不支持的 provider kind: {other}")),
    }
}

/// Ollama：`GET /api/tags` → `models[].name`。
async fn list_ollama(base_url: Option<&str>) -> Result<Vec<String>, String> {
    let base = base_url
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_OLLAMA_BASE);
    let url = format!("{}/api/tags", base.trim_end_matches('/'));
    let resp = http_json(|req| req, &url)
        .await
        .map_err(|e| format!("Ollama /api/tags 请求失败: {e}"))?;
    parse_ollama_tags(&resp)
}

/// `/api/tags` 响应解析（独立成函数供测试）。
fn parse_ollama_tags(resp: &Value) -> Result<Vec<String>, String> {
    let models = resp
        .get("models")
        .and_then(Value::as_array)
        .ok_or_else(|| "Ollama 响应里没有 models 数组".to_string())?;
    Ok(models
        .iter()
        .filter_map(|m| m.get("name").and_then(Value::as_str))
        .map(str::to_string)
        .collect())
}

/// OpenAI / Anthropic 共用形态：`GET {join_version(base)}/models` →
/// `data[].id`（鉴权头差异由 `auth` 闭包注入）。
async fn list_openai_shape(
    base: &str,
    _key: &str,
    auth: impl FnOnce(reqwest::RequestBuilder) -> reqwest::RequestBuilder,
) -> Result<Vec<String>, String> {
    let url = format!("{}/models", join_version(base));
    let resp = http_json(auth, &url)
        .await
        .map_err(|e| format!("{url} 请求失败: {e}"))?;
    parse_models_data(&resp)
}

/// `data[].id` 形态解析（OpenAI / Anthropic 同形；独立成函数供测试）。
fn parse_models_data(resp: &Value) -> Result<Vec<String>, String> {
    let data = resp
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| "响应里没有 data 数组（不是 OpenAI 兼容端点？）".to_string())?;
    Ok(data
        .iter()
        .filter_map(|m| m.get("id").and_then(Value::as_str))
        .map(str::to_string)
        .collect())
}

/// 共用 HTTP：GET + 超时 + 4xx/5xx 带上响应体片段报错（网关错误信息在
/// body 里，只有状态码没法排查）。
async fn http_json(
    auth: impl FnOnce(reqwest::RequestBuilder) -> reqwest::RequestBuilder,
    url: &str,
) -> Result<Value, String> {
    let client = reqwest::Client::builder()
        .timeout(PROBE_TIMEOUT)
        .build()
        .map_err(|e| format!("HTTP 客户端构建失败: {e}"))?;
    let resp = auth(client.get(url))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        let snippet: String = body.chars().take(200).collect();
        return Err(format!("HTTP {status}: {snippet}"));
    }
    resp.json::<Value>()
        .await
        .map_err(|e| format!("响应解析失败: {e}"))
}

#[cfg(test)]
mod tests;
