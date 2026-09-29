//! 阶段五脚本模型：内容驱动的确定性 LLM 替身。真实模型适配器是后续
//! 阶段的插件，同样实现 LlmService，对内核与前端无感知（§7.2）。
//!
//! 决策规则（无状态，只看投影消息的最后一条）：
//! - User 消息含 "echo" → 调用 tool-echo 插件的 echo 工具
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
                } else {
                    Ok(LlmResponse {
                        content: "[演示模型] 收到你的消息。阶段五的模型是脚本替身：\
                                  发送包含 \"echo\" 的消息可以触发 tool-echo 插件的工具调用，\
                                  工具卡片会显示在会话里。"
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
