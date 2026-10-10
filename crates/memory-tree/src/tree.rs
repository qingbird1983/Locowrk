//! MemoryTree（树 v2 §2/§17）：章节表、已整理游标、在途任务、反弹熔断与
//! 整理历史。**树是投影**——只存 `event_range` 引用与生成文本，可随时
//! 删除重建（§1.3）。本结构即落盘 JSON 的形状（storage 原子写）。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::chapter::{ChapterDraft, ChapterId, EventId, MemoryChapter};

/// 根标识（M2 产出的章节都是顶层：parent = None；层级留字段，§16 深度
/// 上限 3 是 M4+ 组装器的约束）。
pub const ROOT_ID: &str = "root";
/// 投影文件版本（字段演进全走 serde default，见 chapter/tree 追加字段）。
pub const SCHEMA_VERSION: u32 = 1;
/// 整理历史上限（FIFO，防投影文件随使用无限膨胀）。
const HISTORY_CAP: usize = 20;

/// 在途整理任务（§5.3 异步 + 崩溃恢复）：任务与占位章节随树持久化，
/// 重启后 storage 扫描重投；`attempts` 记重投次数（观测用，管道自身
/// 恒定收束——记忆管理器失败落规则切分）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PendingTask {
    pub from: EventId,
    pub to: EventId,
    pub placeholder_id: ChapterId,
    pub created_at: i64,
    #[serde(default)]
    pub attempts: u32,
}

/// 反弹熔断账本（§5.5）：`count` = 连续反弹次数；`users_at_last` = 上次
/// 整理收束时会话的用户消息总数（0 = 从未整理，无反弹基线）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ReboundState {
    #[serde(default)]
    pub count: u32,
    #[serde(default)]
    pub users_at_last: u64,
}

/// 一次整理的台账（开发者观测面板数据源；mode = recorder|rule，status
/// = ok|failed|aborted，token 为记忆管理器用量——规则切分为 0）。
/// **落盘兼容**：chapters/manager_input/manager_output 的旧名 bundles/
/// recorder_input/recorder_output 走 serde alias 读回；mode/status 里的
/// "recorder" 是数据词汇本批不改（见设计文档术语注）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrganizeRecord {
    pub ts: i64,
    pub reason: String,
    pub mode: String,
    pub status: String,
    pub from: EventId,
    pub to: EventId,
    #[serde(default, alias = "bundles")]
    pub chapters: usize,
    #[serde(default, alias = "recorder_input")]
    pub manager_input: u64,
    #[serde(default, alias = "recorder_output")]
    pub manager_output: u64,
    #[serde(default)]
    pub duration_ms: u64,
}

/// 一次整理的轮级总结（Organize Summary Design v1 §3）：记忆管理器对本次
/// 整理区间 `[from, to]` 的浓缩叙事（跨章节来龙去脉、去噪），是该轮全部
/// 章节的父节点。编排层在下一轮开场把它排空成会话日志的
/// `SessionEvent::ContextSummary`（断层交接消息面）；投影/人读层
/// （tree.md、记忆页、项目联合视图）直接消费本记录。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunSummary {
    pub from: EventId,
    pub to: EventId,
    pub text: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryTree {
    pub schema_version: u32,
    pub workspace: String,
    pub session_id: String,
    pub root: ChapterId,
    /// **落盘兼容**：字段改名 bundles → chapters，旧投影走 alias 读回
    /// （新文件写 chapters；树节点 id 值空间 `b{n}` 不变，见术语注）。
    #[serde(alias = "bundles")]
    pub chapters: BTreeMap<String, MemoryChapter>,
    /// 已整理游标（排他）：事件 `[0..chaptered_until)` 已归章节；尾巴 =
    /// 其后的内容事件。旧名 bundled_until 走 alias 读回。
    #[serde(alias = "bundled_until")]
    pub chaptered_until: EventId,
    /// 轮级总结链（v2.141，serde default 零迁移）：按整理收束序追加，
    /// 封闭期记录一次成文永不改写（设计稿 §2/§3）。
    #[serde(default)]
    pub summaries: Vec<RunSummary>,
    #[serde(default)]
    pub pending: Option<PendingTask>,
    #[serde(default)]
    pub rebound: ReboundState,
    /// 反弹熔断命中：暂停自动整理（手动 `/整理` 不受限且复位）。
    #[serde(default)]
    pub paused_auto: bool,
    #[serde(default)]
    pub history: Vec<OrganizeRecord>,
}

