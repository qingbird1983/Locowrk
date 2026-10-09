//! 子代理模型面（批 F，LLM.md「生态」节）：每个子 run 一个实例——
//! 「钉档打头 + 可用标记池」的终端错误回退链。
//!
//! 触发口径（PI 案落档）：凡从 [`AppLlm`] 上抛的 `Err` 都是 Fatal（v2.75
//! 韧性纪律：瞬时错误在客户端内不设上限重试，用户停止走 `Ok` 空收束不上
//! 抛）——`Err` = 推进链上下一个未试且窗口装得下已携上下文的候选，同请求
//! 重试；链耗尽返回最后错误。死供应商（持续瞬时故障）不触发回退——与主
//! 会话同款「重试不设次数上限」语义，超时/停止是退路。
//!
//! 窗口装不下已携上下文的候选跳过（PI 案）：`estimate_tokens(req.messages)`
//! 对候选窗口（模型行 context_window ?? 目录 ?? 档位 window_tokens），已知
//! 且估用 ≥ 窗口 → 记因跳过；未知照试（装不下自然溢出推进，宁缺勿假）。
//!
//! 无钉档且池空 = 纯透传（路由参数都不写，逐字节同 ⑮ 以来行为）。子内流
//! 式 sink 已被 `NoLlmStreamSink` 压制（⑳），回退重试无重复 delta 面。

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use kernel_agent::{
    estimate_tokens, ChildModelReport, LlmError, LlmRequest, LlmResponse, LlmService,
};
use serde_json::{json, Value};

use super::app_llm::AppLlm;
use super::catalog::CatalogStore;
use crate::settings::SettingsStore;

/// 链上一个候选：「provider 档位 id + 模型名」。identity 字符串用于
/// attempted 去重与报告记因。
struct Candidate {
    provider_id: String,
    model: String,
    thinking: Option<String>,
}

impl Candidate {
    fn identity(&self) -> String {
        format!("{}/{}", self.provider_id, self.model)
    }

    fn route_value(&self) -> Value {
        let mut v = json!({
            "provider": self.provider_id,
            "model": self.model,
        });
        if let Some(t) = &self.thinking {
            v["thinking"] = json!(t);
        }
        v
    }
}

pub struct SubagentLlm {
    inner: Arc<AppLlm>,
    settings: Arc<SettingsStore>,
    catalog: Arc<CatalogStore>,
    report: Arc<ChildModelReport>,
    /// 链：钉档（若在）打头 + 标记池（构造时快照，run 内身份稳定）。
    chain: Vec<Candidate>,
    /// None = 首请求透传（跟随会话；链头有钉档时构造即置 Some(0)）。
    state: Mutex<RunState>,
}

struct RunState {
    route: Option<usize>,
    attempted: HashSet<String>,
}

impl SubagentLlm {
    /// 组装点工厂入口：构造时从 settings 快照链（读锁一次，run 内身份
    /// 稳定——中途改设置对进行中的子 run 不变形）。
    pub fn new(
        inner: Arc<AppLlm>,
        settings: Arc<SettingsStore>,
        catalog: Arc<CatalogStore>,
        report: Arc<ChildModelReport>,
    ) -> Self {
        let cfg = settings.llm_config();
        let mut chain: Vec<Candidate> = Vec::new();
        // 钉档打头：档位在且启用且模型名非空才生效（缺失静默回落透传，
        // tracing 留痕——保存侧已校验，运行期再变形属环境漂移）。
        if let Some(pin) = &cfg.subagent {
            let usable = cfg
                .profile(&pin.provider_id)
                .is_some_and(|p| p.enabled && !pin.model.trim().is_empty());
            if usable {
                chain.push(Candidate {
                    provider_id: pin.provider_id.clone(),
                    model: pin.model.clone(),
                    thinking: pin.thinking_level.clone(),
                });
            } else {
                tracing::warn!(
                    provider = %pin.provider_id,
                    "子代理钉档档位缺席/停用，本次子 run 回落跟随会话"
                );
            }
        }
        // 标记池（排除与钉档相同的 provider/model）：enabled + has_key 档位
        // × available_for_subagents == true 模型，档位表顺序。无 Key 的档位
        // 请求必败 = 不进链（宁缺勿假）。
        for p in &cfg.providers {
            if !p.enabled || !p.has_key() {
                continue;
            }
            for m in &p.models {
                if m.available_for_subagents != Some(true) {
                    continue;
                }
                let identity = format!("{}/{}", p.id, m.id);
                if chain.iter().any(|c| c.identity() == identity) {
                    continue;
                }
                chain.push(Candidate {
                    provider_id: p.id.clone(),
                    model: m.id.clone(),
                    thinking: None,
                });
            }
        }
        let route = (!chain.is_empty()).then_some(0usize);
        Self {
            inner,
            settings,
            catalog,
            report,
            chain,
            state: Mutex::new(RunState {
                route,
                attempted: HashSet::new(),
            }),
        }
    }

