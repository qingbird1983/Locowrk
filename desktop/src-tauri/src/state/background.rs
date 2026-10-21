//! 组装点·后台任务簇（M2/M3 §3.37 + ⑭ 调度引擎 + 批 D §6.3）：
//! 压缩器构造 → 调度引擎 spawn → 孤儿资产 GC 巡检。
//! S4 刀③c：记忆文件化首启导出（F1 §6）随记忆整线退役，本簇不再持有记忆句柄。
//!
//! 语句自 `state/mod.rs` 的 `init_state` **原样搬出**（批 F-3b·F5 结构性拆分），
//! 一条未重排。三条不能动的规矩：
//! 1. **emit 调用只落在本模块**：桌面单测不引用 state，tauri 运行时符号（连它的
//!    窗口栈 tao）就不会进测试二进制——无 v6 清单的进程链接 tao 后 comctl32 缺
//!    `SetWindowSubclass`/`TaskDialogIndirect`，加载即 `ENTRYPOINT_NOT_FOUND`
//!    （这条是历史上真崩过的形状，不是理论洁癖）；
//! 2. 后台化一律走 `tauri::async_runtime::spawn`，不用 `tokio::spawn`：setup
//!    上下文里没有 tokio reactor，直接 spawn 会 panic（v2.130 走查实抓）；
//! 3. GC 循环每轮**重读设置**（`gc_threshold_days`/`auto_gc`）= 热调随下一次
//!    tick 生效，快照成启动值就是「设置页骗人」。
//!
//! 调度引擎本身是 `crate::scheduler::spawn_engine`（一长串依赖直给的现成 API）。

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
    pub model_overrides: Arc<crate::model_override::ModelOverrideStore>,
    pub catalog: Arc<crate::llm::catalog::CatalogStore>,
    pub fragments: Arc<crate::fragments::FragmentPending>,
    pub kernel: Arc<Kernel>,
    pub sink: Arc<dyn EventSink>,
    pub usage: Arc<crate::usage::UsageRecorder>,
    pub sessions: Arc<Mutex<HashMap<String, Arc<tokio::sync::Mutex<Session>>>>>,
    pub last_chat: Arc<std::sync::RwLock<Option<String>>>,
    // S3 刀④a：这里的 `tree_assembler` 已删——它只为喂 `spawn_engine` 的
    // `tree_assembler` 参数，而那个参数喂的是 `run_turn` 的 `tree`（第二把分子）。
    pub stop_flags: Arc<crate::stop::StopFlags>,
    pub schedules: Option<Arc<crate::scheduler::ScheduleDb>>,
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
    pub compactor: Arc<crate::compact::deep::Compactor>,
}

pub(crate) fn build(init: Init) -> Stack {
    let Init {
        app,
        settings,
        sessions_dir,
        root,
        model_overrides,
        catalog,
        fragments,
        kernel,
        sink,
        usage,
        sessions,
        last_chat,
        stop_flags,
        schedules,
        assets,
        compact_failures,
        compact_refusals,
    } = init;

    // 压缩用的那一台 `AppLlm`（**与主对话那台分开的两个实例**）。
    //
    // 为什么不用同一台：主对话那台是以 `Arc<dyn LlmService>` 注册进内核 Context
    // 的，没有 downcast 回具体类型的路（`LlmService` 不是 `Any`）⇒ 拿不到
    // `Arc<AppLlm>`；而 `Compactor` 要的 `complete_compaction`（总时长预算 +
    // 不推重试事件 + delta 走自己回调）是 `AppLlm` 的固有方法。
    //
    // 两台的差别只有两处，且**都是故意的**：① 这一台不接重试通知（压缩重试不许推
    // `agent://llm-retry`——那会让前端 `resetAssistant` 清掉主对话的 assistant
    // 气泡）；② 这一台不记拒收读数（压缩请求自己被拒是收缩矩阵内部的事，记进
    // 会话占用读数会把"这次压缩有多大"错当成"会话占了多少"）。其余（设置、
    // 会话级模型覆盖、能力目录、资产、落盘句柄、停止标记）都是**同一批句柄**
    // ⇒ 解析结果逐字相同。
    let compaction_llm = Arc::new(
        crate::llm::AppLlm::with_overrides(Arc::clone(&settings), Arc::clone(&model_overrides))
            .with_multimodal(Arc::clone(&assets), Arc::clone(&catalog))
            .with_spill(Arc::new(crate::spill::SpillStore::new(&root)))
            .with_stop(Arc::clone(&stop_flags)),
    );
    let compactor = Arc::new(crate::compact::deep::Compactor::new(
        Arc::clone(&settings),
        // 请求装配：**与主对话轮同一条链**（内核 `prepare_request`）。
        Arc::clone(&kernel),
        Arc::clone(&compaction_llm) as Arc<dyn crate::compact::deep::CompactionLlm>,
        // 熔断表与拒收读数是**两个使用方共用的状态**（组装点构造，两边各收一枚
        // `Arc`；同 `compact::Failures` 头注那条纪律）。
        compact_failures,
        compact_refusals,
        // 停止标记（specs/12 刀①）：压缩在等重试与吐字两处都查它 ⇒ 用户点
        // 「停止生成」能停掉这一条用户看不见的请求。同一柄（组装点已建）。
        Some(Arc::clone(&stop_flags)),
        // 会话日志目录（specs/13 刀⑥）：压缩这一族**自己落盘** ⇒ 手动那条不再
        // 需要命令层替它补一次落盘（漏一次就是丢一次数据）。
        Some(sessions_dir.clone()),
    ));

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
            Arc::clone(&compactor),
            Arc::clone(&stop_flags),
            Arc::clone(&fragments),
        );
    }

    // S4 刀③c：F1 首启导出 / F2 启动 reconcile 随记忆整线退役（`memory_files`
    // 模块已删）。这里不再有幂等对账线程。

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

    Stack { compactor }
}
