# 09-上下文压缩面 S2（把"超限请重开会话"换成有界可续的压缩管线）

- 状态：**已确认 · 刀①②③ 已收 · 刀④ 待做**——本文件是 [`specs/01`](../01-memory-context-redesign/spec.md)
  阶段表里 **S2 压缩面** 那一行的施工 spec（方案稿对应 = B 节，⊘ 不入库，关键条款抄在下面
  「取证」与「设计要点」两节里；决策依据 = [`ADR-0007`](../../docs/adr/0007-memory-flat-files-context-llm-compaction-primary.md)）。
  三处待定项已由用户 **2026-10-19「开工吧」** 一并采纳（见下面「要你定的」那节的裁定结果）
- 负责人：Qoder（取证 + 实现） / 青鸟（三处待定项点头）
- 日期：2026-10-19（现状取证同日）
- 出处 / 上游：方案稿 §B「上下文侧：压缩管线（LLM 深度压缩主位，机械层前置减速 + 护航）」
  + [`specs/01`](../01-memory-context-redesign/spec.md) 的 AC-4…AC-13（本线吃 **AC-8 / AC-11 / AC-13** 三条）

## 目标

让"一个会话窗口里连续工作不重开会话"成立：占用到线时**由程序压缩**，而不是把一句
"上下文超出模型窗口：请新开会话继续"丢给用户。形状 = 主层 LLM 深度压缩（阈值到线即总结、
压到 50% 留缓冲）+ 三层机械前置（落盘 / 微压缩 / 滑窗保护网），**会话日志永不被改写**——
压缩是组装期的投影，撤销 = 换投影参数重放。

## 取证（2026-10-19 读实，逐条带出处；这一节决定下面每一刀的形状）

1. **压缩本体不在了，但它的三件活物还在**。`CtxCompact` 连同 `CompactConfig` /
   `CompactStrategy` / 稳定期账本 / 切点与摘要链已随**审计修复批 C 删除**（删除前 = 306 行本体
   + 509 行测试；它被删的理由是审计报告 H2「存在但从未接线」——构造点全在测试里，注释却写着
   "headless 侧仍注册"，见 `crates/kernel-agent/src/middleware/compact.rs:1-21`）。现在还在服务的：
   `CompactionOutcome`（`compact.rs:30`，是 `RequestPayload::compacted` 的载荷类型）、
   `estimate_tokens`（`compact.rs:42`，全仓唯一粗估尺）、以及 `SessionEvent::ContextCompacted`
   的**重放/折叠链**（`session-store/src/event.rs:211` → `session.rs:134` 折叠 →
   `session/project.rs:110` → 桌面 `commands/chat.rs:135` 找最近压缩点）。
   ⇒ **S2 不是"从历史把死码接回来"**：历史那套的失败正是"写了没人接"。这一刀必须带着
   生产写入点与判据一起落地，否则同一条审计结论会再判一次。
2. **接入点是现成的、契约不用改**：`agent/request` 决策环的
   `RequestPayload { messages, model, params, tools, compacted, system, session_id, tree_directory }`
   （`crates/kernel-agent/src/waterfall.rs:49-65`）里 `compacted` 明写是"留给任何
   `agent/request` 决策方的可扩展契约"，`kernel/request.rs:64-107` 出环时把它带回给 kernel 落事件。
   ⚠ 另一条结构事实要利用：`system` 在 `messages` 之外 ⇒「系统提示永不压缩」由结构保证
   （`waterfall.rs:38-41`），S2 不许把摘要塞进 system。
3. **现状的"上下文节奏"其实由整理（organize）承担**：内核组装侧把 `messages` 切到"未整理尾巴"
   （`waterfall.rs:43-47` 那段 `tree_directory` 说明），占用锚点住在
   `desktop/src-tauri/src/organize/mod.rs:134 anchor()`——优先 provider 真实 input
   （`runner::reported_input_tokens`，v2.138 统一锚点"进度条 = 触发 = 自动收起一把尺"），
   无上报才回落 `estimate_tokens`；自动收起在 `organize/collapse.rs:32-46`。
   ⇒ **锚点这块地长在 S4 要退役的模块里**：S2 不动它，S4 一拔 organize，触发判据就没有出处了。
   搬家因此是 S2 的第一刀（不是重构顺手做，是依赖方向要求）。
4. **超限现在的真实行为 = 一句指向不存在出路的报错**：`llm/retry.rs:53`
   原话"上下文超出模型窗口：请新开会话继续，或把 设置 → 上下文 的整理阈值调低
   （让记忆树更早接管历史），并减少单次工具输出"。这句同时是 AC-11-② 要消灭的状态
   与 S4 的阻塞项——organize 一退役，它指向的东西就没了。
   ⇒ **S2 不做，S4 不能做**（specs/01 表里 S4 依赖 S1+S2+S3 稳定一轮，理由就是这一条）。
