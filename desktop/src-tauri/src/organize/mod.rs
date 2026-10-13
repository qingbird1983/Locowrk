//! 记忆树影子运行编排（M2，《Memory-KV Roadmap》；树 v2 §5.3 异步执行 +
//! 崩溃恢复、§5.4 降级、§5.5 熔断）。**影子纪律：请求组装一行不动**——
//! 本模块只在轮次收束后旁路观察会话日志，产物只落 `memory-tree/` 投影。
//!
//! 结构（v2.157 批 F-7 目录化拆分后）：`trigger`（同步决策：plan_auto /
//! plan_manual，含触发判定、溢出整理与反弹裁决）→ `task`（异步管道：记忆管理器
//! 调用 × 超时重试 → 输出校验 → 不合规转规则切分 → 落树）；`collapse` 管空间
//! 急救（M4 自动收起），`pressure` 管**轮内水位泄压**（队列 Q8b：step 边界同步
//! 一次规则切分，不等模型），`events` 管对模型与界面的告知。本文件留编排器本体：
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
use session_store::{Session, SessionEvent};

use crate::settings::SettingsStore;
use crate::workspace::WorkspaceResolver;

use crate::timeutil::now_ms;

mod assemble;
mod client;
mod collapse;
mod debug;
mod edit;
mod events;
mod facts;
mod pressure;
mod progress;
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
    /// 事实卡落库句柄（MR §5 提案 A · 队列 Q3）：整理副产物写进记忆层走的就是
    /// 这个 store（宿主 upsert + md 双写）。必需字段而非 Option——它是**写路径**，
    /// 拿不到 store 的装配等于事实卡静默不落地，那种"看起来在跑其实没落"比装配
    /// 期报错难查得多；测试夹具用临时数据根的 `MemoryStore::open` 实例。
    memory: Arc<crate::memory::MemoryStore>,
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
        memory: Arc<crate::memory::MemoryStore>,
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
            memory,
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

    /// 尾巴材料（审计队列 **Q3b** · v2.187）：`memory_tree::build_items(events, base)`
    /// 的 `base` 是「**切片首元素**在会话日志里的绝对位」，契约要求调用方自己传尾巴
    /// 切片（头注原话：「调用方传尾巴切片时带出偏移」）。`recovery.rs`、`view.rs` 和
    /// `trigger.rs` 的 `WindowOverflow` 支一直是这个形状；漏写的是 `plan_auto` /
    /// `plan_manual` / `tree_view` 三处——把**全量事件**连同游标一起传进去，于是每次
    /// 整理都把整段历史重喂记录员、`event` 记成「游标 + 下标」的假绝对位、
    /// `chaptered_until` 被推到日志末尾之外。这里把"取尾巴"收成一个点，三个调用方
    /// 不再各自拼切片（同一属性只剩一个出处，才是"修好了"的机器形状）。
    ///
    /// 起点钳到事件数：越界宁可给空尾巴，也不 panic、更不把全段重喂。但**空尾巴不等于
    /// 修好了**——见 [`Self::clamp_cursor`]，越界游标必须当场钳回并落盘，否则该会话
    /// 在切片写法修好之后仍然永久静默。
    fn tail_material(events: &[SessionEvent], chaptered_until: usize) -> (&[SessionEvent], usize) {
        let start = chaptered_until.min(events.len());
        (&events[start..], start)
    }

    /// 游标自愈（Q3b 的后一半，缺它这条缺陷收不干净）：`MemoryTree::apply_organized`
    /// 用 `chaptered_until = max(to + 1)` **单调不回退**，所以历史缺陷推到 13 的那个
    /// 值永远不会自己退回来——只修切片不改游标，`min()` 每次把起点顶到当前事件数，
    /// 新来的事件反倒永远落在游标之下 ⇒ 尾巴恒空、该会话再也不整理，缺陷从"重喂全段"
    /// 变成"永久静默"，是更坏的一种。这里把越界游标钳回事件数并交调用方落盘：钳的
    /// 语义是"越界那截账作废，从现在起重新算尾巴"，不是重整理历史。
    ///
    /// 返回 `true` = 发生过钳制。损坏是既成事实 ⇒ 必须给一条 warn 说得出来，
    /// 并且**当场 save**（调用方 `WindowOverflow` 支会提前 return，不能指望它后面的
    /// 那次 save）。
    fn clamp_cursor(tree: &mut MemoryTree, events_len: usize) -> bool {
        if tree.chaptered_until <= events_len {
            return false;
        }
        tracing::warn!(
            session = %tree.session_id,
            from = tree.chaptered_until,
            to = events_len,
            "整理游标越出会话日志末尾（Q3b 的历史损坏投影）：已钳回事件数并落盘，越界那截账作废"
        );
        tree.chaptered_until = events_len;
        true
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
            // 连续补跑的账在**这里**清（队列 Q6 · 判据 `natural_trigger_resets_the_rerun_budget`）：
            // 能走到这句 = 一次**新的自然触发**到了，说明整理跟得上对话节奏 ⇒ 熔断上限重新计。
            // 不写进 `plan_auto` 内部，是因为补跑路径也调 `plan_auto`——写在那里面就等于
            // 续跑自己抹自己的账，上限永远打不到（变异 M3 的靶）。
            self.clear_rerun_streak(&task.workspace, &task.session_id);
            self.spawn(task);
        }
    }

    /// 从磁盘会话日志重读会话（队列 Q6：补跑用得上，形状照崩溃恢复那条通路）。
    /// 定位失败 / 日志已删 = `None`，调用方按"诚实放弃"处理。
    /// ⚠ `recovery.rs` **没有**复用这里：它要把「定位失败」与「日志已删」分成两条
    /// 不同的 warn、各记一条 `aborted` 台账——撤占位是同一件事，取证文案不是，
    /// 合成就丢了粒度（同一对 API 被两处调用 ≠ 同一逻辑写两遍）。
    fn session_of(&self, session_id: &str) -> Option<Session> {
        let path = session_store::session_file(&self.sessions_dir, session_id).ok()?;
        session_store::load_session(&path).ok().map(|(s, _)| s)
    }

    /// 复位连续补跑账（含消费掉「待整理」标记）。
    ///
    /// 为什么这次触发可以顺手清标记：`plan_auto` 在途期只会记标记并返回 `None`，
    /// 所以能走到这里的必然是「没有在途任务」的轮次，而它产出的材料是从游标起的
    /// **整条尾巴**——被标记的那次触发想整理的内容天然包含在里面，清掉不丢账。
    /// 投影本来就声明为可丢（`MemoryTree::organize_needed` 的头注），这里只是让它
    /// 在"已经补上了"的时刻干净。
    fn clear_rerun_streak(&self, workspace: &str, session_id: &str) {
        let mut tree = self.load_tree(workspace, session_id);
        if tree.rerun_streak == 0 && !tree.organize_needed {
            return; // 正常节奏下绝大多数轮次都是零：不做无谓的投影往返
        }
        tree.rerun_streak = 0;
        tree.organize_needed = false;
        self.tree_store.save(&tree).ok();
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
    memory: Arc<crate::memory::MemoryStore>,
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
        memory,
    ))
}
