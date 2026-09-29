//! 阶段四 demo 的脚本化 LLM：
//! - `PolicyLlm`（Agent A）：echo（mode 取自 request 改写后的 params）→
//!   danger → danger（演示审批拒绝 / 批准两条路径）
//! - `IsolatedLlm`（Agent B）：echo（解析到作用域 stub 工具）

use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use kernel_agent::{LlmError, LlmRequest, LlmResponse, LlmService, ToolCallReq};
use plugin_api::CallId;
use serde_json::json;

pub struct PolicyLlm {
    step: AtomicUsize,
}

impl PolicyLlm {
    pub fn new() -> Self {
        Self {
            step: AtomicUsize::new(0),
        }
    }
}

impl Default for PolicyLlm {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LlmService for PolicyLlm {
    async fn complete(&self, req: LlmRequest) -> Result<LlmResponse, LlmError> {
        let n = self.step.fetch_add(1, Ordering::Relaxed);
        let (content, tool, args) = match n {
            0 => {
                // echo 的 mode 来自 request 决策环改写后的 params（验收点）
                let mode = req
                    .params
                    .get("mode")
                    .and_then(|v| v.as_str())
                    .unwrap_or("plain")
                    .to_string();
                (
                    "按审批插件注入的策略调用 echo".to_string(),
                    "echo",
                    json!({ "text": "hello locowrk", "mode": mode }),
                )
            }
            1 => (
                "删除临时文件（不可逆，应被首次审批拒绝）".into(),
                "danger",
                json!({ "target": "tmp/demo-file.txt" }),
            ),
            _ => (
                "再试一次删除（复核后应放行）".into(),
                "danger",
                json!({ "target": "tmp/demo-file.txt" }),
            ),
        };
        Ok(LlmResponse {
            content,
            tool_calls: vec![ToolCallReq {
                id: CallId(format!("a-call-{}", n + 1)),
                tool: tool.into(),
                args,
            }],
        })
    }
}

pub struct IsolatedLlm {
    step: AtomicUsize,
}

impl IsolatedLlm {
    pub fn new() -> Self {
        Self {
            step: AtomicUsize::new(0),
        }
    }
}

impl Default for IsolatedLlm {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LlmService for IsolatedLlm {
    async fn complete(&self, _req: LlmRequest) -> Result<LlmResponse, LlmError> {
        let n = self.step.fetch_add(1, Ordering::Relaxed);
        Ok(LlmResponse {
            content: "作用域 B：调用同名 echo 工具".into(),
            tool_calls: vec![ToolCallReq {
                id: CallId(format!("b-call-{}", n + 1)),
                tool: "echo".into(),
                args: json!({ "text": "scope-b hello", "mode": "plain" }),
            }],
        })
    }
}
