# 11-单阈值压缩与记忆线整体退役

- 状态：进行中（2026-10-20 决策人指令）
- 负责人：agent
- 日期：2026-10-20
- 出处 / 上游：`specs/01-memory-context-redesign` 的 S4（本 spec **取代并扩展**它的退役面）+
  `specs/09-context-compaction-s2` 与 `specs/10-context-injection-s3` 后来新增的设置/参数面

## 目标

一句话：**压缩只留一条链，记忆整条线删干净，设置面不再让人填数值猜。**

1. 压缩：用户只设一根阈值滑条（100K–800K，每 100K 一档）+ 一个自动压缩开关。
   占用到线 ⇒ 在下一步开始前**同步**压缩（模型停在这一步等）⇒ 摘要与近期原文回填进上下文
   ⇒ 模型继续。没有微压缩、滑窗保护网、整理泄压、组织章节这些中间层。
2. 记忆：左栏入口、MemeryPage 及全部面板、上下文浮层里的记忆行、设置「记忆」分区、
   开发者分区的记忆/整理项、后端记忆与组织模块、记忆树 crate、memory-sqlite 插件全部删除。
   不保留"冻结只读"入口，不保留模型侧记忆工具。
3. 设置数值：普通用户可见的数值输入归零（阈值滑条是唯一例外）；能确定的值写死进代码；
   不能确定的收进「开发者」分区（版本号连点 7 次解锁）；其余改成开关或三档预置选择。

## 验收标准（必须可测）

- [ ] AC-1 单阈值自动压缩：滑条 100K、构造占用 ≥ 100K 的会话跑一轮 ⇒ 日志出现一条
      `ContextCompacted`（`from_tokens ≥ threshold`），时间戳晚于触发轮的 `LlmUsage`、
      早于下一步的 `LlmUsage`；压缩调用期间没有并发 step（同步等待）。
      用例名：`auto_compaction_runs_once_at_the_line_and_the_turn_continues`。
- [ ] AC-2 回填：压缩后的 `derive_messages()` 首条是 `summarized_message`（七节摘要），
      其后是未压的近期原文；下一 step 发给模型的消息以这条摘要开头。撤销 = 删掉那条
      `ContextCompacted` 后逐字节回到全量历史（既有 `compaction_rewind_restores_full_history`
      仍绿）。
- [ ] AC-3 只有一层：`microcompact`、`slide`、`organize::release_pressure` 在代码面不存在
      （判据点名零命中）；`Compactor::run` 是唯一的自动压缩入口；`compactEnabled` 关掉后
      不再自动压，手动 `/压缩` 仍可用。**✅ 刀① 已达成**（2026-10-20：生产面三名词零命中，
      刀① 提交正文带命令与读数）。
- [ ] AC-4 设置面：普通用户页「上下文」只剩三行——阈值滑条 / 自动压缩开关 / 用量条开关；
      全设置页不再有 `type="number"`（开发者分区除外）；「记忆」分区不存在；
      开发者分区不再有记忆树整理、reconcile、章节、重绑、超时重试等记忆项。
      用例：`settings_surface_has_no_user_facing_number_inputs`（扫描非开发者分区模板）。
- [ ] AC-5 记忆 UI 清空：左栏页脚只有设置/定时/通知；路由里没有 `memory` destination；
      命令面板无记忆项；聊天转录无"查看记忆"跳转；用量浮层无记忆树行、无 `memory_expand`
      计数行、无 S3 扣减链三行；中英文案键无残留（`check-i18n` 两语言相等）。
      真窗口：打开应用 → 左栏底部四个图标里没有数据库图标；设置页左侧没有「记忆」。
- [ ] AC-6 后端/插件删除：`memory*` 模块、`organize/`、`recall_log`、`recall_receipt`、
      `inject_view`、`commands/memory.rs`、`commands/organize.rs`、`crates/memory-tree`、
      `plugins-src/memory-sqlite`、kernel 的 `memory_inject`/`tree_context` 全部不存在；
      插件注册数 = 8；Tauri 命令三向闭合（定义=注册=前端引用）；事件通道对账
      `check-channels` 绿（`lw://organize-done/paused` 已摘）。
- [ ] AC-7 指令文件保留：`AGENTS.md` / `CLAUDE.md` 等指令文件仍注入系统提示（用例既有），
      预算写死为 100 KiB；它们不依赖已删除的记忆库模块。
- [ ] AC-8 数值归类表全部落地：见「数值项归类」；`make check` 全绿（宿主与前端条数以实际
      为准，删掉的用例同步删，不保留空转）；`make selftest` 每个新/改判据都有变异报红；
      `make gates` 接线数如实（删除后应无"未接线"）。
