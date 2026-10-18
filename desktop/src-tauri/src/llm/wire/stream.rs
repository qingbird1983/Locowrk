//! 流式累积器与用量映射（同一管道的后半段）：`StreamAcc` 收 chunk 拼正文 / 推理 /
//! 工具调用分片（按 index 拼接、空 id 收束时按序补号），`Delta` = 单块对累积器的作用，
//! `parse_usage` = 各家用量字段映射到一个形状（全 0 = None，宁缺勿假）。
//! 非流式 `complete` 也走这条管道聚合 = **唯一线路**，无第二形状。

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{json, Value};

use kernel_agent::{LlmResponse, LlmUsage, ToolCallReq};
use plugin_api::CallId;

/// 单个 SSE chunk 对累积器的作用（delta 为切片副本——token 级小串，
/// 拷贝换掉自引用生命周期的麻烦）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Delta {
    Content(String),
    Reasoning(String),
    None,
}

#[derive(Default, Clone)]
struct ToolAcc {
    id: String,
    name: String,
    args: String,
}

/// 流式累积器：正文 / 推理 / 工具调用分片（按 index 拼接）/ finish_reason /
/// usage。空 id 的调用在收束时按序补号（本地模型常见，§6.3 关联键需要）。
#[derive(Default)]
pub struct StreamAcc {
    content: String,
    tools: BTreeMap<u32, ToolAcc>,
    finish_reason: Option<String>,
    usage: Option<LlmUsage>,
}

impl StreamAcc {
    pub fn new() -> Self {
        Self::default()
    }

    /// 应用一个 chunk，返回需要实时外推的 delta（正文 / 推理）。解析失败或
    /// 与正文无关的块（如纯 usage 尾块）返回 `None`。
    pub fn apply(&mut self, chunk: &Value) -> Delta {
        if let Some(u) = chunk.get("usage") {
            self.usage = parse_usage(u).or(self.usage.take());
        }
        let Some(choice) = chunk
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|c| c.first())
        else {
            return Delta::None;
        };
        if let Some(fr) = choice.get("finish_reason").and_then(Value::as_str) {
            self.finish_reason = Some(fr.to_string());
        }
        let Some(delta) = choice.get("delta") else {
            return Delta::None;
        };
        // 推理先行（reasoning_content 是 DeepSeek/Qwen/GLM 等的事实标准；
        // 个别网关用 reasoning）——推理不进正文
        for key in ["reasoning_content", "reasoning"] {
            if let Some(r) = delta.get(key).and_then(Value::as_str) {
                if !r.is_empty() {
                    return Delta::Reasoning(r.to_string());
                }
            }
        }
        if let Some(tool_calls) = delta.get("tool_calls").and_then(Value::as_array) {
            for tc in tool_calls {
                self.apply_tool_call(tc);
            }
        }
        match delta.get("content").and_then(Value::as_str) {
            Some(c) if !c.is_empty() => {
                self.content.push_str(c);
                Delta::Content(c.to_string())
            }
            _ => Delta::None,
        }
    }

    /// `delta.tool_calls[]` 单项：按 index 拼接 id / function.name /
    /// arguments（参数分片传输）。
    fn apply_tool_call(&mut self, tc: &Value) {
        let index = tc.get("index").and_then(Value::as_u64).unwrap_or(0) as u32;
        let acc = self.tools.entry(index).or_default();
        if let Some(id) = tc.get("id").and_then(Value::as_str) {
            if !id.is_empty() {
                acc.id = id.to_string();
            }
        }
        if let Some(f) = tc.get("function") {
            if let Some(n) = f.get("name").and_then(Value::as_str) {
                if !n.is_empty() {
                    acc.name = n.to_string();
                }
            }
            if let Some(a) = f.get("arguments").and_then(Value::as_str) {
                acc.args.push_str(a);
            }
        }
    }

    /// 本轮是否已向外推过任何正文/推理 delta（重试重置判定用）。
    pub fn finish_reason(&self) -> Option<&str> {
        self.finish_reason.as_deref()
    }

    /// 收束：正文 + 工具调用（参数字符串解析回 JSON Value；空参 = `{}`；
    /// 解析失败的原始串原样传递——工具执行会以清晰错误回传，不静默丢弃）+
    /// 用量。空 call_id 按序补号。
    pub fn finish(self, next_call: &AtomicU64) -> LlmResponse {
        let tool_calls = self
            .tools
            .into_values()
            .map(|acc| {
                let id = if acc.id.is_empty() {
                    format!("call-{}", next_call.fetch_add(1, Ordering::Relaxed))
                } else {
                    acc.id
                };
                let args = if acc.args.trim().is_empty() {
                    json!({})
                } else {
                    serde_json::from_str(&acc.args)
                        .unwrap_or_else(|_| Value::String(acc.args.clone()))
                };
                ToolCallReq {
                    id: CallId(id),
                    tool: acc.name,
                    args,
                    session_id: None,
                }
            })
            .collect();
        LlmResponse {
            content: self.content,
            tool_calls,
            usage: self.usage,
        }
    }
}

/// 用量映射（LLM.md 字段表）：input/output/total 基础计数；缓存读 =
/// OpenAI `prompt_tokens_details.cached_tokens` 或 DeepSeek
/// `prompt_cache_hit_tokens`；缓存写 = `cache_creation_tokens`（Anthropic
/// 形态，有则取）；推理 = `completion_tokens_details.reasoning_tokens`。
/// 全 0 = None（供应商没报有效用量，宁缺勿假）。
pub fn parse_usage(u: &Value) -> Option<LlmUsage> {
    let num = |v: &Value| -> u64 { v.as_i64().unwrap_or(0).max(0) as u64 };
    let input = num(&u["prompt_tokens"]);
    let output = num(&u["completion_tokens"]);
    if input == 0 && output == 0 {
        return None;
    }
    let prompt_d = &u["prompt_tokens_details"];
    let completion_d = &u["completion_tokens_details"];
    Some(LlmUsage {
        input,
        output,
        cache_read: num(&prompt_d["cached_tokens"]).max(num(&u["prompt_cache_hit_tokens"])),
        cache_write: num(&prompt_d["cache_creation_tokens"]),
        reasoning: num(&completion_d["reasoning_tokens"]),
        total: num(&u["total_tokens"]),
    })
}