5. **设置面几乎是空的**：`settings/context.rs:58-72` 只有 `window_tokens`（旧字段，
   `settings.rs:150-177` 已把它迁进 llm 档位）/ `threshold_tokens` / `usage_bar` /
   `tool_result_budget_kib` 四枚，而 `threshold_tokens` 现在**只被整理用**
   （`commands/chat.rs:56 organize_threshold_tokens()`）。方案稿要的压缩总开关、深度压缩开关、
   spill 阈值、微压缩保留条数与比例、目标比、压缩模型、自定义压缩指令**一枚都不存在**。
   ⇒ 语义变更要写清：`context.thresholdTokens` 从"整理阈值"改判成"压缩全局阈值"，
   设置页那句文案（中英成对）跟着改，不许留一句指向空设置项的话（`retry.rs:49` 已经在抱怨这个）。
6. **"生效窗口"其实有两条链，而且都还没处理"取不到"**（草案初稿只看到第二条，
   开工当日读全）：① 上下文节奏这条 = `settings::session_window`
   （`settings/llm/persist.rs:131`，四层：会话覆盖 → 模型行 `context_window` →
   models.dev 目录命中 → 档位 `window_tokens`），整理触发 / 自动收起 / 两个组装点的
   进度条分母都吃它；② 发送侧这条 = `llm/app_llm.rs:240-243` 的 `RowFacts`
   （两级：模型行 ?? 档位，**故意不查目录**——注释原话"不发明能力：目录里没有的、
   档位里没钉的一律不发"，那是往请求里写参数的纪律）。两条并存是既成事实，本刀
   不合并（合并 = 改发送行为，与刀① 的搬家性质无关，登记在下面「挂账」）。
   方案稿支撑层实证③要求"拿不到窗口时走哪边要写死一个"——改前就是"没决定"的状态；
   裁定 A 已落在 `context_budget::effective_threshold` 的 `None` 分支。
7. **刀① 新增读到的两把"回落尺"**（登记，不在本刀统一）：有供应商上报时进度条与
   锚点确实是同一个数（v2.138 那句「看到的 = 触发用的」证的是这一档）；**无上报**时
   `runner::record_usage`（`runner/mod.rs:358-365`）回落**树组装后口径**（目录 + 尾巴），
   而 `anchor()` 回落**全量历史字符估算**。统一它会改整理的触发读数 = 改行为，
   归刀④（主层触发线接入时一起定，判据形态 = 那一刀的"到线就跑"用例必须说明白的
   到底是哪一个数）。

## 验收标准（必须可测：跑什么命令、看到什么结果）

- [x] AC-S2-1 锚点搬家不改读数（✅ 2026-10-19 刀①）：同一份会话日志上两档来源
      （LlmUsage / Estimate）与搬前同值——证据不是"我抄了一遍原代码"，而是
      **organize 既有测试一字未改仍绿**（619 passed / 0 failed，其中 `organize::tests::pressure`
      那批的 anchor = 9,500 断言就是这一档的行为面）。判据表述订正：草案写的
      "`grep -c reported_input_tokens` 只剩一个出处"太宽——那个名字在生产面上本来就有
      两个合法位置（尺的定义在 `runner/mod.rs:336`、消费者是锚点与进度条），数它等于
      把"共用一把尺"误报成"两个出处"。实际落的三条形状档是 `SS16a`（`fn anchor(` 全生产 1 处
      且在中立面）/ `SS16b`（organize 面上 `reported_input_tokens` 与 `fn anchor(` 各恒 0）/
      `SS16c`（预留两常量 + 算式各 1 处），三条互不遮挡由 `--selftest` 实测（三条变异都没有
      "同时打红"别项）。另跑了一发**磁盘变异**（复跑配方 = 把 `Organizer::anchor` 那段改前原文
      抄回 `organize/mod.rs`、`collapse.rs` 那处调用改回 `Self::anchor`，然后分别跑闸与 cargo；
      脚本住 `tmp/` 不入库，所以证据是那三个读数而不是那个文件）：抄回后能编译、行为等价
      ⇒ `cargo test --lib` 仍 619 全绿，而 SS16a 读 2|1、SS16b 读 1|1 双双报红、SS16c 不动，
      还原后复绿——这一发证的才是"形状档独有、行为面看不见"
