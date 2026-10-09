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
//! - `llm`：真实模型接入（阶段八②，LLM.md v3.0）——自建 OpenAI 兼容统一客户端
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
mod ask;
mod assets;
mod autodisable;
mod commands;
mod data_root;
mod demo_llm;
mod demo_tools;
mod desktop_notify;
mod events;
mod export;
mod idle;
mod llm;
mod memory;
mod memory_files;
mod memory_reconcile;
mod memory_tree_reconcile;
mod media;
mod model_override;
mod notifications;
mod organize;
mod permission_mode;
mod plugins_root;
mod privacy;
mod repo_context;
mod runner;
mod scheduler;
mod search_tools;
mod session_log;
mod settings;
mod state;
mod stop;
mod sub_transcript;
mod subagent_verify;
mod thinking_language;
mod title;
mod tool_source;
mod tray;
mod usage;
mod workspace;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_approval;
#[cfg(test)]
mod tests_permission;
#[cfg(test)]
mod tests_plugins_root;
#[cfg(test)]
mod tests_session_log;
#[cfg(test)]
mod tests_stream;
#[cfg(test)]
mod tests_tree_gate;

pub fn run() {
    use tauri::{Listener, Manager};

    // 启动计时（白屏核查 2026-10-02/03）：main 入口 → setup（≈配置窗口创建点）
    // → init_state 完成。主窗 visible:false 起步（tauri.conf.json），首帧由前端
    // 显窗；splash 闪屏窗（静态 splash.html，零脚本）先创建先出画——冷启动
    // WebView2 初始化 + vite 模块交付动辄数十秒，用户双击后不再"什么都没有"。
    // 前端首帧发 `lw:frontend-ready` → 关闪屏；25s 未就绪则兜底（关闪屏 +
    // 强制显主窗，宁冒白屏风险也不吞窗口，㉕ i18n 白屏先例）。
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
                "boot: setup entered (splash + hidden main created)"
            );
            // 前端首帧信号：主窗 show() 后 emit，这里关掉闪屏（先行注册防丢）
            let ready_handle = app.handle().clone();
            app.listen("lw:frontend-ready", move |_| {
                if let Some(s) = ready_handle.get_webview_window("splash") {
                    let _ = s.close();
                }
            });
            let init = std::time::Instant::now();
            app.manage(state::init_state(app.handle().clone()));
            tracing::info!(
                ms = init.elapsed().as_millis() as u64,
                "boot: init_state done"
            );
            tray::setup(app.handle())?;
            // 兜底：前端崩溃 / 白屏异常时窗口不能永远不出现（闪屏常驻也骗人）。
            // 40s = 实测健康冷启动（vite 冷缓存重建 31.4s，2026-10-03）+ 余量；
            // 更长的加载多半已是异常，交回白屏窗口便于发现问题。
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(40));
                if let Some(win) = handle.get_webview_window("main") {
                    if !win.is_visible().unwrap_or(true) {
                        let _ = win.show();
                        if let Some(s) = handle.get_webview_window("splash") {
                            let _ = s.close();
                        }
                        tracing::warn!(
                            "boot: frontend not ready in 40s, showing main window anyway"
                        );
                    }
                }
            });
            tracing::info!(
                ms = boot.elapsed().as_millis() as u64,
                "boot: setup done — splash up, main hidden until frontend first paint"
            );
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::agent_chat,
            commands::agent_approve,
            commands::agent_stop,
            commands::asset_put,
            commands::asset_put_path,
            commands::asset_read,
            commands::asset_reveal,
            commands::asset_gc,
            commands::asset_save_as,
            commands::ask_respond,
            commands::ask_snooze,
            commands::list_pending_asks,
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
            commands::llm_cache_diagnose,
            commands::memory_list,
            commands::memory_update,
            commands::memory_delete,
            commands::memory_clear_all,
            commands::memory_reconcile,
            commands::memory_reveal_folder,
            commands::tool_exposure_debug,
            commands::pick_directory,
            commands::pick_files,
            commands::session_list,
            commands::session_history,
            commands::session_rename,
            commands::session_associate,
            commands::session_set_permission_mode,
            commands::session_delete,
            commands::session_search,
            commands::session_export,
            commands::session_truncate_after,
            commands::session_resolve_uncertain,
            commands::session_set_model,
            commands::session_set_thinking,
            commands::reveal_in_explorer,
            commands::schedule_list,
            commands::schedule_delete,
            commands::llm_list_models,
            commands::llm_probe_models,
            commands::llm_provider_save,
            commands::llm_provider_delete,
            commands::llm_provider_reorder,
            commands::llm_test_model,
            commands::llm_set_proxy,
            commands::llm_catalog_status,
            commands::llm_catalog_refresh,
            commands::llm_lookup_model,
            commands::llm_import_scan,
            commands::llm_import_apply,
            commands::open_external_url,
            commands::pick_save_path,
            commands::notification_list,
            commands::notification_read_all,
            commands::notification_read,
            commands::notification_clear,
            commands::organize_now,
            commands::memory_tree_view,
            commands::memory_tree_sessions,
            commands::memory_tree_chapters,
            commands::memory_tree_original,
            commands::memory_tree_project,
            commands::memory_tree_edit_chapter,
            commands::memory_tree_delete_chapter,
            commands::subagent_transcript
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