    /// 候选窗口（token）：模型行 context_window ?? 目录命中 ?? 档位
    /// window_tokens；全未知 = None（照试）。
    fn candidate_window(&self, c: &Candidate) -> Option<u64> {
        let cfg = self.settings.llm_config();
        let p = cfg.profile(&c.provider_id)?;
        if let Some(w) = p
            .models
            .iter()
            .find(|m| m.id == c.model)
            .and_then(|m| m.context_window)
        {
            return Some(w);
        }
        if let Some(hit) = self
            .catalog
            .lookup_model(&p.kind, p.base_url.as_deref(), &c.model)
        {
            if let Some(w) = hit.context_window {
                return Some(w);
            }
        }
        p.window_tokens
    }

    /// 终端错误后的链推进：记因上一个候选，取下一个未试且（窗口未知或
    /// 估用 < 窗口）的候选。返回是否推进成功。
    fn advance(&self, failed_req: &LlmRequest, last_error: &str) -> bool {
        let est = estimate_tokens(&failed_req.messages);
        let mut state = self.state.lock().expect("subagent llm state poisoned");
        if let Some(idx) = state.route {
            let identity = self.chain[idx].identity();
            self.report.record_failure(&identity, last_error);
            state.attempted.insert(identity);
        }
        let mut next = state.route.map(|i| i + 1).unwrap_or(0);
        while next < self.chain.len() {
            let c = &self.chain[next];
            let identity = c.identity();
            if state.attempted.contains(&identity) {
                next += 1;
                continue;
            }
            if let Some(window) = self.candidate_window(c) {
                if est >= window {
                    let reason =
                        format!("窗口装不下已携上下文（估算 ≈{est} token ≥ 窗口 {window}），跳过");
                    self.report.record_failure(&identity, &reason);
                    state.attempted.insert(identity);
                    next += 1;
                    continue;
                }
            }
            state.route = Some(next);
            self.report.record_used(&identity);
            return true;
        }
        state.route = None;
        false
    }

    /// 三入口共用的回退循环：`call` 捕获流式闭包，按当前路由注入
    /// `params["route"]` 后调 inner（AppLlm）。Err = 推进重试同请求。
    async fn with_fallback<F, Fut>(
        &self,
        mut req: LlmRequest,
        call: F,
    ) -> Result<LlmResponse, LlmError>
    where
        F: Fn(LlmRequest) -> Fut,
        Fut: std::future::Future<Output = Result<LlmResponse, LlmError>>,
    {
        // 首请求即带钉档路由（透传路径零注入）
        if let Some(idx) = self
            .state
            .lock()
            .expect("subagent llm state poisoned")
            .route
        {
            let c = &self.chain[idx];
            req.params["route"] = c.route_value();
            self.report.record_used(&c.identity());
        }
        loop {
            match call(req.clone()).await {
                Ok(resp) => return Ok(resp),
                Err(e) => {
                    if !self.advance(&req, &e.to_string()) {
                        return Err(e);
                    }
                    let idx = self
                        .state
                        .lock()
                        .expect("subagent llm state poisoned")
                        .route
                        .expect("advance 成功必有路由");
                    req.params["route"] = self.chain[idx].route_value();
                }
            }
        }
    }
}

#[async_trait]
impl LlmService for SubagentLlm {
    async fn complete(&self, req: LlmRequest) -> Result<LlmResponse, LlmError> {
        self.with_fallback(req, |r| self.inner.complete(r)).await
    }

    async fn complete_streaming(
        &self,
        req: LlmRequest,
        on_delta: &(dyn for<'a> Fn(&'a str) -> bool + Send + Sync),
    ) -> Result<LlmResponse, LlmError> {
        let inner = Arc::clone(&self.inner);
        self.with_fallback(req, move |r| {
            let inner = Arc::clone(&inner);
            async move { inner.complete_streaming(r, on_delta).await }
        })
        .await
    }

    async fn complete_streaming_reasoning(
        &self,
        req: LlmRequest,
        on_delta: &(dyn for<'a> Fn(&'a str) -> bool + Send + Sync),
        on_reasoning: &(dyn for<'a> Fn(&'a str) -> bool + Send + Sync),
    ) -> Result<LlmResponse, LlmError> {
        let inner = Arc::clone(&self.inner);
        self.with_fallback(req, move |r| {
            let inner = Arc::clone(&inner);
            async move {
                inner
                    .complete_streaming_reasoning(r, on_delta, on_reasoning)
                    .await
            }
        })
        .await
    }
}