- [x] AC-S2-2 有效阈值只扣一次（✅ 2026-10-19 刀① 的算式面）：`context_budget::effective_threshold`
      是全仓唯一出处（SS16c），夹具 150K / 179K + 两个负对照 158K / 136K 由
      `context_budget/tests.rs::effective_threshold_is_capped_by_the_window_once` 钉住，
      预留三档（None→21K / 64K→21K / 8_192→8_192）由 `reserve_takes_the_smaller_of_ceiling_and_cap`
      钉住；裁定 A 那一档 = `unknown_window_keeps_the_slider`。
      ⚠ 界面那一格不在这里报"已验证"：现网设置下封顶恒不生效（是算出来的，见任务节
      那条「刀① 的现网可见面」），能测出差别的构造配方也写在那里，目检随刀⑥ 一起做。
      ⚠ **草案数字订正**（同一日，开工当天自己读到 ZCode 原文才发现）：本节初稿把
      封顶那档写成"157K 而非 136K"——两个数都是从回忆里的算式倒推的，按你给的本地源码
      `ZCode/apps/zcode-cli/packages/core/src/compact/policy.ts:67-88` 逐行重核后才订正：
      预留 = `min(maxOutput ?? 32_000, 21_000)`（`policy.ts:75-82`），200K − 21K = 179K；
      ZCode 自己在 `policy.ts:84-88` 还再减 13K 的 `AUTOCOMPACT_BUFFER_TOKENS`，
      而我们的滑杆不承担那颗缓冲（方案稿 §B 的"只扣一次"就是这一条），所以是 179K
      而不是 166K。**判据数字必须来自重读原文，不来自回忆**——这一条本来写在
      「两家参照源码逐行核对」的纪律里，初稿仍犯了，记下来是为了让下一刀别再靠回忆
- [x] AC-S2-3 落盘（spill）（✅ 2026-10-19 刀②，盘上一半）：一条超阈值的工具输出 ⇒
      spill 目录多一个文件 + 上下文里是 `<persisted-output> 路径 + 预览 </persisted-output>` +
      日志原文**没被改写**（`llm/tests/spill.rs` 两条各钉一半：落盘文件 = 全文、
      投影后 `req.messages` 里仍是全文——发送侧结构上碰不到日志，这条断言钉的是
      "别哪天顺手把投影写回日志"）。⚠ 三条里"日志 sha 对比"的实跑形态留给刀⑦
      的长会话北极星实测（AC-S2-9 那一轮里顺带取 sha 前后值），本刀只有结构证据
- [x] AC-S2-4 微压缩边界单调（AC-8-③）（✅ 2026-10-19 刀③）：**不回收**与**重放稳定**两条
      都有两级证据——纯函数级 `the_boundary_only_moves_forward`（第一轮清 c0..c3，把输出
      再喂一次 ⇒ `nothing-to-clear`，且同一输入重放两次给同一批），与**环级**
      `the_ring_rewrites_the_payload_and_books_the_outcome`（注册到 `agent/request` 决策环
      上跑到终态：改写落在发出去的载荷上、账写进 `compacted` 带回位、同一消息列再过一遍
      这一环 ⇒ 字节完全相同且不再记空账）。单调性的机制不是额外的状态表而是
      **内容自证**（正文等于 `CLEARED` 就不进候选），所以已清的占据不了"最近 5 组"的名额。
      记账 = `SessionEvent::ContextCompacted{strategy:"micro"}`，写回位复用
      `kernel.rs:253` 那条既有通路（内核零改动）。⚠ 一格**留给刀⑦**：本条没在真窗口里
      断言"会话日志磁盘上多了一行 `strategy:"micro"`"——环级用例证到的是带回位被写入，
      落盘那一跳由内核既有代码承担；北极星长会话那轮本来要读整轮日志，顺带取这个数
- [x] AC-S2-5 微压缩的三条"容易漏"都在实现里（✅ 2026-10-19 刀③）：只清白名单**可再生且可枚举**的工具——
      ZCode 那九枚（`Read/Bash/Grep/Glob/WebFetch/WebSearch/Edit/Write/ApplyPatch`，
      `microcompact.ts:21-31`）在我方的真实对应名是**九枚但不同名**：
      `fs_read` / `fs_list` / `fs_grep` / `shell` / `shell_read` / `fetch` /
      `git_status` / `git_diff` / `git_log`（2026-10-19 按注册面取证实名：
      Glob→`fs_list`，WebFetch/WebSearch 我方只有 `fetch` 一枚，Write/Edit 那一族**不放行**
      ——我方写侧要 `expected_revision`，清掉回执会让模型丢掉它刚拿到的版本号，
      比 ZCode 那种"重写一次"的代价更高；`mcp_*` 名字动态拼成、不可枚举 ⇒ 缺省不清）。
      **错误结果缺省不清**（`error_results_survive_by_default`：既断言失败输出不在 cleared，
      也断言它**不占名额**——清的是 c1..c3 而不是 c0..c2）、
      idle 60 分钟是第二条触发通路（`idle_is_a_second_trigger_path` 用 token 明确够不到的
      夹具，60 分钟整 vs 60 分零 1 秒两侧各一条；另有冷启那一格
      `cold_start_seeds_idle_from_the_session_log_mtime`：日志 mtime 拨到 61 分钟前 ⇒ 触发，
      新会话无日志 ⇒ 一条都不清），
      媒体结果不清（我方 `screenshot` / `read_image` / `pdf_to_images` 三个名字本身就不在白名单里，
      另加一条按结果字段形状 `asset`/`assets` 的兜底判据——投影侧已经有这条形状，
      见 `llm/wire/media.rs:44`；用例 `media_bearing_results_are_protected` 把带 asset 的那条
      顶到"够旧"的位置再验它不动）
