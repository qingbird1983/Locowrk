# AGENTS.md

## 项目概览

- **项目名 / 仓库名 / 文件夹名**：`Locowrk`（用户指定，三者一致）
- **定位**：可热插拔、可回退、带桌面端（Tauri 2）的 Rust Agent 内核。"一切皆插件"——模型适配器、工具、会话存储、沙箱、Agent 循环本身都是插件，无特权模块
- **权威设计文档**：[docs/design/DESIGN.md](docs/design/DESIGN.md)。修改任何核心模块前先读对应章节，不要凭猜测实现；实现与文档冲突时先对齐文档。文档 §8 中的 `agent-kernel/` 指内核 Cargo 工作区，其 crate 命名（`plugin-api`、`cordis-core` 等）沿用文档，对外项目名是 Locowrk
- **当前进度**：工作区骨架已建（`cargo check` 通过），处于落地路径**阶段一**（内核骨架 + 事件日志，见 DESIGN.md §14）

## 目录结构

```
docs/design/DESIGN.md   权威设计文档（唯一需求来源）
crates/
  plugin-api/           冻结 ABI 契约（唯一，改动即破坏兼容）
  cordis-core/          Context / Effect / Fiber / Scope / EventBus
  kernel-agent/         Agent 主循环 / Waterfall / 中间件链
  plugin-host/          dlopen Loader / 多版本 Registry / FFI→Tool 适配
  plugin-service/       插件安装/卸载/更新编排（CLI 与桌面端共用）
  session-store/        会话事件日志 / 快照 / 回放 / 补偿
  kernel-bin/           CLI 可执行入口（组装一切）
desktop/                Tauri 2 + Vue 3 桌面端（阶段五脚手架，目前仅 README）
plugins/                插件安装目录，布局 plugins/{id}/{version}/（多版本并存）
config/profile.toml     当前激活的插件组合
```

各 crate 的 `lib.rs` 顶部注释列有计划模块和对应 DESIGN.md 章节号。

## 常用命令

```bash
cargo check          # 快速类型检查
cargo build          # 构建
cargo test           # 全部测试（workspace）
cargo test -p cordis-core   # 只测某个 crate
cargo clippy --workspace --all-targets
cargo fmt
bash scripts/check_length.sh   # 单文件 400 行上限检查（pre-commit 自动执行）
```

Git hooks 在 `.githooks/`（随仓库提交）。本仓库已通过 `git config core.hooksPath .githooks` 启用 pre-commit 检查；重新克隆后需重新执行一次该命令。

桌面端命令（`pnpm`/`tauri`）在阶段五脚手架后再补充到本节。

## 架构边界（依赖严格单向，违反即设计错误）

```
kernel-bin        → kernel-agent + plugin-service + session-store   （组装点）
desktop/src-tauri → kernel-agent + plugin-service + session-store   （组装点）
kernel-agent      → cordis-core + plugin-api + session-store
plugin-service    → plugin-host + cordis-core
cordis-core       → plugin-api
plugin-host       → plugin-api
session-store     → plugin-api
```

（`X → Y` 表示 X 依赖 Y，与 DESIGN.md §8 一致。kernel-agent 与 plugin-host 互不依赖：
共用的 `Tool` / `CallCtx` 定义在 plugin-api 宿主侧、不进入 C ABI；内核经 `ToolSource`
服务 trait 解析工具，由 plugin-host 实现并注册进 Context）

- `plugin-api` 不依赖任何其他 crate；`cordis-core` 只依赖 `plugin-api`
- `kernel-bin`（CLI）和 `desktop`（GUI）是仅有的两个组装点，业务逻辑不写在这里
- **UI 永远不直接操作 `PluginRegistry`**，只通过 `PluginService`；Tauri command 是薄适配层，只做参数转换，不写业务逻辑
- 流式输出走事件推送（`agent://chunk` 等），不塞进 command 返回值

## 模块与文件规模纪律

- `lib.rs` / `main.rs` 只放模块声明、re-export 和文档注释，不写实现
- 实现必须落到 DESIGN.md 规划的模块文件（如 cordis-core 的
  `context.rs` / `effect.rs` / `fiber.rs` / `scope.rs` / `event_bus.rs` / `runtime.rs`）；
  文档未规划的新职责 = 先建新文件并加 `mod` 声明，禁止堆进已有文件
- **单文件软上限 400 行**，超限按职责拆成目录式子模块（`context/` + `context/mod.rs`）；
  `scripts/check_length.sh` 会机械检查，pre-commit 拒绝超限提交
- `#[cfg(test)]` 测试放独立文件（如 `effect/tests.rs`，模块内以 `#[cfg(test)] mod tests;` 声明）
- 内部类型一律 `pub(crate)`，只有跨 crate 需要的才 `pub`
- 动手写码前，先列出"将新建/修改哪些文件、各放什么"的清单，再按清单实现
- 超长文件的拆分是独立任务：按职责拆子模块，保持外部 API 与既有测试不变

## 设计纪律（写内核代码必须遵守；完整版见 DESIGN.md 附录 C）

1. 所有对 `Context` 的修改必须走 `effect()` 并携带逆操作（Disposer），撤销按 LIFO
2. 永不 dlclose：逻辑卸载 + 物理保留（graveyard），进程重启才真正释放
3. FFI 调用放 `spawn_blocking`；插件与宿主两侧都必须 `catch_unwind`
4. 先写 `ToolCall` 事件再执行工具——崩溃后 replay 能识别悬空调用
5. 会话事件只追加不删除：回退 = 移动游标 + replay，快照带 `schema_version` upcaster
6. `SideEffect::Irreversible` 工具调用前必须审批，审批结果也是一条事件
7. 补偿工具必须幂等，用 `call_id` 做去重键
8. 多版本并存：回退 = 切换 `active` 指针；更新失败自动回滚，旧版本永远保留

## 平台注意事项（Windows 开发机）

- Shell 是 Git Bash，路径写正斜杠
- 设计文档示例中的插件产物 `.so`，在 Windows 构建下是 `.dll`（见 plugins/README.md）
- dlopen 路径区分平台后缀，`plugin-host` 实现时注意 `lib`/`.dll`/`.so`/`.dylib` 前后缀差异

## 工作规则

- `plugin-api` 是冻结 ABI：对外发布后只允许把字段追加到结构体末尾，不改已有签名；`PLUGIN_ABI_VERSION` 变更是 breaking 事件
- 临时实验文件放 `tmp/`（已 gitignore）或系统临时目录，不要放仓库根目录
- 不要把 API Key、Token、密码、私钥写进任何项目文件
- 删除、覆盖、迁移、发布等高风险操作先向用户确认
- 完成任务后说明：改了哪些文件、解决了什么问题、是否运行了测试、遗留风险
