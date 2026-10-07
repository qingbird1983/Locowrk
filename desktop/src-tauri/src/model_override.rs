//! 会话级模型覆盖（v2.58 §3.30）：composer 模型 chip 选择器的后端语义。
//!
//! 覆盖是**会话级属性**：持久层 = 会话旁车 `<id>.meta.json` 的
//! `model` + `model_provider` 字段（成对写入，重启保持）；内存层 =
//! 本模块缓存 + 写穿（[`crate::permission_mode::PermissionModeStore`]
//! 同款结构）。消费方两处：
//! ① [`crate::llm::app_llm::AppLlm`] 每请求按 `LlmRequest.session_id`
//!    解析覆盖（档位路由 + 模型名 + 批 B 推理档位，设置缺省不动）；
//! ② `commands/sessions.rs` `session_set_model` / `session_set_thinking`
//!    命令。
//! 未显式设置的会话 = 跟随设置缺省（get 返回 None）。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use session_store::{read_meta, write_model_override, write_thinking_level};

/// 单会话的覆盖集合（模型对 + 推理档；各自独立可空）。
#[derive(Debug, Clone, Default, PartialEq)]
struct SessionOverride {
    model: Option<(String, String)>,
    thinking: Option<String>,
}

/// 覆盖存储：缓存 + 旁车写穿。`get*` 未命中缓存时读一次旁车（缺字段 /
/// 无会话 = None）并缓存；`set*` 写穿旁车 + 更新缓存。会话删除后残留的
/// 缓存条目无害（id 不复用）。
pub struct ModelOverrideStore {
    sessions_dir: PathBuf,
    cache: Mutex<HashMap<String, SessionOverride>>,
}

impl ModelOverrideStore {
    pub fn new(sessions_dir: PathBuf) -> Self {
        Self {
            sessions_dir,
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// 缓存未命中时从旁车读一次。
    fn load(&self, session_id: &str) -> SessionOverride {
        let mut cache = self.cache.lock().expect("model override cache poisoned");
        if let Some(hit) = cache.get(session_id) {
            return hit.clone();
        }
        let meta = read_meta(&self.sessions_dir, session_id);
        let hit = SessionOverride {
            model: meta
                .as_ref()
                .and_then(|m| match (m.model.clone(), m.model_provider.clone()) {
                    (Some(model), Some(provider)) if !model.is_empty() && !provider.is_empty() => {
                        Some((provider, model))
                    }
                    _ => None,
                }),
            thinking: meta
                .as_ref()
                .and_then(|m| m.thinking_level.clone())
                .filter(|t| !t.is_empty()),
        };
        cache.insert(session_id.to_string(), hit.clone());
        hit
    }

    fn store(&self, session_id: &str, patch: impl FnOnce(&mut SessionOverride)) {
        let mut cache = self.cache.lock().expect("model override cache poisoned");
        let entry = cache.entry(session_id.to_string()).or_default();
        patch(entry);
    }

    /// 会话当前模型覆盖（None = 跟随设置缺省）。旁车读失败当未设置（不致命）。
    pub fn get(&self, session_id: &str) -> Option<(String, String)> {
        self.load(session_id).model
    }

    /// 会话当前推理档位（批 B；None = 未设置，请求不带档位字段）。
    pub fn get_thinking(&self, session_id: &str) -> Option<String> {
        self.load(session_id).thinking
    }

    /// 写穿旁车 + 更新缓存。`model` None = 清除覆盖（回落设置缺省）。
    pub fn set(&self, session_id: &str, model: Option<(&str, &str)>) -> Result<(), std::io::Error> {
        write_model_override(&self.sessions_dir, session_id, model)?;
        self.store(session_id, |o| {
            o.model = model.map(|(p, m)| (p.to_string(), m.to_string()));
        });
        Ok(())
    }

    /// 写会话推理档位（批 B）：`Some(level)` = 设置（调用方已按梯子校验
    /// 语义档取值）；`None` / 空串 = 清除（模型缺省档）。合并写保留其余字段。
    pub fn set_thinking(
        &self,
        session_id: &str,
        level: Option<&str>,
    ) -> Result<(), std::io::Error> {
        let lvl = level.map(str::trim).filter(|s| !s.is_empty());
        write_thinking_level(&self.sessions_dir, session_id, lvl)?;
        self.store(session_id, |o| {
            o.thinking = lvl.map(str::to_string);
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests;
