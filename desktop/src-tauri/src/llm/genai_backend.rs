//! genai 三合一后端（LLM.md 方案一）：内核中立请求 ↔ genai ChatRequest
//! 的映射，provider / model / base_url / api_key 全部来自设置面板。
//!
//! 路由策略：不做模型名前缀魔法——`ServiceTargetResolver` 按设置里的
//! provider 强制 AdapterKind，并按需覆盖端点。用户显式选择优先于
//! genai 的模型名启发式（否则 "qwen2.5" 之类会被误路由到 Ollama）。

use std::sync::atomic::{AtomicU64, Ordering};

use async_trait::async_trait;
use futures::StreamExt;
use genai::adapter::AdapterKind;
use genai::chat::{
    ChatMessage, ChatOptions, ChatRequest, ChatResponse, ChatStreamEvent, ContentPart,
    MessageContent, Tool as GenAiTool, ToolCall as GenAiCall, ToolResponse, Usage,
};
use genai::resolver::{AuthData, AuthResolver, Endpoint, ServiceTargetResolver};
use genai::{Client, ModelIden, ServiceTarget};
use kernel_agent::{LlmError, LlmRequest, LlmResponse, LlmService, LlmUsage, ToolCallReq};
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
                // openai / anthropic 适配器的端点须含版本段（genai 只在后面
                // 追加 `chat/completions` / `messages`）→ `join_version` 归一；
                // ollama 的路径自带 `api/` 前缀，原样使用。
                let endpoint = if matches!(provider.as_str(), "openai" | "anthropic") {
                    crate::llm::models::join_version(url)
                } else {
                    url.clone()
                };
                target.endpoint = Endpoint::from_owned(endpoint);
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
    let usage = map_usage(&res.usage);
    map_parts(content, res.into_tool_calls(), next_call, usage)
}

/// genai 归一化 Usage → 内核 [`LlmUsage`]（v2.58 §3.12 用量明细的数据源）。
/// None 字段计 0；input/output 全 0 = 供应商没报告有效用量 → None
/// （内核不落 LlmUsage 事件，宁缺勿假）。
fn map_usage(u: &Usage) -> Option<LlmUsage> {
    let input = u.prompt_tokens.unwrap_or(0).max(0) as u64;
    let output = u.completion_tokens.unwrap_or(0).max(0) as u64;
    if input == 0 && output == 0 {
        return None;
    }
    let prompt_d = u.prompt_tokens_details.as_ref();
    let completion_d = u.completion_tokens_details.as_ref();
    Some(LlmUsage {
        input,
        output,
        cache_read: prompt_d.and_then(|d| d.cached_tokens).unwrap_or(0).max(0) as u64,
        cache_write: prompt_d
            .and_then(|d| d.cache_creation_tokens)
            .unwrap_or(0)
            .max(0) as u64,
        reasoning: completion_d
            .and_then(|d| d.reasoning_tokens)
            .unwrap_or(0)
            .max(0) as u64,
        total: u.total_tokens.unwrap_or(0).max(0) as u64,
    })
}

/// 正文 + genai 工具调用 → 内核响应（非流式/流式两条路共用的收口）。
fn map_parts(
    content: String,
    calls: Vec<GenAiCall>,
    next_call: &AtomicU64,
    usage: Option<LlmUsage>,
) -> LlmResponse {
    let tool_calls = calls
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
                session_id: None,
            }
        })
        .collect();
    LlmResponse {
        content,
        tool_calls,
        usage,
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
    /// 请求装配（非流式/流式共用）：Key 预检 + 中立投影 → ChatRequest +
    /// 基础 options（waterfall 可注入的 params 目前只透传 temperature）。
    fn build_request(
        cfg: &LlmSettings,
        req: &LlmRequest,
    ) -> Result<(Client, ChatRequest, ChatOptions), LlmError> {
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
        // 请求级 system prompt（⑪ Memory-Inject 记忆注入走这里；普通聊天为
        // None）。genai 的 with_system 收 Into<String>。
        if let Some(sys) = &req.system {
            chat_req = chat_req.with_system(sys.as_str());
        }
        if !req.tools.is_empty() {
            chat_req = chat_req.with_tools(to_genai_tools(&req.tools));
        }

        let options = req
            .params
            .get("temperature")
            .and_then(|v| v.as_f64())
            .map(|t| ChatOptions::default().with_temperature(t))
            .unwrap_or_default();
        Ok((build_client(cfg), chat_req, options))
    }

    /// cfg 由调用方（AppLlm）每次请求前从 SettingsStore 读取。
    pub async fn complete_with(
        &self,
        cfg: &LlmSettings,
        req: LlmRequest,
    ) -> Result<LlmResponse, LlmError> {
        let (client, chat_req, options) = Self::build_request(cfg, &req)?;
        let res = client
            .exec_chat(cfg.model.as_str(), chat_req, Some(&options))
            .await
            .map_err(|e| LlmError::Backend(format!("{} 调用失败: {e}", cfg.provider)))?;
        Ok(map_response(res, &self.next_call))
    }

    /// 流式（⑳）：文本 delta 逐段回调 `on_delta`；v2.57 推理 delta 逐段
    /// 回调 `on_reasoning`（genai `ReasoningChunk`；`capture_reasoning_content`
    /// 开启聚合）。聚合正文与工具调用取自 End 事件的 captured_content
    /// （genai 在 capture_content / capture_tool_calls 开启时聚合，各家
    /// 适配器一致）。空 delta 不回调。推理全文不在此聚合返回——内核闭包
    /// 逐段累积（与 captured 同源），被消费与否都不影响落日志。
    pub async fn complete_streaming_with(
        &self,
        cfg: &LlmSettings,
        req: LlmRequest,
        on_delta: &(dyn for<'a> Fn(&'a str) + Send + Sync),
        on_reasoning: &(dyn for<'a> Fn(&'a str) + Send + Sync),
    ) -> Result<LlmResponse, LlmError> {
        let (client, chat_req, mut options) = Self::build_request(cfg, &req)?;
        options = options
            .with_capture_content(true)
            .with_capture_tool_calls(true)
            .with_capture_reasoning_content(true)
            .with_capture_usage(true);
        let mut stream = client
            .exec_chat_stream(cfg.model.as_str(), chat_req, Some(&options))
            .await
            .map_err(|e| LlmError::Backend(format!("{} 流式调用失败: {e}", cfg.provider)))?;
        while let Some(ev) = stream.stream.next().await {
            match ev.map_err(|e| LlmError::Backend(format!("{} 流中断: {e}", cfg.provider)))? {
                ChatStreamEvent::Chunk(c) => {
                    if !c.content.is_empty() {
                        on_delta(&c.content);
                    }
                }
                ChatStreamEvent::ReasoningChunk(c) => {
                    if !c.content.is_empty() {
                        on_reasoning(&c.content);
                    }
                }
                ChatStreamEvent::End(end) => {
                    // 推理/思考流已经 on_reasoning 逐段带出，不进正文；
                    // 正文以 End 的 captured_content 为准
                    let content = end
                        .captured_content
                        .as_ref()
                        .and_then(|c| c.first_text())
                        .unwrap_or_default()
                        .to_string();
                    let calls = end
                        .captured_content
                        .map(MessageContent::into_tool_calls)
                        .unwrap_or_default();
                    let usage = end.captured_usage.as_ref().and_then(map_usage);
                    return Ok(map_parts(content, calls, &self.next_call, usage));
                }
                _ => {}
            }
        }
        Err(LlmError::Backend(format!(
            "{} 流式响应未以 End 事件收束",
            cfg.provider
        )))
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
