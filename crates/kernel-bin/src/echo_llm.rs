//! 脚本化 LLM（阶段三没有模型插件；阶段四后由 llm-* 插件 provide）。
//! 每次 step 按序产出一条带工具调用的回复，驱动 demo 走完验收叙事。

use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use kernel_agent::{LlmError, LlmRequest, LlmResponse, LlmService, ToolCallReq};
use plugin_api::CallId;
use serde_json::json;

/// 按调用次序轮转的三段脚本：
/// 1. plain 回显（0.1.0 即支持）
/// 2. reverse 回显（0.2.0 新能力，激活后成功）
/// 3. 再次 reverse（回退 0.1.0 后应失败——证明回退生效）
pub struct ScriptedLlm {
    step: AtomicUsize,
}

impl ScriptedLlm {
    pub fn new() -> Self {
        Self {
            step: AtomicUsize::new(0),
        }
    }
}

impl Default for ScriptedLlm {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LlmService for ScriptedLlm {
    async fn complete(&self, _req: LlmRequest) -> Result<LlmResponse, LlmError> {
        let n = self.step.fetch_add(1, Ordering::Relaxed);
        let (content, args) = match n % 3 {
            0 => (
                "我先调用 echo 工具（plain 模式）。",
                json!({ "text": "Hello, Locowrk!", "mode": "plain" }),
            ),
            1 => (
                "升级后试试 0.2.0 的 reverse 模式。",
                json!({ "text": "Hello", "mode": "reverse" }),
            ),
            _ => (
                "回退后再试 reverse——旧版本应该不认识这个模式。",
                json!({ "text": "Hello", "mode": "reverse" }),
            ),
        };
        Ok(LlmResponse {
            content: content.to_string(),
            tool_calls: vec![ToolCallReq {
                id: CallId(format!("call-{}", n % 3 + 1)),
                tool: "echo".to_string(),
                args,
                session_id: None,
            }],
        })
    }
}
