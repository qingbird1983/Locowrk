# [AGENTS.md](http://AGENTS.md)

## 项目概览

* **项目名 / 仓库名 / 文件夹名**：`Locowrk`（用户指定，三者一致）
* **定位**：可热插拔、可回退、带桌面端（Tauri 2）的 Rust Agent 内核。"一切皆插件"约束的是**无特权模块**，不是"所有代码都打成 DLL"：工具、会话存储、Agent 循环扩展面走插件契约；现役形态 = 9 个动态库插件（`plugins-src/`）+ 一批**宿主内置**能力（LLM 客户端按 v2.75 架构定性 = 宿主内置服务，审批、子 Agent 调度、定时任务、记忆树工具、浏览器预览同理），两条通路都经同一 `ToolSource`/目录机制进工具池。**沙箱插件（tool-sandbox）已随 v2.145 整目录退役**——不隔离 fs/网络即非安全边界，命令执行面收敛为 `shell` 单一入口
* **权威设计文档**：[docs/design/DESIGN.md](docs/design/DESIGN.md)。修改任何核心模块前先读对应章节，不要凭猜测实现；实现与文档冲突时先对齐文档。文档 §8 中的 `agent-kernel/` 指内核 Cargo 工作区，其 crate 命名（`plugin-api`、`cordis-core` 等）沿用文档，对外项目名是 Locowrk
* **读文档先分清状态**：`docs/design/` 里既有现役规范也有**已退役/未采纳**的历史稿——动手前先查 [docs/design/INDEX.md](docs/design/INDEX.md)（v2.149 批 E 新立，权威 / 已兑现 / 存档三类 + 逐篇状态）。这条纪律的来源 = 审计 H9「DESIGN.md 0 处 v2.145、照它实现会写出一套与代码相反的策略层」
* **当前进度**：**审计修复批 F-6 已完成（v2.156，2026-10-11；《AUDIT-FIX-PLAN-v1》§1 F5 第二项已落 + **F12b ✅** + §5 证据段；**就地订正两处失实自报** = 「14 条 `pub use`」实测 **17**、F5 余项 organize 621→**614** / session 415→**410**；生产码语义零改动——拆分是结构性搬用，F12b 是一句报文）**：**① 拆分** `settings/llm.rs` 682 行（落盘 / 校验 / 目录合成 / IPC 形状四件事挤一起）→ `settings/llm/` 五簇 + `mod` 38 = **762 行**（净 +80 全是簇头注），全守 400 线内 ⇒ `check_length` **38→37**（基线显式收紧，脚本注释同步改正「六簇 / 350 行」为实测「八簇 / 220 行」）。**② 等价判据 = 本刀唯一证据**（368 个桌面单测对结构搬用零证明力，次序错位编译器根本不看见）：块 **31=31**、**块体 634 非空行原样命中零丢失**（`#[serde(rename_all = "camelCase")]` 属性行也在块体内 ⇒ 展示面契约逐字搬运由脚本证）、倒挂 0、17 条 `pub use` 全命中、五下沉项引用 0。**③ 唯一一处有意改动** = `migrate_flat_llm` 的 `pub(super)`→`pub`（父模块 `pub use` 撞 **E0364**），`mod.rs` 用 `pub(super) use` 收口 ⇒ 对外可见面不变；判据不"放宽"而是三条硬断言（改成 `pub use` 即红）。**④ 五个空转 re-export 摘掉**（`settings.rs` 从未带走 + `mod llm` 私有 ⇒ 无人可达；留着撞 `unused_imports`，挂 `#[allow]` 又是批 A 在清的形态）。**⑤ F12b** `cwd_of` 分病因 + 两条出路，并按 `run()` 实况订正预写的 "workspace root"；预写「既有测试加断言」不实（该报文全仓零覆盖）⇒ 实为**新增**测试。tool-shell **0.1.8**、暂存 **22 份**。**⑥ 新守卫**：`rename_all` 编译器不证而既有断言读的是 Rust 字段 ⇒ 新测试对 IPC JSON **六层各查键集**双向锁（递归收全集的版本三条变异全停在同一个无关键上 = 红但报不出谁掉的）。**⑦ 判据自己骗人三例（本批最值钱，细节在 §5）** 注释喂绿 ⇒ 只解析 `pub use` 语句、引用查证须在**剥掉字符串与行尾注释的代码面**上做；`--exact` 配短名把过滤打成空集（`0 passed` 冒充"守卫没抓到"）⇒ 硬断言 `ran≥1`；还原押外层 `finally` 让上一条变异编进后几条的二进制 ⇒ **per-case 即时还原** + 进入即校验 sha。附纪律：**变异必须单变量**（整行删 serde 会连 `default` 一起摘，炸点跑到反序列化）。**⑧ 变异 9 动作全闭合**（判据 6 条含「抹头注仍 VERIFY_OK」「簇外加真引用必红」两条反向；IPC 6 条含 `NetworkProxy` 不红的 BOUNDARY；F12b 3 条），全部 sha 逐字节还原 + grep 零残留 + 复绿见 `Compiling`。测试账 根 **437→438** / desktop **368→369**；门禁 = 双侧 fmt + clippy **0** + 四闸 = 收紧后基线（**37** · 891=891 · 85·85·85 · 10·10·0）。**边界（记档）**：①**未做设置页模型区真机走查**——IPC 证据 = 属性行逐字 + 往返测试吃原始 JSON + 新按层守卫 + 369 绿，运行期一态未截图；②`PricingView` 未进守卫（目录未命中时 `pricing = null`）；③F5 余三项未动（614 / 237 / 410 + 巨型测试目录化）；④判据脚本仍在 `tmp/` 未入库（「第五道闸」待点头）；⑤F15 待裁定。
* **此前进度**：**审计修复批 F-5 已完成（v2.155，2026-10-11）**——F12 报错教学三支：**病根比登记靠上**（`fs_read`/`fs_grep`/`fs_list` 共吃共享基座 `fsutil.rs::resolve_existing` 的 NotFound 分支 ⇒ 一处补「先 `fs_list` 上级」、四插件同受益；措辞硬约束 = 新文本不得含 `"outside the allowed roots"`，那是守卫拒绝的识别码）+ 元字符报文把替身说出来（`fs_grep`/`fs_list`）+ `proof.rs` 引号不闭合不再吃通用 `structural()` 模板；F13 = `build_market.sh` 的 36 行 heredoc manifest → 与 `build_plugins.sh` 同通路（真源 = 新 `plugins-src/tool-shout/manifest.json`）。**字节级新立 F15**：`plugins/{id}/{ver}/` 每次都用当前 HEAD 重编 ⇒ **开发态的"历史版本目录"不是历史行为快照**（demo 证明的是版本切换机制不是行为回滚；「旧版本永远保留」只在安装态成立），A/B 待裁定。复核时新立 **F12b**（工单「四支」而正文三条**不是笔误**，漏的正是 `cwd` 那支）。三条变异必红 + plugin-host 两条经真实 dlopen 的 FFI 面端到端断言；版本随批四件、暂存 21 份；根 437 / desktop 368 持平。全文见归档与 ROADMAP/CHANGELOG v2.155 节。
* **更早批次流水已归档（v2.149 批 E / E4 立，v2.150–v2.156 七批沿用并更新）**：**v2.155 及其之前 v2.55–v2.154 共 38 条**逐批流水 = [docs/design/archive/AGENTS-progress-archive.md](docs/design/archive/AGENTS-progress-archive.md)（**逐字原样搬出，内容未改写**；E4 的终态校验 = 与 `git show HEAD:AGENTS.md` 行区间逐行比对 missing 0 / extra 0，此后每批搬入同样按逐行 identical 校验。**条数一律按实测**：本批核对时抓到 v2.151 批的归档标题写「共 34 条」而文件里实有 33 条 = 多算 1，已按实测校正；今后这类自报数字由脚本数出来再写，不靠推算）。为什么搬：本文件是每个会话的注入源且有 65,536 字节预算，此前 94% 的字节是历史流水（最大单条 48,838 字节），真正要读的现行纪律反而被挤出预算。**写入纪律（批 E 立，v2.150 / v2.151 / v2.152 / v2.153 / v2.154 / v2.155 六批沿用未改）**：批完成后「当前进度」写本文件、只保留 **当前 + 前一版 + 归档指针**三条（三条合计 ≤6,000 字节；本批实测 3285 + 1396 + 1313 = 5994，重细节进 CHANGELOG/ROADMAP 而不是这里）；被挤出的那条整体搬进归档文件**正文最前**（倒序），不在本文件留残段。

