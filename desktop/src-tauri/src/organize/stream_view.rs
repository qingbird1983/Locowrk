//! 会话流读取（队列 Q14 · MB《Memory Tree Board Design v1》§2.3）：看板第 4 列
//! 「完整会话流」的数据源——**一次 IPC** 把某一轮总结区间内全部已定稿章节的原文
//! 按事件序汇成一块原子视图。
//!
//! 为什么在后端拼装而不是前端循环调 N 次 `memory_tree_original`：三条理由，缺一条
//! 就不该动这一刀——① 行号坐标系必须全局连续（前端自己拼只能每节从 0 重编，
//! 「滚到第 37 条消息」这种定位语义就没了）；② 一次 IPC = 原子视图，循环 N 次期间
//! 投影被整理改写会拼出半新半旧的组；③ N 次调用 = N 次整段会话日志重读
//! （`tree_original` 每次都 `load_session` 读一遍盘）。
//!
//! 复用而非重写：区间过滤走 `view::chapters_in`、切片走 `view::rows_between`
//! （与单章读取同一条通路）——同一属性只许一个出处，本簇零切片逻辑。

use serde::Serialize;

use memory_tree::MemoryChapter;

use super::view::{chapter_seq, chapters_in, rows_between, settled, OriginalItem};
use super::Organizer;

/// 未归总结虚拟组的组键（MB §2.2 第 1 条：规则切分降级轮的章节区间不落任何总结，
/// 给一个虚拟组兜住，画布不空）。
pub const ORPHAN_GROUP: &str = "orphan";

/// 会话流小节（一节 = 一个章节的原文切片；`rows` 的 `index` 是会话日志绝对事件位，
/// 跨节连续，与 `memory_tree_original` 同一坐标系）。
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StreamSection {
    pub chapter_id: String,
    pub title: String,
    /// recorder / rule / placeholder（如实透出，同 `ChapterView.source`）。
    pub source: String,
    /// 该章有人改过（标题或摘要任一手动标记）。流面板小节头只挂一枚「手动」徽章，
    /// 两字段在这里塌缩成一个布尔；要逐字段区分请读 `memory_tree_chapters`。
    pub edited: bool,
    pub from: usize,
    pub to: usize,
    pub rows: Vec<OriginalItem>,
    /// 本节切片读取失败的原因（MB §3.3「某章节切片加载失败 → 该小节行内错误文案」）。
    /// Some = 本节 rows 为空、其余小节照常给出——一节坏掉不撤整组，也不把「读不到」
    /// 演成「这一节没有内容」。
    pub error: Option<String>,
}

/// 整组会话流（看板第 4 列一块面板）。
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StreamView {
    /// 按章节事件区间**升序** = 事件序（会话流的次序契约）。
    pub sections: Vec<StreamSection>,
    /// 组覆盖的事件区间。`sections` 为空时两者是 0——判空态看 `sections` 的长度，
    /// 别看这两个数（空组与「区间恰好 0..0」在数上分不开，这是刻意的：区间不是身份）。
    pub from: usize,
    pub to: usize,
    pub total_rows: usize,
}

/// 组和键。`RunSummary` 没有独立 id，整理区间就是它的身份 ⇒ 组键直接用 `from:to`
/// （MB §2.3 的 group 语义）。
enum StreamGroup {
    Run(usize, usize),
    Orphan,
}

impl StreamGroup {
    fn parse(raw: &str) -> Result<Self, String> {
        if raw == ORPHAN_GROUP {
            return Ok(Self::Orphan);
        }
        let bad = || format!("组和键形状不对（应为 \"orphan\" 或 \"起点:终点\"）：{raw}");
        let (a, b) = raw.split_once(':').ok_or_else(bad)?;
        let from = a.trim().parse::<usize>().map_err(|_| bad())?;
        let to = b.trim().parse::<usize>().map_err(|_| bad())?;
        if from > to {
            return Err(format!("组和键区间起点大于终点：{raw}"));
        }
        Ok(Self::Run(from, to))
    }
}

