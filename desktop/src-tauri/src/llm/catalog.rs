//! 模型能力目录（批 A，供应商稿 §4）：models.dev 快照随包 + 惰性加载
//! （不进启动路径）+ 在线刷新（只进内存永不落盘）+ 投影裁剪（加载时
//! 一次，只留 §4.2 字段）。合并优先级「用户钉死 > 在线 > 快照」中的钉
//! 死在 IPC 合成层（settings/llm.rs `synthesize_view`），本模块只管
//! 快照/刷新两源。AppHandle 不进 llm 模块（v2.66 同款）——资源路径由
//! 组装点解析好传进来。

use std::collections::HashMap;
use std::sync::RwLock;
use std::time::Duration;

use serde::Serialize;
use serde_json::Value;

use super::catalog_match;

/// 在线刷新（供应商稿 §4.1：手动按钮触发）。超时 30s——设计稿写 10s，
/// 实机走查抓到 5.3MB 载荷在本机代理链路（clash TUN）实测 10.2s 撞死
/// 10s 上限：快照是全量下载不是小 API ping，按载荷量级放宽。
const REFRESH_URL: &str = "https://models.dev/api.json";
const REFRESH_TIMEOUT: Duration = Duration::from_secs(30);

/// 单条目录记录（加载时一次裁剪，供应商稿 §4.2）。`pricing` 为 USD 原值
/// 每百万 token；缺分量 = None 不冒充 0。
#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CatalogModel {
    pub provider_key: String,
    pub model_id: String,
    pub display_name: Option<String>,
    pub context_window: Option<u64>,
    pub max_output: Option<u64>,
    pub input_modalities: Vec<String>,
    /// 归一化语义档位（disabled/minimal/low/…；toggle 归一为
    /// disabled+enabled，"none" 归一为 disabled——供应商稿 §5.1）。
    pub reasoning_options: Vec<String>,
    pub tool_call: Option<bool>,
    pub pricing: Option<CatalogPricing>,
    /// models.dev `family`（批 F 推荐三选的族分散键；缺失 = None）。
    pub family: Option<String>,
}

/// 每百万 token 单价（models.dev `cost` 原样，USD）。
#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CatalogPricing {
    pub input: Option<f64>,
    pub output: Option<f64>,
    pub cache_read: Option<f64>,
}

/// 目录数据来源。`bundled` = 随包快照；`refreshed` = 本次会话内在线刷新
/// （只进内存，重启回落快照）。
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CatalogSource {
    Bundled,
    Refreshed,
}

/// 已加载目录快照（不可变；刷新整体换新）。
pub struct CatalogSnapshot {
    pub source: CatalogSource,
    pub loaded_ms: u64,
    /// (provider_key, model_id) → 记录。
    pub models: HashMap<(String, String), CatalogModel>,
    /// provider_key → 官方 api 端点原文（裁剪投影不保留 provider 级字段，
    /// host 匹配需要它；快照里多数供应商无此字段）。
    pub api_index: HashMap<String, String>,
}

impl CatalogSnapshot {
    pub fn status(&self) -> CatalogStatus {
        let providers = self
            .models
            .keys()
            .map(|(p, _)| p.clone())
            .collect::<std::collections::HashSet<_>>();
        CatalogStatus {
            source: self.source,
            updated_ms: self.loaded_ms,
            providers: providers.len(),
            models: self.models.len(),
        }
    }
}

/// 目录状态（`llm_catalog_status` 投影）。
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CatalogStatus {
    pub source: CatalogSource,
    pub updated_ms: u64,
    pub providers: usize,
    pub models: usize,
}

/// 共享目录柄：惰性加载 bundled，刷新换内值。锁只护换入换出，解析在
/// 锁外完成（5MB JSON 不持锁）。
pub struct CatalogStore {
    bundled: std::path::PathBuf,
    inner: RwLock<Option<std::sync::Arc<CatalogSnapshot>>>,
}

impl CatalogStore {
    pub fn new(bundled: std::path::PathBuf) -> Self {
        Self {
            bundled,
            inner: RwLock::new(None),
        }
    }

    /// 当前快照（首访惰性加载 bundled；快照缺失/损坏 = 空目录如实上报
    /// + warn，不 panic——能力未知比崩溃好）。
    pub fn snapshot(&self) -> std::sync::Arc<CatalogSnapshot> {
        if let Some(s) = self.inner.read().expect("catalog poisoned").clone() {
            return s;
        }
        let loaded = load_file(&self.bundled, CatalogSource::Bundled);
        let arc = std::sync::Arc::new(loaded);
        let mut guard = self.inner.write().expect("catalog poisoned");
        // 并发首访：后到者让先到者（避免覆盖一次刷新）
        if guard.is_none() {
            *guard = Some(arc.clone());
        }
        guard.clone().unwrap_or(arc)
    }

