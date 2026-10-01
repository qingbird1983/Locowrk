//! embedding 客户端（⑬ 智能层共享基建）：OpenAI 兼容 `/v1/embeddings`
//! 端点（Ollama / OpenAI / 多数聚合网关同形），批量嵌入。实现内核的
//! [`EmbedPort`] 契约——组装点构建 [`SharedEmbed`] 供 ToolExposure 检索
//! 信号 / search_tools / 经历记忆三处共用；配置保存即热换入（ RwLock）。
//!
//! 故障冷却：embedding 是补充信号，端点宕机时不能每个请求都白等超时
//! ——失败后冷却 [`COOLDOWN`]，期间 `embed` 直接返回 Err（调用方按
//! "未就绪"降级），冷却结束自动恢复尝试。

use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use kernel_agent::EmbedPort;
use serde_json::{json, Value};

use crate::settings::EmbeddingSettings;

/// 单次批量嵌入超时（目录 ~25 条描述 + 查询一次打包）。
const EMBED_TIMEOUT: Duration = Duration::from_secs(15);
/// 失败冷却：期间跳过尝试（不阻塞聊天主链路）。
pub const COOLDOWN: Duration = Duration::from_secs(60);

/// 可用的 embedding 端点描述（settings 三项 + provider 兜底解析后）。
#[derive(Clone, Debug)]
pub struct EmbedClient {
    pub base_url: String,
    pub model: String,
    pub api_key: Option<String>,
}

impl EmbedClient {
    /// 从设置解析（`llm` 区提供 provider / 主端点做兜底）。未配置模型或
    /// 端点解析不出 = None（智能层缺席，调用方走降级）。
    pub fn from_settings(
        embed: &EmbeddingSettings,
        llm: &EmbeddingSettingsLlmView,
    ) -> Option<Self> {
        let model = embed.model.as_deref()?.trim();
        if model.is_empty() {
            return None;
        }
        let base_url = embed.resolved_base_url(&llm.provider, llm.base_url.as_deref())?;
        if base_url.trim().is_empty() {
            return None;
        }
        Some(Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            model: model.to_string(),
            api_key: embed.api_key.clone().filter(|k| !k.is_empty()),
        })
    }

    /// 批量嵌入：返回顺序与入参一致。任何失败 = Err（调用方降级）。
    pub async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let url = format!("{}/v1/embeddings", self.base_url);
        let client = reqwest::Client::builder()
            .timeout(EMBED_TIMEOUT)
            .build()
            .map_err(|e| format!("HTTP 客户端构建失败: {e}"))?;
        let mut req = client.post(&url).json(&json!({
            "model": self.model,
            "input": texts,
        }));
        if let Some(key) = &self.api_key {
            req = req.bearer_auth(key);
        }
        let resp: Value = req
            .send()
            .await
            .and_then(|r| r.error_for_status())
            .map_err(|e| format!("embedding 请求失败: {e}"))?
            .json()
            .await
            .map_err(|e| format!("embedding 响应解析失败: {e}"))?;
        parse_embeddings(&resp, texts.len())
    }
}

/// 主模型区的只读视图（from_settings 只需要 provider 与 base_url）。
#[derive(Clone, Debug)]
pub struct EmbeddingSettingsLlmView {
    pub provider: String,
    pub base_url: Option<String>,
}

/// `/v1/embeddings` 响应：`data: [{index, embedding: [f32…]}…]`。
/// 按 index 归位（OpenAI 规范保证顺序，仍防御性排序）；条数不符 = Err。
fn parse_embeddings(resp: &Value, expect: usize) -> Result<Vec<Vec<f32>>, String> {
    let data = resp
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| "embedding 响应缺少 data 数组".to_string())?;
    if data.len() != expect {
        return Err(format!(
            "embedding 条数不符：请求 {expect}，响应 {}",
            data.len()
        ));
    }
    let mut out: Vec<(usize, Vec<f32>)> = Vec::with_capacity(expect);
    for item in data {
        let index = item
            .get("index")
            .and_then(Value::as_u64)
            .ok_or_else(|| "embedding 条目缺少 index".to_string())? as usize;
        let vec = item
            .get("embedding")
            .and_then(Value::as_array)
            .ok_or_else(|| "embedding 条目缺少 embedding 数组".to_string())?
            .iter()
            .map(|v| {
                v.as_f64()
                    .map(|f| f as f32)
                    .ok_or_else(|| "embedding 含非数字".to_string())
            })
            .collect::<Result<Vec<f32>, String>>()?;
        out.push((index, vec));
    }
    out.sort_by_key(|(i, _)| *i);
    Ok(out.into_iter().map(|(_, v)| v).collect())
}

/// 共享柄：`None` = 未配置（智能层缺席）。settings_set 成功后命令层换入。
#[derive(Clone, Default)]
pub struct SharedEmbed(Arc<RwLock<Option<Arc<EmbedClient>>>>);

impl SharedEmbed {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&self, client: Option<EmbedClient>) {
        *self.0.write().expect("embed cfg poisoned") = client.map(Arc::new);
    }

    /// 带冷却的 [`EmbedPort`] 实现柄（ToolExposure / search_tools 共用）。
    pub fn port(&self) -> Arc<CooledEmbed> {
        Arc::new(CooledEmbed {
            shared: self.clone(),
            cooldown_until: Mutex::new(None),
        })
    }
}

/// [`EmbedPort`] 实现：端点缺席 / 冷却中 → Err（调用方按未就绪降级）。
pub struct CooledEmbed {
    shared: SharedEmbed,
    cooldown_until: Mutex<Option<Instant>>,
}

#[async_trait]
impl EmbedPort for CooledEmbed {
    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
        let client = {
            let guard = self.shared.0.read().expect("embed cfg poisoned");
            guard.clone()
        };
        let Some(client) = client else {
            return Err("embedding 未配置".into());
        };
        // 冷却期直接拒绝（读锁内判，不阻塞）
        if let Some(until) = *self.cooldown_until.lock().expect("cooldown poisoned") {
            if Instant::now() < until {
                return Err("embedding 冷却中".into());
            }
        }
        match client.embed(texts).await {
            Ok(v) => Ok(v),
            Err(e) => {
                *self.cooldown_until.lock().expect("cooldown poisoned") =
                    Some(Instant::now() + COOLDOWN);
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests;
