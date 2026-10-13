//! 记忆管理器（树 v2 §5/§6）：客户端契约 + 提示词两层目录（角色 × 模型
//! 家族，§6.2）+ 输出解析校验（不合规 → 规则切分 §5.4）+ 规则切分本体。
//!
//! 提示词以 `include_str!` 编译期内嵌：会话内逐字节稳定（§6.5 缓存约束），
//! 且不依赖数据根文件存在。`main` 角色是 M4 组装器预埋（影子运行不改
//! 组装，本批只有 recorder 被消费）。**摘要逐字保留纪律（§5.6）写死在
//! 提示词内**——用户给出的明确指令与关键决策必须引号原文保留。

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::chapter::{ChapterDraft, sanitize_draft};
use crate::material::MaterialItem;

use facts::split_facts_section;

/// 单次整理输入章节数上限（防失控输出；超出 = 不合规）。
const MAX_CHAPTERS: usize = 200;

/// 轮级总结的尾随标记（Organize Summary Design v1 §3.4）：记录员输出 =
/// JSON 数组 + 本标记 + 总结正文。数组契约不动——解析层先按标记切分再取
/// 数组，防总结正文里的 `[`/`]` 污染 `extract_array` 的首尾定位。
const SUMMARY_MARKER: &str = "===SUMMARY===";

/// 轮级总结硬钳字符数（提示词要 ≤500 字，这里防垃圾灌爆；≈600-1K token，
/// 滚动收编的尺寸预算顶）。
pub const RUN_SUMMARY_MAX_CHARS: usize = 2400;

// ---- 提示词两层目录（§6.2）：{role}/{family}.md，家族未命中回落 default ----

const MANAGER_DEFAULT: &str = include_str!("prompts/manager/default.md");
const MANAGER_CLAUDE: &str = include_str!("prompts/manager/claude.md");
const MANAGER_DEEPSEEK: &str = include_str!("prompts/manager/deepseek.md");
const MANAGER_QWEN: &str = include_str!("prompts/manager/qwen.md");
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

/// 记忆管理器提示词（家族未命中回落通用版）。
pub fn manager_prompt(family: &str) -> &'static str {
    match family {
        "claude" => MANAGER_CLAUDE,
        "deepseek" => MANAGER_DEEPSEEK,
        "qwen" => MANAGER_QWEN,
        _ => MANAGER_DEFAULT,
    }
}

/// **定性 = 预留待接，不是死码也不是缺陷**（队列 Q7 · v2.191 标注，形态照 `session-store`
/// 的 `snapshot.rs` v2.148 批 D 先例）：`family_of` 的四个家族在这里各带一份
/// `prompts/main/*.md`，而消费方 `ContextAssembler`（记忆树路线 M4）**至今不存在**，四份 md
/// 从未被任何生产代码读过。**预留件清单**：main_prompt
///
/// 批 Q7 取证时《Context Budget Closure》§7.6 把这条列进"死代码清理"，实测它不是坏掉的实现
/// 而是未接线的预留件 ⇒ 保留 + 标注，由 `scripts/check-reserved.mjs` 盯住标注与事实一致：
/// 清单里的件一旦有了生产调用点而标注没摘 = 判红，标注还在而清单里的件被删光 = 也判红
/// （「以预留之名长期挂着没人复验」和「接了线还标预留」是同一种说谎的两个方向）。
/// 接上的那天：M4 组装器按 `family_of` 取词，并复看这四份提示词是否还符合当时的规范正文。
pub fn main_prompt(family: &str) -> &'static str {
    match family {
        "claude" => MAIN_CLAUDE,
        "deepseek" => MAIN_DEEPSEEK,
        "qwen" => MAIN_QWEN,
        _ => MAIN_DEFAULT,
    }
}

/// 记忆管理器用量（成本观测 §四判据；供应商不报告 = 0，如实呈现）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryManagerUsage {
    pub input: u64,
    pub output: u64,
}

/// 记忆管理器调用契约：桌面端用 AppLlm 实现（超时/重试在编排层）；测试用
/// 假实现驱动降级路径。system = 角色提示词，user = 转录材料。
#[async_trait]
pub trait MemoryManagerClient: Send + Sync {
    async fn complete(
        &self,
        system: &str,
        user: &str,
        model: Option<&str>,
    ) -> Result<(String, MemoryManagerUsage), String>;
}

