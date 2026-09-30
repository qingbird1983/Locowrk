//! ctx-compact（Context and Memory.md §3）：`agent/request` waterfall
//! 监听器——估算 token 超阈值就把早期消息压缩（三策略，目标 = 阈值 50%）。
//!
//! 与 mw-audit 同理落在宿主侧：C ABI 插件够不到 LlmService / EventBus。
//! 摘要调用**直连** LlmService（不走 `agent/request` 决策环——否则自我递归）。
//! 降级链：分层 / 仅摘要的摘要失败或 LlmService 缺失 → 就地退滑动窗口
//! （本地裁剪仍保窗口不溢出）；仍不可压（找不到 User 边界）→ 放行原
//! payload，宁可不压也不让请求失败。
//!
//! 压缩结果经 `RequestPayload::compacted` 带回：kernel 据此落
//! `ContextCompacted` 事件进会话日志（折叠语义见 session-store；撤销 =
//! rewind_to）。配置经共享 `RwLock<CompactConfig>` 注入（组装点持有，
//! 设置保存即换入——本 crate 不依赖任何设置存储）。
//!
//! 切点纪律：压缩边界必须落在 User 消息之前——把 Assistant(tool_calls) 与
//! 它的 ToolResult 拆到摘要两侧会破坏真实模型协议（tool_call_id 配对）。

use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use cordis_core::{Context, WaterfallDecision, WaterfallListener};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use session_store::Message;

use crate::kernel::{LlmRequest, LlmService};
use crate::waterfall::{RequestPayload, points};

/// 压缩策略（Context and Memory.md §3.3）。serde 走 snake_case，与
/// settings.json 同形（desktop 设置区直接复用本类型反序列化）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CompactStrategy {
    /// 滑动窗口：纯本地裁剪，不调 LLM，信息损失高、成本零。
    SlidingWindow,
    /// 分层保留（默认）：早期摘要 + 最近原文，平衡成本与连续性。
    #[default]
    Layered,
    /// 仅摘要：除当前轮外全部压成一段摘要，成本最高。
    SummaryOnly,
}

impl CompactStrategy {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SlidingWindow => "sliding_window",
            Self::Layered => "layered",
            Self::SummaryOnly => "summary_only",
        }
    }
}

/// 动态建议阈值（Context and Memory.md §3.2.3）：窗口越大建议越低——
/// 绝对 token 是二次方增长的，大窗口早压才省钱。
pub fn suggested_threshold(window_tokens: u64) -> u32 {
    match window_tokens {
        0..=16_000 => 30,       // 本地小模型
        16_001..=64_000 => 35,  // 中等窗口
        64_001..=256_000 => 25, // 大窗口
        _ => 15,                // 1M 及以上
    }
}

/// 压缩参数（⑨ 起配置驱动）。`threshold_tokens` 是绝对阈值——组装点由
/// 窗口 × 百分比算出（窗口未知回落 32k 保守保护，与 v2.14 行为一致）。
#[derive(Debug, Clone)]
pub struct CompactConfig {
    pub threshold_tokens: u64,
    pub strategy: CompactStrategy,
}

impl Default for CompactConfig {
    fn default() -> Self {
        Self {
            threshold_tokens: 32_000,
            strategy: CompactStrategy::default(),
        }
    }
}

/// 压缩结果：监听器写回 `RequestPayload::compacted`，kernel 据此落
/// `ContextCompacted` 事件。summary 存全文（日志折叠重建需要）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompactionOutcome {
    pub from_tokens: u64,
    pub to_tokens: u64,
    pub strategy: String,
    pub summary: String,
    pub kept_messages: usize,
}

/// 挂在 `agent/request` 上的压缩监听器。构造时持有 Context（运行时才解析
/// LlmService——装配时 LLM 适配器可能尚未注册）与共享配置锁。
pub struct CtxCompact {
    ctx: Arc<Context>,
    cfg: Arc<RwLock<CompactConfig>>,
}

impl CtxCompact {
    pub fn new(ctx: Arc<Context>, cfg: Arc<RwLock<CompactConfig>>) -> Self {
        Self { ctx, cfg }
    }

    /// 摘要直连 LlmService（不经决策环，无递归）；任何一步失败返回 None。
    async fn summarize(&self, head: &[Message], model: Option<String>) -> Option<String> {
        let llm = self.ctx.use_dyn::<dyn LlmService>().ok()?;
        let transcript = serde_json::to_string(head).ok()?;
        let summary_call = LlmRequest {
            messages: vec![Message::User {
                content: format!(
                    "Summarize the earlier conversation below into concise notes for \
                     continuing the work. Keep key facts, decisions, file paths and \
                     unfinished tasks. Reply with the summary only.\n\n<transcript>\n\
                     {transcript}"
                ),
            }],
            tools: vec![],
            model,
            params: Value::Object(serde_json::Map::new()),
        };
        let resp = llm.complete(summary_call).await.ok()?;
        let summary = resp.content.trim().to_string();
        (!summary.is_empty()).then_some(summary)
    }
}

