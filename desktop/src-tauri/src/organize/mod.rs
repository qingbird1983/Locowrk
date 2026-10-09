//! 记忆树影子运行编排（M2，《Memory-KV Roadmap》；树 v2 §5.3 异步执行 +
//! 崩溃恢复、§5.4 降级、§5.5 熔断）。**影子纪律：请求组装一行不动**——
//! 本模块只在轮次收束后旁路观察会话日志，产物只落 `memory-tree/` 投影。
//!
//! 结构：`plan_auto` / `plan_manual`（同步决策，可测——含触发判定与反弹
//! 裁决，会写占位章节与熔断账本进投影文件）→ `run_task`（异步管道：记忆管理器
//! 调用 × 超时重试 → 输出校验 → 不合规转规则切分 → 落树）。调用方持有
//! 会话锁时传 `&Session`（命令层 / agent_chat / 调度引擎三入口同形）。

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use serde::Serialize;

use memory_tree::{
    build_items, family_of, manager_prompt, parse_drafts, rule_split_drafts, should_auto_organize,
    AnchorSource, MaterialItem, MemoryManagerClient, MemoryManagerUsage, MemoryTree,
    OrganizeReason, OrganizeRecord, Pacing, ReboundDecision, TreeStore, TriggerInput,
};
use session_store::{Session, SessionEvent};

use crate::settings::SettingsStore;
use crate::settings::TailStrategy;
use crate::workspace::WorkspaceResolver;

use assemble::window_start_event;

mod assemble;
mod client;
mod debug;
mod edit;
mod recovery;
mod search_titles;
mod tree_tools;
mod view;
pub use assemble::TreeAssembler;
pub use client::AppLlmMemoryManager;
pub use debug::TreeDebug;
pub use search_titles::SearchTitlesTool;
pub use tree_tools::{MemoryCollapseTool, MemoryExpandTool};
pub use view::{OriginalItem, ProjectChapterView, TreeChaptersView, TreeSummaryView};

/// `lw://organize-done` 载荷（M3 §3.37③：整理收束系统消息，三路同源）。
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct OrganizeDoneEvent {
    pub session_id: String,
    pub users: u64,
    pub chapters: usize,
    /// recorder | rule（rule = 降级，前端走如实文案）。
    pub mode: String,
    /// ok | degraded。
    pub status: String,
    pub reason: String,
}

/// `lw://organize-paused` 载荷：仅在熔断翻转瞬间发（§3.37③）。
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct OrganizePausedEvent {
    pub session_id: String,
    pub rebounds: u32,
    pub k: u32,
    pub m: u32,
}