/// 记忆管理器输出不合规（§5.4 降级判据）。
#[derive(Debug, Clone, PartialEq)]
pub enum MemoryManagerError {
    /// 没有可解析的 JSON 数组。
    NoJson,
    /// 数组里有不合法条目（缺字段 / 空标题 / 区间倒置）。
    BadEntry(String),
    /// 区间不能完整铺满材料（缺口 / 重叠 / 越界）。
    Coverage(String),
    TooManyChapters,
}

impl std::fmt::Display for MemoryManagerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoJson => write!(f, "输出中没有 JSON 数组"),
            Self::BadEntry(e) => write!(f, "条目不合法: {e}"),
            Self::Coverage(e) => write!(f, "区间未铺满材料: {e}"),
            Self::TooManyChapters => write!(f, "章节数超上限 {MAX_CHAPTERS}"),
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawDraft {
    title: String,
    summary: String,
    /// 记忆管理器的切分空间 = 材料项下标（含首尾）。
    event_range: (usize, usize),
}

/// 从原始回复提取第一个 JSON 数组文本（容忍 ```json 围栏与前后缀说明）。
pub(crate) fn extract_array(raw: &str) -> Option<&str> {
    let start = raw.find('[')?;
    let end = raw.rfind(']')?;
    (end > start).then_some(&raw[start..=end])
}

/// 按尾随标记切出「数组部分 | 总结部分」。无标记 = 全部是数组部分、无总结。
///
/// ⚠ 先剥 `===FACTS===` 再剥 `===SUMMARY===`（顺序承重，见 `FACTS_MARKER` 注释）：
/// 卡段总在总结段之后，若不先摘掉，总结部分就会把卡数组一起吞进去。
/// **无卡段时本函数的输出与加卡段之前逐字节相同**——那条等价判据钉的就是这一点。
fn split_summary_section(raw: &str) -> (&str, Option<&str>) {
    let (head, _) = split_facts_section(raw);
    match head.split_once(SUMMARY_MARKER) {
        Some((before, after)) => (before, Some(after)),
        None => (head, None),
    }
}

/// 提取轮级总结正文（Organize Summary Design v1 §3.4）：标记后 trim +
/// 空白行折叠 + 硬钳。无标记 / 空正文 = None（成功无总结，目录照常——
/// 降级路径宁缺勿假，与规则切分无总结同一口径）。
pub fn run_summary(raw: &str) -> Option<String> {
    let after = split_summary_section(raw).1?;
    let mut text = after.trim();
    if text.is_empty() {
        return None;
    }
    // 压掉连续空白行（树 md/转录投影按行渲染，冗余空行只添噪）
    let mut collapsed = String::with_capacity(text.len());
    let mut prev_blank = false;
    for line in text.lines() {
        let blank = line.trim().is_empty();
        if !(blank && prev_blank) {
            collapsed.push_str(line.trim_end());
            collapsed.push('\n');
        }
        prev_blank = blank;
    }
    text = collapsed.trim();
    if text.is_empty() {
        return None;
    }
    Some(text.chars().take(RUN_SUMMARY_MAX_CHARS).collect())
}