## 目录结构

```
docs/design/INDEX.md   **文档索引——动手前先查它分清「权威 / 已兑现 / 待研究 / 存档·未采纳」**
docs/design/DESIGN.md   权威设计文档（唯一需求来源）
docs/design/ROADMAP.md  进度看板（唯一进度表：完成打钩 / 未完排期，每批完成先勾它）
docs/design/CHANGELOG.md  逐批落点补记日志（DESIGN.md 不堆补记，新批次写这里；每版一整行倒序）
docs/design/LLM.md      模型接入轨：单线 OpenAI 兼容协议 + 传输韧性 + 推理档位 + 多模态 + 子代理模型档
docs/design/plugins.md  必备能力清单与落地顺序（§一 带「现役状态」列：哪些是插件 / 宿主内置 / 已退役）
docs/design/Plug-in packaging.md  插件元数据与打包 v2（多语言/图标/manifest.json/版本策略 §9.4）
docs/design/Tool exposure strategy and usage statistics.md  工具暴露策略与使用统计（exposure 三层 / EWMA / 闲置分级）
docs/design/Context and Memory.md  上下文与记忆总纲（注入顺序 / 双层项目隔离；压缩三档已随 v2.137 退役）
docs/design/UI shell and patterns.md  UI 唯一权威：导航模型 / 页面模式库 / 事件→UI 流向图 / 弹层规则 / tokens
docs/design/Front-end Design Guidelines.md  前端唯一形态标准（15 类唯一形态 + 特例申请制 + stylelint/check-style 双闸）
docs/design/Memory Tree Design v2.md · KV Prefix Cache Design v2.md  记忆树与前缀缓存规范正身（v1 两份留作对照）
docs/design/NEW/        近几轮设计稿（兑现后不搬走，状态看 INDEX.md）；BUG/ 工单；archive/ 退役与未采纳
crates/
  plugin-api/           冻结 ABI 契约（ffi/manifest/plugin_side/tool）+ 宿主侧共享基座（fsutil/memlayout/naming/policy）
  cordis-core/          Context / Effect / Fiber / Scope / EventBus
  kernel-agent/         Agent 主循环 / Waterfall 监听器 / 洋葱中间件 / subagent 子 Agent 调度
  plugin-host/          dlopen Loader / 多版本 Registry / FFI→Tool 适配 / wasmtime WASM 沙箱加载器
  plugin-service/       插件安装·卸载·更新编排（CLI 与桌面端共用；lockfile + 市场）
  session-store/        会话事件日志 / 快照 / 回放 / 补偿 / uncertain 崩溃恢复扫描
  memory-tree/          记忆树（章节捆 L0-L2 / 整理触发 / 记录员 / 投影存储 / 跨会话检索）
  kernel-bin/           CLI 可执行入口（headless 组装点）
plugins-src/            内置插件源码（workspace 成员，9 个 id；tool-echo 双版本 = 热插拔验收夹具，tool-shout = 市场夹具）
desktop/                Tauri 2 + Vue 3 桌面端（src/ 前端 + src-tauri/ 内核组装；src-tauri 是
                        独立 Cargo 包、不进根 workspace，见根 Cargo.toml 的 exclude）
plugins/                开发态插件安装目录，布局 plugins/{id}/{version}/（产物已 gitignore，版本真源见 scripts/build_plugins.sh）
market/                 本地目录市场（bash scripts/build_market.sh 暂存，产物已 gitignore）
scripts/                构建与门禁脚本（build_plugins / build_market / check_length / check-i18n / check-commands / check-middleware-wiring / check-style）
```