- [ ] AC-9 真窗口一轮：把阈值调到 100K 起，发一段长会话到线 ⇒ 界面出现压缩反馈
      （压缩中/已压缩之一），之后模型继续回答同一个问题；上下文浮层读数在压缩后回落。
      这条由我截图核对后交决策人复看。

## 非目标

- **不删磁盘数据**：`~/.Locowrk/memory/` 与 `usage.sqlite3` 里的 recall 表保留原地；
  清目录需单独批准。
- **不动 `crates/plugin-api` 的冻结 ABI**：`memlayout` 没有消费者也原样保留
  （删它 = 破坏既有签名，属禁区）。插件 `memory-sqlite` 与核心 crate 可以删。
- **不改 `docs/design/authority/DESIGN.md` 的规范语义**：记忆退役与设计文档冲突，
  该文档属禁区，需决策人另行批准后由 S5 重写；本线只在 `specs/`、`STRUCTURE.md`、
  roadmap 里记账。
- 不做记忆数据迁移/兼容/导出。
- 不新增压缩层、不新增用户可调数值；不为已有测试保留死代码。
- 不顺手改与本线无关的界面（外观/聊天/工具分区只动数值控件形态）。

## 设计要点

**压缩**：保留主层深度压缩（`compact/`：判据 + 七节摘要 + `ContextCompacted` 投影），
删掉它上下的所有机械层。有效阈值 = `min(滑条, 窗口 − 输出预留)`；输出预留写死 21K，
目标比写死 50%，近期原文写死 15K。阈值仍走 `context_budget` 一处算式。
`organize` 的泄压调用从 runner 摘除（模块随记忆线删除），压缩成为 step 前唯一动作。
压缩失败保留熔断（连续 3 次停手）与手动 `/压缩`；失败不静默——界面/日志要能读出。

**删除顺序**：按依赖单向、每刀都能 `make check` 全绿：
先删前端调用与设置（同时注销 Tauri 命令），再删后端命令与模块，最后删 crate/插件与内核注入链。
中间不留"定义在、引用无"的状态（命令三向闸会拦）。

**数值项归类**（决策人 2026-10-20 原话：能确定写死；不能确定进开发者；用户视角只留开/关或预置）：

| 项 | 现状 | 处理 |
| --- | --- | --- |
| 压缩阈值 | 滑条 100K–800K | **保留**（唯一用户数值，档位式） |
| 自动压缩 | 深度压缩开关 | **保留开关**（改名「自动压缩」，缺省开） |
| 用量条显隐 | 开关 | **保留开关** |
| 输出预留上限 | 数值 21000 | 写死 21_000 |
| 压缩目标比 | 数值 50 | 写死 50 |
| 近期原文额度 | 数值 15000 | 写死 15_000 |
| 工具结果预算 | 数值 24 KiB | 写死 24 KiB |
| 微压缩开关/保留组数 | 开关+数值 | 随微压缩删除 |
| 滑窗保留段数 | 数值 | 随滑窗删除 |
| 记忆分区全部 14 项 | 开关/数值/下拉 | 全部删除 |
| 开发者记忆树 10 项 | 数值 | 全部删除 |
| 指令文件预算 | 数值 100/100 KiB | 写死 100 KiB |
| 字号 | 数值 12–18 | 三档预置：小/中/大 |
| 对话列宽 | 数值/留空 | 三档预置：窄/标准/宽 |
| 折叠行数 | 数值 | 写死 5 |
| 工具：每次调用数/超时/轮步数/审批超时/问询超时 | 数值 | 移入开发者（值不变） |
| 模型上下文窗口/输出上限 | 数值 | 移入开发者（值不变，缺省自动） |
| 插件自动禁用天数 | 数值 | 移入开发者 |
| 缓存命中告警百分比等既有开发者数值 | 数值 | 原样留在开发者 |

## 被否决的方案

- **只删 UI 保留后端记忆工具**：留下"界面看不见但模型还在写记忆"的第二真源，
  与"删干净"直接冲突。
- **保留记忆树冻结只读**：仍是入口、仍是后端模块、仍要维护，复杂度没有下降。
- **保留微压缩/滑窗当内部兜底**：用户明确只要一条压缩链；保护网可由主层失败后的
  熔断 + 可见错误承担，不再藏一层"有时会动、有时不动"的机械压缩。
- **把阈值也写死**：用户原话保留"压缩阈值的滑条"，它是唯一的用户控制点。

## 计划

1. 刀① 压缩收口：删 `microcompact` / `slide` / runner 的 organize 泄压调用；上下文设置
   收敛成滑条+开关；写死常量；弹层去掉扣减链三行；判据同步删/改。
