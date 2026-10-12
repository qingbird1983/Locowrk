//! 工具目录的打分选择（exposure 文档 §5.3 / §6.2）：把"这一轮该暴露哪些工具"从
//! `tool_exposure` 的监听器本体里搬出来单住一处——本簇是**纯函数**（不碰配置、
//! 不碰钉死缓存、不做 IO），所以能被单测直接喂目录，也是审计 M-24c 唯一改了
//! 数据形状的地方（返回**下标**而不是克隆出来的 `ToolSpec`）。
//!
//! 等值证明 = `../tests/scoring.rs` 的 `select_by_index_equals_old_cloning_select`
//! （把被删的旧「克隆版 select」逐字抄成参考实现，在参差目录 × 同分目录 × 会话热度
//! × EWMA × 检索 × max_tools 的网格上逐格对账 kept / always / 三归因），以及
//! `reorder_falls_back_to_clone_on_duplicate_index`（钉死表重复指向同一条时的退化路径）。

use std::collections::HashMap;

use plugin_api::{Exposure, ToolSpec};
use session_store::Message;

use crate::middleware::tool_retrieval::RETRIEVAL_WEIGHT;

use super::{GLOBAL_WEIGHT, SESSION_WEIGHT};

/// 一次选择的结果（纯函数产出，单测友好）。`kept_idx` = 保留项在目录里的
/// **下标**（审计 M-24c：不再克隆 `ToolSpec`，调用方按下标把元素移动出去）。
pub(super) struct Selection {
    pub(super) kept_idx: Vec<usize>,
    pub(super) always: Vec<String>,
    pub(super) session_hot: Vec<String>,
    pub(super) global_hot: Vec<String>,
    pub(super) retrieved: Vec<String>,
}

/// 会话内各工具的调用次数：从投影消息数 `ToolResult`（§3.5——调用意图
/// 以闭合结果计，错误结果也算用过；回填丢失工具名的悬空条目不计）。
pub(super) fn session_counts(messages: &[Message]) -> HashMap<String, u32> {
    let mut counts: HashMap<String, u32> = HashMap::new();
    for m in messages {
        if let Message::ToolResult { tool, .. } = m {
            if !tool.is_empty() {
                *counts.entry(tool.clone()).or_insert(0) += 1;
            }
        }
    }
    counts
}

/// 检索查询（§4.2：最近 3 条用户消息合并；空会话返回空串 → 调用方跳过）。
pub(super) fn retrieval_query(messages: &[Message]) -> String {
    let mut parts: Vec<&str> = messages
        .iter()
        .filter_map(|m| match m {
            Message::User { content, .. } => Some(content.as_str()),
            _ => None,
        })
        .rev()
        .take(3)
        .collect();
    parts.reverse();
    let joined = parts.join("\n");
    // 嵌入输入设上界：查询只是信号源，不是全文检索
    joined.chars().take(600).collect()
}

/// 带下标的目录项 = `(目录内下标, spec 引用)`（[`select`] 分堆用）。别名有两个用处：
/// 给这对组合一个名字，并让 clippy 的 `type_complexity` 不再盯着分堆那行的嵌套元组
/// （`(Vec<(usize, &ToolSpec)>, Vec<(usize, &ToolSpec)>)` 实测判红 `very complex type`）。
type IndexedSpec<'a> = (usize, &'a ToolSpec);

/// 打分选择（§6.2）：always 无条件入选；候选按 `会话 1.0 + 全局 0.7 ×
/// ewma + 检索 0.5 × similarity` 排序取前 `quota = max_tools - always`
/// 个。同分保持目录原序（stable sort，目录序 = 注册表确定性序）。选中
/// 归因：会话用过记 session_hot，否则全局分 > 0 记 global_hot，否则检索
/// 分 > 0 记 retrieved，纯补位的不归组。
pub(super) fn select(
    catalog: &[ToolSpec],
    session: &HashMap<String, u32>,
    ewma: &HashMap<String, f64>,
    retrieval: &HashMap<String, f64>,
    max_tools: usize,
) -> Selection {
    // 带下标枚举后再分堆：`kept_idx` 是**下标**而不是克隆出来的 spec（M-24c）。
    // `partition` 与 `sort_by` 都保持输入序 ⇒ 同分仍是**目录原序**（stable sort，
    // 目录序 = 注册表确定性序），与改造前逐格一致。
    let (always, candidates): (Vec<IndexedSpec<'_>>, Vec<IndexedSpec<'_>>) = catalog
        .iter()
        .enumerate()
        .partition(|(_, s)| matches!(s.exposure, Exposure::Always));
    let quota = max_tools.saturating_sub(always.len());

    let mut scored: Vec<(usize, &ToolSpec, f64)> = candidates
        .into_iter()
        .map(|(i, s)| {
            let hit = session.contains_key(&s.name);
            let score = SESSION_WEIGHT * u32::from(hit) as f64
                + GLOBAL_WEIGHT * ewma.get(&s.name).copied().unwrap_or(0.0)
                + RETRIEVAL_WEIGHT * retrieval.get(&s.name).copied().unwrap_or(0.0);
            (i, s, score)
        })
        .collect();
    scored.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

    let mut kept_idx: Vec<usize> = always.iter().map(|(i, _)| *i).collect();
    let always: Vec<String> = always.iter().map(|(_, s)| s.name.clone()).collect();
    let mut session_hot = Vec::new();
    let mut global_hot = Vec::new();
    let mut retrieved = Vec::new();
    for (i, spec, _score) in scored.into_iter().take(quota) {
        if session.contains_key(&spec.name) {
            session_hot.push(spec.name.clone());
        } else if ewma.get(&spec.name).copied().unwrap_or(0.0) > 0.0 {
            global_hot.push(spec.name.clone());
        } else if retrieval.get(&spec.name).copied().unwrap_or(0.0) > 0.0 {
            retrieved.push(spec.name.clone());
        }
        kept_idx.push(i);
    }
    Selection {
        kept_idx,
        always,
        session_hot,
        global_hot,
        retrieved,
    }
}

/// 按下标重排并**移动**出元素（审计 M-24c：[`select`] 返回下标而不是克隆 `ToolSpec`；
/// 目录没被裁剪时旧写法要把整个目录连 schema 克隆一遍再赋回 `req.tools`）。
/// 下标重复（目录里出现重名工具、钉死表因此重复指向同一条）时退化为逐个克隆，
/// 与旧写法逐格同值；正常情况下每个元素只被移动一次 = 零克隆。
pub(super) fn reorder(tools: &mut Vec<ToolSpec>, idx: &[usize]) -> Vec<ToolSpec> {
    let mut seen = vec![false; tools.len()];
    if idx.iter().any(|&i| std::mem::replace(&mut seen[i], true)) {
        return idx.iter().map(|&i| tools[i].clone()).collect();
    }
    let mut slots: Vec<Option<ToolSpec>> = std::mem::take(tools).into_iter().map(Some).collect();
    idx.iter()
        .map(|&i| slots[i].take().expect("下标已去重，槽位必有值"))
        .collect()
}
