# desktop/ — Tauri 2 桌面端（尚未脚手架）

按设计文档（DESIGN.md §10–§13、落地路径阶段五）在此创建：

- `src-tauri/`：Rust 后端 = 内核 crate 组装 + Tauri command 薄适配层
- `src/`：Vue 3 + Vite + Pinia + Tailwind 前端（聊天 / 审批 / 插件市场 / 插件管理 / 设置）

## 关键纪律

- **Bridge Layer 必须薄**：`#[tauri::command]` 只做参数解析、调用内核 API、
  结果转 `Result<Value, String>`，不写业务逻辑
- **UI 只通过 `PluginService` 操作内核**，不直接碰 `PluginRegistry`
- **流式内容走事件推送**（`agent://chunk` 等），不塞进 command 返回值
