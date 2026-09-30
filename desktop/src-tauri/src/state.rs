//! 共享状态组装（DESIGN.md §12.3）：Context + PluginRegistry + Kernel
//! （含审批门）+ 会话表 + PluginService（阶段七）。
//!
//! 启动顺序固定（§11.2 对账式恢复 / §12.3 备注）：
//! 1. `startup_reconcile`——清 tombstone、lockfile 与磁盘对账、恢复
//!    enabled 插件。必须在加载任何插件之前调用（此时无文件锁）
//! 2. 开发态装载（`plugins/`，内核开发用，不经 lockfile）
//! 3. 安装态已由第 1 步恢复完毕（`~/.Locowrk`，lockfile 管理）

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use cordis_core::Context;
use kernel_agent::{
    points, Audit, CircuitBreaker, CompactConfig, CtxCompact, JsonlAuditSink, Kernel,
    MemoryInject, MemoryInjectConfig, LlmService, Middleware, Timeout, ToolExposure,
    ToolExposureConfig, ToolSource, Tracing,
};
use plugin_host::{PluginRegistry, RegistryToolSource};
use plugin_service::{LocalDirMarket, PluginService};
use session_store::Session;
use tauri::AppHandle;

use crate::approval::ApprovalHub;
use crate::data_root;
use crate::demo_tools::SendEmailTool;
use crate::events::{forward_plugin_states, EventSink, TauriSink};
use crate::llm::AppLlm;
use crate::memory::MemoryStore;
use crate::plugins_root;
use crate::search_tools::SearchToolsTool;
use crate::settings::SettingsStore;
use crate::tool_source::KernelToolSource;
use crate::usage::{UsageDb, UsageEwma, UsageRecorder};

pub struct AppState {
    /// 阶段八的窗口级命令（对话框、多窗口）从这里取句柄；当前
    /// 事件出口已是共享 sink，字段暂无读取方。
    #[allow(dead_code)]
    pub app: AppHandle,
    pub kernel: Arc<Kernel>,
    /// 插件管理唯一入口（UI 不得直碰 registry，AGENTS.md 架构边界）。
    pub plugins: Arc<PluginService>,
    /// 审批中心（agent_approve / list_pending_approvals 从这里走）。
    pub approval: Arc<ApprovalHub>,
    /// UI 事件出口（§12.2）：runner 与审批请求共用一个 sink。
    pub sink: Arc<dyn EventSink>,
    /// session_id → 会话锁。会话在锁内串行聊天（tokio Mutex 可跨 await）；
    /// 外层 std Mutex 只护 map 存取，绝不跨 await 持有。
    pub sessions: Mutex<HashMap<String, Arc<tokio::sync::Mutex<Session>>>>,
    /// 设置存储（§13.1）。Arc 共享给 AppLlm（每请求重读，改动即生效）。
    pub settings: Arc<SettingsStore>,
    /// ctx-compact 的活配置（kernel-agent 侧只认 CompactConfig）。settings_set
    /// 成功后由命令层换入新值——压缩行为随即生效，无需重启。
    pub compact_cfg: Arc<RwLock<CompactConfig>>,
    /// Memory-Inject 的活配置（⑪，同 compact_cfg 的换入模式）。
    pub memory_cfg: Arc<RwLock<MemoryInjectConfig>>,
    /// 记忆库宿主访问（⑪）：Memory-Inject 检索源 + 记忆管理页存储支撑。
    pub memory: Arc<MemoryStore>,
    /// 使用统计写入端（⑩③，usage.sqlite3 宿主自有）。降级模式 = 丢弃。
    pub usage: Arc<UsageRecorder>,
    /// ToolExposure 的活配置（⑫，同 compact_cfg 的换入模式）。
    pub exposure_cfg: Arc<RwLock<ToolExposureConfig>>,
    /// ToolExposure 本体（⑫）：tool_exposure_debug 命令读最近一次截断快照。
    pub exposure: Arc<ToolExposure>,
}

