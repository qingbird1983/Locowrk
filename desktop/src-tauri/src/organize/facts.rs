//! 整理副产物 = **事实卡落库**（MR 稿 §5 提案 A · 后审计队列 Q3 · v2.186）。
//!
//! **这块治什么**：v2.136 退役 autoExtract 之后，事实层（`memories` 表）唯一的
//! 写入路径是模型主动 `memory_write`，而稿 §0-1 的实证是那条路**一次都没走过**
//! （本机 0 行）。用户 2026-10-12 裁定走提案 A：不再造提取管线，而是**复用现役
//! 整理管道**——记录员本来就在读整段对话、本来就在产出章节与轮级总结，让它顺带
//! 多吐 ≤N 张事实卡。触发/重试/降级/熔断全部白得，零新增调度器、零新增常驻管线。
//!
//! **这是 v2.136 裁定的部分反转，且反转面被刻意压到最小**：退役的是「每轮从
//! 对话自动提取事实」，本件是「树整理收束时的登记副产物」，频率 = 整理频率不是
//! 轮频率；设置 `memory.factCards` 可整面关掉（关掉 = 提示词逐字节回到现役形状，
//! 一张都不落库）。裁定变更留痕写进 CHANGELOG 与《Context and Memory》。
//!
//! 三条不是审美而是实测的形状约束：
//! 1. **`type` 只用来归层，不落库**——`memories` 表没有 type 列，加列属 schema
//!    迁移（不在本批，零迁移纪律）。卡的**出处**由 `source = "organized"` 记录，
//!    前端 `SourceBadge` 单独一档（用户必须看得出这条不是我写的）。
//! 2. **卡段必须在总结段之后被先剥掉**（解析层的顺序承重，见
//!    `memory_tree::manager::FACTS_MARKER`）。
//! 3. **落库失败不许反噬整理结果**——章节与总结已经落树并通知过前端，卡是
//!    副产物；这里任何一条 `write` 失败只 warn 计数，管道照原样返回成功。
//!    （同 Q1b「观测不许拖垮被观测者」那条纪律，方向相反但同源：附加面不许
//!    把主面拖倒。）

use std::collections::HashSet;

use memory_tree::FactCard;

use crate::memory::MemoryStore;

/// 落库出处标记（`memories.source` 的第三个取值；前两档 = `explicit` /
/// `extracted`，后者是 v2.136 退役前的历史遗留）。
pub(crate) const SOURCE_ORGANIZED: &str = "organized";
/// 给记录员的「现有 key 清单」上限（层合算）。稿 §5 要求注入清单供查重
/// （ZCode manifest 同款），但清单本身进的是**整理请求的 user 段**，
/// 与注入预算一样得有顶；200 与 §4.1 的索引行数上限同数，不是新常量族。
pub(crate) const MAX_EXISTING_KEYS: usize = 200;

/// 卡片归层（稿 §3 映射规则的唯一实现处）：`user | feedback → 全局层（""）`、
/// `project | reference → 当前项目键位`。返回 `None` = 类型不在白名单。
///
/// ⚠ 这里返回的是**落库键位**（已过归一），不是原始 workspace 串：全局层固定
/// 空串，项目层 = `workspace::scope_key` 的归一形。落库与清单读取必须用同一个
/// 串，否则「同项目双键位」会从这条新路径长回来（Q2 的欠账由本批 `write()` 归一
/// 一并兑现，这里再归一次是把「归一发生在边界」写成可核对的形状）。
pub(crate) fn layer_of(card_type: &str, workspace: &str) -> Option<String> {
    match card_type {
        "user" | "feedback" => Some(String::new()),
        "project" | "reference" => {
            let ws = crate::workspace::scope_key(Some(workspace));
            // 未归属会话（键位空串）没有项目层可写：宁可不落，也不许把项目事实
            // 塞进全局偏好层（与 v2.68「未归属缺省写拒绝」同一口径，这里取"丢弃"
            // 是因为这是副产物、不该为了它把整理结果挂上）。
            (!ws.is_empty()).then_some(ws)
        }
        _ => None,
    }
}

/// 读现有 key 清单（全局层 + 本项目层），输出**带层标**的行（`key (project)`）。
///
/// 带层标不是装饰：同名 key 允许在两层各存一行（主键含 workspace），模型若看
/// 不到层，就会把一条项目事实标成 `user` 去"更新"全局行 —— 那正是我们最不想要
/// 的污染。读不到 = 空清单（提示词写「（暂无）」），**不把"读不到"说成"没货"**
/// （Q1a 那条口径同源）。
pub(crate) fn existing_keys(store: &MemoryStore, workspace: &str) -> Vec<String> {
    let ws = crate::workspace::scope_key(Some(workspace));
    let mut out: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for (side, label) in [("", "global"), (ws.as_str(), "project")] {
        if side.is_empty() && label == "project" {
            // 未归属会话：项目层不存在，别把全局行报成项目行
            continue;
        }
        match store.index_rows(side, MAX_EXISTING_KEYS) {
            Ok(rows) => {
                for r in rows {
                    let line = format!("{} ({label})", r.key);
                    if seen.insert(line.clone()) {
                        out.push(line);
                    }
                }
            }
            Err(e) => tracing::warn!("事实卡查重清单读取失败（{label} 侧）: {e}"),
        }
    }
    out.truncate(MAX_EXISTING_KEYS);
    out
}

/// 一次落库的账（判据与 warn 用；不进前端事件负载——稿没要求观测面，
/// 卡片可见性走管理页的 source 档）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct FactReport {
    /// 成功落库（含 upsert 覆盖既有 key）的张数
    pub(crate) written: usize,
    /// 解析层丢弃的张数（类型非法 / key 或 value 为空 / 超限截断）
    pub(crate) dropped: usize,
    /// 落库失败的张数（写库报错，不反噬整理）
    pub(crate) failed: usize,
}

/// 解析 + 归层 + 落库。**只由 recorder 成功支调用**：规则切分降级路径不产卡
/// （稿 §5「降级语义：产不出就不产，诚实缺省，不硬造」），所以这里收到的
/// `raw` 永远是"格式契约被满足过"的那次回复。
pub(crate) fn persist(store: &MemoryStore, workspace: &str, raw: &str, limit: usize) -> FactReport {
    let cards: Vec<FactCard> = memory_tree::fact_cards(raw, limit);
    let mut rep = FactReport::default();
    // 解析层的丢弃数 = 卡段里的条目数 - 取回的条目数（只在有条目时才算，
    // 免得"没写标记"这种正常情况被记成"丢了 N 张"）
    for card in cards {
        let Some(ws) = layer_of(&card.card_type, workspace) else {
            rep.dropped += 1;
            continue;
        };
        match store.write(&ws, &card.key, &card.value, &card.tags, SOURCE_ORGANIZED) {
            Ok(()) => rep.written += 1,
            Err(e) => {
                rep.failed += 1;
                tracing::warn!(
                    session_ws = %ws,
                    key = %card.key,
                    "整理事实卡落库失败（不影响已落树的章节与总结）: {e}"
                );
            }
        }
    }
    if rep.written > 0 || rep.failed > 0 || rep.dropped > 0 {
        tracing::debug!("事实卡落库 {rep:?}（上限 {limit}）");
    }
    rep
}

#[cfg(test)]
mod tests;
