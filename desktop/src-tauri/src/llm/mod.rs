//! 真实模型接入（阶段八②，LLM.md v3.0）：自建 OpenAI 兼容统一客户端
//! （一条线格式 + 传输韧性重试）+ 设置面板驱动的服务分发。模型适配器
//! 不特殊，也只是 LlmService 服务（§7.2）。
//! ⑬ 起并列 embedding 客户端（`embed`，OpenAI 兼容 /v1/embeddings）——
//! S4 起只剩工具语义检索一个消费者（记忆面经历/提取已退役）。
//! S2 刀④a 起还含**辅助补全的直呼出口**（`aux_client`：一次 system+user → 一段文本）。
//! S4 刀③a 起它只服务上下文压缩（摘要走主模型）；`SummaryClient` 契约从
//! `memory-tree` 迁入，树整面退役后压缩线不再反向依赖要删的 crate。

mod app_llm;
mod aux_client;
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
/// 摘要调用的用量读数：生产侧解构后丢弃，只有测试面组装假客户端时读它。
#[cfg(test)]
pub(crate) use aux_client::SummaryUsage;
pub(crate) use aux_client::{AppLlmSummarizer, SummaryClient, SummaryRequest};
pub use client::{test_model, CompatLlm};
pub use dialect::dialect_of;
pub use embed::{CooledEmbed, EmbedClient, EmbeddingSettingsLlmView, SharedEmbed};
pub(crate) use retry::{is_budget_exhausted, is_overflow_message};
pub use subagent_llm::SubagentLlm;
