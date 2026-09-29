# desktop/ — Tauri 2 桌面端

阶段五脚手架（DESIGN.md §14）：Tauri 2 IPC 层跑通 `agent_chat`，前端收 `agent://` 事件流。
技术栈：Vue 3 + Vite 7 + Pinia + Tailwind（§10.2 推荐组合）。

## 结构

- `src/` Vue 3 前端：聊天视图 + 工具调用卡片
  - `lib/api.ts` IPC 封装（命令 invoke + 事件 listen，§12.2）
  - `stores/chat.ts` 聊天状态（消费事件流，只渲染不生产）
  - `types.ts` `AgentEvent` 的 TS 镜像（与 `src-tauri/src/events.rs` 对齐）
- `src-tauri/` Rust 组装点（独立 Cargo 包，不进根 workspace）
  - `commands.rs` 薄适配层（§10.3：不写业务逻辑）
  - `runner.rs` 聊天轮次循环：step 循环 + 会话日志投影为 UI 事件
  - `state.rs` 装配 Context / PluginRegistry / Kernel / 会话表（§12.3 阶段五子集）
  - `events.rs` `agent://` 通道定义 + EventSink 抽象（测试可替换）
  - `demo_llm.rs` 脚本模型（真实模型适配器是后续阶段的插件）
  - `plugins_root.rs` plugins/ 目录解析与插件装载

## 运行

前置：Rust 1.85+、Node 20+、pnpm；插件产物已构建（仓库根 `bash scripts/build_plugins.sh`）。

```bash
cd desktop
pnpm install        # 首次
pnpm tauri dev      # 开发模式（热重载）
pnpm tauri build    # 打包
```

插件目录定位：`LOCOWRK_PLUGINS_DIR` 环境变量优先，否则从进程 CWD 与可执行文件目录
逐级向上找 `plugins/`（判据：其下存在 tool-echo 子目录）。

## 说明

- 阶段五的 LLM 是脚本替身（`demo_llm.rs`）：消息含 "echo" → 调 tool-echo 插件。
  真实模型适配器按 `LlmService` 服务接入（§7.2），内核与前端都无需改动
- 事件通道：`agent://chunk` / `agent://tool-call` / `agent://tool-result` /
  `agent://turn-done`（§12.2 的阶段五子集）
- 审批弹窗（Irreversible 工具）是阶段六内容；当前无 ApprovalResolver 时内核
  fail-safe 拒绝（纪律 9），行为正确

## 关键纪律（沿用）

- **Bridge Layer 必须薄**：`#[tauri::command]` 只做参数解析、调用内核 API、
  结果转 `Result<.., String>`，不写业务逻辑
- **UI 只通过 `PluginService` 操作内核**，不直接碰 `PluginRegistry`（阶段七接入）
- **流式内容走事件推送**（`agent://chunk` 等），不塞进 command 返回值
