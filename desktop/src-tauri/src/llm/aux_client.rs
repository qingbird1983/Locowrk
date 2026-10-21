//! 摘要补全的直呼出口（一次 system+user → 一段文本）：默认走**主模型**端点
//! 与主模型档位；模型未配置 = 直接 Err（不落演示替身——脚本回显只会产出垃圾
//! 再走校验失败，如实报错让调用方降级）。
//!
//! 为什么这块地住在模型接入面而不是 `organize/`（S2 刀④a 搬家）：organize 整面
//! 在 S4 退役（刀③c 已删），而"让一次模型补全从设置里找到该用的那一档"是**模型
//! 接入面**的能力。S4 刀③b 起它的唯一消费者是主层压缩（`compact/deep.rs`）——
//! 记忆管理器的模型档位随记忆面退役，压缩只走主模型。
//!
//! `SummaryClient` / `SummaryUsage` 两个类型在 S4 刀③a 从 `memory-tree` 迁来：
//! 树整面退役，压缩线不能再依赖一个要删的 crate。
//!
//! **specs/12 刀①**：这条通路曾经是全项目唯一一处「重试不设上限 + 停不下来」的
//! 组合——它继承了主对话轮的传输韧性，却没继承停止能力，摘要请求一失败就永远
//! 在那儿转（界面表现 = 卡在「思考中」，且按「停止生成」也没用）。
//! 现在它多了两样：**停止标记**（用户能停）与**总时长预算**（超了就是一次失败，
//! 推进熔断计数）。

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use kernel_agent::LlmRequest;
use session_store::Message;

use crate::llm::CompatLlm;
use crate::settings::{LlmSettings, SettingsStore};

use super::client::RetryHooks;

/// 一次摘要调用的入参（specs/12 刀①）。
///
/// 为什么收成结构体而不是继续加位置参数：入参已经四个，其中 `model` 与
/// `session_id` **都是 `Option<&str>`**——位置参数传错一个编译器不报，只会在
/// 运行期把"归属会话"当成"覆盖模型名"。
pub(crate) struct SummaryRequest<'a> {
    /// 角色提示词。
    pub system: &'a str,
    /// 待压缩转录。
    pub user: &'a str,
    /// 只覆盖模型名（None = 主模型）。走主端点与主档位——记忆管理器的服务
    /// 档位不参与（S4：压缩不再有第二个模型出处）。
    pub model: Option<&'a str>,
    /// 归属会话（specs/12 刀①）：**只为停止标记与日志归属**。
    ///
    /// 它**不改** [`LlmRequest::session_id`]——摘要不吃会话级模型覆盖是既有
    /// 口径（`complete_inner` 里的注释），动了就是回归。两条归属各走各的：
    /// 这一枚喂停止标记，那一枚留空。
    pub session_id: Option<&'a str>,
    /// 总时长预算（None = 不设限）。超预算 = 这一次调用失败（Err），
    /// 调用方据此记一次失败、推进熔断计数——见
    /// [`specs/12-aux-call-budget`](../../../../specs/12-aux-call-budget/spec.md)。
    pub budget: Option<Duration>,
}

/// 摘要补全的最小契约：system = 角色提示词，user = 待压缩转录；
/// `model` = 只覆盖模型名（None = 主模型）。走主端点与主档位——
/// 记忆管理器的服务档位不参与（S4：压缩不再有第二个模型出处）。
/// 与宿主的模型路由解耦（测试用脚本化假实现，不碰网络）。
#[async_trait]
pub(crate) trait SummaryClient: Send + Sync {
    async fn complete(&self, req: &SummaryRequest<'_>) -> Result<(String, SummaryUsage), String>;
}

/// 一次摘要调用的 token 读数（供应商没报 = 0，如实透出）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct SummaryUsage {
    pub input: u64,
    pub output: u64,
}

pub(crate) struct AppLlmSummarizer {
    settings: Arc<SettingsStore>,
    real: CompatLlm,
    /// 停止标记（specs/12 刀①）：摘要调用在**重试等待**与**吐字**两处都查它
    /// ⇒ 用户点「停止生成」能停掉一条用户看不见的辅助请求。
    /// None = 不可中止（headless / 测试装配）。
    stop: Option<Arc<crate::stop::StopFlags>>,
}

