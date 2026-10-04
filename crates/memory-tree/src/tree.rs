//! MemoryTree（树 v2 §2/§17）：捆表、已整理游标、在途任务、反弹熔断与
//! 整理历史。**树是投影**——只存 `event_range` 引用与生成文本，可随时
//! 删除重建（§1.3）。本结构即落盘 JSON 的形状（storage 原子写）。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::bundle::{BundleDraft, BundleId, EventId, MemoryBundle};

/// 根标识（M2 产出的捆都是顶层：parent = None；层级留字段，§16 深度
/// 上限 3 是 M4+ 组装器的约束）。
pub const ROOT_ID: &str = "root";
/// 投影文件版本（字段演进全走 serde default，见 bundle/tree 追加字段）。
pub const SCHEMA_VERSION: u32 = 1;
/// 整理历史上限（FIFO，防投影文件随使用无限膨胀）。
const HISTORY_CAP: usize = 20;

/// 在途整理任务（§5.3 异步 + 崩溃恢复）：任务与占位捆随树持久化，
/// 重启后 storage 扫描重投；`attempts` 记重投次数（观测用，管道自身
/// 恒定收束——记录员失败落规则切分）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PendingTask {
    pub from: EventId,
    pub to: EventId,
    pub placeholder_id: BundleId,
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
/// = ok|failed|aborted，token 为记录员用量——规则切分为 0）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrganizeRecord {
    pub ts: i64,
    pub reason: String,
    pub mode: String,
    pub status: String,
    pub from: EventId,
    pub to: EventId,
    #[serde(default)]
    pub bundles: usize,
    #[serde(default)]
    pub recorder_input: u64,
    #[serde(default)]
    pub recorder_output: u64,
    #[serde(default)]
    pub duration_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryTree {
    pub schema_version: u32,
    pub workspace: String,
    pub session_id: String,
    pub root: BundleId,
    pub bundles: BTreeMap<String, MemoryBundle>,
    /// 已整理游标（排他）：事件 `[0..bundled_until)` 已归捆；尾巴 =
    /// 其后的内容事件。
    pub bundled_until: EventId,
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
            root: BundleId(ROOT_ID.into()),
            bundles: BTreeMap::new(),
            bundled_until: 0,
            pending: None,
            rebound: ReboundState::default(),
            paused_auto: false,
            history: Vec::new(),
        }
    }

    /// 下一个捆编号（`b{max+1}`：按既有编号取最大，容忍未来可能的删除）。
    fn next_bundle_seq(&self) -> u64 {
        self.bundles
            .keys()
            .filter_map(|k| k.strip_prefix('b')?.parse::<u64>().ok())
            .max()
            .unwrap_or(0)
            + 1
    }

    /// 草稿 → 捆（id 顺序分配；区间合法性由调用方校验——parse_drafts /
    /// rule_split 产物）。
    pub fn build_bundles(
        &self,
        drafts: &[BundleDraft],
        source: &str,
        now_ms: i64,
    ) -> Vec<MemoryBundle> {
        let base = self.next_bundle_seq();
        drafts
            .iter()
            .enumerate()
            .map(|(i, d)| {
                MemoryBundle::from_draft(
                    BundleId(format!("b{}", base + i as u64)),
                    d,
                    source,
                    now_ms,
                )
            })
            .collect()
    }

    pub fn bundle_count(&self) -> usize {
        self.bundles.len()
    }

    /// 建占位捆 + 登记在途任务（§5.3：对话不等整理，占位先行）。
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
        let id = BundleId(format!("b{}", self.next_bundle_seq()));
        let bundle = MemoryBundle {
            bundle_id: id.clone(),
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
        };
        self.bundles.insert(id.0.clone(), bundle);
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

    /// 收束一次整理（记录员或规则切分的产物）：撤占位捆、落真捆、推进
    /// 游标、记台账。
    pub fn apply_organized(
        &mut self,
        bundles: Vec<MemoryBundle>,
        to: EventId,
        record: OrganizeRecord,
    ) {
        if let Some(task) = self.pending.take() {
            self.bundles.remove(&task.placeholder_id.0);
        }
        for b in bundles {
            self.bundles.insert(b.bundle_id.0.clone(), b);
        }
        self.bundled_until = self.bundled_until.max(to + 1);
        self.push_history(record);
    }

    /// 任务终局放弃（会话日志已删等，重投不可能）：撤占位 + 记 aborted。
    /// 原文无损——尾巴留在流里，下次触发重新覆盖。
    pub fn drop_pending(&mut self, record: OrganizeRecord) {
        if let Some(task) = self.pending.take() {
            self.bundles.remove(&task.placeholder_id.0);
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

    /// 手动 `/整理`：复位熔断（用户显式重来 = 重新信任记录员，§3.36①）。
    pub fn mark_manual(&mut self, users_now: u64) {
        self.paused_auto = false;
        self.rebound.count = 0;
        self.rebound.users_at_last = users_now;
    }
}

#[cfg(test)]
mod tests;
