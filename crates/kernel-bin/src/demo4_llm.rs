//! 阶段四 demo 的脚本化 LLM：
//! - `PolicyLlm`（Agent A）：danger → danger → echo
//!   （前两步演"同一工具首次被拒未执行 / 复核后放行执行"，第三步演"request 决策环
//!   改写后的 params 真的被下游吃到"）
//! - `IsolatedLlm`（Agent B）：echo（解析到作用域 stub 工具）
//!
//! **为什么 danger 必须排在 echo 前面**（台账 T21 查出来的这件事）：审批替身
//! `ApprovalPlugin` 的规则是"**全局**第一次询问拒绝、之后一律放行"，而 tool-echo 的
//! `echo` 也是不可逆声明 ⇒ 旧顺序（echo → danger → danger）里那一次"首次拒绝"落在
//! 了 echo 身上，两条后果都不是这条叙事想演的：① `danger` 两次都被批准、真的执行了
//! 两次，末行却写着"预期 1"；② echo 被拒 ⇒ 没有任何一步证明改写后的 `params.mode`
//! 被工具吃到（"参数改写"这条验收点只剩一行 stdout，磁盘上的会话日志里没有结果）。
//! 排成 danger → danger → echo 之后，"先拒后放"落在同一个工具上，echo 又是被批准的
//! 那一步 ⇒ 两条验收点各有会话事件承重，并由 `demo4.rs` 末尾的自判逐条判定。

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
            0 => (
                "删除临时文件（不可逆，应被首次审批拒绝）".into(),
                "danger",
                json!({ "target": "tmp/demo-file.txt" }),
            ),
            1 => (
                "再试一次删除（复核后应放行）".into(),
                "danger",
                json!({ "target": "tmp/demo-file.txt" }),
            ),
            _ => {
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
        };
        Ok(LlmResponse {
            content,
            tool_calls: vec![ToolCallReq {
                id: CallId(format!("a-call-{}", n + 1)),
                tool: tool.into(),
                args,
                session_id: None,
            }],
            usage: None,
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
                session_id: None,
            }],
            usage: None,
        })
    }
}
