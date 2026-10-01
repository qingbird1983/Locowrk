//! 语义检索信号（exposure 文档 §4.2 层次 2）：对工具 description 做
//! embedding，请求前用最近用户消息算余弦相似度，作为 §5.3 打分的补充项
//! （`+0.5 × similarity`，最高 +0.5——补充信号，不主导选择）。
//!
//! embedding 模型未就绪 = 整层缺席（§1"可降级"）：组装点不装
//! [`EmbedPort`] 或请求失败都返回空表，选择退回"会话 1.0 + 全局 0.7"
//! 的既有行为，fail-open 不拦聊天。
//!
//! [`EmbedPort`] 与 [`GlobalEwma`] / [`MemorySource`](crate::middleware::MemorySource)
//! 同款"内核定契约、组装点实现"：embedding 要访问模型端点与密钥，内核
//! 只认批量接口。描述向量按 `name+description` 缓存——键含描述原文，
//! 插件升级换描述后自动失效，无需手动失效通道。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use plugin_api::ToolSpec;

/// 检索信号权重（§5.3：score += 0.5 × similarity）。
pub const RETRIEVAL_WEIGHT: f64 = 0.5;

/// embedding 契约：一次调用嵌入一批文本（目录描述 + 查询合一次请求，
/// 避免逐条 HTTP）。返回顺序与入参一致；失败返回 Err（调用方降级）。
#[async_trait]
pub trait EmbedPort: Send + Sync + 'static {
    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String>;
}

/// 余弦相似度。维度不齐 / 零向量 / NaN 一律 0——相似度只是补充信号，
/// 任何数值意外都不值得 panic。
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || a.len() != b.len() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    for (x, y) in a.iter().zip(b) {
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    let denom = na.sqrt() * nb.sqrt();
    if denom == 0.0 || !denom.is_finite() {
        0.0
    } else {
        (dot / denom).clamp(-1.0, 1.0)
    }
}

/// 缓存键：名字 + 描述（\x1f 分隔）。描述进键 = 内容寻址，改动即失效。
fn spec_key(spec: &ToolSpec) -> String {
    format!("{}\u{1f}{}", spec.name, spec.description)
}

/// 语义检索器：描述向量缓存 + 批量补算 + 余弦打分。
pub struct Retriever {
    embed: Arc<dyn EmbedPort>,
    cache: Mutex<HashMap<String, Vec<f32>>>,
}

impl Retriever {
    pub fn new(embed: Arc<dyn EmbedPort>) -> Self {
        Self {
            embed,
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// 对 `query` 给全目录打检索分（工具名 → [0,1] 相似度）。目录里缺
    /// 向量的描述与查询合并成一次批量请求补齐；任何失败返回空表。
    pub async fn scores(&self, query: &str, catalog: &[ToolSpec]) -> HashMap<String, f64> {
        if query.is_empty() || catalog.is_empty() {
            return HashMap::new();
        }
        let keys: Vec<String> = catalog.iter().map(spec_key).collect();
        // 先取快照、放锁再 await（缓存锁绝不跨 await 持有）
        let missing: Vec<String> = {
            let Ok(cache) = self.cache.lock() else {
                return HashMap::new();
            };
            keys.iter()
                .filter(|k| !cache.contains_key(*k))
                .cloned()
                .collect()
        };
        // texts[0] = 查询，其后按 missing 顺序补描述向量（一次 HTTP）
        let mut texts: Vec<String> = vec![query.to_string()];
        texts.extend(
            missing
                .iter()
                .map(|k| k.split('\u{1f}').nth(1).unwrap_or_default().to_string()),
        );
        let vecs = match self.embed.embed(&texts).await {
            Ok(v) if v.len() == texts.len() => v,
            Ok(_) | Err(_) => return HashMap::new(),
        };
        let query_vec = &vecs[0];
        if let Ok(mut cache) = self.cache.lock() {
            for (key, vec) in missing.into_iter().zip(vecs[1..].iter()) {
                cache.insert(key, vec.clone());
            }
        }
        let mut out = HashMap::new();
        let Ok(cache) = self.cache.lock() else {
            return out;
        };
        for (spec, key) in catalog.iter().zip(&keys) {
            if let Some(vec) = cache.get(key) {
                let sim = f64::from(cosine(query_vec, vec));
                if sim > 0.0 {
                    out.insert(spec.name.clone(), sim);
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests;
