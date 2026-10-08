//! 模型清单探测（v2.58 §3.30 选择器数据源，DESIGN §13.1 档位制）：按
//! provider kind 探测可用模型——Ollama `GET {base}/api/tags`、OpenAI 兼容
//! `GET {base}/models`（Bearer Key）、Anthropic `GET {base}/models`
//! （x-api-key），`base` 经 `join_version` 归一（缺版本段补 `/v1`，已带
//! `/v<数字>` 原样——智谱 /v4、方舟 /v3 等预设端点由此可用）。5 秒超时；
//! 失败返回 Err 由调用方逐组如实报错（并行探测互不阻塞）。
//! 批 C 增量：自定义请求头 + 网络代理随请求生效；`sweep_candidates` 端点
//! 扫描（§8.2，probe 失败时 ≤4 候选补中转站场景）。

use std::collections::BTreeMap;
use std::time::Duration;

use serde_json::Value;

use crate::settings::NetworkProxy;

/// 单档探测超时（composer 打开选择器要快，云端网关慢时宁可报错）。
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(5);
/// 端点扫描单候选超时（§8.2：3s 快试）。
pub const SWEEP_TIMEOUT: Duration = Duration::from_secs(3);
pub const DEFAULT_OLLAMA_BASE: &str = "http://localhost:11434";
pub const DEFAULT_OPENAI_BASE: &str = "https://api.openai.com/v1";
pub const DEFAULT_ANTHROPIC_BASE: &str = "https://api.anthropic.com/v1";

/// 网络代理档落 ClientBuilder（§8.5）：direct = 强制直连；custom = 指定
/// url（无效地址 warn 回落系统档）；system / None = reqwest 缺省（吃环境
/// 变量，现状）。client.rs 聊天客户端与探测同吃——保存即热换。
pub fn apply_proxy(
    b: reqwest::ClientBuilder,
    proxy: Option<&NetworkProxy>,
) -> reqwest::ClientBuilder {
    match proxy {
        Some(p) if p.mode == "direct" => b.no_proxy(),
        Some(p) if p.mode == "custom" => {
            match p.url.as_deref().map(str::trim).filter(|u| !u.is_empty()) {
                Some(url) => match reqwest::Proxy::all(url) {
                    Ok(px) => b.proxy(px),
                    Err(e) => {
                        tracing::warn!("自定义代理地址无效（{url}），回落系统档: {e}");
                        b
                    }
                },
                None => b,
            }
        }
        _ => b,
    }
}

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

/// 端点扫描候选（§8.2，纯函数供测试）：现值（join_version 归一）→
/// `{origin}/v1` → `{origin}/api/v1` → `{origin}/api`，去重保序，**剔除
/// 现值**（它刚失败过，不重试）；裸域名无 scheme 补全定位：本地形态
///（localhost / `127.*` / `::1` / 无点主机）补 `http://`（本地服务常态），
/// 其余补 `https://`（中转站常态）。返回空 = 无可扫（不是 URL 等）。
pub fn sweep_candidates(input: &str) -> Vec<String> {
    let raw = input.trim();
    if raw.is_empty() {
        return Vec::new();
    }
    let full = if raw.contains("://") {
        raw.to_string()
    } else {
        let host_part = raw.split('/').next().unwrap_or(raw);
        let host = host_part
            .rsplit('@')
            .next()
            .unwrap_or(host_part)
            .split(':')
            .next()
            .unwrap_or(host_part);
        let looks_local = host.eq_ignore_ascii_case("localhost")
            || host.starts_with("127.")
            || host == "::1"
            || !host.contains('.');
        format!("{}://{raw}", if looks_local { "http" } else { "https" })
    };
    let Some(origin) = reqwest::Url::parse(&full)
        .ok()
        .filter(|u| matches!(u.scheme(), "http" | "https"))
        .map(|u| {
            let host = u.host_str().unwrap_or_default();
            match u.port() {
                Some(p) => format!("{}://{host}:{p}", u.scheme()),
                None => format!("{}://{host}", u.scheme()),
            }
        })
        .filter(|o| !o.is_empty())
    else {
        return Vec::new();
    };
    let mut out: Vec<String> = Vec::new();
    let mut push = |c: String| {
        if !out.iter().any(|x| x.eq_ignore_ascii_case(&c)) {
            out.push(c);
        }
    };
    push(join_version(raw));
    push(format!("{origin}/v1"));
    push(format!("{origin}/api/v1"));
    push(format!("{origin}/api"));
    out.into_iter().skip(1).collect()
}

/// 端点扫描结果（§8.2）：`effective_base_url` = 扫描胜出且与传入不同的
/// 候选（join_version 归一形态，表单可直接采纳）。
pub struct SweepOutcome {
    pub result: Result<Vec<String>, String>,
    pub effective_base_url: Option<String>,
}

