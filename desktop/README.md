# desktop/ — Tauri 2 桌面端

阶段六（DESIGN.md §14）：会话视图 + 审批弹窗 + i18n 基础，全 UI 化操作无需 CLI。
技术栈：Vue 3 + Vite 7 + Pinia + Tailwind + vue-i18n（§10.2 / §13.5）。

## 结构

- `src/` Vue 3 前端
  - `lib/api.ts` IPC 封装（命令 invoke + 事件 listen，§12.2）
  - `stores/chat.ts` 聊天状态（消费事件流，只渲染不生产）
  - `stores/approvals.ts` 审批弹窗队列（approval-request 推送 + 启动补拉）
  - `components/ApprovalModal.vue` 不可逆工具确认弹窗（§13.2）
  - `i18n.ts` + `locales/{zh-CN,en}.ts` UI 文案集中地（§13.5，默认中文）
  - `types.ts` `AgentEvent` / `ApprovalRequest` 的 TS 镜像（与 `src-tauri/src/events.rs` 对齐）
- `src-tauri/` Rust 组装点（独立 Cargo 包，不进根 workspace）
  - `commands.rs` 薄适配层（§10.3：不写业务逻辑）
  - `runner.rs` 聊天轮次循环：step 循环 + 会话日志投影为 UI 事件
  - `approval.rs` ApprovalHub：`ApprovalResolver` 的桌面端实现（推事件 + oneshot 等决策）
  - `state.rs` 装配 Context / PluginRegistry / Kernel（含审批门）/ 审批中心 / 会话表
  - `events.rs` `agent://` 通道定义 + EventSink 抽象（测试可替换）
  - `demo_llm.rs` 脚本模型；`demo_tools.rs` 内置演示工具 `send_email`（不可逆）
  - `plugins_root.rs` plugins/ 目录解析与插件装载

## 运行

前置：Rust 1.85+、Node 20+、pnpm；插件产物已构建（仓库根 `bash scripts/build_plugins.sh`）。

```bash
cd desktop
pnpm install        # 首次
pnpm tauri dev      # 开发模式（热重载）
pnpm tauri build    # 打包
cd src-tauri && cargo test   # 桌面端 Rust 测试
```

插件目录定位：`LOCOWRK_PLUGINS_DIR` 环境变量优先，否则从进程 CWD 与可执行文件目录
逐级向上找 `plugins/`（判据：其下存在 tool-echo 子目录）。

## 说明

- LLM 是脚本替身（`demo_llm.rs`）：消息含 "echo" → 调 tool-echo 插件；
  含 "email"/"邮件" → 调内置 `send_email`（不可逆，触发审批弹窗）。
  真实模型适配器按 `LlmService` 服务接入（§7.2），内核与前端都无需改动
- 事件通道：`agent://chunk` / `agent://tool-call` / `agent://tool-result` /
  `agent://turn-done` / `agent://approval-request`（§12.2 阶段五/六子集）
- 审批不设超时：用户不决策，step 一直挂着（工具链的 Timeout 只包执行）
- i18n：语言跟随系统（zh→zh-CN，en→en，其余回落中文）；切换入口与持久化
  随阶段八设置面板落地（§13.5）。会话内容是数据，不翻译

## 关键纪律（沿用）

- **Bridge Layer 必须薄**：`#[tauri::command]` 只做参数解析、调用内核 API、
  结果转 `Result<.., String>`，不写业务逻辑
- **UI 只通过 `PluginService` 操作内核**，不直接碰 `PluginRegistry`（阶段七接入）
- **流式内容走事件推送**（`agent://chunk` 等），不塞进 command 返回值
