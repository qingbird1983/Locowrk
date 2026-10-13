//! 摘要补写的树侧写路径（队列 Q8c）。
//!
//! 规则切分的产物（轮内泄压 Q8b + 记录员降级 §5.4）标题取得到、摘要取不到：
//! `draft_from_items` 只能写一句「（规则切分）本章节涵盖事件 X–Y」。这一截
//! 债过去没人还——游标一推，记录员在轮末看到的是**游标之后的新尾巴**，不会
//! 回头补已经归档的章节（Q8b 收口时在 `organize/pressure.rs` 头注里写破过
//! 这条边界）。本模块补的就是那条通路的数据面：标记（`chapter.rs` 的
//! `summary_pending` / `summary_attempts`）+ 候选挑选 + 落笔。调用模型、
//! 超时重试、并发控制在宿主侧（`organize/backfill.rs`）。
//!
//! 三条设计约束，逐条都有判据钉着：
//!
//! 1. **只换 `summary`，别的字段一个都不动**（`chapter_id` / `title` /
//!    `event_range` / `created_at` / `source`）。理由不是"改动面小"这种偏好，
//!    是前缀缓存：请求组装的目录行字节形是 `\n- {id}: {title}`
//!    （`organize/assemble.rs::directory_text`），标题一动，那一条以及它之后
//!    所有请求字节全变 = 打断一次缓存。补写发生在后台、随时可能发生，所以
//!    它**必须**结构上碰不到目录里出现的任何字段。
//! 2. **不产出没人读的字段**：`keywords` 与 `token_count` 在现网只有写点、
//!    没有读点（全仓核对），补写通路照旧不填它们——为了"将来观测面可能要"
//!    去接一个当前无人消费的形状，正是队列 Q7 抓的那类件。
//! 3. **失败也要落账**（`mark_summary_attempts`）：不落账的话一章打不通的
//!    输出每轮被重新挑出来重烧一次模型，无界。

use crate::chapter::MemoryChapter;
use crate::tree::MemoryTree;

impl MemoryTree {
    /// 待补写章节的 id 列表（旧事优先）：欠债 ∧ 非占位 ∧ 用户没手改过摘要 ∧
    /// 尝试次数未到上限，按 `(created_at, 章节编号)` 升序取前 `max` 条。
    ///
    /// `max == 0` 或 `max_attempts == 0` 都得到空表 = 关掉补写（可配置原则②，
    /// 数字本身由宿主侧的两个开发者设置项给出）。返回值是 id 而不是章节引用：
    /// 宿主拿 id 去会话日志里取材料，中途还要 `&mut` 树，借用不冲突。
    pub fn summary_candidates(&self, max: usize, max_attempts: u32) -> Vec<String> {
        if max == 0 || max_attempts == 0 {
            return Vec::new();
        }
        let mut due: Vec<&MemoryChapter> = self
            .chapters
            .values()
            .filter(|b| {
                b.summary_pending
                    && !b.pending
                    && !b.summary_edited_by_user
                    && b.summary_attempts < max_attempts
            })
            .collect();
        due.sort_by(|a, b| {
            a.created_at
                .cmp(&b.created_at)
                .then_with(|| chapter_seq(&a.chapter_id.0).cmp(&chapter_seq(&b.chapter_id.0)))
        });
        due.into_iter()
            .take(max)
            .map(|b| b.chapter_id.0.clone())
            .collect()
    }

    /// 落一次补写：只改 `summary` 并清债，返回实际改写条数。三种跳过各有理由：
    ///
    /// - **未知 id**：模型编了一个材料里没有的章节号（`parse_summaries` 已在
    ///   解析层拒绝，这里是不信任调用方的兜底）。
    /// - **占位章节**：在途产物，收束时整章节替换，补写插进去会丢。
    /// - **用户手改过摘要**：纪律 5「自动层永不覆盖用户编辑」。注意这一条
    ///   **仍然清债**（`summary_pending` 置假）——人已经写过答案了，继续挂着
    ///   债就是每轮重烧一次模型去打一个不该打的字段。
    pub fn apply_summaries(&mut self, items: &[ChapterSummary]) -> usize {
        let mut applied = 0usize;
        for item in items {
            let Some(chapter) = self.chapters.get_mut(&item.chapter_id) else {
                continue;
            };
            chapter.summary_pending = false;
            if chapter.pending || chapter.summary_edited_by_user {
                continue;
            }
            chapter.summary = item.summary.clone();
            applied += 1;
        }
        applied
    }

    /// 把这一批候选的尝试次数各加一（**成功与失败都要调**：成功那条随后被
    /// `apply_summaries` 清债，计数只作观测；失败那条靠它退出候选池）。
    /// 饱和加，防极端配置下溢出。返回命中章节数。
    pub fn mark_summary_attempts(&mut self, ids: &[&str]) -> usize {
        let mut hit = 0usize;
        for id in ids {
            if let Some(chapter) = self.chapters.get_mut(*id) {
                chapter.summary_attempts = chapter.summary_attempts.saturating_add(1);
                hit += 1;
            }
        }
        hit
    }
}

/// 一次补写的产出形状：章节 id + 该章摘要正文。
///
/// 刻意**不带** `title` 与 `keywords`（见模块头注第 1、2 条），也不带
/// `event_range`——区间是章节成立的前提，改写它等于改写历史归属。
#[derive(Debug, Clone, PartialEq)]
pub struct ChapterSummary {
    pub chapter_id: String,
    pub summary: String,
}

/// 章节编号（`b{n}` 里的 n；不可解析排到最后，稳定序不靠巧合）。
fn chapter_seq(id: &str) -> u64 {
    id.strip_prefix('b')
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests;