/// 探测 + 端点扫描：先按现值探一次，失败且 base_url 自定义时对候选逐一
/// 快试（3s each），首个成功者胜出。候选只变路径不换 host（凭据不发去
/// 陌生主机）。扫描全败 = 原**首探错误**如实上抛（候选错误更吵且无诊断
/// 增益）。
pub async fn list_models_swept(
    kind: &str,
    base_url: Option<&str>,
    api_key: Option<&str>,
    headers: &BTreeMap<String, String>,
    proxy: Option<&NetworkProxy>,
) -> SweepOutcome {
    let first = list_models(kind, base_url, api_key, headers, proxy).await;
    let Some(input) = base_url.map(str::trim).filter(|s| !s.is_empty()) else {
        return SweepOutcome {
            result: first,
            effective_base_url: None,
        };
    };
    let Err(original) = first else {
        return SweepOutcome {
            result: first,
            effective_base_url: None,
        };
    };
    for cand in sweep_candidates(input) {
        let attempt = tokio::time::timeout(
            SWEEP_TIMEOUT,
            list_models(kind, Some(&cand), api_key, headers, proxy),
        )
        .await;
        match attempt {
            Ok(Ok(models)) => {
                // 胜出候选报 join_version 归一形态（采纳后端点归一幂等）
                return SweepOutcome {
                    result: Ok(models),
                    effective_base_url: Some(join_version(&cand)),
                };
            }
            // 超时（Ok(Err) 外层）或请求失败（Err(Err)）都试下一候选
            _ => continue,
        }
    }
    SweepOutcome {
        result: Err(original),
        effective_base_url: None,
    }
}

/// 按 kind 探测单档模型清单。`base_url` None = provider 缺省端点；
/// `api_key` 云端 kind 必需（缺 Key 直接报错，不给空结果装糊涂）。
/// 自定义头 / 网络代理随请求生效（批 C）。
pub async fn list_models(
    kind: &str,
    base_url: Option<&str>,
    api_key: Option<&str>,
    headers: &BTreeMap<String, String>,
    proxy: Option<&NetworkProxy>,
) -> Result<Vec<String>, String> {
    match kind {
        "ollama" => list_ollama(base_url, headers, proxy).await,
        "openai" => {
            let key = api_key.filter(|k| !k.is_empty()).ok_or_else(|| {
                "openai 兼容端点需要 API Key（设置里填写后可拉取模型清单）".to_string()
            })?;
            let base = base_url
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or(DEFAULT_OPENAI_BASE);
            list_openai_shape(base, key, headers, proxy, |req| req.bearer_auth(key)).await
        }
        "anthropic" => {
            let key = api_key.filter(|k| !k.is_empty()).ok_or_else(|| {
                "anthropic 需要 API Key（设置里填写后可拉取模型清单）".to_string()
            })?;
            let base = base_url
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or(DEFAULT_ANTHROPIC_BASE);
            list_openai_shape(base, key, headers, proxy, |req| {
                req.header("x-api-key", key)
                    .header("anthropic-version", "2023-06-01")
            })
            .await
        }
        other => Err(format!("不支持的 provider kind: {other}")),
    }
}

/// Ollama：`GET /api/tags` → `models[].name`。
async fn list_ollama(
    base_url: Option<&str>,
    headers: &BTreeMap<String, String>,
    proxy: Option<&NetworkProxy>,
) -> Result<Vec<String>, String> {
    let base = base_url
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_OLLAMA_BASE);
    let url = format!("{}/api/tags", base.trim_end_matches('/'));
    let resp = http_json(|req| req, &url, headers, proxy, PROBE_TIMEOUT)
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
    headers: &BTreeMap<String, String>,
    proxy: Option<&NetworkProxy>,
    auth: impl FnOnce(reqwest::RequestBuilder) -> reqwest::RequestBuilder,
) -> Result<Vec<String>, String> {
    let url = format!("{}/models", join_version(base));
    let resp = http_json(auth, &url, headers, proxy, PROBE_TIMEOUT)
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
/// body 里，只有状态码没法排查）。自定义头在鉴权之后应用（最后写入者
/// 覆盖，保留键保存侧已拒收）。
async fn http_json(
    auth: impl FnOnce(reqwest::RequestBuilder) -> reqwest::RequestBuilder,
    url: &str,
    headers: &BTreeMap<String, String>,
    proxy: Option<&NetworkProxy>,
    timeout: Duration,
) -> Result<Value, String> {
    let client = apply_proxy(reqwest::Client::builder().timeout(timeout), proxy)
        .build()
        .map_err(|e| format!("HTTP 客户端构建失败: {e}"))?;
    let mut req = auth(client.get(url));
    for (k, v) in headers {
        req = req.header(k.as_str(), v.as_str());
    }
    let resp = req.send().await.map_err(|e| e.to_string())?;
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