各 crate 的 `lib.rs` 顶部注释列有计划模块和对应 [DESIGN.md](http://DESIGN.md) 章节号。

## 常用命令

**日常开发（根 workspace）**

```bash
cargo check / cargo build / cargo test            # 全部测试（workspace）
cargo test -p cordis-core                         # 只测某个 crate
cargo clippy --workspace --all-targets
cargo fmt --all
bash scripts/build_plugins.sh                     # 插件产物暂存到 plugins/（**版本真源 = 脚本内 stage 清单**）
cargo run -p kernel-bin -- demo                   # 热插拔验收叙事（需先跑上一条）
cargo run -p kernel-bin -- demo4                  # waterfall 插桩 + 两 Agent 工具视图隔离
```

**每批收尾门禁（批 B 立的整套；最后一次编辑之后全量复跑，不要在中间态报绿）**

```bash
# 根侧三件（clippy 的 `-D warnings` 必须在 `--` 之后：写成
# `cargo clippy --workspace --all-targets -D warnings` 会被 cargo 当成自己的
# 参数并报 `error: unexpected argument '-D'`——v2.154 批 F-4 的门禁脚本就踩在
# 这里，且因为它把 clippy 接进了管道（`| tail`），退出码被 tail 盖住 = 假绿）
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# 桌面侧三件（src-tauri 不在根 workspace，必须单独跑）
cd desktop/src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test

# 前端三件
cd desktop && pnpm build            # vue-tsc 类型检查 + vite 构建（generate_context! 也吃 dist/）
cd desktop && pnpm run lint:style   # stylelint：CSS 声明维度，一级判错**不可绕过**
node scripts/check-style.mjs        # 模板维度（tailwind 圆角类之类）

# 四道结构闸
bash scripts/check_length.sh                     # 单文件 400 行「基线比较」报告闸（新增超限判红；三问自检可不拆）
node scripts/check-i18n.mjs                      # zh/en 键集双向闭合 + 被引用键缺失/无静态引用键报告
node scripts/check-commands.mjs                  # #[tauri::command] ↔ generate_handler! ↔ 前端 invoke 三向闭合
node scripts/check-middleware-wiring.mjs         # 中间件「有定义、无生产构造点」判红（KNOWN_UNWIRED 白名单现为空）
```

Git hooks 在 `.githooks/`（随仓库提交），pre-commit 按暂存面触发上面四道结构闸；重新克隆后需重新执行一次 `git config core.hooksPath .githooks`。CI = `.github/workflows/ci.yml` 三作业（kernel / desktop(windows-latest) / hygiene），批 B 立：此前桌面端与前端在 CI 上完全不存在。

**验收叙事**（前置 `bash scripts/build_plugins.sh`）：

```bash
cargo run -p kernel-bin -- demo    # 装载 tool-echo 0.1.0 → 激活 0.2.0 → 回退 0.1.0，全程不重启
cargo run -p kernel-bin -- demo4   # 审批中间件经 agent/request 改写参数；两 Agent 工具视图隔离
```

桌面端（desktop/，Tauri 2 + Vue 3；前置 `pnpm install` 一次、插件产物已构建）：

```bash
cd desktop && pnpm tauri dev        # 开发模式（窗口 + 热重载）：聊天、观察工具调用与审批卡
cd desktop && pnpm build            # 仅前端：vue-tsc 类型检查 + vite 构建
cd desktop/src-tauri && cargo test  # 桌面端 Rust 单测（runner 投影 / llm 轨 / 组装点）
```

注意：`desktop/src-tauri` 被 exclude 在根 workspace 之外，`cargo check` 等要在 `desktop/src-tauri` 目录下单独执行；其 `generate_context!` 编译期要求 `desktop/dist/` 存在（先 `pnpm build` 一次）。

网络提示：[crates.io](http://crates.io) 直连超时时，给 cargo 命令加 `--config 'source.crates-io.replace-with="rsproxy"' --config 'source.rsproxy.registry="sparse+https://rsproxy.cn/index/"'` 走镜像（命令行级覆盖，勿改全局 \~/.cargo/config.toml）。

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

（`X → Y` 表示 X 依赖 Y，与 [DESIGN.md](http://DESIGN.md) §8 一致。kernel-agent 与 plugin-host 互不依赖： 共用的 `Tool` / `CallCtx` 定义在 plugin-api 宿主侧、不进入 C ABI。内核经 `ToolSource` 服务 trait 解析工具；依赖图禁止 plugin-host 依赖 kernel-agent，故 plugin-host 只提供 同语义的固有 API（`RegistryToolSource::resolve`），由组装点（kernel-bin 的 `tool_source.rs`）桥接成该 trait 并注册进 Context）

* `plugin-api` 不依赖任何其他 crate；`cordis-core` 只依赖 `plugin-api`
* `kernel-bin`（CLI）和 `desktop`（GUI）是仅有的两个组装点，业务逻辑不写在这里
* **UI 永远不直接操作 `PluginRegistry`**，只通过 `PluginService`；Tauri command 是薄适配层，只做参数转换，不写业务逻辑
* 流式输出走事件推送（`agent://chunk` 等），不塞进 command 返回值

## 模块与文件规模纪律

* `lib.rs` / `main.rs` 只放模块声明、re-export 和文档注释，不写实现
* 实现必须落到 [DESIGN.md](http://DESIGN.md) 规划的模块文件（如 cordis-core 的 `context.rs` / `effect.rs` / `fiber.rs` / `scope.rs` / `event_bus.rs` / `runtime.rs`）； 文档未规划的新职责 = 先建新文件并加 `mod` 声明，禁止堆进已有文件
* **单文件建议不超过 400 行**（2026-10-05 用户裁定，替代原"超限必拆"的机械门禁）：超过时先做**三问自检**——①这个文件是否只做一件事？②它的名字是否准确概括内容？③修改它是否经常牵动不相关的部分？三问皆「是」= 高内聚，可以不拆（提交说明里给理由）；有任一「否」= 即使没到 400 行也应拆成目录式子模块（`context/` + `context/mod.rs`）。`scripts/check_length.sh` 是**基线比较闸**（v2.146 批 B 改造，此前它恒 `exit 0` = 空转门禁）：超限文件总数与脚本内 `BASELINE` 比较，**新增超限判红拦截**、净降则提醒收紧基线；三问判断仍是人的职责，基线只防"顺手把文件写胖"
* `#\\\[cfg(test)\\\]` 测试放独立文件（如 `effect/tests.rs`，模块内以 `#\\\[cfg(test)\\\] mod tests;` 声明）
* 内部类型一律 `pub(crate)`，只有跨 crate 需要的才 `pub`
* 动手写码前，先列出"将新建/修改哪些文件、各放什么"的清单，再按清单实现
* 超长文件的拆分是独立任务：按职责拆子模块，保持外部 API 与既有测试不变

## 设计纪律（写内核代码必须遵守；完整版见 [DESIGN.md](http://DESIGN.md) 附录 C）

1. 所有对 `Context` 的修改必须走 `effect()` 并携带逆操作（Disposer），撤销按 LIFO
2. 永不 dlclose：逻辑卸载 + 物理保留（graveyard），进程重启才真正释放
3. FFI 调用放 `spawn_blocking`；插件与宿主两侧都必须 `catch_unwind`
4. 先写 `ToolCall` 事件再执行工具——崩溃后 replay 能识别悬空调用
   ⚠ **落盘粒度实况（v2.149 批 E 订正）**：桌面端是**每 step 批量落盘**（`run_turn` 在 step 收束后一次 `append_slice`），工具执行中途崩溃时 jsonl 里**没有**该条 `ToolCall`——悬空调用只在 mid-flush 崩溃（步刷盘中途死亡）出现。崩溃恢复的诚实闭合面 = `session-store/src/uncertain.rs`（v2.134）+ 投影层合成 ToolResult（`derive_messages`）；**per-event 落盘是挂账未做**（需内核持久化钩子）
5. 会话事件只追加不删除：回退 = 移动游标 + replay；快照带 `schema_version` upcaster
   ⚠ **磁盘快照从未接线（v2.148 批 D 标 RESERVED）**：`snapshot.rs` 的实现与测试都在位，但**没有任何生产读写点**——恢复真源 = JSONL 逐行回放。别把"快照存在"当成性能优化已生效，也别与内存态 `Session::snapshots` 混
6. `SideEffect::Irreversible` 工具调用前**默认**审批，审批结果也是一条事件
   ⚠ **v2.145 权限模型重设计后不是"必弹卡"（批 E 订正措辞）**：现役是七序决策表（DESIGN §7.2）——计划档硬拒 → 硬门（隐私门 + 越界门，**全档位含 Auto 都拦、只许一次**）→ 项目 deny 规则 → 模式放行（Auto 全放行 / AcceptEdits 放行 Compensable）→ 项目 allow 粘性规则 → 会话豁免 → 决策卡。所以免审路径有五条；写新工具时 `side_effect` 标错=谎报（v2.146 批 A 的 `tool-fetch` manifest 就是反面教材）
7. 补偿工具必须幂等，用 `call_id` 做去重键
8. 多版本并存：回退 = 切换 `active` 指针；更新失败自动回滚，旧版本永远保留

## 平台注意事项（Windows 开发机）

* Shell 是 Git Bash，路径写正斜杠
* 设计文档示例中的插件产物 `.so`，在 Windows 构建下是 `.dll`（见 plugins/README.md）
* dlopen 路径区分平台后缀，`plugin-host` 实现时注意 `lib`/`.dll`/`.so`/`.dylib` 前后缀差异
* 本机 IPv4 回环被过滤驱动拖 \~2s/连接（127.0.0.1 curl 2.02s vs ::1 0.02s，疑 clash TUN/杀软钩子；㉕·微调② v2.45 核查）——vite dev 已钉 `server.host: "::1"` 绕开 （冷启动稳态首帧 108–140ms），**勿改回双栈**，否则 dev 首帧回到 30s+

## 工作规则

* `plugin-api` 是冻结 ABI：对外发布后只允许把字段追加到结构体末尾，不改已有签名；`PLUGIN_ABI_VERSION` 变更是 breaking 事件
* 临时实验文件放 `tmp/`（已 gitignore）或系统临时目录，不要放仓库根目录
* 不要把 API Key、Token、密码、私钥写进任何项目文件
* **可配置原则（用户 2026-10-03 裁定，两条同源）**：①**数值**——凡需要数值设定且存在不确定性的参数，一律进设置页成为可调项，不为用户做死限定（给合理缺省值）；②**禁止项**——所有界面上写死的禁止/限制都要有开关可打开或关闭（含最危险的动作），开发相关与危险相关的开关/数值统一收进\*\*「开发者」设置分区\*\*（默认隐藏：侧栏版本号连点 7 次解锁，安卓式；分区内可锁回）。可控项分「用户」「开发者」两层各归其位
* 删除、覆盖、迁移、发布等高风险操作先向用户确认
* **前端统一标准（用户 2026-10-06 裁定「所有自发明的一律判错」+「特例申请制」）**：《docs/design/Front-end Design Guidelines.md》是前端**唯一**形态标准——同类只许一种形态；动任何前端代码前先查标准对应类目；清单外的新形态/特例 = **先申请（说明是什么/在哪/为什么/影响）→ 用户批准 → 先修订标准再写码**。机械门禁：pre-commit 跑 stylelint（CSS 声明维度，desktop/stylelint.config.cjs）+ `scripts/check-style.mjs`（模板维度），一级判错提交即拒、不可绕过；豁免必须登记标准 §七 + 行内 `stylelint-disable-line` 注释引用批准条目。v2.87 起 --lw-fs-*/--lw-gap-* 令牌已建齐，裸字号/裸间距同为一级判错（豁免 = 行内凭证注释：style 维度 stylelint-disable-line 引 §七 或 §二⑬ 光学修正，模板维度上一行内 lw-waive 注释）
* 完成任务后说明：改了哪些文件、解决了什么问题、是否运行了测试、遗留风险
* 每完成一批，先勾 docs/design/ROADMAP.md（⬜/◐ → ✅ 并补版本与提交哈希），再写 docs/design/CHANGELOG.md 落点补记（**[DESIGN.md](http://DESIGN.md) 不再堆补记**——用户 2026-10-03 裁定：堆数据既危险又会破坏最原始设计，也让文档无限膨胀；DESIGN 只随批次改规范正文与版本行）