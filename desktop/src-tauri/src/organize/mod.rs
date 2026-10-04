//! 记忆树影子运行编排（M2，《Memory-KV Roadmap》；树 v2 §5.3 异步执行 +
//! 崩溃恢复、§5.4 降级、§5.5 熔断）。**影子纪律：请求组装一行不动**——
//! 本模块只在轮次收束后旁路观察会话日志，产物只落 `memory-tree/` 投影。
//!
//! 结构：`plan_auto` / `plan_manual`（同步决策，可测——含触发判定与反弹
//! 裁决，会写占位捆与熔断账本进投影文件）→ `run_task`（异步管道：记录员
//! 调用 × 超时重试 → 输出校验 → 不合规转规则切分 → 落树）。调用方持有
//! 会话锁时传 `&Session`（命令层 / agent_chat / 调度引擎三入口同形）。

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use memory_tree::{
    build_items, family_of, parse_drafts, recorder_prompt, rule_split_drafts, should_auto_organize,
    AnchorSource, MaterialItem, MemoryTree, OrganizeReason, OrganizeRecord, Pacing,
    ReboundDecision, RecorderClient, RecorderUsage, TreeStore, TriggerInput,
};
use session_store::{Session, SessionEvent};

use crate::settings::SettingsStore;
use crate::workspace::WorkspaceResolver;

mod client;
mod debug;
mod recovery;
pub use client::AppLlmRecorder;
pub use debug::TreeDebug;

