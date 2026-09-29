//! 共享状态组装（DESIGN.md §12.3 的阶段五/六子集）。
//!
//! PluginService / LockFile / 市场客户端是阶段七能力，本阶段只装配
//! Context + PluginRegistry + Kernel（含审批门）+ 会话表。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use cordis_core::Context;
use kernel_agent::{CircuitBreaker, Kernel, LlmService, Middleware, Timeout, ToolSource, Tracing};
use plugin_host::{PluginRegistry, RegistryToolSource};
use session_store::Session;
use tauri::AppHandle;

use crate::approval::ApprovalHub;
use crate::demo_llm::DemoLlm;
use crate::demo_tools::SendEmailTool;
use crate::events::{EventSink, TauriSink};
use crate::plugins_root;
use crate::tool_source::KernelToolSource;

pub struct AppState {
    /// 阶段七/八的窗口级命令（对话框、多窗口）从这里取句柄；当前
    /// 事件出口已是共享 sink，字段暂无读取方。
    #[allow(dead_code)]
    pub app: AppHandle,
    pub kernel: Arc<Kernel>,
    /// 阶段七的插件管理命令（plugin_list / 回退等）从这里读状态；
    /// 本阶段只在 init 时装载插件，字段暂无读取方。
    #[allow(dead_code)]
    pub registry: Arc<PluginRegistry>,
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
    let kernel = Arc::new(Kernel::new(ctx, mw).with_approval(resolver));

    // 加载 plugins/ 下的工具插件（阶段五只有 tool-echo）。找不到不致命：
    // 聊天仍可用，工具调用会得到 NotAvailable 的 ToolResult。
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

    AppState {
        app,
        kernel,
        registry,
        approval,
        sink,
        sessions: Mutex::new(HashMap::new()),
    }
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