- [ ] AC-S2-6 主层深度压缩（AC-13 全四条）：构造机械层压不到目标的会话 → 主层触发 →
      ①摘要落断层且含用户意图 / 待办 / 下一步（对照七节模板判据，逐节非空或如实标"无"）
      ②被摘要段出上下文、日志原文 sha 不变 ③近期原文仍在 ④连续失败 3 次熔断生效不再重试；
      另两档：overflow 应急按 SHRINK_STEPS `[0.7, 0.5, 0.35]` 递减重试直至成功、
      含媒体失败降级去媒体重试
- [ ] AC-S2-7 逐字保留两套条款**没有被合成一句话**：安全约束 / 用户消息 / 下一步原文引用
      （ZCode 面）与路径 / 符号 / 命令 / 错误串 / URL / 标识符（OpenCode 面）在模板里
      分节写、各有自己的用例；判据 = 把任一套并成一句时测试报红
- [ ] AC-S2-8 滑窗保护网：只在主层熔断打开后可用，且**丢过程留提问**——
      每轮 `UserMessage` 保留、assistant 往返与工具对丢弃；用例断言"提问一条不少、
      过程一条不留"，并断言熔断未开时这一层零动作
- [ ] AC-S2-9 北极星（AC-11 三条）：模拟重负载长会话（≥40 轮、含大工具输出）连续跑 →
      ①每轮真实 input ≤ 阈值 ×1.1（压缩执行轮瞬时除外）②全程不出现"必须重开会话 /
      手动压缩才能继续"的状态（`llm/retry.rs:53` 那句在整轮日志里零命中）
      ③后半程每轮 input 均值 ≤ 前半程 ×1.2
- [ ] AC-S2-10 设置面全数落地且缺省 = 现状：压缩总开关（关 = 完全不压、接近窗口只提醒）、
      深度压缩开关（缺省开）、spill 阈值字符数、微压缩保留条数与比例、目标比（50%）、
      压缩模型、自定义压缩指令——**每一枚的缺省值必须等于本刀之前那颗硬编码常量**，
      等价性用"既有测试一字未改仍绿"实证；文案中英成对（`check-i18n` 读数两侧相等）
- [ ] AC-S2-11 `/整理` 改语义 = 当场跑主层（三参照同位按钮），跑完界面上能看到
      "压掉了多少 / 摘要落在哪"，且它**不**再触发 organize 那套章节整理
- [ ] AC-S2-12 `make check` 全绿；新闸一律带变异红证（`tmp/mut-rust.mjs` 续 CASE 号）

## 非目标

- 不动 organize / 记忆树 / 树工具族一行（那是 S4 退役面）；本线只把**锚点与阈值**这两块地
  从 organize 里搬出来，搬完 organize 继续用同一份（不许出现两份算尺）
- 不做按模型覆盖阈值（方案稿明写"留作后续"，落点 = 模型管理页，入口对齐 Qoder 路径）
- 不改 `SessionEvent` 既有形状：`ContextCompacted` 只新增 `strategy` 的取值
  （`spill|micro|compact|slide`），字段一个不加（`plugin-api` 冻结 ABI 同一条纪律）
- 不在 `MemoryPage` 那页加任何形态——它两个 tab 已按方案稿 §E 定为整体退役删除（归 S4），
  压缩的界面读数一律走开发者面板/诊断，不给自己在本页雕花的理由
- 不做摘要质量评测集（形状侧风险①在方案稿末尾，第一版只用上面那些可断言的判据）
- 不预造"手动压缩"以外的第二套入口（按钮 + `/整理` 两条已够，第三条 = 过度设计）

## 设计要点

一句话：**机械层推迟并缩小 LLM 压缩，LLM 压缩完成真正的上下文重置**——两层不是"主 vs 兜底"。
被否决过的形状与理由 = `docs/adr/0007` 的「被否决的方案」表 + 方案稿同名节，本文件只给指针。
两处**不许合成一句话**的取舍已在 AC-S2-7 钉住（逐字保留两套对象不同）；触发线两式取小
（`min(主层阈值 × 0.9, 主层阈值 − 2000)`）与微压缩常量（保留 5 条 / 最小节省 256 token /
idle 60 分钟）照 ZCode `microcompact.ts:16-20`/`:79-83` **逐字**，不做"我们觉得更合理"的改动。

## 被否决的方案

- **照 git 历史把 `CtxCompact` 接回来**：否——它被删的理由是"存在但从未接线"（审计 H2），
  接回来等于把死码再养一遍；S2 要的是同一件事的新形状（有生产写入点 + 有判据）。
- **继续让记忆树整理承担上下文节奏**：否——整理是"把历史蒸馏成树章节"，与"这一轮请求发得出去"
  不是同一件事，而且 organize 在 S4 整面退役；把北极星押在一个要删的模块上 = 依赖方向错。
- **压缩时改写会话日志**（Qoder CN 的"改写 + 决策缓存"形态）：否——我们有 append-only 日志
  这个它没有的底座，投影式压缩免费给撤销与重放（方案稿 §B 原则段）。
