# AGENTS.md

## 项目概览

- **项目名 / 仓库名 / 文件夹名**：`Locowrk`（用户指定，三者一致）
- **定位**：可热插拔、可回退、带桌面端（Tauri 2）的 Rust Agent 内核。"一切皆插件"——模型适配器、工具、会话存储、沙箱、Agent 循环本身都是插件，无特权模块
- **权威设计文档**：[docs/design/DESIGN.md](docs/design/DESIGN.md)。修改任何核心模块前先读对应章节，不要凭猜测实现；实现与文档冲突时先对齐文档。文档 §8 中的 `agent-kernel/` 指内核 Cargo 工作区，其 crate 命名（`plugin-api`、`cordis-core` 等）沿用文档，对外项目名是 Locowrk
- **当前进度**：**v2.17 排期——⑨ 上下文管理收口（P0）已完成（v2.19，提交 92f298b）**：settings.json `context` 区（窗口预设+自定义，未知回落 32k 保守保护 / 阈值 10–90% / 策略三档）+ `ContextSection` 替换壳批次占位；compact.rs 配置驱动重写（CompactConfig 共享 RwLock 热换入、滑动窗口/分层/仅摘要、预算制切点压到阈值 50%、摘要失败降级滑动、suggested_threshold 分档建议）；`SessionEvent::ContextCompacted` 进会话日志（摘要存全文，derive/state 折叠留尾部 kept_messages，serde default 兼容旧日志）+ `context_undo_compact`（rewind_to+fork，其后轮次一并回退）；`agent://context-usage` 进度条接线 + 压缩 SystemNotice [查看详情][撤销] 真接线。此前：壳批次（UI 骨架）已完成（v2.18，提交 a2c82a5）：顶层 4 视图定稿（对话/插件/记忆/设置）、页面模式库（`patterns/ListPage` 列表页 / 设置分区一一对应 / `chat/SystemNotice` 会话内系统消息 / IdleBadge + ContextProgressBar）、事件→UI 流向图与弹层规则成文 `docs/design/UI shell and patterns.md`（功能批次照其 §七只扩表不改骨架）、design tokens `--lw-*`、PluginMarket 291→54 行、托盘菜单 i18n 搭车完成。更早：编号阶段①–⑧全部落地（⑧ WASM 加载器 v2.16：PluginLoader trait + WasmLoader wasmtime 34 沙箱、.wasm 扩展名分发、epoch 预算按次重置、宿主 alloc/free 缓冲协议；⑦ 第三批插件 tool-git / memory-sqlite / tool-mcp v2.15；⑥ 第二批插件 + 安全底线 v2.14；⑤ 托盘+快捷键+~/.Locowrk v2.13；④ 元数据 v2 v2.11/v2.12；③ 第一批工具+mw-audit v2.9；② 真实 LLM genai 三合一 v2.8；① i18n+最小设置面板 v2.7）；阶段一～七（内核骨架 / Effect+Fiber / C ABI 热插拔 / Waterfall / Tauri 脚手架 / 审批弹窗 / PluginService+市场）均已提交；落点细节见 DESIGN.md §14 实现备注。**后续排期：下一批 = ⑩ 暴露声明 + 使用统计地基 + 闲置 UI（ToolSpec exposure 字段零破坏追加 / ToolCall 补 plugin_id / usage.sqlite3 宿主自有 5s 刷盘 / IdleLevel 人类尺度文案 / 筛选批量）→ ⑪ 记忆注入 + 记忆管理 UI → ⑫ EWMA 打分截断 + search_tools → ⑬ 智能层（看需求）**；收尾 backlog 随批次搭车。**逐项进度（完成打钩）看 docs/design/ROADMAP.md**

## 目录结构