    /// 在线刷新：拉 models.dev → 解析 → 换内值（只进内存）。失败返回
    /// Err，旧快照原样保留。
    pub async fn refresh(&self) -> Result<CatalogStatus, String> {
        let client = reqwest::Client::builder()
            .timeout(REFRESH_TIMEOUT)
            .build()
            .map_err(|e| format!("HTTP 客户端构建失败: {e}"))?;
        let resp = client
            .get(REFRESH_URL)
            .send()
            .await
            .map_err(|e| format!("models.dev 请求失败: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("models.dev 返回 HTTP {}", resp.status()));
        }
        let body = resp
            .text()
            .await
            .map_err(|e| format!("models.dev 响应读取失败: {e}"))?;
        let snapshot = parse_str(&body, CatalogSource::Refreshed)
            .map_err(|e| format!("models.dev 响应解析失败: {e}"))?;
        let status = snapshot.status();
        *self.inner.write().expect("catalog poisoned") = Some(std::sync::Arc::new(snapshot));
        tracing::info!(models = status.models, "模型目录已在线刷新（只进内存）");
        Ok(status)
    }

    /// 单模型目录查询（供应商稿 §4.5 `llm_lookup_model`）：供应商定位
    /// （预设别名/host 打分）→ 归一化 id 精确命中；无命中 = None（能力
    /// 未知如实，不猜）。
    pub fn lookup_model(
        &self,
        kind: &str,
        base_url: Option<&str>,
        model: &str,
    ) -> Option<CatalogModel> {
        catalog_match::lookup(&self.snapshot(), kind, base_url, model).cloned()
    }
}

/// 从文件加载（缺文件/坏 JSON = 空快照 + warn）。
fn load_file(path: &std::path::Path, source: CatalogSource) -> CatalogSnapshot {
    match std::fs::read_to_string(path)
        .map_err(|e| e.to_string())
        .and_then(|s| parse_str(&s, source))
    {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "模型目录快照加载失败（能力数据缺席）");
            CatalogSnapshot {
                source,
                loaded_ms: now_ms(),
                models: HashMap::new(),
                api_index: HashMap::new(),
            }
        }
    }
}

/// 快照文本 → 裁剪投影。models.dev 形状：`{provider: {models: {id: {...}}}}`。
pub fn parse_str(text: &str, source: CatalogSource) -> Result<CatalogSnapshot, String> {
    let raw: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let providers = raw.as_object().ok_or_else(|| "顶层不是对象".to_string())?;
    let mut models = HashMap::new();
    let mut api_index = HashMap::new();
    for (pkey, pv) in providers {
        if let Some(api) = pv.get("api").and_then(Value::as_str) {
            api_index.insert(pkey.clone(), api.to_string());
        }
        let Some(pmodels) = pv.get("models").and_then(Value::as_object) else {
            continue;
        };
        for (mid, mv) in pmodels {
            models.insert((pkey.clone(), mid.clone()), parse_model(pkey, mid, mv));
        }
    }
    Ok(CatalogSnapshot {
        source,
        loaded_ms: now_ms(),
        models,
        api_index,
    })
}

/// 单模型裁剪（未知字段直接丢弃；缺字段 = None/空，宁缺勿假）。
fn parse_model(provider_key: &str, model_id: &str, mv: &Value) -> CatalogModel {
    CatalogModel {
        provider_key: provider_key.to_string(),
        model_id: model_id.to_string(),
        display_name: mv.get("name").and_then(Value::as_str).map(str::to_string),
        context_window: mv
            .get("limit")
            .and_then(|l| l.get("context"))
            .and_then(Value::as_u64),
        max_output: mv
            .get("limit")
            .and_then(|l| l.get("output"))
            .and_then(Value::as_u64),
        input_modalities: mv
            .get("modalities")
            .and_then(|m| m.get("input"))
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default(),
        reasoning_options: normalize_reasoning(mv.get("reasoning_options")),
        tool_call: mv.get("tool_call").and_then(Value::as_bool),
        pricing: mv
            .get("cost")
            .filter(|c| c.is_object())
            .map(|c| CatalogPricing {
                input: c.get("input").and_then(Value::as_f64),
                output: c.get("output").and_then(Value::as_f64),
                cache_read: c.get("cache_read").and_then(Value::as_f64),
            }),
        family: mv
            .get("family")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string),
    }
}

/// 目录 `reasoning_options` → 语义梯子（供应商稿 §5.1）：`{type:"effort"}`
/// 值直通（none 归一为 disabled）；`{type:"toggle"}` 在**没有** effort 档
/// 时贡献 disabled + enabled（GLM 形态），与 effort 并存时只贡献 disabled
/// ——effort 已是开启态的细分，enabled 档冗余（deepseek 走查口径：梯子 =
/// disabled/low/high/max）。输出按语义序去重，未知值保序追加。
fn normalize_reasoning(options: Option<&Value>) -> Vec<String> {
    const ORDER: [&str; 8] = [
        "disabled", "minimal", "low", "medium", "high", "xhigh", "max", "enabled",
    ];
    let arr = options.and_then(Value::as_array);
    let mut effort: Vec<String> = Vec::new();
    let mut has_toggle = false;
    if let Some(arr) = arr {
        for opt in arr {
            match opt.get("type").and_then(Value::as_str) {
                Some("toggle") => has_toggle = true,
                Some("effort") => {
                    if let Some(values) = opt.get("values").and_then(Value::as_array) {
                        for v in values {
                            if let Some(s) = v.as_str() {
                                let s = s.trim().to_lowercase();
                                if !s.is_empty() && !effort.contains(&s) {
                                    effort.push(s);
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    let had_effort = !effort.is_empty();
    let mut out: Vec<String> = Vec::new();
    if has_toggle {
        out.push("disabled".into());
    }
    for v in effort {
        let normalized = if v == "none" {
            "disabled".to_string()
        } else {
            v
        };
        if normalized == "disabled" {
            if !out.contains(&normalized) {
                out.insert(0, normalized);
            }
            continue;
        }
        if !out.contains(&normalized) {
            out.push(normalized);
        }
    }
    // toggle 独占（无 effort 档）：补 enabled 档（GLM 形态）；effort 里
    // 只有 none 也算有 effort（disabled 已表达关闭态）
    if has_toggle && !had_effort && out.len() == 1 {
        out.push("enabled".into());
    }
    out.sort_by_key(|v| ORDER.iter().position(|o| o == v).unwrap_or(usize::MAX));
    out
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests;