/// 触发时机的反弹裁决（§5.5）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReboundDecision {
    /// 正常（或反弹未到 m）：继续本次整理。
    Proceed,
    /// 连续第 m 次反弹：置 paused_auto 并跳过本次自动整理。
    Pause,
}

impl MemoryTree {
    pub fn new(workspace: &str, session_id: &str) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            workspace: workspace.to_string(),
            session_id: session_id.to_string(),
            root: ChapterId(ROOT_ID.into()),
            chapters: BTreeMap::new(),
            chaptered_until: 0,
            summaries: Vec::new(),
            pending: None,
            rebound: ReboundState::default(),
            paused_auto: false,
            history: Vec::new(),
        }
    }

    /// 下一个章节编号（`b{max+1}`：按既有编号取最大，容忍未来可能的删除）。
    fn next_chapter_seq(&self) -> u64 {
        self.chapters
            .keys()
            .filter_map(|k| k.strip_prefix('b')?.parse::<u64>().ok())
            .max()
            .unwrap_or(0)
            + 1
    }

    /// 草稿 → 章节（id 顺序分配；区间合法性由调用方校验——parse_drafts /
    /// rule_split 产物）。
    pub fn build_chapters(
        &self,
        drafts: &[ChapterDraft],
        source: &str,
        now_ms: i64,
    ) -> Vec<MemoryChapter> {
        let base = self.next_chapter_seq();
        drafts
            .iter()
            .enumerate()
            .map(|(i, d)| {
                MemoryChapter::from_draft(
                    ChapterId(format!("b{}", base + i as u64)),
                    d,
                    source,
                    now_ms,
                )
            })
            .collect()
    }

    pub fn chapter_count(&self) -> usize {
        self.chapters.len()
    }

    /// 建占位章节 + 登记在途任务（§5.3：对话不等整理，占位先行）。
    /// 已有在途任务 = 拒绝（单写者纪律，§16「并发写没加锁」的解法）。
    pub fn insert_placeholder(
        &mut self,
        from: EventId,
        to: EventId,
        now_ms: i64,
    ) -> Option<PendingTask> {
        if self.pending.is_some() || from > to {
            return None;
        }
        let id = ChapterId(format!("b{}", self.next_chapter_seq()));
        let chapter = MemoryChapter {
            chapter_id: id.clone(),
            parent: None,
            children: Vec::new(),
            title: format!("对话 #{from}-{to}"),
            title_edited_by_user: false,
            summary: "整理进行中（占位）。".into(),
            summary_edited_by_user: false,
            event_range: (from, to),
            created_at: now_ms,
            keywords: Vec::new(),
            token_count: 0,
            pending: true,
            source: "placeholder".into(),
            expanded_at: None,
            collapsed_at: None,
        };
        self.chapters.insert(id.0.clone(), chapter);
        let task = PendingTask {
            from,
            to,
            placeholder_id: id,
            created_at: now_ms,
            attempts: 0,
        };
        self.pending = Some(task.clone());
        Some(task)
    }

    /// 收束一次整理（记忆管理器或规则切分的产物）：撤占位章节、落真章节、推进
    /// 游标、记台账。`summary` = 记忆管理器产出的轮级总结（rule 降级 =
    /// None——宁缺勿假，退化到无总结的旧行为）。
    pub fn apply_organized(
        &mut self,
        chapters: Vec<MemoryChapter>,
        to: EventId,
        record: OrganizeRecord,
        summary: Option<String>,
    ) {
        if let Some(task) = self.pending.take() {
            self.chapters.remove(&task.placeholder_id.0);
        }
        for b in chapters {
            self.chapters.insert(b.chapter_id.0.clone(), b);
        }
        self.chaptered_until = self.chaptered_until.max(to + 1);
        if let Some(text) = summary {
            self.summaries.push(RunSummary {
                from: record.from,
                to: record.to,
                text,
                created_at: record.ts,
            });
        }
        self.push_history(record);
    }

    /// 任务终局放弃（会话日志已删等，重投不可能）：撤占位 + 记 aborted。
    /// 原文无损——尾巴留在流里，下次触发重新覆盖。
    pub fn drop_pending(&mut self, record: OrganizeRecord) {
        if let Some(task) = self.pending.take() {
            self.chapters.remove(&task.placeholder_id.0);
        }
        self.push_history(record);
    }

    fn push_history(&mut self, record: OrganizeRecord) {
        self.history.push(record);
        if self.history.len() > HISTORY_CAP {
            let overflow = self.history.len() - HISTORY_CAP;
            self.history.drain(0..overflow);
        }
    }

    /// 自动触发时的反弹裁决（§5.5）：距上次整理 ≤ k 个用户轮 = 反弹；
    /// 连续 m 次置 paused_auto 并跳过本次（手动 `/整理` 不走这里）。
    pub fn on_auto_trigger(&mut self, users_now: u64, k: u32, m: u32) -> ReboundDecision {
        let baseline = self.rebound.users_at_last;
        if baseline == 0 {
            return ReboundDecision::Proceed; // 从未整理过：无反弹可言
        }
        if users_now.saturating_sub(baseline) <= k as u64 {
            self.rebound.count = self.rebound.count.saturating_add(1);
            if self.rebound.count >= m.max(1) {
                self.paused_auto = true;
                return ReboundDecision::Pause;
            }
        } else {
            self.rebound.count = 0; // 健康间隔：连续计数归零
        }
        ReboundDecision::Proceed
    }

    /// 整理收束时的用户轮基线更新（自动与手动同吃——手动整理也是一次
    /// 收束，其后的 k 轮内再触发同样算反弹）。
    pub fn note_organized(&mut self, users_now: u64) {
        self.rebound.users_at_last = users_now;
    }

    /// 手动 `/整理`：复位熔断（用户显式重来 = 重新信任记忆管理器，§3.36①）。
    pub fn mark_manual(&mut self, users_now: u64) {
        self.paused_auto = false;
        self.rebound.count = 0;
        self.rebound.users_at_last = users_now;
    }

    /// memory_expand 记账（M4 §8.2）：章节不存在 / 占位 = false。
    pub fn mark_expanded(&mut self, id: &str, now_ms: i64) -> bool {
        match self.chapters.get_mut(id) {
            Some(b) if !b.pending => {
                b.mark_expanded(now_ms);
                true
            }
            _ => false,
        }
    }

    /// memory_collapse 记账（M4 §8.3）：章节不存在 / 占位 / 从未展开 = false
    /// （收起只对已展开章节有意义——没展开过就没有可收的内容）。
    pub fn mark_collapsed(&mut self, id: &str, now_ms: i64) -> bool {
        match self.chapters.get_mut(id) {
            Some(b) if !b.pending => {
                b.mark_collapsed(now_ms);
                b.collapsed_at.is_some()
            }
            _ => false,
        }
    }

    /// 当前收起中的章节 id（组装侧把它们的 expand 结果换收起桩；插入序 =
    /// BTreeMap 序，组装侧自排）。
    pub fn collapsed_ids(&self) -> Vec<String> {
        self.chapters
            .values()
            .filter(|b| b.collapsed_at.is_some())
            .map(|b| b.chapter_id.0.clone())
            .collect()
    }

    /// 最早展开且未收起的章节 id（自动收起 §8.3「收起最老展开」；并列按 id
    /// 序稳定）。None = 没有可收的。
    pub fn oldest_expanded(&self) -> Option<String> {
        self.chapters
            .values()
            .filter(|b| b.expanded_at.is_some() && b.collapsed_at.is_none())
            .min_by(|a, b| {
                a.expanded_at
                    .cmp(&b.expanded_at)
                    .then_with(|| a.chapter_id.cmp(&b.chapter_id))
            })
            .map(|b| b.chapter_id.0.clone())
    }

    /// 用户编辑章节标题/摘要（M5 编辑面，§14 纪律 5）：改动即打「手动」
    /// 标记，自动层此后永不覆盖该字段。占位章节 = false（在途产物不可编辑）。
    pub fn edit_chapter(&mut self, id: &str, title: Option<&str>, summary: Option<&str>) -> bool {
        let Some(b) = self.chapters.get_mut(id) else {
            return false;
        };
        if b.pending {
            return false;
        }
        if let Some(t) = title {
            b.title = t.trim().to_string();
            b.title_edited_by_user = true;
        }
        if let Some(s) = summary {
            b.summary = s.trim().to_string();
            b.summary_edited_by_user = true;
        }
        true
    }

    /// 用户删除章节（M5 §12.1「删除捆，不影响原文」）：仅从投影摘除——
    /// 游标不动（被删区间停留在已整理侧，不再进目录/尾巴 = 有意不可见；
    /// 整树重建兜底会重新生成，投影语义）。占位章节拒删（等收束）。
    pub fn remove_chapter(&mut self, id: &str) -> bool {
        match self.chapters.get(id) {
            Some(b) if !b.pending => self.chapters.remove(id).is_some(),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests;
