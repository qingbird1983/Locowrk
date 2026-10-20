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

    // 一条直呼通路 = 一个客户端实例：S4 刀③b 起整理线已整面退役，这个实例
    // 只服务主层压缩摘要（`compact/deep.rs`），取主模型端点与档位（`cfg_main`）。
    // 构造点仍留在组装点：压缩器是 runner 的 step 边界依赖，客户端不该由 runner
    // 自己 new（测试面要能注入假件）。
    let summary_aux: Arc<dyn crate::llm::SummaryClient> =
        Arc::new(crate::llm::AppLlmSummarizer::new(Arc::clone(&settings)));
    let compactor = Arc::new(crate::compact::deep::Compactor::new(
        Arc::clone(&settings),
        summary_aux,
        // 熔断表与直呼通路同一条纪律（刀⑤a）：它是**两个使用方共用的状态**，
        // 所以构造点在组装点，不在任何一个使用方体内。
        compact_failures,
        // 拒收读数（刀⑧）：同一张表的读侧——被超窗拒掉的那一次没有用量上报，
        // 只靠锚点这一层就永远追不上已经涨过的窗口。
        compact_refusals,
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
