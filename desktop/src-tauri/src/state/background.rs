//! 组装点·编排器与后台任务簇（M2/M3 §3.37 + ⑭ 调度引擎 + F1 §6 + 批 D §6.3）：
//! 整理事件推送闭包 → `Organizer` 构造 → 调度引擎 spawn → 在途整理重投 →
//! 记忆文件化首启导出 → 孤儿资产 GC 巡检。返回 `AppState` 要接的 `organizer`。
//!
//! 语句自 `state/mod.rs` 的 `init_state` **原样搬出**（批 F-3b·F5 结构性拆分），
//! 一条未重排。四条不能动的规矩：
//! 1. **emit 调用只落在本模块**：桌面单测不引用 state，tauri 运行时符号（连它的
//!    窗口栈 tao）就不会进测试二进制——无 v6 清单的进程链接 tao 后 comctl32 缺
//!    `SetWindowSubclass`/`TaskDialogIndirect`，加载即 `ENTRYPOINT_NOT_FOUND`
//!    （这条是历史上真崩过的形状，不是理论洁癖）；
//! 2. 后台化一律走 `tauri::async_runtime::spawn`，不用 `tokio::spawn`：setup
//!    上下文里没有 tokio reactor，直接 spawn 会 panic（v2.130 走查实抓）；
//! 3. `recover_pending` 必须在 `Organizer` 构造之后、且在调度引擎起来之后跑，
//!    重投的整理任务要与引擎的补跑不冲突；
//! 4. GC 循环每轮**重读设置**（`gc_threshold_days`/`auto_gc`）= 热调随下一次
//!    tick 生效，快照成启动值就是「设置页骗人」。
//!
//! 调度引擎本身是 `crate::scheduler::spawn_engine`（14 参现成 API，本批不动它）。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use kernel_agent::Kernel;
use session_store::Session;
use tauri::AppHandle;

use crate::events::EventSink;
use crate::settings::SettingsStore;

pub(crate) struct Init {
    pub app: AppHandle,
    pub settings: Arc<SettingsStore>,
    pub sessions_dir: PathBuf,
    pub root: PathBuf,
    pub workspace_of: crate::workspace::WorkspaceResolver,
    pub model_overrides: Arc<crate::model_override::ModelOverrideStore>,
    pub catalog: Arc<crate::llm::catalog::CatalogStore>,
    pub fragments: Arc<crate::fragments::FragmentPending>,
    pub kernel: Arc<Kernel>,
    pub sink: Arc<dyn EventSink>,
    pub usage: Arc<crate::usage::UsageRecorder>,
    pub sessions: Arc<Mutex<HashMap<String, Arc<tokio::sync::Mutex<Session>>>>>,
    pub last_chat: Arc<std::sync::RwLock<Option<String>>>,
    pub tree_assembler: Arc<crate::organize::TreeAssembler>,
    pub stop_flags: Arc<crate::stop::StopFlags>,
    pub schedules: Option<Arc<crate::scheduler::ScheduleDb>>,
    pub memory: Arc<crate::memory::MemoryStore>,
    pub tree_store: memory_tree::TreeStore,
    pub memory_files: Arc<crate::memory_files::MemoryFiles>,
    pub files_on: Arc<dyn Fn() -> bool + Send + Sync>,
    pub assets: Arc<crate::assets::AssetStore>,
    /// S2 刀⑤：熔断表由组装点注入，与滑窗保护网（`state/waterfall.rs` 那一环）**同一份**。
    /// 它不在这里 new：两张表 = 主层已熔断而保护网没醒（或反过来），而两档在界面上是
    /// 同一个症状。为什么这张表是共用件 = `compact::Failures` 头注。
    pub compact_failures: Arc<crate::compact::Failures>,
    /// S2 刀⑧：超窗拒收读数表。同一张表的**读侧**（写侧在 `state/llm_stack.rs` 那台
    /// AppLlm 上）。不在这里 new 的理由与上面那枚完全相同 = `compact::Overflow` 头注。
    pub compact_refusals: Arc<crate::compact::Overflow>,
}

pub(crate) struct Stack {
    pub organizer: Arc<crate::organize::Organizer>,
    pub compactor: Arc<crate::compact::deep::Compactor>,
}

