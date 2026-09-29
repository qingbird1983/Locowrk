//! genai 三合一后端（LLM.md 方案一）：内核中立请求 ↔ genai ChatRequest
//! 的映射，provider / model / base_url / api_key 全部来自设置面板。
//!
//! 路由策略：不做模型名前缀魔法——`ServiceTargetResolver` 按设置里的
//! provider 强制 AdapterKind，并按需覆盖端点。用户显式选择优先于
//! genai 的模型名启发式（否则 "qwen2.5" 之类会被误路由到 Ollama）。

use std::sync::atomic::{AtomicU64, Ordering};

use async_trait::async_trait;
use genai::adapter::AdapterKind;
use genai::chat::{
    ChatMessage, ChatOptions, ChatRequest, ChatResponse, ContentPart, MessageContent,
    Tool as GenAiTool, ToolCall as GenAiCall, ToolResponse,
};
use genai::resolver::{AuthData, AuthResolver, Endpoint, ServiceTargetResolver};
use genai::{Client, ModelIden, ServiceTarget};
use kernel_agent::{LlmError, LlmRequest, LlmResponse, LlmService, ToolCallReq};
use plugin_api::{CallId, ToolSpec};
use session_store::Message;

use crate::settings::LlmSettings;

/// provider → genai 适配器种类（设置层已校验三选一，未知值兜底 OpenAI）。
pub fn adapter_kind(provider: &str) -> AdapterKind {
    match provider {
        "anthropic" => AdapterKind::Anthropic,
        "ollama" => AdapterKind::Ollama,
        _ => AdapterKind::OpenAI,
    }
}

/// openai / anthropic 缺 Key 时的环境变量逃生舱（genai 默认行为）。
fn env_key_name(provider: &str) -> Option<&'static str> {
    match provider {
        "openai" => Some("OPENAI_API_KEY"),
        "anthropic" => Some("ANTHROPIC_API_KEY"),
        _ => None,
    }
}

/// 中立投影 → genai 消息。Assistant 的正文 + 工具调用合成一条消息
/// （各家协议都要求 tool 结果挂在发起调用的同一条 assistant 消息上）。
pub fn to_genai_messages(messages: &[Message]) -> Vec<ChatMessage> {
    messages
        .iter()
        .map(|m| match m {
            Message::User { content } => ChatMessage::user(content.clone()),
            Message::Assistant {
                content,
                tool_calls,
            } => {
                if tool_calls.is_empty() {
                    ChatMessage::assistant(content.clone())
                } else {
                    let mut parts: Vec<ContentPart> = Vec::new();
                    if !content.is_empty() {
                        parts.push(ContentPart::Text(content.clone()));
                    }
                    parts.extend(tool_calls.iter().map(|tc| {
                        ContentPart::ToolCall(GenAiCall {
                            call_id: tc.id.0.clone(),
                            fn_name: tc.tool.clone(),
                            fn_arguments: tc.args.clone(),
                            thought_signatures: None,
                        })
                    }));
                    ChatMessage::assistant(MessageContent::from_parts(parts))
                }
            }
            Message::ToolResult {
                call_id,
                tool,
                output,
                error,
                ..
            } => {
                // 协议没有独立错误位：错误作为内容回传，模型可自行处理
                let content = match error {
                    Some(e) => serde_json::json!({ "error": e }).to_string(),
                    None => output.to_string(),
                };
                ChatMessage::from(ToolResponse {
                    call_id: call_id.0.clone(),
                    // Gemini 按 fn_name 关联结果，带上更稳
                    fn_name: Some(tool.clone()),
                    content,
                })
            }
        })
        .collect()
}

/// ToolSpec → genai Tool（OpenAI 风格：name / description / JSON Schema）。
pub fn to_genai_tools(tools: &[ToolSpec]) -> Vec<GenAiTool> {
    tools
        .iter()
        .map(|t| {
            GenAiTool::new(&t.name)
                .with_description(&t.description)
                .with_schema(t.schema.clone())
        })
        .collect()
}

