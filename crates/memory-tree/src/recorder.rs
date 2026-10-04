//! 会话记录员（树 v2 §5/§6）：客户端契约 + 提示词两层目录（角色 × 模型
//! 家族，§6.2）+ 输出解析校验（不合规 → 规则切分 §5.4）+ 规则切分本体。
//!
//! 提示词以 `include_str!` 编译期内嵌：会话内逐字节稳定（§6.5 缓存约束），
//! 且不依赖数据根文件存在。`main` 角色是 M4 组装器预埋（影子运行不改
//! 组装，本批只有 recorder 被消费）。**摘要逐字保留纪律（§5.6）写死在
//! 提示词内**——用户给出的明确指令与关键决策必须引号原文保留。

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use session_store::SessionEvent;

use crate::bundle::{BundleDraft, sanitize_draft};
use crate::material::MaterialItem;

/// 单次整理输入捆数上限（防失控输出；超出 = 不合规）。
const MAX_BUNDLES: usize = 200;

// ---- 提示词两层目录（§6.2）：{role}/{family}.md，家族未命中回落 default ----

const RECORDER_DEFAULT: &str = include_str!("prompts/recorder/default.md");
const RECORDER_CLAUDE: &str = include_str!("prompts/recorder/claude.md");
const RECORDER_DEEPSEEK: &str = include_str!("prompts/recorder/deepseek.md");
const RECORDER_QWEN: &str = include_str!("prompts/recorder/qwen.md");
const MAIN_DEFAULT: &str = include_str!("prompts/main/default.md");
const MAIN_CLAUDE: &str = include_str!("prompts/main/claude.md");
const MAIN_DEEPSEEK: &str = include_str!("prompts/main/deepseek.md");
const MAIN_QWEN: &str = include_str!("prompts/main/qwen.md");

/// 模型名 → 家族（提示词选择用）：子串匹配，其余回落 default。
pub fn family_of(model: &str) -> &'static str {
    let m = model.to_ascii_lowercase();
    if m.contains("claude") {
        "claude"
    } else if m.contains("deepseek") {
        "deepseek"
    } else if m.contains("qwen") || m.contains("qwq") {
        "qwen"
    } else {
        "default"
    }
}

/// 记录员提示词（家族未命中回落通用版）。
pub fn recorder_prompt(family: &str) -> &'static str {
    match family {
        "claude" => RECORDER_CLAUDE,
        "deepseek" => RECORDER_DEEPSEEK,
        "qwen" => RECORDER_QWEN,
        _ => RECORDER_DEFAULT,
    }
}

/// 主模型提示词（M4 组装器消费；本批仅供 loader 契约完整）。
pub fn main_prompt(family: &str) -> &'static str {
    match family {
        "claude" => MAIN_CLAUDE,
        "deepseek" => MAIN_DEEPSEEK,
        "qwen" => MAIN_QWEN,
        _ => MAIN_DEFAULT,
    }
}

/// 记录员用量（成本观测 §四判据；供应商不报告 = 0，如实呈现）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecorderUsage {
    pub input: u64,
    pub output: u64,
}

/// 记录员调用契约：桌面端用 AppLlm 实现（超时/重试在编排层）；测试用
/// 假实现驱动降级路径。system = 角色提示词，user = 转录材料。
#[async_trait]
pub trait RecorderClient: Send + Sync {
    async fn complete(
        &self,
        system: &str,
        user: &str,
        model: Option<&str>,
    ) -> Result<(String, RecorderUsage), String>;
}

/// 记录员输出不合规（§5.4 降级判据）。
#[derive(Debug, Clone, PartialEq)]
pub enum RecorderError {
    /// 没有可解析的 JSON 数组。
    NoJson,
    /// 数组里有不合法条目（缺字段 / 空标题 / 区间倒置）。
    BadEntry(String),
    /// 区间不能完整铺满材料（缺口 / 重叠 / 越界）。
    Coverage(String),
    TooManyBundles,
}

impl std::fmt::Display for RecorderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoJson => write!(f, "输出中没有 JSON 数组"),
            Self::BadEntry(e) => write!(f, "条目不合法: {e}"),
            Self::Coverage(e) => write!(f, "区间未铺满材料: {e}"),
            Self::TooManyBundles => write!(f, "捆数超上限 {MAX_BUNDLES}"),
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawDraft {
    title: String,
    summary: String,
    /// 记录员的切分空间 = 材料项下标（含首尾）。
    event_range: (usize, usize),
}

/// 从原始回复提取第一个 JSON 数组文本（容忍 ```json 围栏与前后缀说明）。
fn extract_array(raw: &str) -> Option<&str> {
    let start = raw.find('[')?;
    let end = raw.rfind(']')?;
    (end > start).then_some(&raw[start..=end])
}

