//! 章节（MemoryChapter，树 v2 §3/§17）：L0 标题 / L1 摘要 / L2 事件引用，
//! 原文一个字不存。`pending` / `source` 是实现追加字段（serde default，
//! 旧投影文件兼容）——`pending` 标占位章节（§5.3 异步整理期），`source`
//! 标生成方式（recorder / rule / placeholder，质量抽查与 A/B 用）。
//!
//! 队列 Q8c 再追加两个同样走 serde default 的字段：`summary_pending` 标
//! 「这一章的 L1 摘要还是机械说明，欠一次模型补写」，`summary_attempts`
//! 记补写试过几次（失败也落账，见 `summary` 模块的注释）。

use serde::{Deserialize, Serialize};

/// 事件位（SessionEvent 在会话日志中的下标）。流 append-only，位恒定。
pub type EventId = usize;

/// 树节点标识（`b1`、`b2` …顺序分配；root 见 `tree::ROOT_ID`）。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ChapterId(pub String);

/// 一章节：标签 + 摘要 + 指向原文的事件区间。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryChapter {
    /// **落盘兼容**：字段改名 bundle_id → chapter_id，旧投影走 alias 读回。
    #[serde(alias = "bundle_id")]
    pub chapter_id: ChapterId,
    pub parent: Option<ChapterId>,
    pub children: Vec<ChapterId>,
    /// L0（~10 token；记忆管理器生成或规则切分取首条用户消息前 20 字）。
    pub title: String,
    pub title_edited_by_user: bool,
    /// L1（~200 token；用户编辑后 `summary_edited_by_user` 置真，自动层
    /// 永不覆盖——纪律 5）。
    pub summary: String,
    pub summary_edited_by_user: bool,
    /// L2：原文区间（闭区间，含首尾事件位）。
    pub event_range: (EventId, EventId),
    pub created_at: i64,
    pub keywords: Vec<String>,
    pub token_count: u32,
    /// 占位章节（任务在途，§5.3）：记忆管理器产出后整章节替换。
    #[serde(default)]
    pub pending: bool,
    /// recorder / rule / placeholder。
    #[serde(default)]
    pub source: String,
    /// **摘要债标记**（队列 Q8c）：真 = 这条 summary 还是规则切分的机械说明，
    /// 欠一次模型补写。置位点只有一处（`from_draft` 按 `source == "rule"`），
    /// 覆盖泄压支与记录员降级支两个来源——写第二处置位点就是第二个出处
    /// （§11「改一处要两处对齐」的反面）。
    #[serde(default)]
    pub summary_pending: bool,
    /// 补写已尝试次数（失败也落账）：到 `organize_summary_max_attempts` 就不再
    /// 重烧模型。与 `pending::attempts`（§5.3 重投次数）分开记：那本账的语义是
    /// 「一次整理任务被崩溃重投了几次」，塞补写进去会让崩溃恢复的观测失真。
    #[serde(default)]
    pub summary_attempts: u32,
    /// 最近一次被 memory_expand 的时刻（Unix 毫秒；None = 从未展开）。
    /// M4 惰性加载记账：自动收起（§8.3）按它挑「最早展开」。
    #[serde(default)]
    pub expanded_at: Option<i64>,
    /// 收起时刻（Unix 毫秒；None = 未收起）：组装侧把该章节已展开的工具
    /// 结果替换为收起桩（§8.3 空间急救；再次 expand 自动解除）。
    #[serde(default)]
    pub collapsed_at: Option<i64>,
}

/// 记忆管理器（或规则切分）的一条产出：系统校验后才落成章节。
#[derive(Debug, Clone, PartialEq)]
pub struct ChapterDraft {
    pub title: String,
    pub summary: String,
    pub event_range: (EventId, EventId),
}

/// 标题硬上限：提示词要 10–20 字，校验层放宽到 40 防截断误杀。
pub const TITLE_MAX_CHARS: usize = 40;
/// 摘要硬上限（200 字是给记忆管理器的软约束，这里是防垃圾灌爆的硬钳）。
pub const SUMMARY_MAX_CHARS: usize = 2000;

impl MemoryChapter {
    /// 从草稿落章节（调用方已校验区间合法性）。
    pub fn from_draft(id: ChapterId, draft: &ChapterDraft, source: &str, now_ms: i64) -> Self {
        Self {
            chapter_id: id,
            parent: None,
            children: Vec::new(),
            title: draft.title.clone(),
            title_edited_by_user: false,
            summary: draft.summary.clone(),
            summary_edited_by_user: false,
            event_range: draft.event_range,
            created_at: now_ms,
            keywords: Vec::new(),
            token_count: 0,
            pending: false,
            source: source.to_string(),
            // 规则切分的产物一律欠摘要：泄压支（Q8b）与记录员降级支（§5.4）都
            // 经 `build_chapters(…, "rule")` 走到这一行，所以置位点只需要这一处。
            summary_pending: source == "rule",
            summary_attempts: 0,
            expanded_at: None,
            collapsed_at: None,
        }
    }

