//! 摘要补全的直呼出口（一次 system+user → 一段文本）：默认走**主模型**端点
//! 与主模型档位；模型未配置 = 直接 Err（不落演示替身——脚本回显只会产出垃圾
//! 再走校验失败，如实报错让调用方降级）。
//!
//! 为什么这块地住在模型接入面而不是 `organize/`（S2 刀④a 搬家）：organize 整面
//! 在 S4 退役，而"让一次模型补全从设置里找到该用的那一档"是**模型接入面**的
//! 能力。S4 刀③b 起它的唯一消费者是主层压缩（`compact/deep.rs`）——记忆管理器
//! 的模型档位随记忆面退役，压缩只走主模型。
//!
//! `SummaryClient` / `SummaryUsage` 两个类型在 S4 刀③a 从 `memory-tree` 迁来：
//! 树整面退役，压缩线不能再依赖一个要删的 crate。

use std::sync::Arc;

use async_trait::async_trait;
use kernel_agent::LlmRequest;
use session_store::Message;

use crate::llm::CompatLlm;
use crate::settings::{LlmSettings, SettingsStore};

/// 摘要补全的最小契约：system = 角色提示词，user = 待压缩转录；
/// `model` = 只覆盖模型名（None = 主模型）。走主端点与主档位——
/// 记忆管理器的服务档位不参与（S4：压缩不再有第二个模型出处）。
/// 与宿主的模型路由解耦（测试用脚本化假实现，不碰网络）。
#[async_trait]
pub(crate) trait SummaryClient: Send + Sync {
    async fn complete(
        &self,
        system: &str,
        user: &str,
        model: Option<&str>,
    ) -> Result<(String, SummaryUsage), String>;
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
}

impl AppLlmSummarizer {
    pub(crate) fn new(settings: Arc<SettingsStore>) -> Self {
        Self {
            settings,
            real: CompatLlm::new(),
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
        system: &str,
        user: &str,
    ) -> Result<(String, SummaryUsage), String> {
        if cfg.model.trim().is_empty() {
            return Err("摘要模型未配置".into());
        }
        let req = LlmRequest {
            messages: vec![Message::user(user.to_string())],
            tools: vec![],
            model: None,
            // 辅助调用标记（批 B §5.4）：推理档降到方言最低档、不发 max_tokens
            params: serde_json::json!({ "aux": true }),
            system: Some(system.to_string()),
            // 摘要调用不属于任何会话轮：会话级模型覆盖 / 调度绑定都
            // 不该波及它（ctx-compact 摘要调用同款约定）
            session_id: None,
        };
        // 直呼 CompatLlm（绕过 AppLlm.prepare）：辅助降档在此就地解析
        //（方言最低档；无方言 = 不发档位字段）
        let mut req = req;
        if let Some(lvl) = crate::llm::dialect_of(cfg.base_url.as_deref()).aux_level() {
            req.params["thinking_level"] = serde_json::Value::String(lvl.into());
        }
        let resp = self
            .real
            .complete_with(&cfg, Default::default(), req, None)
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

#[async_trait]
impl SummaryClient for AppLlmSummarizer {
    async fn complete(
        &self,
        system: &str,
        user: &str,
        model: Option<&str>,
    ) -> Result<(String, SummaryUsage), String> {
        self.complete_inner(self.cfg_main(model), system, user)
            .await
    }
}
