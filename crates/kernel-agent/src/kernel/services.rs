//! 内核的两条服务契约与载荷（v2.57 自 kernel.rs 按单文件上限拆出——
//! 拆分是独立任务、外部 API 不变）：LLM 适配器（LlmService/
//! LlmRequest/LlmResponse/LlmError）与工具目录（ToolSource/ToolCallReq）。
//! kernel.rs `pub use` 保持 `kernel_agent::LlmService` 等路径不变。

use std::sync::Arc;

use async_trait::async_trait;
use plugin_api::{Tool, ToolError, ToolSpec};
use serde_json::Value;
use session_store::{Message, SessionEvent};

/// LLM 请求：直接复用 session-store 的中立投影，模型适配层再映射各家协议。`tools`
/// 是本 step 的工具目录快照（LLM.md：统一 OpenAI 风格，适配器负责转成各家格式）；
/// `model` / `params` 可被 `agent/request` 决策环改写（§7.2）；`system` 是请求级系统
/// 提示（⑪ Memory-Inject 走这里，会话日志与压缩不触碰）。`session_id`（v2.58）是
/// 发起会话的身份（`RequestPayload` 已有，此前投影时丢失）——适配器可按会话解析
/// 会话级配置（如桌面端的会话模型覆盖）；内核只透传不解释。
#[derive(Debug, Clone, Default)]
pub struct LlmRequest {
    pub messages: Vec<Message>,
    pub tools: Vec<ToolSpec>,
    pub model: Option<String>,
    pub params: Value,
    pub system: Option<String>,
    pub session_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ToolCallReq {
    pub id: plugin_api::CallId,
    pub tool: String,
    pub args: Value,
    /// 发起调用的会话（step_inner 填充；LLM 解析产物初始为 None）。
    /// 审批门（⑮）据此给子 Agent 的 Irreversible 请求打标识。
    pub session_id: Option<String>,
}

/// 单次 LLM 用量（v2.58）：供应商报告的真实计数（genai Usage 归一化口径）。
/// `input` 含缓存读取（OpenAI 兼容口径 cached ⊆ prompt，命中率 = cache_read/input）；
/// `total` = 供应商报告的总量（0 = 未报告）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LlmUsage {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub reasoning: u64,
    pub total: u64,
}

#[derive(Debug, Clone, Default)]
pub struct LlmResponse {
    pub content: String,
    pub tool_calls: Vec<ToolCallReq>,
    /// 供应商报告的单次用量（v2.58）；内核落 `SessionEvent::LlmUsage`，
    /// 不报告 = None（宁缺勿假，不用估算冒充真实计数）。
    pub usage: Option<LlmUsage>,
}

#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("llm backend failed: {0}")]
    Backend(String),
}

/// 模型适配器不特殊，也只是插件：以 trait 对象服务获取
/// （插件用 provide_dyn::<dyn LlmService> 注册，§7.2）。
#[async_trait]
pub trait LlmService: Send + Sync + 'static {
    async fn complete(&self, req: LlmRequest) -> Result<LlmResponse, LlmError>;

    /// 流式完成（⑳）：文本 delta 逐段回调 `on_delta`；完整正文与工具调用仍在
    /// 返回值（日志/投影只认返回值，delta 只是实时通道）。缺省回落 `complete`
    /// 并把正文一次性回调——未实现流式的适配器（DemoLlm 等）零改动获得流式形状。
    async fn complete_streaming(
        &self,
        req: LlmRequest,
        // 显式 HRTB：async_trait 会把 `Fn(&str)` 的内层生命周期误绑到本引用上
        on_delta: &(dyn for<'a> Fn(&'a str) + Send + Sync),
    ) -> Result<LlmResponse, LlmError> {
        let resp = self.complete(req).await?;
        let content = resp.content.clone();
        if !content.is_empty() {
            on_delta(&content);
        }
        Ok(resp)
    }

    /// 带推理通道的流式完成（v2.57）：同 complete_streaming，另加 `on_reasoning`
    /// 逐段回调供应商 reasoning。缺省回落 complete_streaming 并忽略推理通道——未实现
    /// 推理的适配器零改动（推理只是增量入口，全文以回调累积为准，不进 LlmResponse）。
    async fn complete_streaming_reasoning(
        &self,
        req: LlmRequest,
        on_delta: &(dyn for<'a> Fn(&'a str) + Send + Sync),
        on_reasoning: &(dyn for<'a> Fn(&'a str) + Send + Sync),
    ) -> Result<LlmResponse, LlmError> {
        let _ = on_reasoning;
        self.complete_streaming(req, on_delta).await
    }
}

