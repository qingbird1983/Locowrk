//! Locowrk 桌面端（DESIGN.md §10 / §12）：Tauri 2 组装点。
//!
//! 模块：
//! - `state`：AppState 组装（Context / PluginRegistry / Kernel / 审批中心 /
//!   会话表 / PluginService）
//! - `commands`：`#[tauri::command]` 薄适配层（§10.3：不写业务逻辑）
//! - `runner`：聊天轮次主循环（ReAct step 循环 + 会话日志投影为 UI 事件）
//! - `events`：`agent://` 与 `plugin://` 事件通道、EventSink 抽象（§12.2 / §13.4）
//! - `approval`：审批中心 ApprovalHub——`ApprovalResolver` 的桌面端实现（§13.2）
//! - `data_root`：安装态数据根与市场目录解析（§11.3）
//! - `settings`：设置存储——settings.json 于数据根；API Key 只落盘不出
//!   IPC，投影仅 api_key_set（§13.1 / §13.5）
//! - `memory`：记忆库宿主访问（⑪）——Memory-Inject 检索源（kernel-agent
//!   MemorySource trait 的桌面实现）+ 记忆管理页存储支撑（memory.sqlite3
//!   归 memory-sqlite 插件所有，宿主只做管理面访问）
//! - `experiences`：经历记忆库（⑬，Context 文档 §4.3 宿主侧 memory-vector）
//!   ——语义检索 experiences.sqlite3，ExperienceSource 契约的桌面实现
//! - `llm`：真实模型接入（阶段八②，LLM.md 方案一）——genai 三合一
//!   适配器（openai / anthropic / ollama），由设置面板驱动分发
//! - `demo_llm`：阶段五脚本模型（现为 llm 模块的未配置回落替身）
//! - `demo_tools`：阶段六内置演示工具 send_email（不可逆 → 走审批弹窗）
//! - `plugins_root`：plugins/ 目录解析与插件装载
//! - `tool_source`：RegistryToolSource → kernel ToolSource 胶水
//!   （与 kernel-bin 各持一份：依赖图禁止 plugin-host 实现 kernel-agent trait）
//! - `tray`：系统托盘 + 全局快捷键（阶段八⑤）——左键显窗 / 菜单显窗与
//!   退出 / 关窗进托盘；快捷键切窗，键位存 settings.json `shortcut`
//! - `scheduler`：⑭ 定时调度——宿主内置 schedule_* 工具面 + 触发引擎
//!   （schedule.sqlite3；触发 = 注入一轮，忙顺延 / 启动补跑）
//! - `desktop_notify`：⑭ 桌面系统通知（tool-notify desktop 通道的 vtable
//!   落点 + 调度触发的窗口不在位兜底）
//!
//! 纪律：命令只做参数转换；流式输出走事件推送，不塞 command 返回值（§12.2）。

mod approval;
mod autodisable;
mod commands;
mod data_root;
mod demo_llm;
mod demo_tools;
mod desktop_notify;
mod events;
mod experiences;
mod extract;
mod idle;
mod llm;
mod memory;
mod plugins_root;
mod runner;
mod scheduler;
mod search_tools;
mod session_log;
mod settings;
mod state;
mod tool_source;
mod tray;
mod usage;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_plugins_root;
#[cfg(test)]
mod tests_session_log;
#[cfg(test)]
mod tests_stream;

pub fn run() {
    use tauri::Manager;

    // 启动计时（白屏核查 2026-10-02）：main 入口 → setup（≈配置窗口创建点）
    // → init_state 完成。窗口 visible:false 起步（tauri.conf.json），首帧由
    // 前端显窗——WebView2 初始化 + 前端加载期间桌面不再露白；前端 8s 未就绪
    // 则兜底显窗（宁冒白屏风险也不吞窗口，㉕ i18n 白屏先例）。
    let boot = std::time::Instant::now();

    tracing_subscriber::fmt()
        .with_target(true)
        .with_max_level(tracing::Level::INFO)
        .init();
    tracing::info!("boot: entering builder");

    tauri::Builder::default()
        .plugin(tray::plugin())
        // ⑭ 系统通知（tauri-plugin-notification）：Rust 侧 app.notification()
        // 投递，无 JS guest、无需 capability；desktop_notify.rs 统一出口
        .plugin(tauri_plugin_notification::init())
        // ⑱ 原生目录选择（UI shell 文档 §7 规则 8）：pick_directory 命令
        // Rust 侧直呼，无 JS guest、无需 capability
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            tracing::info!(
                ms = boot.elapsed().as_millis() as u64,
                "boot: setup entered (config window created, hidden)"
            );
            let init = std::time::Instant::now();
            app.manage(state::init_state(app.handle().clone()));
            tracing::info!(
                ms = init.elapsed().as_millis() as u64,
                "boot: init_state done"
            );
            tray::setup(app.handle())?;
            // 兜底显窗：前端崩溃 / 白屏异常时窗口不能永远不出现
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(8));
                if let Some(win) = handle.get_webview_window("main") {
                    if !win.is_visible().unwrap_or(true) {
                        let _ = win.show();
                        tracing::warn!("boot: frontend not ready in 8s, showing window anyway");
                    }
                }
            });
            tracing::info!(
                ms = boot.elapsed().as_millis() as u64,
                "boot: setup done — window hidden until frontend first paint"
            );
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::agent_chat,
            commands::agent_approve,
            commands::list_pending_approvals,
            commands::context_undo_compact,
            commands::plugin_list,
            commands::plugin_list_with_usage,
            commands::plugin_registry_list,
            commands::plugin_install,
            commands::plugin_set_enabled,
            commands::plugin_batch_set_enabled,
            commands::plugin_uninstall,
            commands::plugin_batch_uninstall,
            commands::plugin_rollback,
            commands::plugin_update,
            commands::settings_get,
            commands::settings_set,
            commands::llm_probe_window,
            commands::llm_probe_embedding,
            commands::memory_list,
            commands::memory_update,
            commands::memory_delete,
            commands::memory_clear_all,
            commands::experience_list,
            commands::experience_delete,
            commands::experience_clear_all,
            commands::tool_exposure_debug,
            commands::pick_directory,
            commands::session_list,
            commands::session_history
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
