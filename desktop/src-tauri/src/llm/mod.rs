//! 真实模型接入（阶段八②，LLM.md 方案一）：genai 三合一适配器
//! （openai / anthropic / ollama）+ 设置面板驱动的服务分发。
//! 模型适配器不特殊，也只是 LlmService 服务（§7.2）。

mod app_llm;
mod genai_backend;
pub mod probe;
#[cfg(test)]
mod tests;

pub use app_llm::AppLlm;