pub fn init_state(app: AppHandle) -> AppState {
    let ctx = Arc::new(Context::new());
    let registry = Arc::new(PluginRegistry::new());
    let sink: Arc<dyn EventSink> = Arc::new(TauriSink::new(app.clone()));

    // 数据根绑定一次，设置 / 插件编排 / LLM 配置共用（settings.json 同目录）。
    let root = data_dir();

    // 使用统计库（⑩③，exposure 文档 §3）：宿主自有 usage.sqlite3，
    // 5s 批量刷盘。打不开只丢统计，不拦聊天。⑫ 的 EWMA 积累与
    // search_tools 高频回退也从这里读。
    let usage = match UsageDb::open(&root.join("usage.sqlite3")) {
        Ok(db) => UsageRecorder::spawn(db),
        Err(e) => {
            tracing::warn!("usage db unavailable: {e}；使用统计降级为丢弃");
            UsageRecorder::disabled()
        }
    };

    // 工具目录：始终提供（哪怕 registry 为空 → resolve 报 NotAvailable，
    // 错误以失败的 ToolResult 闭合，而不是服务缺失让整步崩掉，§7.2）。
    // 内置：send_email 是阶段六的演示替身（不可逆 → 走审批弹窗）；
    // search_tools（⑫）持有不含自身的全目录视图做检索（含 hidden，§4.1
    // 显式检索通路）+ usage 做空结果高频回退。
    let send_email: Arc<dyn plugin_api::Tool> = Arc::new(SendEmailTool);
    let search_catalog = Arc::new(
        KernelToolSource::new(RegistryToolSource::new(Arc::clone(&registry)))
            .with_builtin(Arc::clone(&send_email)),
    );
    ctx.provide_dyn::<dyn ToolSource>(Arc::new(
        KernelToolSource::new(RegistryToolSource::new(Arc::clone(&registry)))
            .with_builtin(send_email)
            .with_builtin(SearchToolsTool::new(search_catalog, Arc::clone(&usage))),
    ));

    let settings: Arc<SettingsStore> = Arc::new(match SettingsStore::open(&root) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("{e}；设置降级到临时目录");
            let fallback = std::env::temp_dir().join("locowrk-degraded-data");
            SettingsStore::open(&fallback).expect("settings unavailable even in fallback dir")
        }
    });

    // S1（清单 §2.7/§3）：高危解锁开关在插件加载前注入进程 env——
    // tool-shell 的 init 读一次 `LOCOWRK_SHELL_UNLOCK_RISK`；设置改动
    // 重启生效（插件 init 只跑一次）。硬锁类不受此开关影响。
    if settings.shell_unlock_risky() {
        std::env::set_var("LOCOWRK_SHELL_UNLOCK_RISK", "1");
    }

    // 模型适配器不特殊，也只是服务（§7.2）。阶段八②：genai 三合一适配器，
    // 由设置面板驱动（model 未配置时回落演示替身，见 llm/app_llm.rs）。
    ctx.provide_dyn::<dyn LlmService>(Arc::new(AppLlm::new(Arc::clone(&settings))));

    // 记忆库宿主访问（⑪）：Memory-Inject 检索源 + 记忆管理页存储支撑。
    // MemoryStore::new 不做 IO（连接按操作开、失败逐次降级），无启动风险。
    let memory = Arc::new(MemoryStore::new(&root));

    // agent/request 决策环（exposure 文档 §7，注册次序 = 执行次序）：
    // ToolExposure 先（先定暴露面，压缩才知道留多少空间）、Memory-Inject
    // 次（Context 文档 §3.6：记忆是"原料"）、ctx-compact 最后（压缩按含
    // 注入后的总量分配预算）。三件共享同一模式：配置经 RwLock 注入
    // （kernel-agent 不依赖设置存储），settings_set 成功后由命令层换入
    // 即生效。
    let exposure_cfg: Arc<RwLock<ToolExposureConfig>> =
        Arc::new(RwLock::new(settings.exposure_config()));
    let exposure = Arc::new(ToolExposure::new(
        UsageEwma::new(Arc::clone(&usage)),
        Arc::clone(&exposure_cfg),
    ));
    ctx.events.on_waterfall(points::REQUEST, Arc::clone(&exposure) as Arc<_>);

    let memory_cfg: Arc<RwLock<MemoryInjectConfig>> =
        Arc::new(RwLock::new(settings.memory_inject_config()));
    ctx.events.on_waterfall(
        points::REQUEST,
        Arc::new(MemoryInject::new(
            crate::memory::memory_source(&memory),
            Arc::clone(&memory_cfg),
        )),
    );

    // ctx-compact（Context and Memory.md §3）：agent/request 决策环上的压缩
    // 监听器——三策略配置驱动（阈值 = 窗口 × 百分比，压到阈值 50%），压缩
    // 结果经 payload 带回 kernel 落 ContextCompacted 事件。
    let compact_cfg: Arc<RwLock<CompactConfig>> = Arc::new(RwLock::new(settings.compact_config()));
    ctx.events.on_waterfall(
        points::REQUEST,
        Arc::new(CtxCompact::new(ctx.clone(), Arc::clone(&compact_cfg))),
    );

    // 中间件链组装次序（§7.1）：Tracing → Audit → Timeout → CircuitBreaker。
    // Audit（plugins.md mw-audit）挂在超时/熔断外侧：被拒的调用也留痕；
    // 记录追加写数据根 audit.jsonl（JSONL，只增不改，与会话日志同纪律）。
    let mw: Vec<Arc<dyn Middleware>> = vec![
        Arc::new(Tracing),
        Arc::new(Audit::new(Arc::new(JsonlAuditSink::new(
            root.join("audit.jsonl"),
        )))),
        Arc::new(Timeout::new(Duration::from_secs(30))),
        Arc::new(CircuitBreaker::new(5, Duration::from_secs(30))),
    ];
    // 审批门（纪律 9）：Irreversible 工具先经 ApprovalHub 问人，
    // 决策由 Kernel 写成 ApprovalDecided 事件进日志。
    let approval = Arc::new(ApprovalHub::new(Arc::clone(&sink)));
    let resolver: Arc<dyn kernel_agent::ApprovalResolver> = approval.clone();
    let kernel = Arc::new(Kernel::new(ctx.clone(), mw).with_approval(resolver));

    // 插件编排（§11.2）：数据根缺省 ~/.Locowrk（LOCOWRK_DATA_DIR
    // 可隔离）；打不开时退化到临时目录并明确告警——聊天不受影响。
    let plugins = match open_plugin_service(&root, Arc::clone(&registry), &ctx) {
        Ok(svc) => Arc::new(svc),
        Err(primary_err) => {
            tracing::error!("plugin service init failed: {primary_err}；降级到临时目录");
            let fallback = std::env::temp_dir().join("locowrk-degraded-data");
            match open_plugin_service(&fallback, Arc::clone(&registry), &ctx) {
                Ok(svc) => Arc::new(svc),
                Err(e) => panic!("plugin service unavailable even in fallback dir: {e}"),
            }
        }
    };

    // 1. 启动对账（任何插件加载之前）
    if let Err(e) = plugins.startup_reconcile() {
        tracing::warn!("startup reconcile failed: {e}");
    }
    // 2. 开发态装载 plugins/（阶段五逻辑；找不到不致命）
    match plugins_root::resolve() {
        Some(root) => match plugins_root::load_all(&root, &registry) {
            Ok(loaded) if loaded.is_empty() => {
                tracing::warn!(dir = %root.display(), "plugins/ 存在但没有加载任何插件")
            }
            Ok(loaded) => tracing::info!(dir = %root.display(), ?loaded, "plugins loaded"),
            Err(e) => tracing::warn!("plugin scan failed: {e}"),
        },
        None => tracing::warn!("plugins/ 目录未找到（可设 LOCOWRK_PLUGINS_DIR）；工具调用将不可用"),
    }
    // 3. 安装态已在对账中恢复；4. 状态广播转发给 UI（§13.4）
    forward_plugin_states(app.clone(), plugins.subscribe());

    AppState {
        app,
        kernel,
        plugins,
        approval,
        sink,
        sessions: Mutex::new(HashMap::new()),
        settings,
        compact_cfg,
        memory_cfg,
        memory,
        usage,
        exposure_cfg,
        exposure,
    }
}

fn data_dir() -> std::path::PathBuf {
    match data_root::resolve_data_dir() {
        Ok(dir) => dir,
        Err(e) => {
            tracing::warn!("{e}；退回系统临时目录");
            std::env::temp_dir().join("locowrk-data")
        }
    }
}

fn open_plugin_service(
    data_dir: &std::path::Path,
    registry: Arc<PluginRegistry>,
    ctx: &Arc<Context>,
) -> Result<PluginService, String> {
    let market_root =
        data_root::resolve_market_dir().unwrap_or_else(|| data_dir.join("market-absent"));
    tracing::info!(
        data = %data_dir.display(),
        market = %market_root.display(),
        "plugin service opening"
    );
    PluginService::open(
        data_dir.to_path_buf(),
        registry,
        Arc::clone(ctx),
        Box::new(LocalDirMarket::new(market_root)),
    )
}

impl AppState {
    /// 取（或惰性建）某会话的锁。首聊即用，无需显式建会话。
    pub fn session_lock(&self, id: &str) -> Arc<tokio::sync::Mutex<Session>> {
        self.sessions
            .lock()
            .expect("sessions poisoned")
            .entry(id.to_string())
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(Session::new())))
            .clone()
    }
}
