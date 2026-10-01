//! 真实模型接入（阶段八②，LLM.md 方案一）：genai 三合一适配器
//! （openai / anthropic / ollama）+ 设置面板驱动的服务分发。
//! 模型适配器不特殊，也只是 LlmService 服务（§7.2）。
//! ⑬ 起并列 embedding 客户端（`embed`，OpenAI 兼容 /v1/embeddings）——
//! 工具语义检索 / 经历记忆 / 隐式提取的共享基建。

mod app_llm;
mod embed;
mod genai_backend;
pub mod probe;
#[cfg(test)]
mod tests;

pub use app_llm::AppLlm;
pub use embed::{CooledEmbed, EmbedClient, EmbeddingSettingsLlmView, SharedEmbed};
