//! 常驻**进展行**（MR 稿 §4.2，队列 Q2）= 正式解冻《Cross-Session Recall
//! Design v1》批 2（该稿 §6/§77/§94 三处当时挂账「实测确有需要再立批」，
//! MR 稿 §0 那次真实失败就是凭证）。
//!
//! **这块治什么**：树目录注入只含**当前会话**的 L0 章节 ⇒ 新会话看到的是空目录，
//! 于是"这个项目早前做过 27 KB 的真章节"这件事在上下文里**完全没有痕迹**
//! （MR 稿 §0-3 的注入面三重盲第二条）。进展行把跨会话的项目级读数压成一段
//! ≤400 字符的话放进 system 尾部：整理过几个会话、共几章、最近一次什么时候、
//! 最近几章的标题，末了指路三件工具。
//!
//! **字节钉死**（稿 §4.2 的硬要求，KV v2 纪律 2 的同族）：首请求算毕即钉，
//! 会话中途 organize 产出新章**不许**改写本会话已注入的字节——那会打断前缀缓存。
//! 缓存形态照 `middleware/memory_inject.rs` 的 `PREFETCH_CAP` 纪律：
//! 键 = `(session_id, workspace)`（会话改归属 = 换键 = 重算），FIFO 128 驱逐，
//! "确实没货"这个结论也缓存（否则每个 step 都要重扫一遍投影目录）。
//!
//! **空态**：全新项目（`scan_workspace` 无行 / 每棵都只有占位章节）⇒ `None` =
//! 一行都不注，零开销。
//!
//! **预算优先序**（超 400 时砍谁）：标题 → 省略标记 → 主体截断。理由：
//! "几个会话 / 最近什么时候 / 用哪三件工具"这三件事是这一段的存在意义，
//! 标题只是引子；砍标题不砍指路。

use std::collections::{HashMap, VecDeque};

use memory_tree::{MemoryChapter, MemoryTree};

/// 进展行总长上限（稿 §6 预算表：400 字符）。
pub(crate) const MAX_PROGRESS_CHARS: usize = 400;
/// 标题候选上限（稿 §4.2「最近 1-2 份树的最新若干章节」）。
const MAX_TITLES: usize = 3;
/// 参与标题聚合的树份数。
const MAX_RECENT_TREES: usize = 2;
/// 钉死表容量（与 `memory_inject::PREFETCH_CAP` 同款保险丝）。
const PIN_CAP: usize = 128;

/// 会话级进展行钉死表。
#[derive(Default)]
pub(crate) struct ProgressPin {
    map: HashMap<String, Option<String>>,
    order: VecDeque<String>,
}

impl ProgressPin {
    /// `None`（"这个项目确实没货"）也缓存，避免每个 step 重扫目录。
    ///
    /// 可见性只到 `organize`（父模块）：钉死表的读写面必须留在 `TreeAssembler`
    /// 一侧，别处绕过它去查 = 破坏"会话内字节钉死"。
    pub(super) fn get(&self, key: &str) -> Option<Option<String>> {
        self.map.get(key).cloned()
    }

    pub(super) fn insert(&mut self, key: String, value: Option<String>) {
        if !self.map.contains_key(&key) {
            self.order.push_back(key.clone());
            if self.order.len() > PIN_CAP {
                if let Some(old) = self.order.pop_front() {
                    self.map.remove(&old);
                }
            }
        }
        self.map.insert(key, value);
    }
}

/// 钉死键：会话 + 键位（归属变更 = 换键 = 重算，与记忆块同纪律）。
pub(crate) fn pin_key(session_id: &str, workspace: &str) -> String {
    // U+0001 不会出现在会话 id（`desktop-<ms>-<n>` / uuid 形态）里
    format!("{session_id}\u{1}{workspace}")
}

/// 聚合出的项目级读数。
struct Stats {
    /// 有实章节的会话数（"整理过几个会话"）。
    sessions: usize,
    /// 这些会话里的非占位章节总数。
    chapters: usize,
    /// 最近一次整理的投影 mtime（毫秒）。
    last_ms: i64,
    /// 标题候选（已按"新鲜份优先 + 份内创建序新→旧"排好，去重）。
    titles: Vec<String>,
}

/// 进展行**内文**（纯函数，判据主力；目录段的其余文本由调用方拼在它之后）。
///
/// `rows` = [`memory_tree::TreeStore::scan_workspace`] 的结果
/// （树, 会话 id, 投影 mtime 毫秒）。全空 = `None`。
pub(crate) fn render(rows: &[(MemoryTree, String, i64)], now_ms: i64) -> Option<String> {
    let stats = collect(rows)?;
    Some(compose(&stats, now_ms))
}