/// 组成员覆盖的事件区间；空组给 `None`（呈现侧收成 0/0）。哨兵值不当缺省用——
/// `usize::MAX` 一旦进视图就成了「区间终点 18446744073709551615」的假读数，
/// 而首章恰好落在 `0..0` 时拿 `(0, 0)` 当初值又会把首章从区间里丢掉。
fn bounds_of(hits: &[&MemoryChapter]) -> Option<(usize, usize)> {
    let lo = hits.iter().map(|b| b.event_range.0).min()?;
    let hi = hits.iter().map(|b| b.event_range.1).max()?;
    Some((lo, hi))
}

impl Organizer {
    /// 整组会话流（MB §2.3）。Err 只留给「整组根本读不出」：投影缺失 / 会话日志
    /// 定位或读取失败 / 组和键不成立 / 该区间在本会话的总结链里不存在（前端只会
    /// 拿到 `memory_tree_chapters` 给过的键，凑一个不存在的就是调用方出错——演成
    /// 空面板等于把 bug 藏进空态）。区间越界这类**单节**失败走 `StreamSection.error`。
    pub fn tree_stream(
        &self,
        session_id: &str,
        workspace: &str,
        group: &str,
    ) -> Result<StreamView, String> {
        let tree = self
            .tree_store
            .load(workspace, session_id)
            .ok_or_else(|| "树投影不存在或已损坏".to_string())?;
        let (from, to, hits) = match StreamGroup::parse(group)? {
            StreamGroup::Run(f, t) => {
                if !tree.summaries.iter().any(|s| s.from == f && s.to == t) {
                    return Err(format!("该会话没有区间为 {f}:{t} 的轮级总结"));
                }
                (f, t, chapters_in(&tree, f, t))
            }
            StreamGroup::Orphan => {
                // 「哪些章节已归组」逐条总结问 `chapters_in`，不在这里重写区间包含
                // 判据（那是它的第 2 个出处）。
                let grouped: std::collections::HashSet<&str> = tree
                    .summaries
                    .iter()
                    .flat_map(|s| chapters_in(&tree, s.from, s.to))
                    .map(|b| b.chapter_id.0.as_str())
                    .collect();
                let mut hits: Vec<&MemoryChapter> = settled(&tree)
                    .filter(|b| !grouped.contains(b.chapter_id.0.as_str()))
                    .collect();
                hits.sort_by_key(|b| chapter_seq(&b.chapter_id.0));
                // 空孤儿组 = 0/0 + 空 sections（前端以 sections 长度判空态）
                let (lo, hi) = bounds_of(&hits).unwrap_or((0, 0));
                (lo, hi, hits)
            }
        };
        let path = session_store::session_file(&self.sessions_dir, session_id)
            .map_err(|e| format!("会话日志定位失败: {e}"))?;
        let (session, _) =
            session_store::load_session(&path).map_err(|e| format!("会话日志读取失败: {e}"))?;
        let mut sections: Vec<StreamSection> = Vec::with_capacity(hits.len());
        for b in &hits {
            let (f, t) = b.event_range;
            let (rows, error) = match rows_between(session.events(), f, t) {
                Ok(rows) => (rows, None),
                Err(e) => (Vec::new(), Some(e)),
            };
            sections.push(StreamSection {
                chapter_id: b.chapter_id.0.clone(),
                title: b.title.clone(),
                source: b.source.clone(),
                edited: b.title_edited_by_user || b.summary_edited_by_user,
                from: f,
                to: t,
                rows,
                error,
            });
        }
        // 次序契约 = 事件序（区间起点升序；同起点按终点，再同按 id 数值序收口 = 稳定）
        sections.sort_by(|x, y| {
            (x.from, x.to, chapter_seq(&x.chapter_id)).cmp(&(
                y.from,
                y.to,
                chapter_seq(&y.chapter_id),
            ))
        });
        let total_rows = sections.iter().map(|s| s.rows.len()).sum();
        Ok(StreamView {
            sections,
            from,
            to,
            total_rows,
        })
    }
}
