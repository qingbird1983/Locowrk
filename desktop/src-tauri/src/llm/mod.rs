//! 真实模型接入（阶段八②，LLM.md v3.0）：自建 OpenAI 兼容统一客户端
//! （一条线格式 + 传输韧性重试）+ 设置面板驱动的服务分发。模型适配器
//! 不特殊，也只是 LlmService 服务（§7.2）。
//! ⑬ 起并列 embedding 客户端（`embed`，OpenAI 兼容 /v1/embeddings）——
//! 工具语义检索 / 经历记忆 / 隐式提取的共享基建。

mod app_llm;
pub mod catalog;
mod catalog_match;
mod client;
pub mod diagnose;
mod dialect;
mod embed;
pub mod models;
pub mod probe;
mod retry;
mod sse;
#[cfg(test)]
mod tests;
mod wire;

pub use app_llm::{AppLlm, RetryNotify};
pub use client::CompatLlm;
pub use dialect::dialect_of;
pub use embed::{CooledEmbed, EmbedClient, EmbeddingSettingsLlmView, SharedEmbed};