/// 读数聚合（不做任何字符串渲染，便于单独判"占位章节不算货"）。
fn collect(rows: &[(MemoryTree, String, i64)]) -> Option<Stats> {
    let real = |b: &MemoryChapter| !b.pending;
    let mut organized = 0usize;
    let mut chapters = 0usize;
    let mut last_ms = 0i64;
    // 只从"最近两份"里取标题：第 3 份往后的章节对"最近做了什么"没有信息量，
    // 而把它们算进标题会让这一段变成历史清单（那是 project_history 的职责）。
    let mut fresh: Vec<&(MemoryTree, String, i64)> = Vec::new();
    for row in rows {
        let (tree, _sid, ms) = row;
        let n = tree.chapters.values().filter(|b| real(b)).count();
        if n == 0 {
            continue; // 只有占位章节（在途整理）= 这个项目还没有货
        }
        organized += 1;
        chapters += n;
        last_ms = last_ms.max(*ms);
        fresh.push(row);
    }
    if organized == 0 {
        return None;
    }
    fresh.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| b.1.cmp(&a.1)));
    let mut titles: Vec<String> = Vec::new();
    for (tree, _, _) in fresh.iter().take(MAX_RECENT_TREES) {
        let mut latest: Vec<&MemoryChapter> = tree.chapters.values().filter(|b| real(b)).collect();
        // 份内按创建序新→旧（`created_at` 单调；章节 id 是 `b{n}` 字典序会倒）
        latest.sort_by_key(|b| std::cmp::Reverse(b.created_at));
        for b in latest {
            let t = b.title.trim();
            if !t.is_empty() && !titles.iter().any(|x| x == t) {
                titles.push(t.to_string());
            }
            if titles.len() >= MAX_TITLES {
                break;
            }
        }
    }
    Some(Stats {
        sessions: organized,
        chapters,
        last_ms,
        titles,
    })
}

/// 渲染 + 预算收口：从全部候选开始，装不下就少放一个标题（并留省略标记），
/// 一个标题都放不下时仍输出完整的"读数 + 指路"主体。
fn compose(s: &Stats, now_ms: i64) -> String {
    let base = format!(
        "[project memory] This project has {} organized session(s) and {} chapter(s); \
         last organize {}.",
        s.sessions,
        s.chapters,
        human_age(s.last_ms, now_ms)
    );
    let steer = " Timeline: memory_project_history. Older topics: memory_search_titles. \
                 Chapter text: memory_expand.";
    for keep in (0..=s.titles.len()).rev() {
        let omitted = s.titles.len() - keep;
        let joined = s.titles[..keep]
            .iter()
            .map(|t| format!("“{t}”"))
            .collect::<Vec<_>>()
            .join("、");
        // 省略必须**说出来**：一个标题都放不下时也要留一句"另有章节"，
        // 否则进展行看起来像"这个项目就只有这些"（静默少列 = 假账）。
        let tail = if keep == 0 {
            if s.titles.is_empty() {
                String::new()
            } else {
                " Recent chapters omitted (space budget).".to_string()
            }
        } else {
            format!(
                " Recent chapters: {joined}{}.",
                if omitted > 0 { " …" } else { "" }
            )
        };
        let text = format!("{base}{tail}{steer}");
        if text.chars().count() <= MAX_PROGRESS_CHARS || keep == 0 {
            // keep==0 仍超限（读数 + 指路主体自己就超）：硬预算优先，截断并留标记
            return fit(text);
        }
    }
    // 循环必然在 keep==0 那轮 return
    unreachable!("progress line always resolves at keep==0")
}

/// 硬预算收口。**当前文本常量下不可达**（主体约 230 字符 < 400）：留着它是为了
/// 将来有人改 `steer`/`base` 措辞时不会静默突破预算——真要突破就在这里截断并留
/// `…` 标记，而不是让一段超预算文本进 system。
fn fit(text: String) -> String {
    let n = text.chars().count();
    if n <= MAX_PROGRESS_CHARS {
        return text;
    }
    let keep = MAX_PROGRESS_CHARS - " …".chars().count();
    let cut: String = text.chars().take(keep).collect();
    format!("{cut} …")
}

/// 人类可读的时间差（N 分钟前 / N 小时前 / N 天前）；未来时间戳不发明负数。
fn human_age(at_ms: i64, now_ms: i64) -> String {
    const MIN: i64 = 60_000;
    const HOUR: i64 = 60 * MIN;
    const DAY: i64 = 24 * HOUR;
    let delta = now_ms.saturating_sub(at_ms);
    if delta < MIN {
        "just now".to_string()
    } else if delta < HOUR {
        format!("{} min ago", delta / MIN)
    } else if delta < DAY {
        format!("{} h ago", delta / HOUR)
    } else {
        format!("{} days ago", delta / DAY)
    }
}

#[cfg(test)]
mod tests;
