//! 设置驱动的模型服务分发（阶段八②）：model 已配置 → genai 三合一
//! 适配器；未配置 → DemoLlm 脚本替身（无网络开箱即用）。
//!
//! 每次请求前重读设置：改 provider / model / key 立即生效，无需重启
//! （设置面板保存即可用，§13.1）。

use std::sync::Arc;

use async_trait::async_trait;
use kernel_agent::{LlmError, LlmRequest, LlmResponse, LlmService};

use super::genai_backend::GenAiLlm;
use crate::demo_llm::DemoLlm;
use crate::settings::SettingsStore;

pub struct AppLlm {
    settings: Arc<SettingsStore>,
    demo: DemoLlm,
    real: GenAiLlm,
}

impl AppLlm {
    pub fn new(settings: Arc<SettingsStore>) -> Self {
        Self {
            settings,
            demo: DemoLlm::new(),
            real: GenAiLlm::new(),
        }
    }
}

#[async_trait]
impl LlmService for AppLlm {
    async fn complete(&self, req: LlmRequest) -> Result<LlmResponse, LlmError> {
        let cfg = self.settings.llm();
        if cfg.model.trim().is_empty() {
            tracing::debug!("LLM model 未配置，走演示替身（设置 → LLM 配置后走真实模型）");
            return self.demo.complete(req).await;
        }
        self.real.complete_with(&cfg, req).await
    }

    /// ⑳ 流式分发：同 complete 的设置驱动——未配置走演示替身（缺省实现
    /// = 整段一次回调，流式形状一致），真实模型走 genai 流式。
    async fn complete_streaming(
        &self,
        req: LlmRequest,
        on_delta: &(dyn for<'a> Fn(&'a str) + Send + Sync),
    ) -> Result<LlmResponse, LlmError> {
        let cfg = self.settings.llm();
        if cfg.model.trim().is_empty() {
            tracing::debug!("LLM model 未配置，走演示替身（设置 → LLM 配置后走真实模型）");
            return self.demo.complete_streaming(req, on_delta).await;
        }
        self.real.complete_streaming_with(&cfg, req, on_delta).await
    }
}