/// 轮次收束后的整理编排器（AppState 持有 Arc；spawn 用 Arc receiver）。
pub struct Organizer {
    settings: Arc<SettingsStore>,
    sessions_dir: PathBuf,
    client: Arc<dyn RecorderClient>,
    workspace_of: WorkspaceResolver,
    tree_store: TreeStore,
    /// 会话级在途去重（进程内；跨重启由投影 pending 字段守卫）。
    inflight: StdMutex<HashSet<String>>,
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
type Outcome = (String, String, usize, RecorderUsage);

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
        client: Arc<dyn RecorderClient>,
        workspace_of: WorkspaceResolver,
        data_root: PathBuf,
    ) -> Self {
        Self {
            settings,
            sessions_dir,
            client,
            workspace_of,
            tree_store: TreeStore::new(&data_root),
            inflight: StdMutex::new(HashSet::new()),
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
        bundles: usize,
        usage: RecorderUsage,
        duration_ms: u64,
    ) -> OrganizeRecord {
        OrganizeRecord {
            ts: now_ms(),
            reason: reason.as_str().into(),
            mode: mode.into(),
            status: status.into(),
            from,
            to,
            bundles,
            recorder_input: usage.input,
            recorder_output: usage.output,
            duration_ms,
        }
    }

    /// 自动触发决策（轮次收束后、持有会话锁的同步上下文调用）。写路径：
    /// 反弹账本落投影文件；返回 None = 不整理。
    pub(crate) fn plan_auto(&self, session: &Session, session_id: &str) -> Option<TaskPayload> {
        let ctx = self.settings.context();
        let mem = self.settings.memory();
        let dev = self.settings.developer();
        let workspace = (self.workspace_of)(session_id).unwrap_or_default();
        let mut tree = self.load_tree(&workspace, session_id);
        if tree.paused_auto || tree.pending.is_some() {
            return None;
        }
        let items = build_items(session.events(), tree.bundled_until);
        let tail_tokens: u64 = items.iter().map(|i| i.tokens).sum();
        let (anchor_tokens, anchor_source) = Self::anchor(session);
        let reason = should_auto_organize(&TriggerInput {
            anchor_source,
            anchor_tokens,
            window_tokens: self.settings.effective_window().unwrap_or(0),
            threshold_pct: ctx.threshold_pct,
            pacing: Pacing::parse(&mem.organize_pacing),
            chunk_tokens: mem.organize_chunk_tokens,
            tail_tokens,
            tail_events: items.len(),
        })?;
        // 反弹熔断（§5.5）：连续 m 次反弹 → 暂停自动整理并跳过本次
        let users_total = memory_tree::user_count(session.events());
        if tree.on_auto_trigger(users_total, dev.organize_rebound_k, dev.organize_rebound_m)
            == ReboundDecision::Pause
        {
            tracing::warn!(session = %session_id, "整理反弹熔断：暂停本会话自动整理（/整理 不受限）");
            self.tree_store.save(&tree).ok();
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
        let items = build_items(session.events(), tree.bundled_until);
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
        if let Some(task) = self.plan_auto(session, session_id) {
            self.spawn(task);
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
            Ok((mode, status, bundles, usage)) => tracing::info!(
                session = %task.session_id, %mode, %status, bundles,
                input = usage.input, output = usage.output,
                ms = elapsed_ms(&started), "整理收束"
            ),
            Err(e) => tracing::warn!(session = %task.session_id, "整理未收束: {e}"),
        }
    }

    /// 管道本体：占位捆持久化 → 记录员（超时 × 重试）→ 校验 → 降级规则
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
        // 续跑；否则登记占位捆并落盘（此后任意点崩溃都可重投）。
        if tree.pending.is_none() {
            if tree.insert_placeholder(from, to, now_ms()).is_none() {
                return Err("占位登记失败（并发整理）".into());
            }
            self.tree_store.save(&tree)?;
        }

        // 记录员调用：模型家族匹配提示词（§6.2；未配记录员模型 = 主模型），
        // 超时 + 重试（§5.4）
        let model = mem
            .recorder_model
            .clone()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| self.settings.llm().model);
        let system = recorder_prompt(family_of(&model));
        let user = memory_tree::transcript(&task.items);
        let model_override = mem.recorder_model.clone().filter(|s| !s.trim().is_empty());
        let timeout = Duration::from_secs(dev.recorder_timeout_secs.max(1));
        let mut last_err = String::new();
        for _attempt in 0..=dev.recorder_max_retries {
            match tokio::time::timeout(
                timeout,
                self.client
                    .complete(system, &user, model_override.as_deref()),
            )
            .await
            {
                Err(_) => last_err = "记录员超时".into(),
                Ok(Err(e)) => last_err = e,
                Ok(Ok((raw, usage))) => match parse_drafts(&raw, &task.items) {
                    Ok(drafts) => {
                        let bundles = tree.build_bundles(&drafts, "recorder", now_ms());
                        let n = bundles.len();
                        tree.apply_organized(
                            bundles,
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
                        tree.note_organized(task.users_total);
                        self.tree_store.save(&tree)?;
                        return Ok(("recorder".into(), "ok".into(), n, usage));
                    }
                    Err(e) => {
                        last_err = format!("输出不合规: {e}");
                        tracing::warn!(session = %task.session_id, "{last_err}，重试或降级");
                    }
                },
            }
        }
        // 降级：规则切分（§5.4，复用 ctx-compact「切点落在 User 之前」纪律）
        let drafts = rule_split_drafts(&task.items, dev.rule_chunk_tokens);
        let bundles = tree.build_bundles(&drafts, "rule", now_ms());
        let n = bundles.len();
        tree.apply_organized(
            bundles,
            to,
            Self::record(
                task.reason,
                "rule",
                "degraded",
                from,
                to,
                n,
                RecorderUsage::default(),
                elapsed_ms(&started),
            ),
        );
        tree.note_organized(task.users_total);
        self.tree_store.save(&tree)?;
        tracing::warn!(session = %task.session_id, "记录员不可用（{last_err}），已降级规则切分");
        Ok((
            "rule".into(),
            "degraded".into(),
            n,
            RecorderUsage::default(),
        ))
    }
}

#[cfg(test)]
mod tests;

/// 组装点构造（state/init_state 调用）：记录员客户端 = 设置路由的 genai
/// 直呼；㉘ 会话→项目解析器同源复用（(workspace, session) 键位对齐 28·A）。
pub fn build_organizer(
    settings: Arc<SettingsStore>,
    sessions_dir: PathBuf,
    workspace_of: WorkspaceResolver,
    data_root: PathBuf,
) -> Arc<Organizer> {
    Arc::new(Organizer::new(
        Arc::clone(&settings),
        sessions_dir,
        Arc::new(AppLlmRecorder::new(settings)),
        workspace_of,
        data_root,
    ))
}
