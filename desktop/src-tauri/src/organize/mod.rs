//! 记忆树影子运行编排（M2，《Memory-KV Roadmap》；树 v2 §5.3 异步执行 +
//! 崩溃恢复、§5.4 降级、§5.5 熔断）。**影子纪律：请求组装一行不动**——
//! 本模块只在轮次收束后旁路观察会话日志，产物只落 `memory-tree/` 投影。
//!
//! 结构（v2.157 批 F-7 目录化拆分后）：`trigger`（同步决策：plan_auto /
//! plan_manual，含触发判定、溢出整理与反弹裁决）→ `task`（异步管道：记忆管理器
//! 调用 × 超时重试 → 输出校验 → 不合规转规则切分 → 落树）；`collapse` 管空间
//! 急救（M4 自动收起），`events` 管对模型与界面的告知。本文件留编排器本体：
//! 状态与构造、投影与台账小工具、三个门面入口、组装点构造。调用方持有会话锁
//! 时传 `&Session`（命令层 / agent_chat / 调度引擎三入口同形）。
//!
//! 拆分是机械搬用：每个顶层项与 `impl Organizer` 的每个方法（含其文档注释与属性
//! 行）整块搬进目标簇，块内字节原样、簇内保持原相对次序；唯一一处字面改动 = 五个
//! 跨簇被调用的私有方法加 `pub(super)`（**可见面与拆分前逐字相同**）。判据脚本
//! `tmp/v157-*` 留在台账里可复跑。

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex as StdMutex};

use memory_tree::{
    AnchorSource, MemoryManagerClient, MemoryManagerUsage, MemoryTree, OrganizeReason,
    OrganizeRecord, TreeStore,
};
use session_store::Session;

use crate::settings::SettingsStore;
use crate::workspace::WorkspaceResolver;

use crate::timeutil::now_ms;

mod assemble;
mod client;
mod collapse;
mod debug;
mod edit;
mod events;
mod project_history;
mod recovery;
mod search_titles;
mod summary;
mod task;
mod tree_tools;
mod trigger;
mod view;
pub use assemble::TreeAssembler;
pub use client::AppLlmMemoryManager;
pub use debug::TreeDebug;
pub use events::OrganizeNotifier;
pub use project_history::ProjectHistoryTool;
pub use search_titles::SearchTitlesTool;
pub use tree_tools::{MemoryCollapseTool, MemoryExpandTool};
pub(crate) use trigger::TaskPayload;
pub use view::{OriginalItem, ProjectTreeView, TreeChaptersView, TreeSummaryView};

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
    /// 会话态碎片 pending 表（提示词分层设计 §4.4）：整理收束置位
    /// 「历史去哪了、怎么找回」碎片，下一轮开场由 FragmentInject 拼接。
    /// None = 测试装配（不置位）。
    fragments: Option<Arc<crate::fragments::FragmentPending>>,
}

fn elapsed_ms(started: &std::time::Instant) -> u64 {
    started.elapsed().as_millis() as u64
}

impl Organizer {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        settings: Arc<SettingsStore>,
        sessions_dir: PathBuf,
        client: Arc<dyn MemoryManagerClient>,
        workspace_of: WorkspaceResolver,
        data_root: PathBuf,
        model_overrides: Arc<crate::model_override::ModelOverrideStore>,
        catalog: Arc<crate::llm::catalog::CatalogStore>,
        notifier: OrganizeNotifier,
        fragments: Option<Arc<crate::fragments::FragmentPending>>,
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
            fragments,
        }
    }

    fn load_tree(&self, workspace: &str, session_id: &str) -> MemoryTree {
        self.tree_store
            .load(workspace, session_id)
            .unwrap_or_else(|| MemoryTree::new(workspace, session_id))
    }

    /// 占用锚点（§4.2：优先 provider 真实 input——日志最后一条 LlmUsage；
    /// 无上报回落字符估算，锚点来源如实标注）。扫描逻辑与用量面共用
    /// `runner::reported_input_tokens`（v2.138 统一锚点：进度条 = 触发 =
    /// 自动收起，一把尺）。
    fn anchor(session: &Session) -> (u64, AnchorSource) {
        match crate::runner::reported_input_tokens(session) {
            Some(input) => (input, AnchorSource::LlmUsage),
            None => (
                kernel_agent::estimate_tokens(&session.derive_messages()),
                AnchorSource::Estimate,
            ),
        }
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

    /// 轮次收束钩子（agent_chat / 调度引擎在锁内调用）：判定通过即后台
    /// 整理，不阻塞返回值（extract / title 同款 fire-and-forget）。
    pub fn after_turn(self: &Arc<Self>, session: &Session, session_id: &str) {
        // M4 自动收起（§8.3 空间急救）：先于整理判定——泄压不依赖触发。
        self.auto_collapse(session, session_id);
        if let Some(task) = self.plan_auto(session, session_id) {
            self.spawn(task);
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
}

#[cfg(test)]
mod tests;

/// 组装点构造（state/init_state 调用）：记忆管理器客户端 = 设置路由的兼容
/// 直呼；㉘ 会话→项目解析器同源复用（(workspace, session) 键位对齐 28·A）；
/// 事件推送闭包由 state 组装（emit 调用留在 state/mod.rs，见其注释）；
/// v2.137 起持模型覆盖旁车 + 目录（自动收起的生效窗口四层链）。
#[allow(clippy::too_many_arguments)]
pub fn build_organizer(
    settings: Arc<SettingsStore>,
    sessions_dir: PathBuf,
    workspace_of: WorkspaceResolver,
    data_root: PathBuf,
    model_overrides: Arc<crate::model_override::ModelOverrideStore>,
    catalog: Arc<crate::llm::catalog::CatalogStore>,
    notifier: OrganizeNotifier,
    fragments: Option<Arc<crate::fragments::FragmentPending>>,
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
        fragments,
    ))
}
