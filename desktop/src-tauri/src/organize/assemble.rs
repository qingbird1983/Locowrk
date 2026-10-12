//! 树会话组装器（M4 按会话切流，树 v2 §7/§11.2 阶段 2；UI shell §3.38②）：
//! [`TreeAssembler`] 实现 kernel-agent 的 `TreeContextSource`——树模式会话
//! 的请求 = **L0 目录（挂 system 尾部）+ 未整理尾巴 + 本轮**；legacy 会话
//! 返回 None（组装零变化）。
//!
//! 目录格式 = **稳定头行 + 追加清单行**（无闭合标签——闭合标签会随追加
//! 移动，破坏「目录只增不改」的字节前缀纪律，kernel 测试实证后定稿）。
//! 占位章节（整理在途）不进目录：占位行出现又消失 = 隐性前缀改写。
//!
//! 尾巴二档（§7.2）：A 边界结算（缺省）= `chaptered_until` 排他游标切片；
//! B 固定窗口 = `max(chaptered_until, 最近 N 轮起点)`——窗口滑出的孤儿轮由
//! organize 侧溢出整理（reason=window_overflow）及时归章节，绝不静默丢失。
//!
//! `context_tokens`（§3.38② 用量口径）：树会话的用量环按组装后估算
//! （目录 + 尾巴），不再用全量历史——runner `record_usage` 消费。

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use session_store::{Session, SessionEvent};

use kernel_agent::{estimate_tokens, TreeContextSource, TreePlan};

use memory_tree::{MemoryTree, TreeStore};

use super::progress;
use crate::settings::SettingsStore;
use crate::settings::TailStrategy;
use crate::timeutil::now_ms;
use crate::workspace::WorkspaceResolver;

/// 目录头行（稳定字节——追加清单行不改它）。
const DIRECTORY_HEADER: &str = "[memory tree] Earlier turns of this conversation were \
organized into chapters (oldest first). Each line below is a table-of-contents entry; \
to read a chapter, call memory_expand(chapter_id, level=1 for its summary or 2 for \
its full transcript).";

/// 章节 id 数值序（`b2` < `b10`——BTreeMap 字典序会倒，M3 view 同款排序）。
fn numeric_id(id: &str) -> u64 {
    id.strip_prefix('b')
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(u64::MAX)
}

/// 最近 N 个用户轮的起点事件下标（模式 B 窗口）：从尾部数第 N 条
/// UserMessage 的位置；用户轮不足 N = 0（全部为尾巴）。共享给 organize
/// 侧溢出整理判定（plan_auto 持会话锁直接用内存事件）。
pub(crate) fn window_start_event(events: &[SessionEvent], turns: u32) -> usize {
    let users: Vec<usize> = events
        .iter()
        .enumerate()
        .filter(|(_, e)| matches!(e, SessionEvent::UserMessage { .. }))
        .map(|(i, _)| i)
        .collect();
    let n = turns.max(1) as usize;
    if users.len() <= n {
        0
    } else {
        users[users.len() - n]
    }
}

/// 目录文本：非占位且完全在尾巴边界之前的章节，按创建序排清单。
/// 只增不改的支撑：章节集合单调增长、id 不复用、标题稳定（M5 用户编辑 =
/// 显式变更接受失效）。
fn directory_text(tree: &MemoryTree, before_event: usize) -> String {
    let mut rows: Vec<&memory_tree::MemoryChapter> = tree
        .chapters
        .values()
        .filter(|b| !b.pending && b.event_range.1 < before_event)
        .collect();
    rows.sort_by_key(|b| (b.created_at, numeric_id(&b.chapter_id.0)));
    if rows.is_empty() {
        return String::new();
    }
    let mut out = String::from(DIRECTORY_HEADER);
    for b in rows {
        out.push_str(&format!("\n- {}: {}", b.chapter_id.0, b.title));
    }
    out
}

/// 组装器（AppState 持 Arc；同时注册进内核 Context 服务表）。
pub struct TreeAssembler {
    settings: Arc<SettingsStore>,
    workspace_of: WorkspaceResolver,
    tree_store: TreeStore,
    sessions_dir: PathBuf,
    /// Q2（MR 稿 §4.2）：项目级**进展行**的会话级钉死表（见 `super::progress`）。
    /// 只缓存文本，不缓存树——树内容仍按现行路径每 step 现取（尾巴边界要用）。
    progress: Mutex<progress::ProgressPin>,
}

impl TreeAssembler {
    pub fn new(
        settings: Arc<SettingsStore>,
        workspace_of: WorkspaceResolver,
        tree_store: TreeStore,
        sessions_dir: PathBuf,
    ) -> Self {
        Self {
            settings,
            workspace_of,
            tree_store,
            sessions_dir,
            progress: Mutex::new(progress::ProgressPin::default()),
        }
    }

