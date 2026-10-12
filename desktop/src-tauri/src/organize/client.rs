//! 记忆管理器客户端的桌面实现（organize 编排的 LLM 出口）：按设置路由——
//! 记忆管理器模型配了服务档（memory_manager_provider）→ 整档切换（kind/base/key，
//! v2.58 档位语义）；只配了模型名 → 主模型端点 + 模型名覆盖；都没配 →
//! 主模型。**模型未配置 = 直接 Err**（记忆管理器不落演示替身——脚本回显只会
//! 产出垃圾再走校验失败，如实报错让管道降级规则切分）。

use std::sync::Arc;

use async_trait::async_trait;
use kernel_agent::LlmRequest;
use session_store::Message;

use crate::llm::CompatLlm;
use crate::settings::{LlmSettings, SettingsStore};

pub struct AppLlmMemoryManager {
    settings: Arc<SettingsStore>,
    real: CompatLlm,
}

impl AppLlmMemoryManager {
    pub fn new(settings: Arc<SettingsStore>) -> Self {
        Self {
            settings,
            real: CompatLlm::new(),
        }
    }

    /// 本调用的生效配置（设置现读——保存即生效）。
    fn cfg_for(&self, model: Option<&str>) -> LlmSettings {
        // `mem` 必须在闭包外取：它是同一个 Store 的另一档区读面，闭包内再取 = 自死锁。
        let mem = self.settings.memory();
        let memory_manager_model = model.filter(|m| !m.trim().is_empty());
        // 批 F-24b / M-18c：一次取锁、零 `LlmConfig` 副本（改前同一分支里成对两次全量克隆，
        // 含各档明文 Key）。
        self.settings.with_llm_config(|llm| {
            // 记忆管理器配了服务档 → 整档换（与 v2.58 会话覆盖同一档位语义）
            if let (Some(m), Some(pid)) = (
                &memory_manager_model,
                mem.memory_manager_provider.as_deref(),
            ) {
                if !pid.trim().is_empty() {
                    if let Some(p) = llm.profile(pid) {
                        // flat_profile：keyring 双读 + headers + 全区代理随行（批 C）
                        let mut flat = llm.flat_profile(p);
                        flat.model = m.to_string();
                        return flat;
                    }
                }
            }
            let mut cfg = llm.flat();
            if let Some(m) = memory_manager_model {
                cfg.model = m.to_string();
            }
            cfg
        })
    }
}

#[async_trait]
impl memory_tree::MemoryManagerClient for AppLlmMemoryManager {
    async fn complete(
        &self,
        system: &str,
        user: &str,
        model: Option<&str>,
    ) -> Result<(String, memory_tree::MemoryManagerUsage), String> {
        let cfg = self.cfg_for(model);
        if cfg.model.trim().is_empty() {
            return Err("记忆管理器模型未配置".into());
        }
        let req = LlmRequest {
            messages: vec![Message::user(user.to_string())],
            tools: vec![],
            model: None,
            // 辅助调用标记（批 B §5.4）：推理档降到方言最低档、不发 max_tokens
            params: serde_json::json!({ "aux": true }),
            system: Some(system.to_string()),
            // 记忆管理器调用不属于任何会话轮：会话级模型覆盖 / 调度绑定都
            // 不该波及它（ctx-compact 摘要调用同款约定）
            session_id: None,
        };
        // 记录员直呼 CompatLlm（绕过 AppLlm.prepare）：辅助降档在此就地解析
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
            .map(|u| memory_tree::MemoryManagerUsage {
                input: u.input,
                output: u.output,
            })
            .unwrap_or_default();
        Ok((resp.content, usage))
    }
}