- **留空口径用 OpenCode 的比例带下限**（`RESERVE_MIN=16_000` + 保 10% 空闲）：否——
  滑杆本来就是绝对 token 语义，掺比例会让人看不懂自己设的数扣了多少（方案稿实证④原话）。

## 计划（刀序草案，每刀独立可回退、一提交一刀）

1. **刀① 锚点与阈值搬家**（✅ 2026-10-19）：中立模块 `desktop/src-tauri/src/context_budget.rs`
   收 `anchor()`（从 `Organizer::anchor` 整块搬，数值与回落表达式逐字未动）+
   `output_reserve()` + `effective_threshold()`（含裁定 A 那一档）。四个真实消费点：
   `organize/{collapse,pressure,trigger,debug}` 改吃中立锚点，**两个组装点**
   （`commands/chat.rs` / `scheduler/engine.rs`）与 **organize 的批量档触发线**
   （`Organizer::budget_threshold`）改吃有效阈值——分母与触发线不许一个封顶一个不封顶
   （那就是 v2.138 修过的分裂形状）。取数面加 `settings::session_capacity`
   （窗口 + 输出上限一次解析，`session_window` 退成薄封装）。判据 = `SS16a/b/c` +
   `context_budget/tests.rs` 六条 + 变异红证。**不在本刀预造** `armed()`（裁定 A 第二半
   归刀④），不统一无上报回落尺（取证⑦）。
2. **刀② spill（落盘）**（✅ 2026-10-19）：新 `desktop/src-tauri/src/spill.rs`——
   `<数据根>/spill/<会话>/<sha256>.txt`（内容寻址 ⇒ 同内容同路径 ⇒ 占位文本逐字节相同 =
   M1 前缀稳定判据在这一刀免费拿到），会话 id 先过 `plugin_api::naming::valid_session_id`
   （那把唯一尺），组装点只交数据根、目录名由 `SPILL_DIR` 一枚常量决定。
   **发送侧是一处判定、两种后果**：`clip_tool_output` 升成 `project_tool_output`
   ——落得下就发 `<persisted-output path=… total_bytes=… preview_bytes=…>` 包裹的预览 +
   一句如实措辞（`fs_read` 现在**没有** `offset`，所以那句写的是"读该路径、比预览更靠后的
   部分读不回来"，见下面挂账 ⓐ 与 `70f0cc6`），落不下（无句柄 / 无会话身份 / 写失败）就
   退回**改前逐字节相同**的那句截断提示并 warn。没有新增第二枚设置：阈值仍是
   「工具结果预算」（`toolResultBudgetKib`，0 = 不限制 = 既不截也不落）。
   取证抓到的一条：**不能落进 `assets/`**——那里的 GC 按"文件名的 sha 是否出现在会话
   日志文本里"判孤儿，而 spill 文件的 sha 不在日志里 ⇒ 天生孤儿，宽限期一过就被删，
   模型回头就读空（理由写在 `spill.rs` 头注，判据 SS17b 钉住目录名零手拼）。
   判据 = `spill/tests.rs` 7 条 + `llm/tests/spill.rs` 6 条 + `SS17a/b/c` 三条形状档
   （各自窄变异、selftest 无连带打红）；`tool_budget.rs` 那批**一字未改仍绿** = 降级
   通路字节等价的证据。
   ⚠ 两件如实挂着：① **spill 目录还没有清理通路**（按会话分子目录就是为了让"删会话
   顺手删它的 spill"成为可能，但那要动删除路径 = 禁区动作，单独点头那一刀做）；
   ② 真机跑一轮（发一条 >24 KiB 的工具输出，看 `spill/<会话>/` 出现文件、转录里工具
   内容变成 `<persisted-output …>`）没做——单测覆盖了两个分支与注入点调用，
   但端到端那一格只有实跑才算证，等桌面端重启时一起给你看。
