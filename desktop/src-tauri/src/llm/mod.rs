//! 真实模型接入（阶段八②，LLM.md v3.0）：自建 OpenAI 兼容统一客户端
//! （一条线格式 + 传输韧性重试）+ 设置面板驱动的服务分发。模型适配器
//! 不特殊，也只是 LlmService 服务（§7.2）。
//! ⑬ 起并列 embedding 客户端（`embed`，OpenAI 兼容 /v1/embeddings）——
//! 工具语义检索 / 经历记忆 / 隐式提取的共享基建。
//! S2 刀④a 起还含**辅助补全的直呼出口**（`aux_client`：一次 system+user → 一段文本，
//! 按设置选档）——整理章节总结与上下文压缩的摘要调用共用这一条；它原来长在
//! `organize/client.rs`，搬来的理由写在那个文件的头注与 organize 头注里。

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
pub(crate) use aux_client::AppLlmMemoryManager;
pub use client::{test_model, CompatLlm};
pub use dialect::dialect_of;
pub use embed::{CooledEmbed, EmbedClient, EmbeddingSettingsLlmView, SharedEmbed};
pub(crate) use retry::is_overflow_message;
pub use subagent_llm::SubagentLlm;
