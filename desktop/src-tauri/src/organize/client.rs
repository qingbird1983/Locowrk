//! 记录员客户端的桌面实现（organize 编排的 LLM 出口）：按设置路由——
//! 记录员模型配了服务档（recorder_provider）→ 整档切换（kind/base/key，
//! v2.58 档位语义）；只配了模型名 → 主模型端点 + 模型名覆盖；都没配 →
//! 主模型。**模型未配置 = 直接 Err**（记录员不落演示替身——脚本回显只会
//! 产出垃圾再走校验失败，如实报错让管道降级规则切分）。

use std::sync::Arc;

use async_trait::async_trait;
use kernel_agent::LlmRequest;
use session_store::Message;

use crate::llm::GenAiLlm;
use crate::settings::{LlmSettings, SettingsStore};

pub struct AppLlmRecorder {
    settings: Arc<SettingsStore>,
    real: GenAiLlm,
}

impl AppLlmRecorder {
    pub fn new(settings: Arc<SettingsStore>) -> Self {
        Self {
            settings,
            real: GenAiLlm::new(),
        }
    }

    /// 本调用的生效配置（设置现读——保存即生效）。
    fn cfg_for(&self, model: Option<&str>) -> LlmSettings {
        let mem = self.settings.memory();
        let recorder_model = model.filter(|m| !m.trim().is_empty());
        // 记录员配了服务档 → 整档换（与 v2.58 会话覆盖同一档位语义）
        if let (Some(m), Some(pid)) = (&recorder_model, mem.recorder_provider.as_deref()) {
            if !pid.trim().is_empty() {
                if let Some(p) = self.settings.llm_config().profile(pid) {
                    return LlmSettings {
                        provider: p.kind.clone(),
                        model: m.to_string(),
                        base_url: p.base_url.clone(),
                        api_key: p.api_key.clone(),
                        window_tokens: p.window_tokens,
                    };
                }
            }
        }
        let mut cfg = self.settings.llm();
        if let Some(m) = recorder_model {
            cfg.model = m.to_string();
        }
        cfg
    }
}

#[async_trait]
impl memory_tree::RecorderClient for AppLlmRecorder {
    async fn complete(
        &self,
        system: &str,
        user: &str,
        model: Option<&str>,
    ) -> Result<(String, memory_tree::RecorderUsage), String> {
        let cfg = self.cfg_for(model);
        if cfg.model.trim().is_empty() {
            return Err("记录员模型未配置".into());
        }
        let req = LlmRequest {
            messages: vec![Message::User {
                content: user.to_string(),
            }],
            tools: vec![],
            model: None,
            params: serde_json::Value::Object(serde_json::Map::new()),
            system: Some(system.to_string()),
            // 记录员调用不属于任何会话轮：会话级模型覆盖 / 调度绑定都
            // 不该波及它（ctx-compact 摘要调用同款约定）
            session_id: None,
        };
        let resp = self
            .real
            .complete_with(&cfg, req)
            .await
            .map_err(|e| e.to_string())?;
        let usage = resp
            .usage
            .map(|u| memory_tree::RecorderUsage {
                input: u.input,
                output: u.output,
            })
            .unwrap_or_default();
        Ok((resp.content, usage))
    }
}
