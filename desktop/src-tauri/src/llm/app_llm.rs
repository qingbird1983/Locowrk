//! 设置驱动的模型服务分发（阶段八②）：model 已配置 → 自建 OpenAI 兼容
//! 客户端（LLM.md v3.0，含传输韧性重试）；未配置 → DemoLlm 脚本替身（无
//! 网络开箱即用）。
//!
//! 每次请求前重读设置：改 provider / model / key 立即生效，无需重启
//! （设置面板保存即可用，§13.1）。
//!
//! v2.58 会话级模型覆盖（§3.30）：可选注入 [`ModelOverrideStore`]——
//! `LlmRequest.session_id` 命中覆盖时按会话旁车解析：先按
//! `model_provider` 路由到档位（kind/base/key/窗口整档换），再以覆盖
//! 模型名替换 model；waterfall 契约的 `req.model`（§7.2 决策环改写）
//! 优先级最高。设置缺省完全不动。
//!
//! 重试面（LLM.md v3.0）：组装点注入停止标记与重试通知——重试等待期间
//! 轮询「停止生成」；已推过 delta 的重试先发 `agent://llm-retry` 让前端
//! 重置流式缓冲。未注入（测试/记录员）= 无钩子，重试静默进行。

use std::sync::Arc;

use async_trait::async_trait;
use kernel_agent::{LlmError, LlmRequest, LlmResponse, LlmService};
use serde_json::Value;

use super::client::{CompatLlm, RetryHooks};
use super::dialect;
use super::wire::WireCaps;
use crate::demo_llm::DemoLlm;
use crate::model_override::ModelOverrideStore;
use crate::settings::{LlmSettings, SettingsStore};

/// 重试通知出口（组装点闭包 emit `agent://llm-retry`；v2.66 同款组装闭包
/// 模式——AppHandle 不进 llm 模块）。
pub type RetryNotify = Arc<dyn Fn(&str, u32, u64) + Send + Sync>;

pub struct AppLlm {
    settings: Arc<SettingsStore>,
    /// 会话级模型覆盖（v2.58）；None = 不解析（测试路径）。
    overrides: Option<Arc<ModelOverrideStore>>,
    demo: DemoLlm,
    real: CompatLlm,
    /// 停止标记（v4.11）：重试等待期间轮询；None = 不轮询。
    stop: Option<Arc<crate::stop::StopFlags>>,
    /// 重试通知（流式重置）；None = 不通知。
    notify_retry: Option<RetryNotify>,
    /// 前缀断裂诊断（M6.1）：开关开时每 step 记分块 hash 链；命令层读。
    diagnoser: Arc<super::diagnose::Diagnoser>,
}

impl AppLlm {
    pub fn new(settings: Arc<SettingsStore>) -> Self {
        Self {
            settings,
            overrides: None,
            demo: DemoLlm::new(),
            real: CompatLlm::new(),
            stop: None,
            notify_retry: None,
            diagnoser: Arc::new(super::diagnose::Diagnoser::default()),
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
            real: CompatLlm::new(),
            stop: None,
            notify_retry: None,
            diagnoser: Arc::new(super::diagnose::Diagnoser::default()),
        }
    }

    /// 组装点注入外部诊断账本（M6.1：AppState 持同一线程安全句柄供
    /// 命令层读取——不经 dyn LlmService downcast）。
    pub fn with_diagnoser(mut self, diagnoser: Arc<super::diagnose::Diagnoser>) -> Self {
        self.diagnoser = diagnoser;
        self
    }

    /// 组装点注入重试面（停止标记 + 通知出口）。链在 with_overrides 后。
    pub fn with_retry_surface(
        mut self,
        stop: Arc<crate::stop::StopFlags>,
        notify: RetryNotify,
    ) -> Self {
        self.stop = Some(stop);
        self.notify_retry = Some(notify);
        self
    }

    /// M6.1 断裂诊断：开关开且带会话身份才记（一次性请求纪律 6 不写
    /// 缓存，记了也无意义）；失败静默不拦请求。
    fn diagnose(&self, req: &LlmRequest) {
        if !self.settings.developer().cache_diagnostics {
            return;
        }
        if let Some(r) = self.diagnoser.record(req) {
            if !r.healthy() {
                tracing::info!(
                    session = ?req.session_id, step = r.step,
                    breaks = ?r.breaks,
                    "前缀缓存断裂（M6.1 诊断）"
                );
            }
        }
    }

    /// 本请求的重试钩子：会话身份 + 停止标记 + 通知出口齐备才生效。
    fn hooks(&self, session_id: Option<&str>) -> Option<RetryHooks> {
        let sid = session_id?;
        let stop = Arc::clone(self.stop.as_ref()?);
        let notify = Arc::clone(self.notify_retry.as_ref()?);
        let sid_stop = sid.to_string();
        let sid_notify = sid.to_string();
        Some(RetryHooks {
            should_stop: Box::new(move || stop.is_stopped(&sid_stop)),
            on_retry: Box::new(move |attempt, delay| notify(&sid_notify, attempt, delay)),
        })
    }

