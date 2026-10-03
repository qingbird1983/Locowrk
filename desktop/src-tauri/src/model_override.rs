//! 会话级模型覆盖（v2.58 §3.30）：composer 模型 chip 选择器的后端语义。
//!
//! 覆盖是**会话级属性**：持久层 = 会话旁车 `<id>.meta.json` 的
//! `model` + `model_provider` 字段（成对写入，重启保持）；内存层 =
//! 本模块缓存 + 写穿（[`crate::permission_mode::PermissionModeStore`]
//! 同款结构）。消费方两处：
//! ① [`crate::llm::app_llm::AppLlm`] 每请求按 `LlmRequest.session_id`
//!    解析覆盖（档位路由 + 模型名，设置缺省不动）；
//! ② `commands/sessions.rs` `session_set_model` 命令。
//! 未显式设置的会话 = 跟随设置缺省（get 返回 None）。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use session_store::{read_meta, write_model_override};

/// 覆盖存储：缓存 + 旁车写穿。`get` 未命中缓存时读一次旁车（缺字段 /
/// 无会话 = None）并缓存；`set` 写穿旁车 + 更新缓存。会话删除后残留的
/// 缓存条目无害（id 不复用）。
pub struct ModelOverrideStore {
    sessions_dir: PathBuf,
    cache: Mutex<HashMap<String, Option<(String, String)>>>,
}

impl ModelOverrideStore {
    pub fn new(sessions_dir: PathBuf) -> Self {
        Self {
            sessions_dir,
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// 会话当前覆盖（None = 跟随设置缺省）。旁车读失败当未设置（不致命）。
    pub fn get(&self, session_id: &str) -> Option<(String, String)> {
        let mut cache = self.cache.lock().expect("model override cache poisoned");
        if let Some(hit) = cache.get(session_id) {
            return hit.clone();
        }
        let hit = read_meta(&self.sessions_dir, session_id).and_then(|m| {
            match (m.model, m.model_provider) {
                (Some(model), Some(provider)) if !model.is_empty() && !provider.is_empty() => {
                    Some((provider, model))
                }
                _ => None,
            }
        });
        cache.insert(session_id.to_string(), hit.clone());
        hit
    }

    /// 写穿旁车 + 更新缓存。`model` None = 清除覆盖（回落设置缺省）。
    pub fn set(&self, session_id: &str, model: Option<(&str, &str)>) -> Result<(), std::io::Error> {
        write_model_override(&self.sessions_dir, session_id, model)?;
        let hit = model.map(|(p, m)| (p.to_string(), m.to_string()));
        self.cache
            .lock()
            .expect("model override cache poisoned")
            .insert(session_id.to_string(), hit);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