/// 解析并校验记忆管理器输出（§4.6/§5.4）：每章节 title/summary 净化后成草稿；
/// 切分区间必须按材料项下标**完整铺满** `[0..n)` 且不重叠不越界——任何
/// 一条不满足整批判垃圾（调用方转规则切分，绝不半信半疑落树）。
/// 成功时把项下标翻回会话事件位（L2 引用锚点）。**只吃数组部分**——
/// 尾随总结段（`===SUMMARY===` 之后）由 `run_summary` 单独取，防总结
/// 正文里的 `[`/`]` 污染数组首尾定位。
pub fn parse_drafts(
    raw: &str,
    items: &[MaterialItem],
) -> Result<Vec<ChapterDraft>, MemoryManagerError> {
    let (array_part, _) = split_summary_section(raw);
    let arr_text = extract_array(array_part).ok_or(MemoryManagerError::NoJson)?;
    let arr: Vec<RawDraft> =
        serde_json::from_str(arr_text).map_err(|e| MemoryManagerError::BadEntry(e.to_string()))?;
    if arr.len() > MAX_CHAPTERS {
        return Err(MemoryManagerError::TooManyChapters);
    }
    if arr.is_empty() {
        return Err(MemoryManagerError::Coverage("空数组".into()));
    }
    let mut drafts = Vec::with_capacity(arr.len());
    let mut expect = 0usize;
    for (i, r) in arr.iter().enumerate() {
        let (a, b) = r.event_range;
        if a != expect || b < a {
            return Err(MemoryManagerError::Coverage(format!(
                "第 {i} 章节区间 [{a},{b}] 与预期起点 {expect} 不衔接"
            )));
        }
        // **越界必须是 Err 而不是 panic**（v2.187 / 队列 Q3b 逼出来的第二条）：下面
        // 那句 `items[b].event` 直接按下标取，模型给一个超出材料数的区间就当场
        // index out of bounds。这不是假设中的风险——本批把材料从"全量事件"改成"尾巴
        // 切片"之后，第二次整理的 items 只剩新增那一轮（2 项），而记录员照旧可能按
        // 它看到的整段语境给出 `[0,5]` ⇒ 既有测试当场 panic 在这一行。既然「模型输出
        // 不可信」是 §5.4 降级设计的第一前提，校验层就不许存在能被打穿的下标访问；
        // 返回 Coverage 后由重试 → 规则切分那张既有安全网接住。
        if b >= items.len() {
            return Err(MemoryManagerError::Coverage(format!(
                "第 {i} 章节区间 [{a},{b}] 越出材料（共 {} 项）",
                items.len()
            )));
        }
        expect = b + 1;
        let draft = sanitize_draft(&r.title, &r.summary, (items[a].event, items[b].event))
            .ok_or_else(|| MemoryManagerError::BadEntry(format!("第 {i} 章节标题/摘要为空")))?;
        drafts.push(draft);
    }
    if expect != items.len() {
        return Err(MemoryManagerError::Coverage(format!(
            "区间止于 {expect}，未铺满 {n} 项",
            n = items.len()
        )));
    }
    Ok(drafts)
}

/// 规则切分（§5.4 降级，复用现网 ctx-compact 的"切点落在 User 消息之前"
/// 纪律——不拆散 tool_calls ↔ ToolResult 配对）：按 `chunk_tokens` 估算
/// 累计，跨过预算后的**下一条用户消息**处断章节；标题 = 该章节首条用户消息
/// 前 20 字；摘要 = 机械说明 + 首条消息原文截断。
pub fn rule_split_drafts(items: &[MaterialItem], chunk_tokens: u64) -> Vec<ChapterDraft> {
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
fn draft_from_items(items: &[MaterialItem]) -> ChapterDraft {
    let first_user = items
        .iter()
        .find(|i| i.kind == crate::material::MaterialKind::User);
    let title_src = first_user.map_or(&items[0].text, |i| &i.text);
    let title: String = title_src.trim().chars().take(20).collect();
    let summary = match first_user {
        Some(u) => format!(
            "（规则切分）本章节涵盖事件 {}–{}。首条用户消息：{}",
            items[0].event,
            items[items.len() - 1].event,
            u.text.chars().take(120).collect::<String>()
        ),
        None => format!(
            "（规则切分）本章节涵盖事件 {}–{}（无用户消息段）。",
            items[0].event,
            items[items.len() - 1].event
        ),
    };
    ChapterDraft {
        title,
        summary,
        event_range: (items[0].event, items[items.len() - 1].event),
    }
}

/// 会话切片里的用户消息数已收回**一个出处**（队列 Q7 · CB §7.6 P3）：这里原先留着一个
/// `pub fn count_user_messages = material::user_count` 的转发别名，注释写着「编排层反弹账本
/// 用」——实测编排层四处（`organize/trigger.rs` ×3、`organize/recovery.rs`）调的全是
/// `memory_tree::user_count`，**注释描述的那个使用者从未存在**。一个零调用点还带着假出处的
/// 转发，比零调用点更糟：它让下一个读者以为删了会坏事。
mod facts;

pub use facts::{FACT_TYPES, FACT_VALUE_MAX_CHARS, FactCard, fact_cards, fact_request};

#[cfg(test)]
mod tests;