/// 工具目录服务：kernel-agent 只依赖这个抽象。plugin-host 负责把
/// PluginRegistry 适配成 ToolSource 注册进 Context——主循环对 dlopen /
/// Registry 一无所知，"工具目录"也只是个服务（§7.2）。
#[async_trait]
pub trait ToolSource: Send + Sync + 'static {
    /// `session_id`（㉘·B）：目录可按会话过滤——计划模式 = 目录级围栏（非 Pure
    /// 工具不出目录、解析报 NotAvailable），未来"插件按项目过滤可见性"同落点。
    /// 内核只透传 id，不做任何模式判断。
    async fn resolve(
        &self,
        session_id: Option<&str>,
        name: &str,
    ) -> Result<Arc<dyn Tool>, ToolError>;
    /// 全目录快照：主循环每个 step 枚举给 LLM（真实模型要看到工具才会调）。
    /// 只读枚举不失败——没有目录服务就返回空表，聊天照常。
    async fn list(&self, session_id: Option<&str>) -> Vec<ToolSpec>;
}

/// 树会话组装计划（M4 记忆树切流，树 v2 §7）：宿主实现返回；`None` =
/// legacy 会话（组装零变化）。内核据此把请求消息切片到未整理尾巴，并把
/// L0 目录交给 `agent/request` 链追加到 system 尾部。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TreePlan {
    /// L0 目录块全文（使用说明 + `- b2: 标题` 清单；空串 = 尚无捆）。
    /// 只增不改纪律（占位捆不进目录）归宿主生成侧保证。
    pub directory: String,
    /// 尾巴起点（会话事件下标，排他游标语义——`[0..from)` 已归捆）。
    /// 模式 B = `max(bundled_until, 窗口起点)`（宿主算好）。
    pub tail_from_event: usize,
    /// 收起中的捆 id（§8.3 空间急救）：组装侧把这些捆已展开的工具结果
    /// 替换为收起桩（协议配对保全，事件流零改动）。
    pub collapsed_bundles: Vec<String>,
}

/// 树会话组装源（树 v2 §11.2 阶段 2）：宿主提供实现（desktop 读
/// memory-tree 投影 + 会话旁车 organizer 档），headless / 未注册 = 全局
/// legacy 语义不变。内核只透传会话 id，不解释 organizer 概念——与
/// MemorySource 同款"内核定契约、组装点实现"模式。
#[async_trait]
pub trait TreeContextSource: Send + Sync + 'static {
    async fn plan(&self, session_id: &str) -> Option<TreePlan>;
}

/// 尾巴起点兜底（M4 §3.38②）：从 `from` 起找第一条 User 消息事件——
/// 整理切点纪律保证落在 User 之前，但记录员切分不硬校验，尾巴起点落在
/// 半轮上时（Assistant / ToolResult 开头 = 协议非法）前进到下一条 User。
/// 被跳过的事件已在树目录覆盖范围内；None = 尾巴内没有 User（空表组装）。
pub fn tail_user_start(events: &[SessionEvent], from: usize) -> Option<usize> {
    events
        .iter()
        .enumerate()
        .skip(from)
        .find_map(|(i, e)| matches!(e, SessionEvent::UserMessage { .. }).then_some(i))
}

/// 收起桩替换（M4 §8.3）：`collapsed` 中各捆的 memory_expand 工具结果
/// 替换为桩文本——不删事件、不拆 tool_calls ↔ ToolResult 配对（真实模型
/// 协议要求 id 成对），只把内容从"已发送历史"里换成一句说明。
pub fn collapse_expanded(mut messages: Vec<Message>, collapsed: &[String]) -> Vec<Message> {
    if collapsed.is_empty() {
        return messages;
    }
    for m in &mut messages {
        if let Message::ToolResult {
            tool, args, output, ..
        } = m
        {
            if tool != "memory_expand" {
                continue;
            }
            let Some(id) = args.get("bundle_id").and_then(Value::as_str) else {
                continue;
            };
            if collapsed.iter().any(|c| c == id) {
                *output = Value::String(format!(
                    "[memory bundle {id} collapsed to save context window; \
                     call memory_expand again if you need it]"
                ));
            }
        }
    }
    messages
}
