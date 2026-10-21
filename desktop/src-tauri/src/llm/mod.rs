//! 真实模型接入（阶段八②，LLM.md v3.0）：自建 OpenAI 兼容统一客户端
//! （一条线格式 + 传输韧性重试）+ 设置面板驱动的服务分发。模型适配器
//! 不特殊，也只是 LlmService 服务（§7.2）。
//! ⑬ 起并列 embedding 客户端（`embed`，OpenAI 兼容 /v1/embeddings）——
//! S4 起只剩工具语义检索一个消费者（记忆面经历/提取已退役）。
//! **specs/13**：摘要的独立直呼通路（`aux_client`）整条删除——上下文压缩改走
//! 主对话那条管线（`AppLlm::complete_compaction`），提示词与请求形态都照
//! OpenCode V2 复刻。

mod app_llm;
pub mod catalog;
mod catalog_match;
mod client;
pub mod diagnose;
mod dialect;
mod embed;
pub(crate) mod import;
pub(crate) mod keyring;
pub mod models;
pub mod probe;
pub mod recommend;
mod retry;
mod sse;
mod subagent_llm;
#[cfg(test)]
mod tests;
mod wire;

pub use app_llm::{AppLlm, RetryNotify};
pub use client::test_model;
pub use dialect::dialect_of;
pub use embed::{CooledEmbed, EmbedClient, EmbeddingSettingsLlmView, SharedEmbed};
pub(crate) use retry::{is_budget_exhausted, is_overflow_message};
pub use subagent_llm::SubagentLlm;
