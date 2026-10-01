//! 决策环 + LLM 请求管线（§7.2，2026-09-30 自 kernel.rs 按职责拆出——
//! 单文件 400 行上限）：pre-step / request / request-error 三个 waterfall
//! 决策点、工具目录快照、带重试预算的请求闭环。方法都在 `impl Kernel`
//! 里，本文件不定义新类型；子模块可访问父模块私有字段。

use std::time::Duration;

use cordis_core::WaterfallDecision;
use plugin_api::ToolSpec;
use serde_json::Value;
use session_store::Message;

use super::{
    CompactionOutcome, Kernel, KernelError, LlmRequest, LlmResponse, LlmService, ToolSource,
};
use crate::waterfall::{PreStepPayload, RequestErrorPayload, RequestPayload, points};

impl Kernel {
    /// `agent/pre-step` 决策环：放行 → 取改写后的消息；拒绝/重试 → 报错
    /// （重试只对请求失败点有意义，§7.2）。
    pub(super) async fn waterfall_pre(
        &self,
        messages: Vec<Message>,
    ) -> Result<Vec<Message>, KernelError> {
        let payload = serde_json::to_value(PreStepPayload { messages })?;
        let out = self.ctx.events.waterfall(points::PRE_STEP, payload).await;
        match out.decision {
            WaterfallDecision::Continue | WaterfallDecision::Replace(_) => {
                let parsed: PreStepPayload = serde_json::from_value(out.payload)?;
                Ok(parsed.messages)
            }
            WaterfallDecision::Reject(reason) => Err(KernelError::Rejected {
                point: points::PRE_STEP,
                reason: reason.reason,
            }),
            WaterfallDecision::Retry { .. } => Err(KernelError::Rejected {
                point: points::PRE_STEP,
                reason: "retry is meaningless before the first request".into(),
            }),
        }
    }

    /// `agent/request` 决策环：插件可改 model / params / system（记忆注入见
    /// middleware::memory_inject）。tools 随 payload 进决策环（决策方理论上
    /// 可裁剪目录），出环后进 LlmRequest；ctx-compact 的压缩结果经
    /// `compacted` 一并带回（kernel 落日志）。
    async fn waterfall_request(
        &self,
        messages: Vec<Message>,
        tools: Vec<ToolSpec>,
    ) -> Result<(LlmRequest, Option<CompactionOutcome>), KernelError> {
        let payload = serde_json::to_value(RequestPayload {
            messages,
            model: None,
            // 空对象而非 null：改写环可以直接 entry/or_insert（§7.2）
            params: Value::Object(serde_json::Map::new()),
            tools,
            compacted: None,
            system: None,
        })?;
        let out = self.ctx.events.waterfall(points::REQUEST, payload).await;
        match out.decision {
            WaterfallDecision::Continue | WaterfallDecision::Replace(_) => {
                let parsed: RequestPayload = serde_json::from_value(out.payload)?;
                Ok((
                    LlmRequest {
                        messages: parsed.messages,
                        tools: parsed.tools,
                        model: parsed.model,
                        params: parsed.params,
                        system: parsed.system,
                    },
                    parsed.compacted,
                ))
            }
            WaterfallDecision::Reject(reason) => Err(KernelError::Rejected {
                point: points::REQUEST,
                reason: reason.reason,
            }),
            WaterfallDecision::Retry { .. } => Err(KernelError::Rejected {
                point: points::REQUEST,
                reason: "retry without a failure is meaningless".into(),
            }),
        }
    }

    /// 工具目录快照：目录服务缺失 = 空表（纯聊天无需 ToolSource，§7.2）。
    pub(super) async fn tool_catalog(&self) -> Vec<ToolSpec> {
        match self.scoped_dyn::<dyn ToolSource>() {
            Ok(source) => source.list().await,
            // 目录没装配就不给模型工具；真调到不存在的工具仍会走
            // resolve 的 NotAvailable 失败路径，不会静默
            Err(_) => Vec::new(),
        }
    }

    /// LLM 请求 + request-error 决策环。`agent/request-error` 返回
    /// `Retry { delay }` 时在 step 内部重试（重新走 request 决策环），
    /// 累计次数 / 累计延迟超预算即失败（§7.2 重试预算）。压缩结果取
    /// 最后一次成功的决策环产出（重试轮会在原始消息上重压，幂等守卫
    /// 见 compact.rs；落日志只记一次）。
    pub(super) async fn request_with_retry(
        &self,
        messages: Vec<Message>,
        tools: Vec<ToolSpec>,
    ) -> Result<(LlmResponse, Option<CompactionOutcome>), KernelError> {
        let mut retries: u32 = 0;
        let mut total_delay = Duration::ZERO;
        loop {
            let (req, compacted) = self
                .waterfall_request(messages.clone(), tools.clone())
                .await?;
            let llm = self.scoped_dyn::<dyn LlmService>()?;
            match llm.complete(req).await {
                Ok(resp) => return Ok((resp, compacted)),
                Err(e) => {
                    let payload = serde_json::to_value(RequestErrorPayload {
                        error: e.to_string(),
                    })?;
                    let out = self
                        .ctx
                        .events
                        .waterfall(points::REQUEST_ERROR, payload)
                        .await;
                    match out.decision {
                        WaterfallDecision::Retry { delay } => {
                            retries += 1;
                            total_delay += delay;
                            if retries > self.retry_budget.max_retries
                                || total_delay > self.retry_budget.max_total_delay
                            {
                                return Err(KernelError::RetryBudgetExhausted {
                                    retries,
                                    total_delay_ms: total_delay.as_millis() as u64,
                                    source: e,
                                });
                            }
                            tokio::time::sleep(delay).await;
                        }
                        // Continue / Replace / Reject 一律按原错误失败（§7.2）
                        _ => return Err(e.into()),
                    }
                }
            }
        }
    }
}
