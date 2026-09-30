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
//! - `llm`：真实模型接入（阶段八②，LLM.md 方案一）——genai 三合一
//!   适配器（openai / anthropic / ollama），由设置面板驱动分发
//! - `demo_llm`：阶段五脚本模型（现为 llm 模块的未配置回落替身）
//! - `demo_tools`：阶段六内置演示工具 send_email（不可逆 → 走审批弹窗）
//! - `plugins_root`：plugins/ 目录解析与插件装载
//! - `tool_source`：RegistryToolSource → kernel ToolSource 胶水
//!   （与 kernel-bin 各持一份：依赖图禁止 plugin-host 实现 kernel-agent trait）
//! - `tray`：系统托盘 + 全局快捷键（阶段八⑤）——左键显窗 / 菜单显窗与
//!   退出 / 关窗进托盘；快捷键切窗，键位存 settings.json `shortcut`
//!
//! 纪律：命令只做参数转换；流式输出走事件推送，不塞 command 返回值（§12.2）。

mod approval;
mod commands;
mod data_root;
mod demo_llm;
mod demo_tools;
mod events;
mod idle;
mod llm;
mod memory;
mod plugins_root;
mod runner;
mod settings;
mod state;
mod tool_source;
mod tray;
mod usage;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_plugins_root;

pub fn run() {
    use tauri::Manager;

    tracing_subscriber::fmt()
        .with_target(true)
        .with_max_level(tracing::Level::INFO)
        .init();

    tauri::Builder::default()
        .plugin(tray::plugin())
        .setup(|app| {
            app.manage(state::init_state(app.handle().clone()));
            tray::setup(app.handle())?;
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
            commands::memory_list,
            commands::memory_update,
            commands::memory_delete,
            commands::memory_clear_all
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
