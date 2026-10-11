# [AGENTS.md](http://AGENTS.md)

## 项目概览

* **项目名 / 仓库名 / 文件夹名**：`Locowrk`（用户指定，三者一致）
* **定位**：可热插拔、可回退、带桌面端（Tauri 2）的 Rust Agent 内核。"一切皆插件"约束的是**无特权模块**，不是"所有代码都打成 DLL"：工具、会话存储、Agent 循环扩展面走插件契约；现役形态 = 9 个动态库插件（`plugins-src/`）+ 一批**宿主内置**能力（LLM 客户端按 v2.75 架构定性 = 宿主内置服务，审批、子 Agent 调度、定时任务、记忆树工具、浏览器预览同理），两条通路都经同一 `ToolSource`/目录机制进工具池。**沙箱插件（tool-sandbox）已随 v2.145 整目录退役**——不隔离 fs/网络即非安全边界，命令执行面收敛为 `shell` 单一入口
* **权威设计文档**：[docs/design/DESIGN.md](docs/design/DESIGN.md)。修改任何核心模块前先读对应章节，不要凭猜测实现；实现与文档冲突时先对齐文档。文档 §8 中的 `agent-kernel/` 指内核 Cargo 工作区，其 crate 命名（`plugin-api`、`cordis-core` 等）沿用文档，对外项目名是 Locowrk
* **读文档先分清状态**：`docs/design/` 里既有现役规范也有**已退役/未采纳**的历史稿——动手前先查 [docs/design/INDEX.md](docs/design/INDEX.md)（v2.149 批 E 新立，权威 / 已兑现 / 存档三类 + 逐篇状态）。这条纪律的来源 = 审计 H9「DESIGN.md 0 处 v2.145、照它实现会写出一套与代码相反的策略层」
* **当前进度**：**审计修复批 F-19 已完成（v2.169，2026-10-12；F6 第五刀 C13 = `stores/settings.ts` 十份保存同构收进唯一实现 + 兑现「`setTimeout` 无句柄复用」缺陷 + 学到一条新失败模式 = **判据「崩而不红」比红更危险**；除该缺陷修正外逐字等价、零 Rust 改动、外部 API 未变；根 **438** / desktop **369** 持平，**21 项动作读数全 0 = `GATE_ALL_GREEN`**，`check_length` 25 == 基线）**：`settings.ts` **263→210**（净 −53），十份同构体（防连点门→清 error→`setSettings`→投影回写→闪『已保存』→失败进 error）收成 `patchSettings(patch, {guard, flash})` + `markSaved()`。登记规模实测订正 263 行、**10 份**（审计写 135 行/9 份），且其中**两份带从未写下来的真实例外** ⇒ 按参数保留 + 理由入注释：`saveThinking` 的 `flash:false` 连 `savedFlash=false` 都不执行（旧形态没那句清零）；`saveChat` 的 `guard:false`——列宽由**拖拽**驱动，门会把"最后一次"当连点丢掉 = 松手后宽度回不去。缺陷兑现：旧十份各自 `setTimeout` 不留句柄 ⇒ 连续保存时**前一个 timer 提前关掉后一个刚亮的提示**（真 pinia + 假时钟 `{at2001:false}`→`{at2001:true}`）。**判据 `tmp/v173-equiv.mts` 四档（打印 6 段读数）**（本仓无前端单测 runner）= 把**改前与改后两版都装起来**跑同批场景（真 `pinia` options-store + 真 store + `api`/`i18n`/`theme` 三面替身 + 受控假时钟）：L1 轨迹 52 组逐字段相等／**L2 委托键名**逐个相同且十键互不相同／L0 结构／L3 三条缺陷与例外正证。**最值钱**：变异 N9 首跑没红在指定档而是把判据整个崩掉——绕过唯一实现就没有 try/catch，替身 reject 成未捕获异常 ⇒ 后面三档一行没跑，缺失读数看着像"还没跑到"。修 = 未捕获异常本身写进轨迹（`threw`）+ 门禁新立 **`criterion_segments`**（少一段即红）= **判据的完整性也要有机器闸**。变异 10 条（N0 见绿 + N1–N9 各红指定档）另带两条口径自纠（L0 锚点带 `guard && ` 前缀数到 0；门禁 `bare=1` 抓的是注释里的**来历指称** = 批 F-18 同族第二次）与一条改靶记录（N4 首跑是靶子非法不是判据有洞：`{ tools }` 是取变量 ⇒ ReferenceError；合法形 `{ tools: memory }`）。**边界**：①判据里 `api`/`i18n`/`theme` 与 `@vue/devtools-api` 是替身（后者 no-op——硬去 `.pnpm` 挑版本 = 判据造假）；②未做 CDP 走查；③判据未入库（靠 `git show HEAD:` 取改前形态，提交后 HEAD 即改后 ⇒ 入库先钉参考提交）；④**F6 仍余 C12**，逐组件取证后实测分三族（A 4 处逐字同形 / B 5 处双宿主 + capture + `emit('close')` / C 条件 Esc）⇒ 下一刀只收族 A，其余走特例申请制，**capture 与冒泡的时机差和"谁拥有 close"是真语义差不许拧**。文档：PLAN §1「二十批」+ F6 行改写 + §5 段；ROADMAP v2.169 + 回补 v2.168 哈希 `5359e33`；CHANGELOG v2.169；v2.168 逐字入归档。
* **此前进度**：**审计修复批 F-18 已完成（v2.168，2026-10-12，提交 `5359e33`；F6 前端四刀 C8–C11 下沉 + 顺带修掉**两条审计未覆盖**的用户可见缺陷 + 把抓不到缺陷的闸洞补成机器断言；除那两处外逐字等价、零 Rust 改动；根 438 / desktop 369 持平，18 项读数全 0 = GATE_ALL_GREEN，`check_length` 25 == 基线）**：新 `lib/tool-args.ts` 顶掉两份命令剥包装与相对化（**合并的是匹配核不是显示语义** ⇒ 折叠行只给裸文件名、明细多留目录并剥 `\\?\`）；`format.ts` **单位进函数名**（`relativeTimeSec`/`relativeTimeMs` + 唯一渲染处 `relativeTimeLabel`）+ `use-copy.ts::copyDelegatedCode`。缺陷 (a) 记忆树 `created_at=now_ms` 喂进按秒算的旧函数 ⇒ `diffMs` 变负 ⇒「多久以前」**恒「刚刚」**；(b) 旧 `fmtTime` 拼 `time.${key}` 而该前缀在语言包里**一个键都没有** ⇒ 直显 `time.hours`（运行证据由真实 vue-i18n + 两份语言包给出）。判据七档偏差 0 = 改前实现**从 `git show HEAD:` 逐字节切片再 import 执行**与改后逐字比 + **L5 八个调用点的单位口逐个钉住**；变异 10 条各红指定档，首跑两条没红**都判在证据链自己**（CRLF 锚点命中 0；`includes(名字)` 被解构语句喂成恒真 ⇒ 改断言**接点**）。最值钱 = `check-i18n` 对缺陷 (b) **双向瞎** ⇒ 新立「**动态前缀必须命中至少一个真实键**」+ 点名出处（终态 前缀 22 · 死前缀 0）。边界：未做走查；C11 文本级（无 jsdom）；明细面双重 clip 故意保留；`memory.time.*` 改名 = 独立一刀 ⬜。
* **更早批次流水已归档（v2.149 批 E / E4 立，v2.150–v2.169 二十批沿用并更新）**：**v2.168 及其之前共 52 个非空条目行**（行首 `* ` 51 条 + 1 条历史无前缀段 ㉚ v2.58）逐批流水 = [docs/design/archive/AGENTS-progress-archive.md](docs/design/archive/AGENTS-progress-archive.md)（**逐字原样搬出，内容未改写**；终态校验 = 与 `git show HEAD:AGENTS.md` 逐行比对 missing 0 / extra 0。**条数一律按实测并写清口径**（按行首 `* ` 数与按非空行数差一条：历史段 ㉚ v2.58 无前缀）。为什么搬：本文件是每个会话的注入源且有 65,536 字节预算，此前 94% 的字节是历史流水（最大单条 48,838 字节），真正要读的现行纪律反而被挤出预算。**写入纪律（批 E 立，v2.150–v2.167 十八批沿用未改）**：批完成后「当前进度」写本文件、只保留 **当前 + 前一版 + 归档指针**三条（三条合计 ≤6,000 字节；本批实测 2862 + 1511 + 1177 = 5550，重细节进 CHANGELOG/ROADMAP 而不是这里）；被挤出的那条整体搬进归档文件**正文最前**（倒序），不在本文件留残段。

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
bash scripts/build_plugins.sh                     # 插件产物暂存到 plugins/（**版本真源 = 脚本内 PLAN 清单**；每 id 单版本，仅 tool-echo 双份 = 机制夹具，重复 stage 判错——F15-A）
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

# 五道结构闸
bash scripts/check_length.sh                     # 单文件 400 行「基线比较」报告闸（新增超限判红；三问自检可不拆）
node scripts/check-i18n.mjs                      # zh/en 键集双向闭合 + 被引用键缺失/无静态引用键报告
node scripts/check-commands.mjs                  # #[tauri::command] ↔ generate_handler! ↔ 前端 invoke 三向闭合
node scripts/check-middleware-wiring.mjs         # 中间件「有定义、无生产构造点」判红（KNOWN_UNWIRED 白名单现为空）
node scripts/check-split-equivalence.mjs         # 第五道（v2.161 批 F-11 立）：目录式模块形状——mod 声明↔文件闭合 /
                                                 #   测试 mod.rs 不混住用例 / 私有项不被兄弟簇引用 / 测试簇名不撞被测子模块 /
                                                 #   子簇引用父模块私有项必须有 `use super::`（v2.164 批 F-14 加）/
                                                 #   glob 传导链不断裂：簇用到的名字须有出处（本簇 use·父定义·父 use 经 glob）
                                                 #   （v2.165 批 F-15 加；「父模块类型面经 mod.rs 的 glob 传导」一档随 v2.166 批 F-16 补）。
                                                 #   测试目录判据 = 父声明带 #[cfg(test)]（现管 10 个目录）
node scripts/check-split-equivalence.mjs --dbg-tests   # 测试目录识别口径漂移时用它打印判定过程
node scripts/check-split-equivalence.mjs --split --ledger scripts/split-ledger/<name>.json   # 从历史原文重放某一刀的等价证明
```