/// 整理事件推送窄接口（state 组装点用 `AppHandle::emit` 实现，测试传哑
/// 闭包可断言）。隔离 tauri 运行时符号——编排器本体不触 emit，测试
/// 二进制就不会被拉进窗口栈（tao，见 state/mod.rs 组装点注释）。
pub type OrganizeNotifier = Arc<dyn Fn(&'static str, serde_json::Value) + Send + Sync>;

/// 轮次收束后的整理编排器（AppState 持有 Arc；spawn 用 Arc receiver）。
pub struct Organizer {
    settings: Arc<SettingsStore>,
    sessions_dir: PathBuf,
    client: Arc<dyn MemoryManagerClient>,
    workspace_of: WorkspaceResolver,
    tree_store: TreeStore,
    /// F1 人类层：整理落树后同步项目 tree.md（人读面，文件化设计 §3）。
    files: crate::memory_files::MemoryFiles,
    /// v2.137 会话生效窗口四层链的数据源（自动收起的占用锚点口径）。
    model_overrides: Arc<crate::model_override::ModelOverrideStore>,
    catalog: Arc<crate::llm::catalog::CatalogStore>,
    /// 会话级在途去重（进程内；跨重启由投影 pending 字段守卫）。
    inflight: StdMutex<HashSet<String>>,
    /// 整理系统消息推送（M3 §3.37③；测试传哑闭包 = 静默）。
    notifier: OrganizeNotifier,
}

/// 决策产物：管道输入（材料项克隆自触发瞬间的会话日志——事件流
/// append-only，区间恒定有效）。
pub(crate) struct TaskPayload {
    pub session_id: String,
    pub workspace: String,
    pub items: Vec<MaterialItem>,
    pub users_total: u64,
    pub reason: OrganizeReason,
}

/// 管道收束台账（run_task 返回值，观测日志用）。
type Outcome = (String, String, usize, MemoryManagerUsage);

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn elapsed_ms(started: &std::time::Instant) -> u64 {
    started.elapsed().as_millis() as u64
}

impl Organizer {
    pub fn new(
        settings: Arc<SettingsStore>,
        sessions_dir: PathBuf,
        client: Arc<dyn MemoryManagerClient>,
        workspace_of: WorkspaceResolver,
        data_root: PathBuf,
        model_overrides: Arc<crate::model_override::ModelOverrideStore>,
        catalog: Arc<crate::llm::catalog::CatalogStore>,
        notifier: OrganizeNotifier,
    ) -> Self {
        Self {
            settings,
            sessions_dir,
            client,
            workspace_of,
            tree_store: TreeStore::new(&data_root),
            files: crate::memory_files::MemoryFiles::new(&data_root),
            model_overrides,
            catalog,
            inflight: StdMutex::new(HashSet::new()),
            notifier,
        }
    }

    /// 整理收束系统消息（§3.37③）：三路（手动/自动/恢复）同源。
    fn notify_done(
        &self,
        session_id: &str,
        users: u64,
        chapters: usize,
        mode: &str,
        status: &str,
        reason: OrganizeReason,
    ) {
        let ev = OrganizeDoneEvent {
            session_id: session_id.into(),
            users,
            chapters,
            mode: mode.into(),
            status: status.into(),
            reason: reason.as_str().into(),
        };
        match serde_json::to_value(&ev) {
            Ok(v) => (self.notifier)("lw://organize-done", v),
            Err(e) => tracing::warn!("organize-done 序列化失败: {e}"),
        }
    }

    /// 熔断翻转告知（§3.37③ 如实）。
    fn notify_paused(&self, session_id: &str, rebounds: u32, k: u32, m: u32) {
        let ev = OrganizePausedEvent {
            session_id: session_id.into(),
            rebounds,
            k,
            m,
        };
        match serde_json::to_value(&ev) {
            Ok(v) => (self.notifier)("lw://organize-paused", v),
            Err(e) => tracing::warn!("organize-paused 序列化失败: {e}"),
        }
    }

    fn load_tree(&self, workspace: &str, session_id: &str) -> MemoryTree {
        self.tree_store
            .load(workspace, session_id)
            .unwrap_or_else(|| MemoryTree::new(workspace, session_id))
    }

    /// 占用锚点（§4.2：优先 provider 真实 input——日志最后一条 LlmUsage；
    /// 无上报回落字符估算，锚点来源如实标注）。
    fn anchor(session: &Session) -> (u64, AnchorSource) {
        session
            .events()
            .iter()
            .rev()
            .find_map(|e| match e {
                SessionEvent::LlmUsage { input, .. } => Some((*input, AnchorSource::LlmUsage)),
                _ => None,
            })
            .unwrap_or_else(|| {
                (
                    kernel_agent::estimate_tokens(&session.derive_messages()),
                    AnchorSource::Estimate,
                )
            })
    }

    #[allow(clippy::too_many_arguments)]
    fn record(
        reason: OrganizeReason,
        mode: &str,
        status: &str,
        from: usize,
        to: usize,
        chapters: usize,
        usage: MemoryManagerUsage,
        duration_ms: u64,
    ) -> OrganizeRecord {
        OrganizeRecord {
            ts: now_ms(),
            reason: reason.as_str().into(),
            mode: mode.into(),
            status: status.into(),
            from,
            to,
            chapters,
            manager_input: usage.input,
            manager_output: usage.output,
            duration_ms,
        }
    }

    /// 自动触发决策（轮次收束后、持有会话锁的同步上下文调用）。写路径：
    /// 反弹账本落投影文件；返回 None = 不整理。
    pub(crate) fn plan_auto(&self, session: &Session, session_id: &str) -> Option<TaskPayload> {
        let mem = self.settings.memory();
        let dev = self.settings.developer();
        let workspace = (self.workspace_of)(session_id).unwrap_or_default();
        let mut tree = self.load_tree(&workspace, session_id);
        // M4 模式 B 溢出整理（§3.38②）：固定窗口档，窗口滑出的
        // 孤儿轮（chaptered_until < 窗口起点）既不在尾巴也不在目录，必须及时
        // 归章节——例行维护：**先于熔断判定**（熔断暂停的是阈值/增量烧钱信号，
        // 不是固定节奏）且**不记反弹**（管道按 reason 跳过 note_organized）。
        if self.settings.tail_config().0 == TailStrategy::Window && tree.pending.is_none()
        {
            let start = window_start_event(session.events(), mem.tail_window_turns);
            if tree.chaptered_until < start && start > 0 {
                let events = session.events();
                let items = build_items(&events[tree.chaptered_until..start], tree.chaptered_until);
                if !items.is_empty() {
                    let users_total = memory_tree::user_count(session.events());
                    return Some(TaskPayload {
                        session_id: session_id.to_string(),
                        workspace,
                        items,
                        users_total,
                        reason: OrganizeReason::WindowOverflow,
                    });
                }
            }
        }
        if tree.paused_auto || tree.pending.is_some() {
            return None;
        }
        let items = build_items(session.events(), tree.chaptered_until);
        let tail_tokens: u64 = items.iter().map(|i| i.tokens).sum();
        let (anchor_tokens, anchor_source) = Self::anchor(session);
        let reason = should_auto_organize(&TriggerInput {
            anchor_source,
            anchor_tokens,
            threshold_tokens: self.settings.organize_threshold_tokens(),
            pacing: Pacing::parse(&mem.organize_pacing),
            chunk_tokens: mem.organize_chunk_tokens,
            tail_tokens,
            tail_events: items.len(),
        })?;
        // 反弹熔断（§5.5）：连续 m 次反弹 → 暂停本会话自动整理并跳过本次
        let users_total = memory_tree::user_count(session.events());
        if tree.on_auto_trigger(users_total, dev.organize_rebound_k, dev.organize_rebound_m)
            == ReboundDecision::Pause
        {
            tracing::warn!(session = %session_id, "整理反弹熔断：暂停本会话自动整理（/整理 不受限）");
            self.tree_store.save(&tree).ok();
            // plan_auto 在 paused_auto 已置位时早退，走到这里即翻转瞬间
            self.notify_paused(
                session_id,
                tree.rebound.count,
                dev.organize_rebound_k,
                dev.organize_rebound_m,
            );
            return None;
        }
        self.tree_store.save(&tree).ok();
        Some(TaskPayload {
            session_id: session_id.to_string(),
            workspace,
            items,
            users_total,
            reason,
        })
    }

    /// 手动 `/整理`（不受熔断限制且复位熔断态，§3.36①）。
    pub(crate) fn plan_manual(
        &self,
        session: &Session,
        session_id: &str,
    ) -> Result<TaskPayload, String> {
        let workspace = (self.workspace_of)(session_id).unwrap_or_default();
        let mut tree = self.load_tree(&workspace, session_id);
        if tree.pending.is_some() {
            return Err("已有整理任务在途".into());
        }
        let items = build_items(session.events(), tree.chaptered_until);
        if items.is_empty() {
            return Err("没有可整理的尾巴".into());
        }
        let users_total = memory_tree::user_count(session.events());
        tree.mark_manual(users_total);
        self.tree_store.save(&tree).ok();
        Ok(TaskPayload {
            session_id: session_id.to_string(),
            workspace,
            items,
            users_total,
            reason: OrganizeReason::Manual,
        })
    }

    /// 轮次收束钩子（agent_chat / 调度引擎在锁内调用）：判定通过即后台
    /// 整理，不阻塞返回值（extract / title 同款 fire-and-forget）。
    pub fn after_turn(self: &Arc<Self>, session: &Session, session_id: &str) {
        // M4 自动收起（§8.3 空间急救）：先于整理判定——泄压不依赖触发。
        self.auto_collapse(session, session_id);
        if let Some(task) = self.plan_auto(session, session_id) {
            self.spawn(task);
        }
    }

    /// 自动收起（M4 §8.3，缺省关）：collapse 开启 + 占用 ≥ 会话生效窗口
    /// 阈值 → 收起最早展开的章节（一次一个，逐步泄压）。窗口走 v2.137
    /// 四层链自动识别（模型行 ?? 目录 ?? 档位）；全未知不猜口径。
    fn auto_collapse(&self, session: &Session, session_id: &str) {
        let (enabled, threshold_pct) = self.settings.collapse_config();
        if !enabled {
            return;
        }
        let ov = self.model_overrides.get(session_id);
        let Some(window) = crate::settings::session_window(
            &self.settings.llm_config(),
            ov.as_ref(),
            Some(&self.catalog),
        ) else {
            return;
        };
        if window == 0 {
            return;
        }
        let (anchor, _) = Self::anchor(session);
        if anchor.saturating_mul(100) < window.saturating_mul(threshold_pct as u64) {
            return;
        }
        let workspace = (self.workspace_of)(session_id).unwrap_or_default();
        let mut tree = self.load_tree(&workspace, session_id);
        let Some(id) = tree.oldest_expanded() else {
            return;
        };
        tree.mark_collapsed(&id, now_ms());
        if self.tree_store.save(&tree).is_ok() {
            self.sync_tree_md(&workspace);
            tracing::info!(
                session = %session_id, chapter = %id,
                "自动收起（占用 {anchor} / {window} ≥ {threshold_pct}%）"
            );
        }
    }

    /// F1 tree.md 同步（文件化设计 §3：整理收束 → 更新人读层）：投影保存
    /// 成功后重建该项目 tree.md（读全部投影 + 旁车标题，人改保护在写侧）。
    /// 纯人类层——失败只 warn，总开关关 = 跳过。
    fn sync_tree_md(&self, workspace: &str) {
        if !self.settings.memory().files {
            return;
        }
        if let Err(e) = crate::memory_files::sync_workspace_tree_md(
            &self.files,
            &self.tree_store,
            &self.sessions_dir,
            workspace,
        ) {
            tracing::warn!("tree.md 同步失败（{workspace}）: {e}");
        }
    }

    /// `/整理` 命令入口（命令层锁内调用）。Err = 发起失败（文案直显）。
    pub fn organize_now(
        self: &Arc<Self>,
        session: &Session,
        session_id: &str,
    ) -> Result<(), String> {
        let task = self.plan_manual(session, session_id)?;
        self.spawn(task);
        Ok(())
    }

    /// 开发者观测面板（§3.36③）：树投影 + 活触发配置 + 尾巴/锚点现状。
    /// 持有会话锁的同步上下文调用（数据组装在 debug.rs）。
    pub fn tree_view(&self, session: &Session, session_id: &str) -> TreeDebug {
        self.assemble(session, session_id)
    }

    fn spawn(self: &Arc<Self>, task: TaskPayload) {
        let this = Arc::clone(self);
        tauri::async_runtime::spawn(async move {
            this.run_task(task).await;
        });
    }

    /// 管道入口（进程内在途去重后跑 `pipeline`，收束如实记日志）。
    async fn run_task(self: Arc<Self>, task: TaskPayload) {
        if !self
            .inflight
            .lock()
            .expect("inflight poisoned")
            .insert(task.session_id.clone())
        {
            return; // 同会话已在整理
        }
        let started = std::time::Instant::now();
        let outcome = self.pipeline(&task, started).await;
        self.inflight
            .lock()
            .expect("inflight poisoned")
            .remove(&task.session_id);
        match outcome {
            Ok((mode, status, chapters, usage)) => tracing::info!(
                session = %task.session_id, %mode, %status, chapters,
                input = usage.input, output = usage.output,
                ms = elapsed_ms(&started), "整理收束"
            ),
            Err(e) => tracing::warn!(session = %task.session_id, "整理未收束: {e}"),
        }
    }

    /// 管道本体：占位章节持久化 → 记忆管理器（超时 × 重试）→ 校验 → 降级规则
    /// 切分 → 落树。占位先落盘（崩在任意点 = 重启重投，§5.3）。
    async fn pipeline(
        self: &Arc<Self>,
        task: &TaskPayload,
        started: std::time::Instant,
    ) -> Result<Outcome, String> {
        let mem = self.settings.memory();
        let dev = self.settings.developer();
        let mut tree = self.load_tree(&task.workspace, &task.session_id);
        let from = task.items.first().unwrap().event;
        let to = task.items.last().unwrap().event;
        // pending 在位 = 本次运行正是它的执行者（崩溃重投，§5.3）——直接
        // 续跑；否则登记占位章节并落盘（此后任意点崩溃都可重投）。
        if tree.pending.is_none() {
            if tree.insert_placeholder(from, to, now_ms()).is_none() {
                return Err("占位登记失败（并发整理）".into());
            }
            self.tree_store.save(&tree)?;
        }

        // 记忆管理器调用：模型家族匹配提示词（§6.2；未配记忆管理器模型 = 主模型），
        // 超时 + 重试（§5.4）
        let model = mem
            .memory_manager_model
            .clone()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| self.settings.llm().model);
        let system = manager_prompt(family_of(&model));
        let user = memory_tree::transcript(&task.items);
        let model_override = mem
            .memory_manager_model
            .clone()
            .filter(|s| !s.trim().is_empty());
        let timeout = Duration::from_secs(dev.memory_manager_timeout_secs.max(1));
        let mut last_err = String::new();
        for _attempt in 0..=dev.memory_manager_max_retries {
            match tokio::time::timeout(
                timeout,
                self.client
                    .complete(system, &user, model_override.as_deref()),
            )
            .await
            {
                Err(_) => last_err = "记忆管理器超时".into(),
                Ok(Err(e)) => last_err = e,
                Ok(Ok((raw, usage))) => match parse_drafts(&raw, &task.items) {
                    Ok(drafts) => {
                        // 单写者补漏（走查实抓 F1 暴露的 M4 竞态）：收起可在
                        // 记忆管理器调用期间落盘——重载最新投影再产出，防旧快照
                        // 回写抹掉 collapsed_at（占位章节已持久化，重载即在）。
                        tree = self.load_tree(&task.workspace, &task.session_id);
                        let chapters = tree.build_chapters(&drafts, "recorder", now_ms());
                        let n = chapters.len();
                        tree.apply_organized(
                            chapters,
                            to,
                            Self::record(
                                task.reason,
                                "recorder",
                                "ok",
                                from,
                                to,
                                n,
                                usage,
                                elapsed_ms(&started),
                            ),
                        );
                        // 溢出整理（模式 B 例行维护）不碰反弹基线——§5.5
                        // 只度量「整理后占用立刻再超」的烧钱信号
                        if !matches!(task.reason, OrganizeReason::WindowOverflow) {
                            tree.note_organized(task.users_total);
                        }
                        self.tree_store.save(&tree)?;
                        self.sync_tree_md(&task.workspace);
                        self.notify_done(
                            &task.session_id,
                            task.users_total,
                            n,
                            "recorder",
                            "ok",
                            task.reason,
                        );
                        return Ok(("recorder".into(), "ok".into(), n, usage));
                    }
                    Err(e) => {
                        last_err = format!("输出不合规: {e}");
                        tracing::warn!(session = %task.session_id, "{last_err}，重试或降级");
                    }
                },
            }
        }
        // 降级：规则切分（§5.4，复用 ctx-compact「切点落在 User 之前」纪律）。
        // 重载同上——降级路径同样不得用旧快照回写。
        tree = self.load_tree(&task.workspace, &task.session_id);
        let drafts = rule_split_drafts(&task.items, dev.rule_chunk_tokens);
        let chapters = tree.build_chapters(&drafts, "rule", now_ms());
        let n = chapters.len();
        tree.apply_organized(
            chapters,
            to,
            Self::record(
                task.reason,
                "rule",
                "degraded",
                from,
                to,
                n,
                MemoryManagerUsage::default(),
                elapsed_ms(&started),
            ),
        );
        // 溢出整理不碰反弹基线（同上）
        if !matches!(task.reason, OrganizeReason::WindowOverflow) {
            tree.note_organized(task.users_total);
        }
        self.tree_store.save(&tree)?;
        self.sync_tree_md(&task.workspace);
        tracing::warn!(session = %task.session_id, "记忆管理器不可用（{last_err}），已降级规则切分");
        self.notify_done(
            &task.session_id,
            task.users_total,
            n,
            "rule",
            "degraded",
            task.reason,
        );
        Ok((
            "rule".into(),
            "degraded".into(),
            n,
            MemoryManagerUsage::default(),
        ))
    }
}

#[cfg(test)]
mod tests;

/// 组装点构造（state/init_state 调用）：记忆管理器客户端 = 设置路由的兼容
/// 直呼；㉘ 会话→项目解析器同源复用（(workspace, session) 键位对齐 28·A）；
/// 事件推送闭包由 state 组装（emit 调用留在 state/mod.rs，见其注释）；
/// v2.137 起持模型覆盖旁车 + 目录（自动收起的生效窗口四层链）。
pub fn build_organizer(
    settings: Arc<SettingsStore>,
    sessions_dir: PathBuf,
    workspace_of: WorkspaceResolver,
    data_root: PathBuf,
    model_overrides: Arc<crate::model_override::ModelOverrideStore>,
    catalog: Arc<crate::llm::catalog::CatalogStore>,
    notifier: OrganizeNotifier,
) -> Arc<Organizer> {
    Arc::new(Organizer::new(
        Arc::clone(&settings),
        sessions_dir,
        Arc::new(AppLlmMemoryManager::new(settings)),
        workspace_of,
        data_root,
        model_overrides,
        catalog,
        notifier,
    ))
}
