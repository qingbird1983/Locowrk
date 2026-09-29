//! 共享状态组装（DESIGN.md §12.3）：Context + PluginRegistry + Kernel
//! （含审批门）+ 会话表 + PluginService（阶段七）。
//!
//! 启动顺序固定（§11.2 对账式恢复 / §12.3 备注）：
//! 1. `startup_reconcile`——清 tombstone、lockfile 与磁盘对账、恢复
//!    enabled 插件。必须在加载任何插件之前调用（此时无文件锁）
//! 2. 开发态装载（`plugins/`，内核开发用，不经 lockfile）
//! 3. 安装态已由第 1 步恢复完毕（`~/.agent-kernel`，lockfile 管理）

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use cordis_core::Context;
use kernel_agent::{CircuitBreaker, Kernel, LlmService, Middleware, Timeout, ToolSource, Tracing};
use plugin_host::{PluginRegistry, RegistryToolSource};
use plugin_service::{LocalDirMarket, PluginService};
use session_store::Session;
use tauri::AppHandle;

use crate::approval::ApprovalHub;
use crate::data_root;
use crate::demo_llm::DemoLlm;
use crate::demo_tools::SendEmailTool;
use crate::events::{forward_plugin_states, EventSink, TauriSink};
use crate::plugins_root;
use crate::tool_source::KernelToolSource;

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
}

pub fn init_state(app: AppHandle) -> AppState {
    let ctx = Arc::new(Context::new());
    let registry = Arc::new(PluginRegistry::new());
    let sink: Arc<dyn EventSink> = Arc::new(TauriSink::new(app.clone()));

    // 工具目录：始终提供（哪怕 registry 为空 → resolve 报 NotAvailable，
    // 错误以失败的 ToolResult 闭合，而不是服务缺失让整步崩掉，§7.2）。
    // send_email 是阶段六的内置演示替身（不可逆 → 走审批弹窗）。
    ctx.provide_dyn::<dyn ToolSource>(Arc::new(
        KernelToolSource::new(RegistryToolSource::new(Arc::clone(&registry)))
            .with_builtin(Arc::new(SendEmailTool)),
    ));

    // 模型适配器不特殊，也只是服务（§7.2）。阶段五用脚本替身。
    ctx.provide_dyn::<dyn LlmService>(Arc::new(DemoLlm::new()));

    // 中间件链组装次序（§7.1）：Tracing → Timeout → CircuitBreaker
    let mw: Vec<Arc<dyn Middleware>> = vec![
        Arc::new(Tracing),
        Arc::new(Timeout::new(Duration::from_secs(30))),
        Arc::new(CircuitBreaker::new(5, Duration::from_secs(30))),
    ];
    // 审批门（纪律 9）：Irreversible 工具先经 ApprovalHub 问人，
    // 决策由 Kernel 写成 ApprovalDecided 事件进日志。
    let approval = Arc::new(ApprovalHub::new(Arc::clone(&sink)));
    let resolver: Arc<dyn kernel_agent::ApprovalResolver> = approval.clone();
    let kernel = Arc::new(Kernel::new(ctx.clone(), mw).with_approval(resolver));

    // 插件编排（§11.2）：数据根缺省 ~/.agent-kernel（LOCOWRK_DATA_DIR
    // 可隔离）；打不开时退化到临时目录并明确告警——聊天不受影响。
    let plugins = match open_plugin_service(&data_dir(), Arc::clone(&registry), &ctx) {
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