2. 刀② 记忆前端与设置：删左栏入口、`MemoryPage` 与 `memory/` 全部组件、store/API/types、
   浮层记忆行、设置「记忆」分区、开发者记忆项、中英文案；同时删 `commands/memory.rs`、
   `commands/organize.rs` 与注册（命令三向闸保持绿）。
3. 刀③ 记忆后端与插件：删 `memory*.rs`/`memory_*`、`organize/`、`recall_*`、`inject_view`、
   kernel `memory_inject`/`tree_context`、`crates/memory-tree`、`plugins-src/memory-sqlite`；
   迁移 `AnchorSource` 与摘要客户端（压缩改用主模型，不再有记忆管理器模型设置）；
   插件数改 8；事件/通道与接线闸同步。
4. 刀④ 数值归类：字号/列宽预置，工具/模型/插件数值进开发者，其余写死；扫描确认普通
   分区零数字输入。
5. 刀⑤ 收尾：判据与变异、文档（specs/01 S4 指针、specs/09/10 被取代处、STRUCTURE、
   roadmap）、真窗口截图核对、`make check`。

## 任务

- [x] 刀① 压缩收口（2026-10-20：微压缩 / 滑窗 / 轮内泄压三层连同测试删除，上下文页三格，
      四值写死；`make check` 全绿）
- [ ] 刀② 记忆前端与设置 + 命令注销
- [ ] 刀③ 记忆后端、crate、插件、注入链
- [ ] 刀④ 全设置面数值归类
- [ ] 刀⑤ 判据/文档/真窗口收口
- [ ] `make check` 全绿

## 影响范围

| 文件 / 目录 | 改动 |
| --- | --- |
| `desktop/src-tauri/src/microcompact.rs` `slide.rs` | 整文件删除（含测试与判据消费） |
| `runner/mod.rs` `state/waterfall.rs` `state/mod.rs` `state/background.rs` | 摘 organizer/泄压/micro/slide 接线；重排注入链 |
| `desktop/src-tauri/src/settings/{context,memory,developer,view}.rs` `settings.rs` | 删记忆整区与上下文数值；保留阈值/开关 |
| `desktop/src/components/shell/Sidebar.vue` `App.vue` `ChatTranscript.vue` `stores/shell.ts` | 删记忆入口/路由/跳转 |
| `desktop/src/components/MemoryPage.vue` `components/memory/**` `MemoryTreePanel.vue` `MemorySection.vue` | 整目录/整文件删除 |
| `desktop/src/stores/tree-board.ts` `lib/board-model.ts` `types/memory.ts` | 删除 |
| `desktop/src/lib/api/{memory,tree}.ts` `lib/{memory-type,tree-labels}.ts` | 删除 |
| `desktop/src/components/chat/ContextUsagePopover.vue` | 删记忆树行、展开行、扣减链三行 |
| `desktop/src-tauri/src/{memory*.rs,memory/**,organize/**,recall_log/**,recall_receipt.rs,inject_view.rs}` | 删除 |
| `desktop/src-tauri/src/commands/{memory,organize}.rs` + `lib.rs` 注册 | 删除 |
| `crates/memory-tree/` `plugins-src/memory-sqlite` + `scripts/build_plugins.sh` 清单 | 删除/改插件数 |
| `crates/kernel-agent/src/middleware/memory_inject/**` `tree_context.rs` `kernel.rs` | 删注入中间件与树上下文源 |
| `compaction`：`compact/deep.rs` `llm/aux_client.rs` `context_budget.rs` | 摘要走主模型；`AnchorSource` 本地化；目标/保留/预留写死 |
| `desktop/tests/**` `scripts/check-single-source.mjs` | 记忆/压缩用例与判据同步删改 |

## 风险与回滚

- 风险①：压缩失败后原上下文照发，可能超窗被拒。保留连续失败熔断与手动 `/压缩`，
  界面与日志如实报"压缩失败"；不静默加回机械层。
- 风险②：删除面大（前端页面、后端模块、crate、插件、注入中间件）。每刀独立提交、
  可单独回退；数据目录不动；`plugin-api` 冻结 ABI 不动。
- 风险③：DESIGN.md 与实现冲突。禁区，本线不改；待决策人批准后 S5 重写。
- 风险④：旧测试/判据按新形状失效。删测试必须同刀删判据并在提交正文写明，不允许放宽。
- 挂账：磁盘记忆数据清理、`usage.sqlite3` 残留表、`plugin-api` memlayout 无消费者，
  登记在 specs/03。
- 回滚：逐刀 revert；数据侧无需回滚（未动）。