#[async_trait]
impl WaterfallListener for CtxCompact {
    async fn handle(&self, key: &str, payload: &Value) -> WaterfallDecision {
        if key != points::REQUEST {
            return WaterfallDecision::Continue;
        }
        let Ok(mut req) = serde_json::from_value::<RequestPayload>(payload.clone()) else {
            return WaterfallDecision::Continue;
        };
        // 幂等守卫：payload 已带压缩结果（重试轮 / 其他决策方）就不再压
        if req.compacted.is_some() {
            return WaterfallDecision::Continue;
        }
        let cfg = self
            .cfg
            .read()
            .map(|c| c.clone())
            .unwrap_or_else(|_| CompactConfig::default());
        let from_tokens = estimate_tokens(&req.messages);
        if from_tokens <= cfg.threshold_tokens {
            return WaterfallDecision::Continue;
        }
        let target = (cfg.threshold_tokens / 2).max(1);
        let cut = cut_point(&req.messages, cfg.strategy, target);
        if cut == 0 {
            return WaterfallDecision::Continue; // 没有可压边界（不该硬压）
        }

        // 分层 / 仅摘要要 LLM 摘要；缺失或失败 → 就地降级滑动窗口（§3.5）
        let mut strategy = cfg.strategy;
        let mut summary = String::new();
        if strategy != CompactStrategy::SlidingWindow {
            match self
                .summarize(&req.messages[..cut], req.model.clone())
                .await
            {
                Some(s) => summary = s,
                None => {
                    tracing::warn!("compact summary unavailable, degrading to sliding window");
                    strategy = CompactStrategy::SlidingWindow;
                }
            }
        }

        let mut compacted: Vec<Message> = Vec::new();
        if !summary.is_empty() {
            compacted.push(Message::User {
                content: format!("[earlier conversation summarized]\n{summary}"),
            });
        }
        compacted.extend(req.messages[cut..].iter().cloned());
        let outcome = CompactionOutcome {
            from_tokens,
            to_tokens: estimate_tokens(&compacted),
            strategy: strategy.as_str().into(),
            kept_messages: req.messages.len() - cut,
            summary,
        };
        req.messages = compacted;
        req.compacted = Some(outcome);
        // Replace 是逐跳改写：终态决策仍是 Continue，改写结果在 payload 里
        // （cordis-core::waterfall 语义）。to_value 失败按 fail-open 放行。
        match serde_json::to_value(&req) {
            Ok(v) => WaterfallDecision::Replace(v),
            Err(_) => WaterfallDecision::Continue,
        }
    }
}

/// 粗估 token：全消息序列化字节 / 4（转义带来的高估当安全余量；CJK 偏
/// 低估，宁早压不爆仓）。runner 进度条与压缩判定共用同一把尺。
pub fn estimate_tokens(messages: &[Message]) -> u64 {
    messages.iter().map(estimate_one).sum()
}

fn estimate_one(m: &Message) -> u64 {
    serde_json::to_string(m).map_or(0, |s| s.len() as u64) / 4
}

/// 切点选择：返回把 `[0..cut)` 压掉的边界，0 = 不可压。
/// 滑动窗口 / 分层：按预算（目标 token）从尾部累计；仅摘要：切到最后
/// 一条 User（当前轮原文保留）。
fn cut_point(messages: &[Message], strategy: CompactStrategy, target: u64) -> usize {
    if strategy == CompactStrategy::SummaryOnly {
        let is_user = |m: &Message| matches!(m, Message::User { .. });
        let last = messages.iter().rposition(is_user).unwrap_or(0);
        if last == 0 {
            return 0; // 只剩当前轮：压无可压
        }
        return last;
    }
    budget_cut(messages, target)
}

/// 预算切点：从尾部往前累计到 target 为止，再回退到最近的 User 起点
/// （切在 User 之前不会拆散 tool_calls ↔ ToolResult 配对）。
fn budget_cut(messages: &[Message], target: u64) -> usize {
    let is_user = |m: &Message| matches!(m, Message::User { .. });
    let mut acc = 0u64;
    let mut i = messages.len();
    while i > 0 && acc < target {
        i -= 1;
        acc += estimate_one(&messages[i]);
    }
    if i == 0 {
        return 0; // 一轮就超预算：压无可压
    }
    if is_user(&messages[i]) {
        return i;
    }
    (0..i).rev().find(|&j| is_user(&messages[j])).unwrap_or(0)
}

#[cfg(test)]
mod tests;
