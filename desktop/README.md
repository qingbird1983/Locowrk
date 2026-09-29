# desktop/ — Tauri 2 桌面端

阶段七（DESIGN.md §14）：PluginService + 插件管理命令 + 市场 UI——安装/启停/
更新/回退/卸载全 UI 化，`plugin://state-changed` 实时同步。
技术栈：Vue 3 + Vite 7 + Pinia + Tailwind + vue-i18n（§10.2 / §13.5）。

## 结构

- `src/` Vue 3 前端
  - `lib/api.ts` IPC 封装（命令 invoke + 事件 listen，§12.2 / §11.1）
  - `stores/chat.ts` 聊天状态（消费事件流，只渲染不生产）
  - `stores/approvals.ts` 审批弹窗队列（approval-request 推送 + 启动补拉）
  - `stores/plugins.ts` 插件面板状态（双页签数据 + 动作包装；任何
    state-changed 整表重拉，本地 IO 毫秒级不值得增量补丁）
  - `components/ApprovalModal.vue` 不可逆工具确认弹窗（§13.2）
  - `components/PluginMarket.vue` 市场：发现（搜索 + 卡片 + 版本下拉）/
    已安装（按分类分组，行内启停/更新/回退/卸载）（§13.3）
  - `i18n.ts` + `locales/{zh-CN,en}.ts` UI 文案集中地（§13.5，默认中文）
  - `types.ts` `AgentEvent` / `ApprovalRequest` / 插件类型的 TS 镜像
- `src-tauri/` Rust 组装点（独立 Cargo 包，不进根 workspace）
  - `commands.rs` 薄适配层（§10.3：不写业务逻辑）；plugin_* 七条命令直通
    `PluginService`
  - `runner.rs` 聊天轮次循环：step 循环 + 会话日志投影为 UI 事件
  - `approval.rs` ApprovalHub：`ApprovalResolver` 的桌面端实现（推事件 + oneshot 等决策）
  - `state.rs` 装配 Context / PluginRegistry / Kernel（含审批门）/ 审批中心 /
    会话表 / PluginService；启动顺序 = `startup_reconcile`（清 tombstone +
    对账 + 恢复 enabled）→ 开发态装载 → 广播转发（§12.3）
  - `events.rs` `agent://` 通道 + EventSink 抽象 + `plugin://state-changed`
    转发器（§13.4）
  - `data_root.rs` 安装态数据根（缺省 `%USERPROFILE%\.Locowrk`，
    `LOCOWRK_DATA_DIR` 可隔离）与市场目录解析（`LOCOWRK_MARKET_DIR` /
    锚点向上找 `market/`）（§11.3）
  - `demo_llm.rs` 脚本模型；`demo_tools.rs` 内置演示工具 `send_email`（不可逆）
  - `plugins_root.rs` plugins/ 目录解析与插件装载

编排本体在 `crates/plugin-service/`（CLI 与桌面端共用），桌面端只是组装点。

## 运行

前置：Rust 1.85+、Node 20+、pnpm；开发态与市场插件产物都要构建：

```bash
bash scripts/build_plugins.sh   # 开发态 plugins/（tool-echo 双版本）
bash scripts/build_market.sh    # 市场 market/（tool-shout 双版本；缺省时市场页为空）
cd desktop
pnpm install        # 首次
pnpm tauri dev      # 开发模式（热重载）
pnpm tauri build    # 打包
cd src-tauri && cargo test   # 桌面端 Rust 测试
```

目录定位：开发态 `LOCOWRK_PLUGINS_DIR` 优先（否则锚点向上找 `plugins/`）；
市场 `LOCOWRK_MARKET_DIR` 优先（否则锚点向上找 `market/`）；安装态数据根
`LOCOWRK_DATA_DIR` 优先（否则 `%USERPROFILE%\.Locowrk`）。

## 说明

- LLM 是脚本替身（`demo_llm.rs`）：消息含 "echo" → 调 tool-echo 插件；
  含 "shout"/"喊" → 调 tool-shout（先在插件页从市场安装，"重复"⇒ repeat 模式，
  1.1.0 才支持——版本差异在聊天里可观察）；
  含 "email"/"邮件" → 调内置 `send_email`（不可逆，触发审批弹窗）。
  真实模型适配器按 `LlmService` 服务接入（§7.2），内核与前端都无需改动
- 事件通道：`agent://chunk` / `agent://tool-call` / `agent://tool-result` /
  `agent://turn-done` / `agent://approval-request`（§12.2）+
  `plugin://state-changed`（§13.4，载荷 = PluginStateChange 序列化，type 标签）
- 审批不设超时：用户不决策，step 一直挂着（工具链的 Timeout 只包执行）
- 卸载与 Windows 文件锁：已加载 DLL 无法即删 → tombstone（lockfile
  `pending_delete`），下次启动 `startup_reconcile` 在加载任何插件之前清理（纪律 2）
- i18n：语言跟随系统（zh→zh-CN，en→en，其余回落中文）；切换入口与持久化
  随阶段八设置面板落地（§13.5）。会话内容是数据，不翻译

## 关键纪律（沿用）

- **Bridge Layer 必须薄**：`#[tauri::command]` 只做参数解析、调用内核 API、
  结果转 `Result<.., String>`，不写业务逻辑
- **UI 只通过 `PluginService` 操作内核**，不直接碰 `PluginRegistry`
- **流式内容走事件推送**（`agent://chunk` 等），不塞进 command 返回值
