//! Locowrk 桌面端（DESIGN.md §10 / §12）：Tauri 2 组装点。
//!
//! 模块：
//! - `state`：AppState 组装（Context / PluginRegistry / Kernel / 会话表）
//! - `commands`：`#[tauri::command]` 薄适配层（§10.3：不写业务逻辑）
//! - `runner`：聊天轮次主循环（ReAct step 循环 + 会话日志投影为 UI 事件）
//! - `events`：`agent://` 事件通道与 EventSink 抽象（§12.2）
//! - `demo_llm`：阶段五脚本模型（真实模型适配器是后续阶段的插件）
//! - `plugins_root`：plugins/ 目录解析与插件装载
//! - `tool_source`：RegistryToolSource → kernel ToolSource 胶水
//!   （与 kernel-bin 各持一份：依赖图禁止 plugin-host 实现 kernel-agent trait）
//!
//! 纪律：命令只做参数转换；流式输出走事件推送，不塞 command 返回值（§12.2）。

mod commands;
mod demo_llm;
mod events;
mod plugins_root;
mod runner;
mod state;
mod tool_source;

#[cfg(test)]
mod tests;

pub fn run() {
    use tauri::Manager;

    tracing_subscriber::fmt()
        .with_target(true)
        .with_max_level(tracing::Level::INFO)
        .init();

    tauri::Builder::default()
        .setup(|app| {
            app.manage(state::init_state(app.handle().clone()));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![commands::agent_chat])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
