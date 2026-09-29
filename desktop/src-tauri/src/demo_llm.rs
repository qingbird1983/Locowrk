//! 阶段五脚本模型：内容驱动的确定性 LLM 替身。真实模型适配器是后续
//! 阶段的插件，同样实现 LlmService，对内核与前端无感知（§7.2）。
//!
//! 决策规则（无状态，只看投影消息的最后一条）：
//! - User 消息含 "echo" → 调用 tool-echo 插件的 echo 工具
//! - User 消息含 "email" / "邮件" → 调用内置 send_email（不可逆，触发审批）
//! - User 消息不含 → 直接回答（提示怎么触发工具）
//! - ToolResult → 汇总工具输出作答

use std::sync::atomic::{AtomicU64, Ordering};

use async_trait::async_trait;
use kernel_agent::{LlmError, LlmRequest, LlmResponse, LlmService, ToolCallReq};
use plugin_api::CallId;
use serde_json::json;
use session_store::Message;

pub struct DemoLlm {
    next_call: AtomicU64,
}

impl DemoLlm {
    pub fn new() -> Self {
        Self {
            next_call: AtomicU64::new(1),
        }
    }
}

impl Default for DemoLlm {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LlmService for DemoLlm {
    async fn complete(&self, req: LlmRequest) -> Result<LlmResponse, LlmError> {
        match req.messages.last() {
            Some(Message::User { content }) => {
                if content.to_lowercase().contains("echo") {
                    let n = self.next_call.fetch_add(1, Ordering::Relaxed);
                    Ok(LlmResponse {
                        content: String::new(),
                        tool_calls: vec![ToolCallReq {
                            id: CallId(format!("call-{n}")),
                            tool: "echo".into(),
                            args: json!({ "text": content }),
                        }],
                    })
                } else if content.to_lowercase().contains("email") || content.contains("邮件") {
                    let n = self.next_call.fetch_add(1, Ordering::Relaxed);
                    Ok(LlmResponse {
                        content: String::new(),
                        tool_calls: vec![ToolCallReq {
                            id: CallId(format!("call-{n}")),
                            tool: "send_email".into(),
                            args: json!({
                                "to": "user@example.com",
                                "subject": content,
                                "body": content,
                            }),
                        }],
                    })
                } else {
                    Ok(LlmResponse {
                        content: "[演示模型] 收到你的消息。阶段六的模型是脚本替身：\
                                  发送包含 \"echo\" 的消息触发 tool-echo 插件；\
                                  发送包含 \"email\"（或\"邮件\"）的消息触发 send_email，\
                                  它是不可逆工具，会弹出审批确认。"
                            .into(),
                        tool_calls: vec![],
                    })
                }
            }
            Some(Message::ToolResult { output, error, .. }) => {
                let body = match error {
                    Some(e) => format!("工具执行失败：{e}"),
                    None => format!("工具返回：{output}"),
                };
                Ok(LlmResponse {
                    content: format!("[演示模型] {body}"),
                    tool_calls: vec![],
                })
            }
            _ => Ok(LlmResponse {
                content: "[演示模型] ……（没有可回应用户输入）".into(),
                tool_calls: vec![],
            }),
        }
    }
}
