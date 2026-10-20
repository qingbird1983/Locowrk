//! embedding 模型区（⑬ 智能层共享基建）：工具语义检索靠它把文本变向量。
//! 与主模型（llm 区）解耦——DeepSeek 等主
//! 模型没有 embedding 端点，embedding 换模型不影响聊天配置。
//!
//! 三项全空 = 智能层整体缺席（§1"可降级"：search_tools 退关键词）。base_url
//! 未填时按主 provider 兜底：ollama → 本机
//! 11434（OpenAI 兼容 /v1/embeddings），其余回落 llm.base_url。
//! API Key 只落盘不出 IPC（投影只有 `api_key_set` 布尔，同主 Key 纪律）。

use serde::{Deserialize, Serialize};

use crate::llm::models::DEFAULT_OLLAMA_BASE;

/// embedding 模型设置。三项全 None = 未配置（智能层缺席）。
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct EmbeddingSettings {
    /// OpenAI 兼容端点根（不带 /v1/embeddings 后缀）；None = 按 provider 兜底。
    pub base_url: Option<String>,
    /// embedding 模型名（如 nomic-embed-text / text-embedding-3-small）。
    pub model: Option<String>,
    /// 仅落盘，永不出 IPC。
    pub api_key: Option<String>,
}

impl EmbeddingSettings {
    /// 生效端点：本区 base_url 优先；未填时 ollama 回本机缺省，其余回落
    /// 主模型 base_url。返回 None = 端点无法确定（视为未配置）。
    pub fn resolved_base_url(&self, provider: &str, llm_base: Option<&str>) -> Option<String> {
        if let Some(b) = &self.base_url {
            return Some(b.clone());
        }
        match provider {
            "ollama" => Some(DEFAULT_OLLAMA_BASE.into()),
            _ => llm_base.map(str::to_string),
        }
    }

    /// 智能层是否就绪（端点可解析 + 模型已填）。组装点用
    /// `EmbedClient::from_settings` 直接裁决；本 helper 留给设置侧语义
    /// 判断（embedding/tests.rs 与未来的 UI 就绪提示）。
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn is_ready(&self, provider: &str, llm_base: Option<&str>) -> bool {
        self.model.as_deref().is_some_and(|m| !m.is_empty())
            && self
                .resolved_base_url(provider, llm_base)
                .is_some_and(|b| !b.is_empty())
    }

    /// 校验 + 应用（台账 T15 从 `settings/update.rs` **机械搬移**：两条变换 =
    /// `next.embedding.` → `self.`、缩进减一层，其余逐字节不动；签名吃消费型值，
    /// 理由与等价性证据同 `plugins.rs`）。本区无区间校验——"传空串 = 清除"
    /// 就是它的校验（`(!x.is_empty()).then_some(x)` 那三行原样搬来）。
    pub(crate) fn apply_update(&mut self, u: EmbeddingUpdate) -> Result<(), String> {
        if let Some(b) = u.base_url {
            self.base_url = (!b.is_empty()).then_some(b);
        }
        if let Some(m) = u.model {
            self.model = (!m.is_empty()).then_some(m);
        }
        if let Some(k) = u.api_key {
            self.api_key = (!k.is_empty()).then_some(k);
        }
        Ok(())
    }
}

/// embedding 区投影（无秘钥字段）。
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingView {
    /// 设置原值（None = 未填，前端展示兜底来源的提示）。
    pub base_url: Option<String>,
    pub model: Option<String>,
    pub api_key_set: bool,
}

/// embedding 区部分更新：字段 None = 保持不变；传空串 = 清除。
#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct EmbeddingUpdate {
    pub base_url: Option<String>,
    pub model: Option<String>,
    pub api_key: Option<String>,
}

#[cfg(test)]
mod tests;