```
docs/design/DESIGN.md   权威设计文档（唯一需求来源）
docs/design/ROADMAP.md  进度看板（唯一进度表：完成打钩 / 未完排期，每批完成先勾它）
docs/design/LLM.md      阶段八②参考：三合一 LLM 适配器（方案一 = genai 统一库封装）
docs/design/plugins.md  阶段八③参考：必备插件清单与落地顺序（P0 四件套起步）
docs/design/Plug-in packaging.md  阶段八④参考：插件元数据与打包 v2（多语言/图标/manifest.json）
docs/design/Tool exposure strategy and usage statistics.md  ⑩⑫参考：工具暴露策略与使用统计（exposure 三层 / EWMA / 闲置分级 / 闲置 UI）
docs/design/Context and Memory.md  ⑨⑪参考：上下文与记忆管理（压缩策略三档 / ContextCompacted 撤销 / Memory-Inject / 记忆管理页）
docs/design/UI shell and patterns.md  壳批次落点：UI 导航模型 / 页面模式库 / 事件→UI 流向图 / 弹层规则 / design tokens（功能批次只扩表不改骨架）
crates/
  plugin-api/           冻结 ABI 契约（唯一，改动即破坏兼容；ffi/manifest/plugin_side/tool 四模块）
  cordis-core/          Context / Effect / Fiber / Scope / EventBus
  kernel-agent/         Agent 主循环 / Waterfall / 中间件链
  plugin-host/          dlopen Loader / 多版本 Registry / FFI→Tool 适配
  plugin-service/       插件安装/卸载/更新编排（CLI 与桌面端共用；阶段七落地）
  session-store/        会话事件日志 / 快照 / 回放 / 补偿
  kernel-bin/           CLI 可执行入口（组装一切）
plugins-src/            内置插件源码（workspace 成员；tool-echo 为阶段三验收插件，tool-shout 为阶段七市场验收插件）
desktop/                Tauri 2 + Vue 3 桌面端（src/ 前端 + src-tauri/ 内核组装；src-tauri 是
                        独立 Cargo 包、不进根 workspace，见根 Cargo.toml 的 exclude）
plugins/                开发态插件安装目录，布局 plugins/{id}/{version}/（产物已 gitignore）
market/                 本地目录市场（阶段七；bash scripts/build_market.sh 暂存，产物已 gitignore）
config/profile.toml     当前激活的插件组合（阶段八设置面板接入）
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
bash scripts/build_plugins.sh  # 插件产物暂存到 plugins/（tool-echo 双版本 + tool-fs / tool-fetch）
bash scripts/check_length.sh   # 单文件 400 行上限检查（pre-commit 自动执行）
cargo run -p kernel-bin -- demo  # 阶段三验收叙事（需先跑上一条）
```

Git hooks 在 `.githooks/`（随仓库提交）。本仓库已通过 `git config core.hooksPath .githooks` 启用 pre-commit 检查；重新克隆后需重新执行一次该命令。

验收叙事（前置 `bash scripts/build_plugins.sh`）：

```bash
cargo run -p kernel-bin -- demo    # 阶段三：加载 1.2.0 → 激活 1.3.0 → 回退 1.2.0，全程不重启
cargo run -p kernel-bin -- demo4   # 阶段四：审批插件经 agent/request 改写参数；两 Agent 工具视图隔离
```

桌面端（desktop/，Tauri 2 + Vue 3；前置 `pnpm install` 一次、插件产物已构建）：

```bash
cd desktop && pnpm tauri dev        # 开发模式（窗口 + 热重载）：聊天、观察 tool-echo 工具调用
cd desktop && pnpm build            # 仅前端：vue-tsc 类型检查 + vite 构建
cd desktop/src-tauri && cargo test  # 桌面端 Rust 单测（runner 投影 / demo_llm / plugins_root）
```

注意：`desktop/src-tauri` 被 exclude 在根 workspace 之外，`cargo check` 等要在
`desktop/src-tauri` 目录下单独执行；其 `generate_context!` 编译期要求 `desktop/dist/`
存在（先 `pnpm build` 一次）。

网络提示：crates.io 直连超时时，给 cargo 命令加
`--config 'source.crates-io.replace-with="rsproxy"' --config 'source.rsproxy.registry="sparse+https://rsproxy.cn/index/"'`
走镜像（命令行级覆盖，勿改全局 ~/.cargo/config.toml）。

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
共用的 `Tool` / `CallCtx` 定义在 plugin-api 宿主侧、不进入 C ABI。内核经 `ToolSource`
服务 trait 解析工具；依赖图禁止 plugin-host 依赖 kernel-agent，故 plugin-host 只提供
同语义的固有 API（`RegistryToolSource::resolve`），由组装点（kernel-bin 的
`tool_source.rs`）桥接成该 trait 并注册进 Context）

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
- 每完成一批，先勾 docs/design/ROADMAP.md（⬜/◐ → ✅ 并补版本与提交哈希），再写 DESIGN.md 落点补记
