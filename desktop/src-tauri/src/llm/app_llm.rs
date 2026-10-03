//! 设置驱动的模型服务分发（阶段八②）：model 已配置 → genai 三合一
//! 适配器；未配置 → DemoLlm 脚本替身（无网络开箱即用）。
//!
//! 每次请求前重读设置：改 provider / model / key 立即生效，无需重启
//! （设置面板保存即可用，§13.1）。
//!
//! v2.58 会话级模型覆盖（§3.30）：可选注入 [`ModelOverrideStore`]——
//! `LlmRequest.session_id` 命中覆盖时按会话旁车解析：先按
//! `model_provider` 路由到档位（kind/base/key/窗口整档换），再以覆盖
//! 模型名替换 model；waterfall 契约的 `req.model`（§7.2 决策环改写）
//! 优先级最高。设置缺省完全不动。

use std::sync::Arc;

use async_trait::async_trait;
use kernel_agent::{LlmError, LlmRequest, LlmResponse, LlmService};

use super::genai_backend::GenAiLlm;
use crate::demo_llm::DemoLlm;
use crate::model_override::ModelOverrideStore;
use crate::settings::{LlmSettings, SettingsStore};

pub struct AppLlm {
    settings: Arc<SettingsStore>,
    /// 会话级模型覆盖（v2.58）；None = 不解析（测试路径）。
    overrides: Option<Arc<ModelOverrideStore>>,
    demo: DemoLlm,
    real: GenAiLlm,
}

impl AppLlm {
    pub fn new(settings: Arc<SettingsStore>) -> Self {
        Self {
            settings,
            overrides: None,
            demo: DemoLlm::new(),
            real: GenAiLlm::new(),
        }
    }

    /// 组装点注入会话覆盖存储（init_state）。
    pub fn with_overrides(
        settings: Arc<SettingsStore>,
        overrides: Arc<ModelOverrideStore>,
    ) -> Self {
        Self {
            settings,
            overrides: Some(overrides),
            demo: DemoLlm::new(),
            real: GenAiLlm::new(),
        }
    }

    /// 本请求的生效配置：设置活动档 → 会话覆盖改写 → 决策环 model 改写。
    pub(crate) fn effective_cfg(&self, req: &LlmRequest) -> LlmSettings {
        let mut cfg = self.settings.llm();
        if let (Some(store), Some(sid)) = (&self.overrides, &req.session_id) {
            if let Some((provider_id, model)) = store.get(sid) {
                if let Some(p) = self.settings.llm_config().profile(&provider_id) {
                    cfg = LlmSettings {
                        provider: p.kind.clone(),
                        model: p.model.clone(),
                        base_url: p.base_url.clone(),
                        api_key: p.api_key.clone(),
                        window_tokens: p.window_tokens,
                    };
                }
                cfg.model = model;
            }
        }
        // §7.2 决策环的 model 改写是最高优先级（会话覆盖只在缺省层生效）
        if let Some(m) = &req.model {
            cfg.model = m.clone();
        }
        cfg
    }
}

#[async_trait]
impl LlmService for AppLlm {
    async fn complete(&self, req: LlmRequest) -> Result<LlmResponse, LlmError> {
        let cfg = self.effective_cfg(&req);
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
        let cfg = self.effective_cfg(&req);
        if cfg.model.trim().is_empty() {
            tracing::debug!("LLM model 未配置，走演示替身（设置 → LLM 配置后走真实模型）");
            return self.demo.complete_streaming(req, on_delta).await;
        }
        self.real
            .complete_streaming_with(&cfg, req, on_delta, &|_| {})
            .await
    }

    /// v2.57 推理通道分发：演示替身走缺省回落（无推理），真实模型把
    /// genai 的 ReasoningChunk 逐段交给 `on_reasoning`。
    async fn complete_streaming_reasoning(
        &self,
        req: LlmRequest,
        on_delta: &(dyn for<'a> Fn(&'a str) + Send + Sync),
        on_reasoning: &(dyn for<'a> Fn(&'a str) + Send + Sync),
    ) -> Result<LlmResponse, LlmError> {
        let cfg = self.effective_cfg(&req);
        if cfg.model.trim().is_empty() {
            tracing::debug!("LLM model 未配置，走演示替身（设置 → LLM 配置后走真实模型）");
            return self.demo.complete_streaming(req, on_delta).await;
        }
        self.real
            .complete_streaming_with(&cfg, req, on_delta, on_reasoning)
            .await
    }
}