Git hooks 在 `.githooks/`（随仓库提交），pre-commit 按暂存面触发上面五道结构闸；重新克隆后需重新执行一次 `git config core.hooksPath .githooks`。CI = `.github/workflows/ci.yml` 三作业（kernel / desktop(windows-latest) / hygiene），批 B 立：此前桌面端与前端在 CI 上完全不存在。

**验收叙事**（前置 `bash scripts/build_plugins.sh`）：

```bash
cargo run -p kernel-bin -- demo    # 装载 tool-echo 0.1.0 → 激活 0.2.0 → 回退 0.1.0（切 active 指针），全程不重启
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
8. 多版本并存：回退 = 切换 `active` 指针；更新失败自动回滚。⚠ **"旧版本永远保留"成立于注册表内（load 进来的实例常驻）与安装态磁盘（installer 复制的是当时字节），不成立于开发态暂存面**：`build_plugins.sh` 永远用当前 HEAD 重编，多 stage 一个版本只会造出"版本串不同、字节相同"的**假历史目录**并把 UI「回退」入口指到行为如新的产物（v2.162 F15-A 裁定：开发态每 id 只 stage 当前版本，双版本仅 `tool-echo` 这类**机制夹具**——其可观察差异来自插件自身的运行时版本闸门，脚本里有预校验把守）

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