/// 解析并校验记录员输出（§4.6/§5.4）：每捆 title/summary 净化后成草稿；
/// 切分区间必须按材料项下标**完整铺满** `[0..n)` 且不重叠不越界——任何
/// 一条不满足整批判垃圾（调用方转规则切分，绝不半信半疑落树）。
/// 成功时把项下标翻回会话事件位（L2 引用锚点）。
pub fn parse_drafts(raw: &str, items: &[MaterialItem]) -> Result<Vec<BundleDraft>, RecorderError> {
    let arr_text = extract_array(raw).ok_or(RecorderError::NoJson)?;
    let arr: Vec<RawDraft> =
        serde_json::from_str(arr_text).map_err(|e| RecorderError::BadEntry(e.to_string()))?;
    if arr.len() > MAX_BUNDLES {
        return Err(RecorderError::TooManyBundles);
    }
    if arr.is_empty() {
        return Err(RecorderError::Coverage("空数组".into()));
    }
    let mut drafts = Vec::with_capacity(arr.len());
    let mut expect = 0usize;
    for (i, r) in arr.iter().enumerate() {
        let (a, b) = r.event_range;
        if a != expect || b < a {
            return Err(RecorderError::Coverage(format!(
                "第 {i} 捆区间 [{a},{b}] 与预期起点 {expect} 不衔接"
            )));
        }
        expect = b + 1;
        let draft = sanitize_draft(&r.title, &r.summary, (items[a].event, items[b].event))
            .ok_or_else(|| RecorderError::BadEntry(format!("第 {i} 捆标题/摘要为空")))?;
        drafts.push(draft);
    }
    if expect != items.len() {
        return Err(RecorderError::Coverage(format!(
            "区间止于 {expect}，未铺满 {n} 项",
            n = items.len()
        )));
    }
    Ok(drafts)
}

/// 规则切分（§5.4 降级，复用现网 ctx-compact 的"切点落在 User 消息之前"
/// 纪律——不拆散 tool_calls ↔ ToolResult 配对）：按 `chunk_tokens` 估算
/// 累计，跨过预算后的**下一条用户消息**处断捆；标题 = 该捆首条用户消息
/// 前 20 字；摘要 = 机械说明 + 首条消息原文截断。
pub fn rule_split_drafts(items: &[MaterialItem], chunk_tokens: u64) -> Vec<BundleDraft> {
    let chunk = chunk_tokens.max(1);
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut acc = 0u64;
    let mut cut_at: Option<usize> = None; // 预算超时后的下一处 User 下标（断点）
    for (i, it) in items.iter().enumerate() {
        acc += it.tokens;
        if acc >= chunk && cut_at.is_none() {
            // 断点取**当前项之后**的第一条用户消息——切点永远落在 User
            // 之前且不在已累计内容中间；其后再无 User = 切到末尾。
            cut_at = Some(
                items[i + 1..]
                    .iter()
                    .position(|x| x.kind == crate::material::MaterialKind::User)
                    .map_or(items.len(), |p| i + 1 + p),
            );
        }
        if let Some(cut) = cut_at {
            if i + 1 == cut {
                out.push(draft_from_items(&items[start..=i]));
                start = cut;
                acc = 0;
                cut_at = None;
            }
        }
    }
    if start < items.len() {
        out.push(draft_from_items(&items[start..]));
    }
    out
}

/// 机械草稿：标题取首条用户消息前 20 字（无用户消息 = 首项文本截 20），
/// 摘要如实标注规则切分 + 首条用户消息原文（截 120）。
fn draft_from_items(items: &[MaterialItem]) -> BundleDraft {
    let first_user = items
        .iter()
        .find(|i| i.kind == crate::material::MaterialKind::User);
    let title_src = first_user.map_or(&items[0].text, |i| &i.text);
    let title: String = title_src.trim().chars().take(20).collect();
    let summary = match first_user {
        Some(u) => format!(
            "（规则切分）本捆涵盖事件 {}–{}。首条用户消息：{}",
            items[0].event,
            items[items.len() - 1].event,
            u.text.chars().take(120).collect::<String>()
        ),
        None => format!(
            "（规则切分）本捆涵盖事件 {}–{}（无用户消息段）。",
            items[0].event,
            items[items.len() - 1].event
        ),
    };
    BundleDraft {
        title,
        summary,
        event_range: (items[0].event, items[items.len() - 1].event),
    }
}

/// 会话切片里的用户消息数（编排层反弹账本用；material 同函数的转发，
/// 放这里省一层 import）。
pub fn count_user_messages(events: &[SessionEvent]) -> u64 {
    crate::material::user_count(events)
}

#[cfg(test)]
mod tests;