impl AppLlmSummarizer {
    pub(crate) fn new(settings: Arc<SettingsStore>, stop: Arc<crate::stop::StopFlags>) -> Self {
        Self {
            settings,
            real: CompatLlm::new(),
            stop: Some(stop),
        }
    }

    /// 压缩摘要的生效配置：**主模型端点 + 主档位**，`model` 只覆盖模型名。
    pub(super) fn cfg_main(&self, model: Option<&str>) -> LlmSettings {
        self.settings.with_llm_config(|llm| {
            let mut cfg = llm.flat();
            if let Some(m) = model.filter(|m| !m.trim().is_empty()) {
                cfg.model = m.to_string();
            }
            cfg
        })
    }

    async fn complete_inner(
        &self,
        cfg: LlmSettings,
        req: &SummaryRequest<'_>,
    ) -> Result<(String, SummaryUsage), String> {
        if cfg.model.trim().is_empty() {
            return Err("摘要模型未配置".into());
        }
        // 先取两枚只读入参——下面 `req` 这个名字会被 `LlmRequest` 遮蔽。
        let session_id = req.session_id;
        let budget = req.budget;
        let mut req = LlmRequest {
            messages: vec![Message::user(req.user.to_string())],
            tools: vec![],
            model: None,
            // 辅助调用标记（批 B §5.4）：推理档降到方言最低档、不发 max_tokens
            params: serde_json::json!({ "aux": true }),
            system: Some(req.system.to_string()),
            // 摘要调用不属于任何会话轮：会话级模型覆盖 / 调度绑定都
            // 不该波及它（ctx-compact 摘要调用同款约定）
            // ⚠ specs/12 刀①：归属会话走 `SummaryRequest.session_id` 那一枚
            // （只喂停止标记），这里**仍留 None**——动了就等于让摘要吃会话覆盖。
            session_id: None,
        };
        // 直呼 CompatLlm（绕过 AppLlm.prepare）：辅助降档在此就地解析
        // （方言最低档；无方言 = 不发档位字段）
        if let Some(lvl) = crate::llm::dialect_of(cfg.base_url.as_deref()).aux_level() {
            req.params["thinking_level"] = serde_json::Value::String(lvl.into());
        }
        // 停止探针（specs/12 刀①）：重试等待、吐字回调两处共用同一枚。
        let probe = stop_probe(self.stop.as_ref(), session_id);
        let on_stop = {
            let p = Arc::clone(&probe);
            move |_: &str| p()
        };
        let hooks = RetryHooks {
            should_stop: Box::new(move || probe()),
            // 摘要重试**不推** `agent://llm-retry`：那个事件会让前端
            // `resetAssistant` 清空主对话的 assistant 气泡 ⇒ 制造新回归。
            on_retry: Box::new(|_attempt, _delay_ms| {}),
        };
        let resp = self
            .real
            .complete_streaming_with(
                &cfg,
                Default::default(),
                req,
                None,
                &on_stop,
                &on_stop,
                Some(&hooks),
                budget,
            )
            .await
            .map_err(|e| e.to_string())?;
        let usage = resp
            .usage
            .map(|u| SummaryUsage {
                input: u.input,
                output: u.output,
            })
            .unwrap_or_default();
        Ok((resp.content, usage))
    }
}

/// 停止探针：按会话查停止标记。三处共用同一枚（重试等待 / 吐字回调 / 钩子）。
fn stop_probe(
    stop: Option<&Arc<crate::stop::StopFlags>>,
    session_id: Option<&str>,
) -> Arc<dyn Fn() -> bool + Send + Sync> {
    let session_id = session_id.map(str::to_string);
    let stop = stop.cloned();
    Arc::new(move || match (&session_id, &stop) {
        (Some(sid), Some(stop)) => stop.is_stopped(sid),
        _ => false,
    })
}

#[async_trait]
impl SummaryClient for AppLlmSummarizer {
    async fn complete(&self, req: &SummaryRequest<'_>) -> Result<(String, SummaryUsage), String> {
        self.complete_inner(self.cfg_main(req.model), req).await
    }
}