    /// memory_expand 记账（M4）：刷新展开时刻并解除收起（§8.3「再次展开
    /// 自动解除收起」）。占位章节忽略（不可展开）。
    pub fn mark_expanded(&mut self, now_ms: i64) {
        if self.pending {
            return;
        }
        self.expanded_at = Some(now_ms);
        self.collapsed_at = None;
    }

    /// memory_collapse 记账（M4 §8.3）：置收起时刻（保留 expanded_at 作
    /// 展开史——自动收起挑选只看未收起者）。占位章节 / 从未展开 = 忽略。
    pub fn mark_collapsed(&mut self, now_ms: i64) {
        if self.pending || self.expanded_at.is_none() {
            return;
        }
        self.collapsed_at = Some(now_ms);
    }
}

/// 草稿净化：trim → 空拒绝 → 标题/摘要硬钳（§16 常见坑「标题太长」的
/// 系统侧兜底）。失败 = 该条不可用（调用方整批转规则切分）。
pub fn sanitize_draft(
    title: &str,
    summary: &str,
    range: (EventId, EventId),
) -> Option<ChapterDraft> {
    let title = title.trim();
    let summary = summary.trim();
    if title.is_empty() || summary.is_empty() || range.0 > range.1 {
        return None;
    }
    Some(ChapterDraft {
        title: title.chars().take(TITLE_MAX_CHARS).collect(),
        summary: summary.chars().take(SUMMARY_MAX_CHARS).collect(),
        event_range: range,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_rejects_empty_and_inverted_range() {
        assert!(sanitize_draft("", "s", (1, 2)).is_none());
        assert!(sanitize_draft("t", "  ", (1, 2)).is_none());
        assert!(sanitize_draft("t", "s", (5, 2)).is_none());
    }

    #[test]
    fn sanitize_clips_overlong_title_and_summary() {
        let d = sanitize_draft(&"标".repeat(60), &"摘".repeat(3000), (1, 2)).unwrap();
        assert_eq!(d.title.chars().count(), TITLE_MAX_CHARS);
        assert_eq!(d.summary.chars().count(), SUMMARY_MAX_CHARS);
    }

    #[test]
    fn bundle_from_draft_marks_source() {
        let d = sanitize_draft("标题", "摘要", (3, 9)).unwrap();
        let b = MemoryChapter::from_draft(ChapterId("b1".into()), &d, "recorder", 42);
        assert_eq!(b.event_range, (3, 9));
        assert_eq!(b.source, "recorder");
        assert!(!b.pending);
        assert!(!b.title_edited_by_user);
    }

    /// 摘要债只由 `source == "rule"` 决定（队列 Q8c：置位点一处，泄压支与
    /// 记录员降级支共用）。三种来源逐一钉住，防止将来新增第四种来源时
    /// 顺手把债判成「非 recorder 都欠」——placeholder 不欠（它没有可补的
    /// 内容，且补写通路本就把它排除在外）。
    #[test]
    fn only_rule_chapters_carry_the_summary_debt() {
        let d = sanitize_draft("标题", "摘要", (3, 9)).unwrap();
        for (source, debt) in [("rule", true), ("recorder", false), ("placeholder", false)] {
            let b = MemoryChapter::from_draft(ChapterId("b1".into()), &d, source, 42);
            assert_eq!(b.summary_pending, debt, "{source} 的摘要债判定");
            assert_eq!(b.summary_attempts, 0, "新章节的补写计数从 0 起");
        }
    }

    /// 旧投影无这两个键 = false / 0（serde default，零迁移）：磁盘上任何一份
    /// Q8c 之前写的投影文件读回来都不该凭空欠债。
    #[test]
    fn summary_debt_fields_default_on_legacy_projection() {
        let raw = r#"{"chapter_id":"b1","parent":null,"children":[],"title":"t","title_edited_by_user":false,"summary":"s","summary_edited_by_user":false,"event_range":[0,1],"created_at":1,"keywords":[],"token_count":0,"pending":false,"source":"rule"}"#;
        let b: MemoryChapter = serde_json::from_str(raw).unwrap();
        assert!(
            !b.summary_pending,
            "旧投影不欠债（它已经是当时那条通路的终态）"
        );
        assert_eq!(b.summary_attempts, 0);
    }
}