/// 按设置构建 genai 客户端（每次请求新建：配置变更即时生效，成本可忽略）。
fn build_client(cfg: &LlmSettings) -> Client {
    let mut builder = Client::builder();

    // Key：设置优先；未填则不装 resolver，交给 genai 的环境变量解析
    if let Some(key) = &cfg.api_key {
        let key = key.clone();
        builder = builder.with_auth_resolver(AuthResolver::from_resolver_fn(move |_model_iden| {
            Ok(Some(AuthData::from_single(key.clone())))
        }));
    }

    let provider = cfg.provider.clone();
    let base_url = cfg.base_url.clone();
    builder = builder.with_service_target_resolver(ServiceTargetResolver::from_resolver_fn(
        move |mut target: ServiceTarget| {
            if let Some(url) = &base_url {
                target.endpoint = Endpoint::from_owned(url.clone());
            }
            target.model = ModelIden::new(adapter_kind(&provider), target.model.model_name.clone());
            Ok(target)
        },
    ));

    builder.build()
}

/// genai 响应 → 内核响应。空 call_id（部分本地模型不给）按序补号，
/// 保证 ToolCall / ToolResult 事件关联与补偿去重键可用（§6.3）。
pub(crate) fn map_response(res: ChatResponse, next_call: &AtomicU64) -> LlmResponse {
    let content = res.first_text().unwrap_or_default().to_string();
    let tool_calls = res
        .into_tool_calls()
        .into_iter()
        .map(|tc| {
            let id = if tc.call_id.is_empty() {
                format!("call-{}", next_call.fetch_add(1, Ordering::Relaxed))
            } else {
                tc.call_id
            };
            ToolCallReq {
                id: CallId(id),
                tool: tc.fn_name,
                args: tc.fn_arguments,
            }
        })
        .collect();
    LlmResponse {
        content,
        tool_calls,
    }
}

pub struct GenAiLlm {
    next_call: AtomicU64,
}

impl GenAiLlm {
    pub fn new() -> Self {
        Self {
            next_call: AtomicU64::new(1),
        }
    }
}

impl Default for GenAiLlm {
    fn default() -> Self {
        Self::new()
    }
}

impl GenAiLlm {
    /// cfg 由调用方（AppLlm）每次请求前从 SettingsStore 读取。
    pub async fn complete_with(
        &self,
        cfg: &LlmSettings,
        req: LlmRequest,
    ) -> Result<LlmResponse, LlmError> {
        // 缺 Key 先行报错（genai 的环境变量报错对桌面用户不友好）；
        // ollama 无需 Key
        if let Some(env_name) = env_key_name(&cfg.provider) {
            if cfg.api_key.is_none() && std::env::var(env_name).is_err() {
                return Err(LlmError::Backend(format!(
                    "provider `{}` 缺少 API Key：请在 设置 → LLM 配置 里填写，\
                     或设置环境变量 {env_name}",
                    cfg.provider
                )));
            }
        }

        let mut chat_req = ChatRequest::new(to_genai_messages(&req.messages));
        if !req.tools.is_empty() {
            chat_req = chat_req.with_tools(to_genai_tools(&req.tools));
        }

        // waterfall 改写环可注入的 params 目前只透传 temperature
        let options = req
            .params
            .get("temperature")
            .and_then(|v| v.as_f64())
            .map(|t| ChatOptions::default().with_temperature(t));

        let client = build_client(cfg);
        let res = client
            .exec_chat(cfg.model.as_str(), chat_req, options.as_ref())
            .await
            .map_err(|e| LlmError::Backend(format!("{} 调用失败: {e}", cfg.provider)))?;
        Ok(map_response(res, &self.next_call))
    }
}

#[async_trait]
impl LlmService for GenAiLlm {
    async fn complete(&self, _req: LlmRequest) -> Result<LlmResponse, LlmError> {
        // 裸 GenAiLlm 不作为服务注册（无设置来源）；入口一律走 AppLlm
        Err(LlmError::Backend(
            "GenAiLlm 需要设置来源，请通过 AppLlm 使用".into(),
        ))
    }
}