pub(crate) fn build(init: Init) -> Stack {
    let Init {
        app,
        settings,
        sessions_dir,
        root,
        workspace_of,
        model_overrides,
        catalog,
        fragments,
        kernel,
        sink,
        usage,
        sessions,
        last_chat,
        tree_assembler,
        stop_flags,
        schedules,
        memory,
        tree_store,
        memory_files,
        files_on,
        assets,
        compact_failures,
        compact_refusals,
    } = init;

    // M2 记忆树影子运行编排器：记忆管理器客户端 = 设置路由的兼容客户端直呼；
    // ㉘ 会话→项目解析器同源复用（(workspace, session) 键位对齐 28·A）。
    // 整理事件推送闭包在此组装（M3 §3.37③）：emit 调用只落在本模块——
    // 桌面单测不引用 state，tauri 运行时符号（及其窗口栈 tao）就不会进
    // 测试二进制；无 v6 清单的进程链接 tao 后 comctl32 缺
    // SetWindowSubclass/TaskDialogIndirect，加载即 ENTRYPOINT_NOT_FOUND。
    let organizer_notifier: crate::organize::OrganizeNotifier = {
        let app = app.clone();
        std::sync::Arc::new(move |event, payload| {
            use tauri::Emitter;
            if let Err(e) = app.emit(event, payload) {
                tracing::warn!("{event} 推送失败: {e}");
            }
        })
    };
    // 一条直呼通路 = 一个客户端实例（S2 刀④c）：整理章节总结（`organize/task.rs`、
    // `organize/backfill.rs`）与主层压缩摘要（`compact/deep.rs`）共用
    // `AppLlmMemoryManager`。构造点从 `build_organizer` 提到组装点，为的是让**共用件
    // 不归属于任何一个使用方**——organize 在 S4 整面退役而压缩留下（刀④a 搬文件的
    // 同一条理由，这一步搬的是实例）。判据 = SS19b/b2 的期望随之改判（organize 面
    // 构造 0 次、组装点 1 次）。
    let aux: Arc<dyn memory_tree::MemoryManagerClient> =
        Arc::new(crate::llm::AppLlmMemoryManager::new(Arc::clone(&settings)));
    let organizer = crate::organize::build_organizer(
        Arc::clone(&settings),
        sessions_dir.clone(),
        Arc::clone(&aux),
        Arc::clone(&workspace_of),
        root.clone(),
        Arc::clone(&model_overrides),
        Arc::clone(&catalog),
        organizer_notifier,
        Some(Arc::clone(&fragments)),
        // 事实卡落库句柄（MR §5 提案 A · 队列 Q3）：与注入面/管理页同一个
        // `MemoryStore` ⇒ 卡片写完，索引注入与记忆页立刻读得到同一行（不分两套库）
        Arc::clone(&memory),
    );
    let compactor = Arc::new(crate::compact::deep::Compactor::new(
        Arc::clone(&settings),
        Arc::clone(&aux),
        // 熔断表与直呼通路同一条纪律（刀⑤a）：它是**两个使用方共用的状态**，
        // 所以构造点在组装点，不在任何一个使用方体内。
        compact_failures,
        // 拒收读数（刀⑧）：同一张表的读侧——被超窗拒掉的那一次没有用量上报，
        // 只靠锚点这一层就永远追不上已经涨过的窗口。
        compact_refusals,
    ));
    // §5.3 崩溃恢复：扫描投影里的在途整理任务重投（会话已删 = 终局放弃）

    // 6. ⑭ 调度引擎（库可用时）：1s tick；错过的 run_at 落在过去，首次 tick 的 due() 天然覆盖启动补跑
    if let Some(db) = &schedules {
        crate::scheduler::spawn_engine(
            Arc::clone(db),
            Arc::clone(&kernel),
            Arc::clone(&sink),
            Arc::clone(&usage),
            Arc::clone(&settings),
            Arc::clone(&model_overrides),
            Arc::clone(&catalog),
            Arc::clone(&sessions),
            sessions_dir.clone(),
            Arc::clone(&last_chat),
            app.clone(),
            Arc::clone(&organizer),
            Arc::clone(&compactor),
            Arc::clone(&tree_assembler),
            Arc::clone(&stop_flags),
            Arc::clone(&fragments),
        );
    }
    // 在途整理任务重投（后台，不阻塞窗口出现）
    organizer.recover_pending();

    // F1 首启导出（文件化设计 §6）+ F2 启动 reconcile（先对账再补缺）：
    // 后台线程幂等，总开关关 = 什么都不做。
    let roots_on = {
        let s = Arc::clone(&settings);
        Arc::new(move || s.fs_roots()) as Arc<dyn Fn() -> Vec<String> + Send + Sync>
    };
    crate::memory_files::spawn_initial_export(
        Arc::clone(&memory),
        tree_store.clone(),
        (*memory_files).clone(),
        sessions_dir.clone(),
        Arc::clone(&files_on),
        roots_on,
    );

    // 批 D 孤儿资产 GC（§6.3）：autoGc 开时启动跑一遍 + 每 24h 后台巡检
    //（手动命令 asset_gc 不受开关限制）。fire-and-forget；gcDays 每轮重读
    // 设置（热调随下一次 tick 生效）。
    if settings.multimodal().auto_gc {
        let gc_assets = Arc::clone(&assets);
        let gc_settings = Arc::clone(&settings);
        let gc_root = root.clone();
        // setup 阶段不在 tokio 运行时上下文里——必须走 tauri 的 async
        // runtime 句柄（tokio::spawn 直接 panic，走查实抓）
        tauri::async_runtime::spawn(async move {
            loop {
                let days = gc_settings.multimodal().gc_threshold_days;
                let report = crate::assets::gc::run_gc(
                    gc_assets.root(),
                    &gc_root.join("sessions"),
                    &gc_root.join("sub-transcripts"),
                    days,
                );
                if report.scanned > 0 || report.removed > 0 {
                    tracing::info!(
                        scanned = report.scanned,
                        removed = report.removed,
                        "asset gc tick"
                    );
                }
                tokio::time::sleep(std::time::Duration::from_secs(24 * 3600)).await;
            }
        });
    }

    Stack {
        organizer,
        compactor,
    }
}