3. **刀③ micro（微压缩）**（✅ 2026-10-19）：新 `desktop/src-tauri/src/microcompact.rs`
   （+ `microcompact/tests.rs` 16 条）——七颗参数逐字照 ZCode `microcompact.ts:12-31`
   （保留 5 组 / idle 60 分钟 / 最小节省 256 / 触发线 `min(0.9×主层, 主层−2000)` /
   错误结果缺省不清 / 省不够整批还原 `:151-156` / 幂等靠正文与占位全等 `:230`）。
   **两处与参照不同、各有理由**：白名单按我方真实名九枚（写侧回执与 `mcp_*` 不放，见
   AC-S2-5），主层阈值由**组装点经闭包供给**（`state/waterfall.rs` 里
   `session_capacity` + `context_budget::effective_threshold` 现场算，本模块面里那三个
   形状恒 0 = 判据 SS18b，与刀① "organize 不许自己拿尺"同族）。
   **idle 的时间源换了实现**：计划原写"runner 把轮末时间戳传给环"，那要动 `run_turn`
   的长签名与全部调用点；实际做成环内一张 `会话 → 上次活动` 的表，本进程没见过这个会话时
   用**会话日志的 mtime 播种**（轮末 append ⇒ mtime 天然等于最后收束时刻，一次 stat 就够），
   于是"隔天回来接着聊"那一格跨重启没有空窗。与 ZCode 的 `lastAssistantCompletedAtMs`
   差一轮时长，注释里说破、不写成同一个东西。
   **注册位**：`agent/request` 决策环第八环（末尾）。前缀缓存红线管的是 system 静态段序，
   这一环不往 system 写东西、只改若干条最旧工具结果的正文 ⇒ 可以在红线之后（理由写在
   `state/waterfall.rs` 头注，那条注释同时把"八环"的序补全）。
   判据 = 16 条用例（13 条纯函数 + 3 条**接线面**：改写落回载荷 / 开关关得掉 / 冷启播种）
   + `SS18a/b/c` 三条形状档（各配窄变异，selftest 无连带打红）+ 两轮磁盘变异红证：
   `tmp/mut-s18-rust.mjs` 七条（M1..M7 各红 1–3 条用例、还原逐字节相同）与
   `tmp/mut-s18b-ring.mjs` 四条（N1..N4 破的是环本体：不改写、不记账、开关不参与、不播种）。
   ⚠ 一件**故意没做**：界面上那枚开关保存即生效、没有按会话锁死——它是"要不要动"的门
   而不是数值，锁进会话反而违背可配置原则；对照之下思考语言那类**数值/文本**才需要锁字节。
4. **刀④ 主层 compact**：七节骨架模板 + 两套逐字条款 + 压到 50% + 近期原文保留 +
   熔断 3 + overflow `SHRINK_STEPS` + 媒体降级 + 压缩模型可配 + 自定义指令钩子；
   写入点挂在 `agent/request` 决策环（`compacted` 带回位）。
5. **刀⑤ slide（滑窗保护网）**：仅熔断打开后可用，丢过程留提问。
6. **刀⑥ 设置面与文案**：全部数值设置化（缺省 = 现状常量）+ `thresholdTokens` 语义改判 +
   中英成对 + 设置页那一条分区（按《Front-end Design Guidelines》现有形态，不新造）。
7. **刀⑦ `/整理` 改语义** + 北极星长会话实测（AC-S2-9 那三条跑一次并留读数）。
8. 每刀收尾按 [`CONTRIBUTING.md`](../../CONTRIBUTING.md)：进度只在 roadmap 那一行改勾选，
   实测读数与遗留风险写提交正文。

## 任务

- [x] 三处待定项已定（2026-10-19「开工吧」= 三条建议全采纳，见上面「要你定的」的裁定结果）
- [x] **刀① 锚点与有效阈值搬家 ✅**（2026-10-19）：AC-S2-1 / AC-S2-2 两条打勾，判据 SS16a/b/c +
      六条单测 + 一条同源用例 + 一发磁盘变异全绿（`cargo test --lib` 619 passed / 0 failed，
      `make check` 见提交正文）
- [x] **刀① 的现网可见面 = 零变化（2026-10-19 按真实 settings.json 算过，没跑窗口）**：
      读 `~/.Locowrk/settings.json` 实测——滑杆 `thresholdTokens = 100_000`（区间 100K–800K），
      活动档 `MiniMax-M3.1-Flash-Preview` 的 `contextWindow = 1_048_576`、`maxOutput = null`
      ⇒ 预留走缺省 21K、封顶线 = 1_027_576 ⇒ `min(100K, 1_027_576)` 恒等滑杆；
      同档另一行 `MiniMax-M2.7` 窗口 262,144 ⇒ 封顶线 241,144，也高于当前滑杆。
      **所以别拿缺省设置去窗口里看"没变化"就报验证通过**——那一格什么都证不出来。
      能测出差别的配方（留给刀⑥ 与设置页文案一起目检）：会话里选 `MiniMax-M2.7` 那一行 +
      把滑杆推到 300K ⇒ 期望看到进度条**分母 = 241,144**（而不是 300,000），
      百分比按它重算；`anchor_source` 与"窗口未知时按未设置呈现"那条现状行为不变
- [x] **刀② spill 落盘 ✅**（2026-10-19）：AC-S2-3 打勾（盘上一半），判据 SS17a/b/c +
      `spill/tests.rs` 7 条 + `llm/tests/spill.rs` 6 条 + `tool_budget.rs` 那批一字未改仍绿；
      真机端到端那一格按内文登记，等桌面端重启时一起给你看
- [x] **刀③ 微压缩 ✅**（2026-10-19）：AC-S2-4 / AC-S2-5 两条打勾，判据 SS18a/b/c +
      16 条用例（含三条接线面）+ 两轮磁盘变异（七条破判据 / 四条破环本体，全部窄红、
      还原逐字节相同）；设置面加了 `context.microEnabled`（缺省开）与开关一枚
- [ ] 刀④→刀⑦ 逐刀落地 + 补测试 → 按改动面选档跑门禁，全绿才算完成
- [ ] 数据侧改动前把相关目录复制进备份目录（specs/01 那条纪律在本线同样适用：
      压缩虽不改日志，但 spill 会往会话附件目录写文件）