    /// 本请求的生效配置：设置活动档 → 会话覆盖改写 → 决策环 model 改写。
    /// 档位路由经 `flat_profile`（keyring 双读 + headers + 全区代理随行）。
    pub(crate) fn effective_cfg(&self, req: &LlmRequest) -> LlmSettings {
        let mut cfg = self.settings.llm();
        if let (Some(store), Some(sid)) = (&self.overrides, &req.session_id) {
            if let Some((provider_id, model)) = store.get(sid) {
                if let Some(p) = self.settings.llm_config().profile(&provider_id) {
                    cfg = self.settings.llm_config().flat_profile(p);
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

    /// 批 B 请求计划：生效配置 + 模型能力上限（钉死 max_output / 生效窗口）
    /// 与推理档位落 params。会话旁车 thinking_level 经方言 clamp（就近降档
    /// 或剥除）；`params["aux"]=true`（辅助调用标记）则取方言最低档。内核
    /// 零改动——params 是 opaque Value（§7.2 决策环同通道）。
    fn prepare(&self, mut req: LlmRequest) -> (LlmRequest, LlmSettings, WireCaps) {
        let cfg = self.effective_cfg(&req);
        // 模型行能力（settings 钉死面）：按生效模型名在活动/覆盖档的
        // models 里找；窗口 = 模型行 context_window ?? 档位 window_tokens
        let mut caps = WireCaps::default();
        let llm_cfg = self.settings.llm_config();
        let profile = req
            .session_id
            .as_deref()
            .and_then(|sid| self.overrides.as_ref().and_then(|o| o.get(sid)))
            .and_then(|(pid, _)| llm_cfg.profile(&pid))
            .or_else(|| llm_cfg.active_profile());
        if let Some(p) = profile {
            let row = p.models.iter().find(|m| m.id == cfg.model);
            // settings 只存用户钉死值（save 时打 source=user），有值即钉死
            caps.max_output = row.and_then(|m| m.max_output);
            caps.ctx_window = row.and_then(|m| m.context_window).or(p.window_tokens);
        }
        // 推理档位解析：辅助降档优先（aux 标记），否则会话旁车档
        let dialect = dialect::dialect_of(cfg.base_url.as_deref());
        let aux = req
            .params
            .get("aux")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        // 辅助调用不发送 max_tokens（§5.4 宁缺勿假；ZCode 发 min(5000,max)
        // 的做法记档不跟）
        if aux {
            caps.max_output = None;
        }
        let level = if aux {
            dialect.aux_level().map(str::to_string)
        } else {
            req.session_id
                .as_deref()
                .and_then(|sid| self.overrides.as_ref().and_then(|o| o.get_thinking(sid)))
                .and_then(|lvl| dialect.clamp(&lvl).map(str::to_string))
        };
        if let Some(lvl) = level {
            req.params["thinking_level"] = Value::String(lvl);
        }
        (req, cfg, caps)
    }
}

#[async_trait]
impl LlmService for AppLlm {
    async fn complete(&self, req: LlmRequest) -> Result<LlmResponse, LlmError> {
        self.diagnose(&req);
        let (req, cfg, caps) = self.prepare(req);
        if cfg.model.trim().is_empty() {
            tracing::debug!("LLM model 未配置，走演示替身（设置 → 模型配置后走真实模型）");
            return self.demo.complete(req).await;
        }
        let _ = super::wire::check_modalities(&req);
        self.real.complete_with(&cfg, caps, req).await
    }

    /// ⑳ 流式分发：同 complete 的设置驱动——未配置走演示替身（缺省实现
    /// = 整段一次回调，流式形状一致），真实模型走兼容客户端流式（含重试）。
    async fn complete_streaming(
        &self,
        req: LlmRequest,
        on_delta: &(dyn for<'a> Fn(&'a str) -> bool + Send + Sync),
    ) -> Result<LlmResponse, LlmError> {
        self.diagnose(&req);
        let (req, cfg, caps) = self.prepare(req);
        if cfg.model.trim().is_empty() {
            tracing::debug!("LLM model 未配置，走演示替身（设置 → 模型配置后走真实模型）");
            return self.demo.complete_streaming(req, on_delta).await;
        }
        let hooks = self.hooks(req.session_id.as_deref());
        self.real
            .complete_streaming_with(&cfg, caps, req, on_delta, &|_| false, hooks.as_ref())
            .await
    }

    /// v2.57 推理通道分发：演示替身走缺省回落（无推理），真实模型把
    /// reasoning_content delta 逐段交给 `on_reasoning`。
    async fn complete_streaming_reasoning(
        &self,
        req: LlmRequest,
        on_delta: &(dyn for<'a> Fn(&'a str) -> bool + Send + Sync),
        on_reasoning: &(dyn for<'a> Fn(&'a str) -> bool + Send + Sync),
    ) -> Result<LlmResponse, LlmError> {
        self.diagnose(&req);
        let (req, cfg, caps) = self.prepare(req);
        if cfg.model.trim().is_empty() {
            tracing::debug!("LLM model 未配置，走演示替身（设置 → 模型配置后走真实模型）");
            return self.demo.complete_streaming(req, on_delta).await;
        }
        let hooks = self.hooks(req.session_id.as_deref());
        self.real
            .complete_streaming_with(&cfg, caps, req, on_delta, on_reasoning, hooks.as_ref())
            .await
    }
}
