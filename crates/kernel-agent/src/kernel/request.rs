//! 决策环 + LLM 请求管线（§7.2，2026-09-30 自 kernel.rs 按职责拆出——
//! 单文件 400 行上限）：pre-step / request / request-error 三个 waterfall
//! 决策点、工具目录快照、带重试预算的请求闭环。方法都在 `impl Kernel`
//! 里，本文件不定义新类型；子模块可访问父模块私有字段。

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use cordis_core::WaterfallDecision;
use plugin_api::ToolSpec;
use serde_json::Value;
use session_store::Message;

use super::{
    CompactionOutcome, Kernel, KernelError, LlmRequest, LlmResponse, LlmService, ToolSource,
};
use crate::waterfall::{PreStepPayload, RequestErrorPayload, RequestPayload, points};

/// 一次 LLM 请求的流式痕迹（⑳ + v2.57/58）：文本/推理 delta 是否已被 sink
/// 真实消费（投影方据此抑制整段投影）、推理全文（内核闭包累积——与
/// 供应商 reasoning 流同源，被消费与否都收集，sink 故障时投影方
/// 仍能整段补投）、本次请求耗时（v2.58，`LlmUsage` 事件的速度口径）。
pub(super) struct StreamTrace {
    pub streamed: bool,
    pub reasoned: bool,
    pub reasoning: String,
    pub duration_ms: u64,
}

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

    /// `agent/request` 决策环：插件可改 model / params / system（注入层见
    /// middleware 的各个 waterfall 监听器）。tools 随 payload 进决策环（决策方
    /// 理论上可裁剪目录），出环后进 LlmRequest；压缩结果经 `compacted` 一并带回
    /// （kernel 落日志）——该字段是 waterfall 决策方的可扩展契约，现网无生产
    /// 写入点（ctx-compact 已随批 C 删除），但历史 `ContextCompacted` 事件的
    /// 重放面仍在读它的形状，不能拔。
    async fn waterfall_request(
        &self,
        messages: Vec<Message>,
        tools: Vec<ToolSpec>,
        session_id: Option<&str>,
    ) -> Result<(LlmRequest, Option<CompactionOutcome>), KernelError> {
        let payload = serde_json::to_value(RequestPayload {
            messages,
            model: None,
            // 空对象而非 null：改写环可以直接 entry/or_insert（§7.2）
            params: Value::Object(serde_json::Map::new()),
            tools,
            compacted: None,
            system: None,
            session_id: session_id.map(str::to_string),
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
                        session_id: parsed.session_id,
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
    /// ㉘·B：带会话身份（目录可按会话过滤，计划模式 = 非 Pure 不出目录）。
    pub(super) async fn tool_catalog(&self, session_id: Option<&str>) -> Vec<ToolSpec> {
        match self.scoped_dyn::<dyn ToolSource>() {
            Ok(source) => source.list(session_id).await,
            // 目录没装配就不给模型工具；真调到不存在的工具仍会走
            // resolve 的 NotAvailable 失败路径，不会静默
            Err(_) => Vec::new(),
        }
    }

    /// 目录快照的对外只读口（v2.58）：组装点（桌面端）做「工具上下文」
    /// 估算用——与请求实际携带的目录同源同过滤（含会话围栏），不是第二
    /// 真相源。
    pub async fn catalog_snapshot(&self, session_id: Option<&str>) -> Vec<ToolSpec> {
        self.tool_catalog(session_id).await
    }

    /// LLM 请求 + request-error 决策环。`agent/request-error` 返回
    /// `Retry { delay }` 时在 step 内部重试（重新走 request 决策环），
    /// 累计次数 / 累计延迟超预算即失败（§7.2 重试预算）。压缩结果取
    /// 最后一次成功的决策环产出（重试轮会在原始消息上重压，幂等守卫
    /// 见 compact.rs；落日志只记一次）。⑳：`LlmStreamSink` 在册走流式
    /// （delta 逐段回调）；v2.57 起走 `complete_streaming_reasoning`，
    /// 推理 delta 同路逐段回调并全文累积。已知边界：Retry 重试轮的
    /// delta 不回滚——重试仅在 request-error 决策环配置时发生，届时
    /// 上层应考虑清空流式缓冲（桌面端的传输重试在适配器内自带重置，
    /// LLM.md v3.0；本决策环留给插件）。
    pub(super) async fn request_with_retry(
        &self,
        messages: Vec<Message>,
        tools: Vec<ToolSpec>,
        session_id: Option<&str>,
    ) -> Result<(LlmResponse, Option<CompactionOutcome>, StreamTrace), KernelError> {
        let mut retries: u32 = 0;
        let mut total_delay = Duration::ZERO;
        // ⑳ delta 通道：sink 在册才走 complete_streaming；查询失败 = 无
        // sink，走 complete（delta 不发给任何人的流式没有意义）。
        let sink = self.scoped_dyn::<dyn crate::stream::LlmStreamSink>().ok();
        loop {
            let (req, compacted) = self
                .waterfall_request(messages.clone(), tools.clone(), session_id)
                .await?;
            let llm = self.scoped_dyn::<dyn LlmService>()?;
            let streamed = AtomicBool::new(false);
            let reasoned = AtomicBool::new(false);
            let reasoning = std::sync::Mutex::new(String::new());
            let started = std::time::Instant::now();
            let resp = match &sink {
                Some(sink) => {
                    let sink = Arc::clone(sink);
                    // 回调返回值 = sink.should_stop()（v4.11 停止生成）：适配器
                    // 流循环收到 true 即尽快收束本次调用（部分正文原样返回）。
                    let on_delta = |delta: &str| -> bool {
                        if sink.on_delta(session_id, delta) {
                            streamed.store(true, Ordering::Relaxed);
                        }
                        sink.should_stop(session_id)
                    };
                    let on_reasoning = |delta: &str| -> bool {
                        // 全文照收（guard 只在闭包内持有，不跨 await）：
                        // 消费失败时投影方还有整段可投
                        reasoning.lock().expect("reasoning buf").push_str(delta);
                        if sink.on_reasoning(session_id, delta) {
                            reasoned.store(true, Ordering::Relaxed);
                        }
                        sink.should_stop(session_id)
                    };
                    llm.complete_streaming_reasoning(req, &on_delta, &on_reasoning)
                        .await
                }
                None => llm.complete(req).await,
            };
            match resp {
                Ok(resp) => {
                    return Ok((
                        resp,
                        compacted,
                        StreamTrace {
                            streamed: streamed.load(Ordering::Relaxed),
                            reasoned: reasoned.load(Ordering::Relaxed),
                            reasoning: reasoning.into_inner().expect("reasoning buf"),
                            duration_ms: started.elapsed().as_millis() as u64,
                        },
                    ));
                }
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