## 要你定的（三处 · 2026-10-19 全部已定，采纳建议）

1. **窗口取不到时走哪边**（方案稿支撑层实证③要求写死一个）⇒ **裁定 A**：不触发自动
   压缩，且界面上如实说"窗口未知，压缩未启用"。落地节奏：阈值那一半已在刀① 落地
   （`context_budget::effective_threshold` 的 `None` 分支 = 不假定窗口、原样吃滑杆）；
   "因此不自动压"那一半**不在刀① 预造**——现网还没有自动压缩的执行者，先写一个
   `armed()` 判据就是取证①点名的"存在但从未接线"，它归刀④ 的主层触发线。
2. **`context.thresholdTokens` 的语义改判**：从"整理阈值"改成"压缩全局阈值"（整理继续
   吃同一枚直到 S4 退役）⇒ **定：改**。设置页文案与 `llm/retry.rs:53` 那句报错的指向
   跟着改（那两处归刀⑥，因为要和其余压缩设置一次成对，见刀序）。
3. **机械层压过之后主层是否仍到线就跑** ⇒ **定：照 ZCode，到线就跑**，不因机械层压过
   而推迟（推迟会把"更小的总结面"变成"更晚的重置"，北极星那条占用有界被拖长）。

## 影响范围

| 文件 / 目录 | 改动 |
| --- | --- |
| `desktop/src-tauri/src/context_budget.rs`（+ `context_budget/tests.rs`） | 刀① ✅：锚点 + 输出侧预留 + 有效阈值 + 裁定 A 那一档的唯一出处 |
| `desktop/src-tauri/src/organize/{mod,collapse,pressure,trigger,debug}.rs` | 刀① ✅：`Organizer::anchor` 删除、四处改吃中立锚点；批量档触发线改吃 `Organizer::budget_threshold`（观测面 `debug.rs` 同改，不留"界面一个数、决策另一个数"） |
| `desktop/src-tauri/src/settings/llm/persist.rs` `llm/mod.rs` `settings.rs` | 刀① ✅：新增 `session_capacity`（窗口 + 输出上限一次解析），`session_window` 退成薄封装；`settings/tests/window.rs` 加一条同源用例 |
| `desktop/src-tauri/src/commands/chat.rs` `scheduler/engine.rs` | 刀① ✅：两个组装点的 `UsagePolicy.threshold_tokens` 改吃有效阈值 |
| `scripts/check-single-source.mjs` | 刀① ✅：SS16a/b/c 三条 + 两条扫描面下限 + 三条窄变异；selftest 的分工读数（"同时打红谁"）也在这一刀加上 |
| `desktop/src-tauri/src/spill.rs`（+ `spill/tests.rs`） | 刀② ✅：落盘句柄（数据根 + 会话子目录 + 内容寻址文件名）与两句文本（`<persisted-output>` 模板 / 降级提示）的唯一出处 |
| `desktop/src-tauri/src/llm/wire/{payload,messages}.rs` | 刀② ✅：`clip_tool_output` → `project_tool_output`（一处判定、两种后果），`WireCaps` 带落盘句柄因而从 `Copy` 降为 `Clone`，`attempt_stream` 改吃借用 |
| `desktop/src-tauri/src/llm/{app_llm,client}.rs` `state/llm_stack.rs` | 刀② ✅：`AppLlm.spill` 字段 + `with_spill` 注入器；组装点交**数据根**（不手拼目录名）。app_llm 由此 394→411 行，按三问入 `check_length` 名单（理由写在该脚本注释与提交正文） |
| `scripts/check-single-source.mjs` | 刀① ✅ SS16a/b/c；刀② ✅ SS17a/b/c + spill 面下限；刀③ ✅ SS18a/b/c + micro 面两条下限 + 三条窄变异 |
| `desktop/src-tauri/src/microcompact.rs`（+ `microcompact/tests.rs`） | 刀③ ✅：微压缩判据本体（七颗参数、候选分组、idle 播种表、`agent/request` 监听器） |
| `desktop/src-tauri/src/state/waterfall.rs` `state/mod.rs` | 刀③ ✅：第八环注册位 + 主层阈值闭包（`Init` 新增 `catalog` / `model_overrides` / `sessions_dir` 三份读数）；头注把"八环"的序与"为什么这一环可以在红线之后"补全 |
| `desktop/src-tauri/src/settings/{context,view}.rs` `settings.rs` | 刀③ ✅：`context.microEnabled` 一枚（缺省开）+ 后端读面 `context_micro_enabled()`；刀⑥ 剩下的才是其余数值 |
| `desktop/src/{types/settings.ts,components/settings/ContextSection.vue,locales/{zh-CN,en}/settings.ts}` | 刀③ ✅：视图多一个键、分区多一枚开关、中英各一条文案（`check-i18n` 读数两侧相等） |
| `crates/kernel-agent/src/middleware/`（新目录或 `compact/` 下新文件） | **刀③ 没进内核**（微压缩落在宿主监听器 + 发送侧，内核只提供 `compacted` 带回位）；这一格只剩**刀④** 的主层压缩本体 |
| `crates/kernel-agent/src/kernel/request.rs` `waterfall.rs` | 只**用**现有 `compacted` 带回位；契约形状零改动 |
| `desktop/src-tauri/src/settings/context.rs` + 设置页「上下文」分区 | 刀⑥：新字段 + 文案（中英成对） |
| `desktop/src-tauri/src/llm/retry.rs` | 刀①/刀⑥：那句"请新开会话"的指向改对；压缩全关时它仍是最终兜底 |
| `crates/session-store/` | 不动形状，只新增 `strategy` 取值 |
| 前端用量环 / 开发者面板 | 只读展示"这一轮压了什么"，不在 `MemoryPage` 上加东西 |