    fn load_tree(&self, workspace: &str, session_id: &str) -> MemoryTree {
        self.tree_store
            .load(workspace, session_id)
            .unwrap_or_else(|| MemoryTree::new(workspace, session_id))
    }

    /// 磁盘会话事件（模式 B 窗口起点计算用；读失败 = 空表回落边界语义）。
    fn disk_events(&self, session_id: &str) -> Vec<SessionEvent> {
        session_store::session_file(&self.sessions_dir, session_id)
            .ok()
            .and_then(|p| session_store::load_session(&p).ok())
            .map(|(s, _)| s.events().to_vec())
            .unwrap_or_default()
    }

    /// 尾巴起点 + 目录（策略判定集中此处；legacy 会话调用方不应到达）。
    fn boundary_and_directory(
        &self,
        tree: &MemoryTree,
        disk: Option<&[SessionEvent]>,
    ) -> (usize, String) {
        let (strategy, turns) = self.settings.tail_config();
        match strategy {
            TailStrategy::Boundary => {
                let dir = directory_text(tree, tree.chaptered_until);
                (tree.chaptered_until, dir)
            }
            TailStrategy::Window => {
                let start = match disk {
                    Some(events) => window_start_event(events, turns),
                    None => window_start_event(&self.disk_events(&tree.session_id), turns),
                };
                let from = tree.chaptered_until.max(start);
                let dir = directory_text(tree, from);
                (from, dir)
            }
        }
    }

    /// 组装后上下文估算（§3.38② 用量口径）= 目录 + 尾巴（v2.137 起
    /// 一切会话皆树，无 legacy 分支）。`session` 是调用方在册
    /// 的内存会话（与磁盘镜像一致，模式 B 边界直接用，免盘读）。
    pub fn context_tokens(&self, session: &Session, session_id: &str) -> Option<u64> {
        let workspace = (self.workspace_of)(session_id).unwrap_or_default();
        let tree = self.load_tree(&workspace, session_id);
        let (from, directory) = self.boundary_and_directory(&tree, Some(session.events()));
        // 用量口径**把进展行一起算**：它确实进 system，漏算等于少报一截
        let directory = self.with_progress(session_id, &workspace, directory);
        let tail = session.derive_messages_from(from);
        let dir_tokens = (directory.len() as u64) / 4;
        Some(dir_tokens + estimate_tokens(&tail))
    }

    /// 项目级进展行（Q2 · MR §4.2），**会话内钉死**：首请求扫一遍投影目录，
    /// 之后逐字节复用——`scan_workspace` 因此每会话至多一次（不是每 step）。
    ///
    /// 未归属会话（键位空串）直接 `None`：「这个项目」不成立，目录段照旧。
    fn progress_line(&self, session_id: &str, workspace: &str) -> Option<String> {
        if workspace.is_empty() {
            return None;
        }
        let key = progress::pin_key(session_id, workspace);
        if let Some(hit) = self
            .progress
            .lock()
            .expect("progress pin poisoned")
            .get(&key)
        {
            return hit;
        }
        let rows = self.tree_store.scan_workspace(workspace);
        let line = progress::render(&rows, now_ms());
        self.progress
            .lock()
            .expect("progress pin poisoned")
            .insert(key, line.clone());
        line
    }

    /// 进展行拼在目录**之前**（顺序固定 = 字节形稳定）：它是"这个项目有过什么"
    /// 的项目级读数，目录是"本会话有过什么"。两者皆空 = 空串（`TreeDirectoryInject`
    /// 对空串不追加）。
    fn with_progress(&self, session_id: &str, workspace: &str, directory: String) -> String {
        match self.progress_line(session_id, workspace) {
            Some(p) if directory.is_empty() => p,
            Some(p) => format!("{p}\n\n{directory}"),
            None => directory,
        }
    }
}

#[async_trait]
impl TreeContextSource for TreeAssembler {
    async fn plan(&self, session_id: &str) -> Option<TreePlan> {
        let workspace = (self.workspace_of)(session_id).unwrap_or_default();
        let tree = self.load_tree(&workspace, session_id);
        // 模式 B 需要窗口起点：盘读一次（树会话 + 实验档才会走）。
        let disk = match self.settings.tail_config().0 {
            TailStrategy::Window => Some(self.disk_events(session_id)),
            TailStrategy::Boundary => None,
        };
        let (from, directory) = self.boundary_and_directory(&tree, disk.as_deref());
        Some(TreePlan {
            directory: self.with_progress(session_id, &workspace, directory),
            tail_from_event: from,
            collapsed_chapters: tree.collapsed_ids(),
        })
    }
}

#[cfg(test)]
mod tests;
