# Locowrk

可热插拔、可回退、带桌面端（Tauri 2 + Vue 3）的 Rust Agent 内核。

核心思想：**一切皆插件**——工具、模型适配器、会话存储、Agent 循环的扩展面都走插件
契约，无特权模块；用 Rust 所有权模型实现零开销服务组合与精确副作用撤销；用 Tauri 2
提供插件管理与会话界面。

> **⚠ v2.149 批 E 订正**：本文件此前停在「当前状态 = 阶段一（内核骨架 + 事件日志）」，
> 与实际相差 148 个版本。另外两点口径纠正：①**LLM 客户端不是插件**，是宿主内置服务
> （内核只认 `LlmService` trait，实现在桌面组装点注册，v2.75 架构定性）；②**沙箱插件
> （tool-sandbox）已随 v2.145 权限模型重设计整目录退役**——不隔离 fs/网络即非安全边界，
> 第二入口只多撞锁面，命令执行面收敛为 `shell` 单一入口。

## 现在是什么

* **版本**：v2.149（2026-10-11）；进度唯一看板 =
  [docs/design/ROADMAP.md](docs/design/ROADMAP.md)，逐批落点 =
  [docs/design/CHANGELOG.md](docs/design/CHANGELOG.md)
* **可用的产品形态**：桌面端（真 LLM 轮、流式、思考链、多模态输入、工具审批、
  会话持久化与重放、记忆树整理、跨会话取回、子代理、定时任务、工作面板内嵌浏览器）
* **规模参考**：根 workspace 8 个 crate + `desktop/src-tauri`（独立包，不进根
  workspace）+ 前端 Vue/TS；内置插件 9 个 id（`plugins-src/`，见
  [plugins/README.md](plugins/README.md)）；Tauri 命令 85 个；测试根 436 · 桌面 356
* **权限模型**：四档（计划 / 询问 / 自动编辑 / 完全访问）+ 项目粘性授权规则 +
  两道硬门（隐私门、越界门），**无命令级禁令**（v2.145 对齐 ZCode）

## 结构

```
crates/
  plugin-api/       冻结 ABI 契约（唯一，改动即破坏兼容）+ 宿主侧共享基座（fsutil/memlayout/policy/naming）
  cordis-core/      Context / Effect / Fiber / Scope / EventBus
  kernel-agent/     Agent 主循环 / Waterfall / 中间件链 / 子 Agent 调度
  plugin-host/      dlopen Loader / 多版本 Registry / FFI→Tool 适配 / WASM 沙箱加载器
  plugin-service/   插件安装·卸载·更新编排（CLI 与桌面端共用）
  session-store/    会话事件日志 / 快照 / 回放 / 补偿 / uncertain 崩溃恢复扫描
  memory-tree/      记忆树（章节捆 L0/L1/L2 / 整理触发 / 记录员 / 投影存储）
  kernel-bin/       CLI 可执行入口（headless 组装点）
plugins-src/        内置插件源码（workspace 成员）
plugins/            开发态插件安装目录，布局 plugins/{id}/{version}/（产物 gitignore）
market/             本地目录市场（tool-shout 双版本夹具）
desktop/            Tauri 2 + Vue 3 桌面端（src/ 前端 + src-tauri/ 组装点）
docs/design/        设计文档（先读 INDEX.md 分清权威 / 已兑现 / 存档）
docs/reports/       审计与执行台账（CODE-AUDIT-v1 / AUDIT-FIX-PLAN-v1）
scripts/            构建与门禁脚本
```

## 常用命令

```bash
cargo test --workspace                    # 根 workspace（内核 + 插件 + 存储）
cargo clippy --workspace --all-targets -D warnings
cargo run -p kernel-bin -- demo           # 热插拔验收叙事（前置 bash scripts/build_plugins.sh）

cd desktop && pnpm install                # 前端依赖（一次）
cd desktop && pnpm tauri dev              # 桌面端开发模式
cd desktop && pnpm build                  # vue-tsc 类型检查 + vite 构建
cd desktop/src-tauri && cargo test        # 桌面端 Rust 单测（generate_context! 需先 pnpm build）

bash scripts/check_length.sh              # 单文件 400 行报告闸（基线比较）
node scripts/check-i18n.mjs               # 中英键集双向闭合 + 无引用键报告
node scripts/check-commands.mjs           # #[tauri::command] ↔ generate_handler! ↔ invoke 三向
node scripts/check-middleware-wiring.mjs  # 中间件「写了但没接线」判红
```

Hooks 在 `.githooks/`，克隆后需执行一次 `git config core.hooksPath .githooks` 启用
pre-commit（按暂存面触发上述门禁）。

## 文档入口

**改任何东西之前先读 [docs/design/INDEX.md](docs/design/INDEX.md)** ——它列出哪些文档是
现役规范、哪些是已兑现的历史设计稿、哪些是存档或未采纳方案（避免「照过期文档实现」）。

* 权威设计文档：[docs/design/DESIGN.md](docs/design/DESIGN.md)（唯一需求来源）
* 进度看板：[docs/design/ROADMAP.md](docs/design/ROADMAP.md) · 逐批落点：
  [docs/design/CHANGELOG.md](docs/design/CHANGELOG.md)
* 专题规范：[LLM.md](docs/design/LLM.md)（模型接入）·
  [plugins.md](docs/design/plugins.md)（插件清单）·
  [Plug-in packaging.md](docs/design/Plug-in%20packaging.md)（元数据与打包 v2）·
  [UI shell and patterns.md](docs/design/UI%20shell%20and%20patterns.md)（UI 唯一权威）·
  [Front-end Design Guidelines.md](docs/design/Front-end%20Design%20Guidelines.md)（前端唯一形态标准）·
  [Context and Memory.md](docs/design/Context%20and%20Memory.md) ·
  [Tool exposure strategy and usage statistics.md](docs/design/Tool%20exposure%20strategy%20and%20usage%20statistics.md)