## 风险与回滚

- 风险①（specs/01 已记）：微压缩边界实现不当会打断前缀缓存 ⇒ AC-S2-4 的"重放两次 hash 相同"
  与"二次构造不回收"两条判据**先于实现写**；出问题回退 = 关压缩总开关（滑窗也不开 =
  回到现状超限报错，不比现在更坏）。
- 风险②：主层摘要质量不可控（形状侧风险，方案稿末尾）⇒ 第一版只保证 AC-S2-6/7 那些
  可断言的结构性质，不承诺摘要"聪明"；防膨胀条款必须随模板走（OpenCode #48058 教训）。
- 风险③：本线一旦落地，`llm/retry.rs:53` 那句话的命中率应该掉到零——**如果没掉**，
  说明触发线或锚点算错，这一条本身就是 AC-S2-9-② 的判据。
- 挂账 / 遗留：S4 退役 organize 时要复查刀①搬走的那两块地有没有被留成双份；
  按模型覆盖阈值留后续（落点 = 模型管理页）。**刀② 落地时读实工具面，抓到三件登记**：
  ⓐ `fs_read` **没有 `offset` 入参**（`tool-fs/src/read.rs` 全量读入后 `truncate(max_bytes)`，
  缺省 32 KiB、上限 1 MiB）⇒ 刀② 第一版占位文案里"传 offset / max_bytes"是教模型用一个
  不存在的参数，已当场订正为如实措辞（见 `spill::persisted` 的头注）；给 `fs_read` 补
  `offset` 要同时改 manifest 与 describe（SS7 那条清单↔describe 对账会盯着），单独立一刀。
  ⓑ **同一句假措辞在改前的截断提示里也存在**（`llm/tests/tool_budget.rs:91` 还钉着
  "含 offset / max_bytes"）——那是 v2.192 Q8 留下的既有缺陷，本刀**故意不动**：
  降级文案与改前逐字节相同是刀② 的等价性证据，动它就等于用另一件事换掉这条证明。
  等 ⓐ 那刀一起改（届时同步更新那条断言，不许悄悄放宽）。
  ⓒ 白名单不能按名字放行 `mcp_<server>_<local>` 那一族（名字是动态拼的，
  `tool-mcp/src/registry.rs:157`）⇒ 刀③ 的微压缩白名单只列可枚举的真实名，MCP 结果缺省不清。**刀① 收下来又读到的三件，都登记不修**：
  ① "生效窗口"仍有两条链——上下文节奏走 `session_capacity`（四层，含 models.dev 目录），
  发送侧走 `app_llm.rs:240-243` 的 `RowFacts`（两级，故意不查目录，理由是"不发明能力"）；
  合并它会改请求下发行为，不属于搬家这一刀。② 无供应商上报时进度条回落树组装口径、
  锚点回落全量估算（取证⑦），统一它 = 改整理触发读数，归刀④。
  ③ `AnchorSource` 这个枚举仍住在 `memory-tree`（搬家只动位置不动形状），S4 退役树时
  它的落点就是 `context_budget`。**刀③ 收下来登记两件，都不在本刀修**：
  ⓓ 设置页那句「工具结果预算」说明（`locales/zh-CN/settings.ts` 的 `toolBudgetKibHint`
  与 en 同键）还写着"超出只在发给模型时**截断**并附一句**续读指引**"——刀② 之后超出是
  **落盘 + 预览**，且那句占位明说"比预览更靠后的部分读不回来"（没有 offset 可续读）。
  这是文案与事实不符，属刀⑥「设置面与文案」那一刀的正文，不在刀③ 顺手改（改它要连
  同 ⓐ 的 `fs_read` offset 一起定：等真有了 offset，那句"续读指引"才该回来）。
  ⓔ 微压缩的账在**会话日志里那一行**还没被断言：环级用例证到 `compacted` 带回位被写入，
  落盘那一跳由 `kernel.rs:253` 既有代码承担。北极星长会话那轮（AC-S2-9）读整轮日志时
  顺带取这个数，不在本刀为它单独搭一套内核夹具。
- 回滚：每刀一提交；总开关关 = 行为回到本线开工前；数据侧（会话附件目录）改动前先备份。
