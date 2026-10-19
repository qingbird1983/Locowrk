# 10-上下文注入面 S3（索引常驻、正文按需 + 占用读数一嘴到底）

- 状态：**进行中 · 刀①② ✅**（开工三问由决策人 **2026-10-20「三问照推荐案，开跑」** 一并
  采纳，裁定结果记在下面「开工三问与裁定」那节；本文件是
  [`specs/01`](../01-memory-context-redesign/spec.md) 阶段表里
  **S3 注入面** 那一行的施工 spec；方案稿对应 = **§C**（+ §A④ 索引制、§F 交接层），
  ⊘ 不入库、只在作者本机，关键条款抄在下面「取证」与「目标」里；决策依据 =
  [`ADR-0007`](../../docs/adr/0007-memory-flat-files-context-llm-compaction-primary.md)）
- 负责人：Qoder（取证 + 实现） / 青鸟（下面「要你定的」三处点头）
- 日期：2026-10-20（现状取证同日）
- 出处 / 上游：方案稿 §C「注入面：索引常驻，正文按需」+ [`specs/01`](../01-memory-context-redesign/spec.md)
  的 AC-4 / AC-5 / AC-7（本线吃这三条）+ S1 留给本线的两格（
  [`specs/05`](../05-memory-data-plane-s1/spec.md) 非目标第 1 条「注入面形状 = S3」与风险③
  「mtime 进预取缓存键 = S3 的接线」）+ S2 长跑落下的账 ⓟ（
  [`specs/09`](../09-context-compaction-s2/spec.md) 挂账区「ⓟ 剩下那一半」那条，原文在
  该文件 514–519 行），**ⓟ 按决策人 2026-10-20 指令
  归进本线刀④**，不再单立一刀。

## 目标

两件事，都是"把形状扳到设计稿那一格"：

1. **注入面**：现在注入的是「全局事实 top-K **正文**（还有一条按 sqlite 加权的通路）+
   项目索引指针行」两段分开；目标形状 = **两段索引指针行**（全局 `MEMORY.md` + 项目
   `MEMORY.md`，钳 200 行）常驻，**正文一个字都不注入**、模型按需读文件。连带三件：
   人改 `MEMORY.md` 下一轮就生效（mtime 进预取缓存键）、`<memory_guidance>` 按新工具面
   与排除清单改写（它现在还在承诺树工具）、树目录段随本线移除。
2. **占用读数一嘴到底**（ⓟ）：压缩面判断到线用的是**三档取最大**，界面进度条用的是
   **另两档**——同一个"占用"在同一个进程里有两个算法，失败期间界面停在旧数；而
   「输出预留 ≥ 一轮压缩增量」这条关系界面上没有任何一格说破。这一格要收成**一个出处**
   并把扣减链**显形**。

## 取证（2026-10-20 读实，逐条带出处；这一节决定下面每一刀的形状）

1. **注入面今天是五个段**，组装全在 `crates/kernel-agent/src/middleware/memory_inject/mod.rs`：
   `<environment>`(:255) → `<repo_context>`(:259) → `<persistent_memory>`(:164/182，
   由 `recall_relevant` 出条目 :276-280) → `<project_memory_index>`(:285) →
   `<memory_guidance>`(:85-99 内核常量)，另有一族**追加**在后的树目录段
   （`crates/kernel-agent/src/middleware/tree_context.rs:24` `TreeDirectoryInject`，
   内文由宿主 `desktop/src-tauri/src/organize/assemble.rs:191/203` 生成）。
2. **数据源是三种混着**（宿主实现 `desktop/src-tauri/src/memory/source.rs`）：
   `<persistent_memory>` **两档**——`memory.learnEnabled` 开 = 走 sqlite 四信号加权
   （`memory_learn::candidates`，`source.rs:97-106`），关 = 走 md 文件
   （`memory/recall.rs:81-99` `memlayout::iter_facts`，**只全局层**）；
   `<project_memory_index>` = md（`source.rs:217-218` → `memory_index.rs:66 from_md`）。
   注释自己说破了处境：`source.rs:11`「本片是宿主读面里**最后一条仍按库寻址的通路**」。
   ⇒ S1 把真源扳到 md 之后，`learnEnabled` 那一档是**残留的第二真源**。
3. **全局 `MEMORY.md` 从没进过注入面**：`grep global_index|global/MEMORY desktop/src-tauri/src`
   零命中。设计稿 §A④ 要的"每会话注入两个索引"现在只有项目那一个。
4. **`<repo_context>` 是纯静态文本**：`desktop/src-tauri/src/repo_context.rs:69`
   `pub fn build(_repo: &Path)` 参数不消费，返回固定串（文案点名 `requirements.md` /
   `handoff.md` / `notes/`，还写着 "These files are NOT auto-injected"）。§F 要的
   "`<repo_context>` 携带 `MEMORY.md` 正文 + requirements 提醒行"**没接线**；
   `handoff.md` 这个旧名字还活在文案里（§F 已把它换成 `MEMORY.md` + `journal/`）。
5. **预取缓存键不含 mtime**：`memory_inject/mod.rs:229-232` 键 =
   `session_id + '\u{1}' + scope_id`，条目形状 `{ block, top_k }`（:101-135），失效通路只有
   换会话 / 换键位 / `top_k` 变三种；`source.rs:147-151` 的注释明写「交接文件 mtime 指纹
   **退出**缓存键」。⇒ 人改 `MEMORY.md` 在同一会话内**不生效**（S1 风险③ 登记的就是这一格）。
6. **`<memory_guidance>` 现文在承诺退役物**：`mod.rs:85-99` 里写着「Empty results …
   do NOT mean there is no history: project history lives in the memory tree — call
   `memory_project_history` first, and `memory_search_titles` for topics」。这两枚工具
   （`organize/tree_tools.rs:44`、`organize/search_titles.rs:43`）是 §D 点名要退役的
   树工具族，归 S4。⇒ 本线要么同刀改掉承诺，要么留下"指引指向活物、S4 一拔就指空"的债。
7. **硬钳有唯一出处但只施在项目段**：`crates/plugin-api/src/memlayout/index.rs:30`
   `MAX_INDEX_LINES = 200` / `:32` `MAX_INDEX_CHARS = 25_000`（写侧钳 :43-49，落盘 :114-118）；
   注入侧 `desktop/src-tauri/src/memory_index.rs:36` 再导出、`:173-191 clamp` 施在
   `<project_memory_index>` 的指针行上。`<persistent_memory>` 走的是**另一把尺**
   = 条数 `top_k` + 单条 500 字符（`memory_inject/mod.rs:67` `VALUE_INJECT_MAX_CHARS`）。
8. **顺序被钉住的部分**：装配点唯一 = `desktop/src-tauri/src/state/waterfall.rs`
   （:5-8 头注「注册序就是执行序，一条不许重排」，:108 `MemoryInject` → :132
   `TreeDirectoryInject`）；`crates/kernel-agent/tests/prompt_layering.rs:74-101` 钉
   「静态前缀 → 记忆居中 → 目录殿后」与「目录增长不破前缀」，五段**内部**序由
   `memory_inject/tests/project_index.rs:89-95` 钉。`scripts/check-middleware-wiring.mjs`
   只查"有定义无构造点"，不查顺序。
9. **ⓟ 那一格的事实（三条，全部读实）**：
   ① 分子两副算法——界面 = `desktop/src-tauri/src/runner/mod.rs:370-403 record_usage`
   （:378 只吃 `reported_input_tokens`，无上报才回落估算），压缩面 =
   `desktop/src-tauri/src/compact/deep.rs:117-131 Compactor::used`（上报 / 压后投影估算 /
   拒收读数**三档取最大**，:130 `max(refusals.of(...))`）。两副算法**没有共同出处**，
   且 `used()` 生产调用点只有一个（`deep.rs:178`）、**对界面零暴露**（`/压缩` 的返回值
   `commands/compact.rs:39-42` 只有 `{ compacted, reason }`）。
   ② 通道只有一条：`SessionEvent::ContextUsage` → `AgentEvent::ContextUsage` →
   `agent://context-usage`（`events.rs:23/387`）→ `stores/chat/projection.ts:211`；
   `ContextCompacted` 带 `from_tokens/to_tokens` 但**不带当前占用**（`events.rs:178-185`）。
   界面三处读同一份 `contextUsage`：`ContextProgressBar.vue:19-34`（`used/threshold` 与
   `used/windowTokens`）、`UsageRing.vue:22-26`、`ContextUsagePopover.vue:47-60`。
   ③ 扣减链在界面上是**暗的**：`context_budget.rs:25/32` 那两颗常量（输出预留封顶
   21 000 / 缺省 32 000）与 `:73-82 effective_threshold` 的式子，界面零呈现；
   输出上限本体住**模型**分区（`settings/model/ModelPanes.vue:505-511 m.maxOutput`），
   和「压缩阈值」不在同一屏 ⇒ 用户看不见"这两枚相减才是要守的那条线"。
   ④ 本仓**没有**"A 必须 ≥ B"这类跨字段校验的先例（`settings/context.rs:265-353
   apply_update` 全是单字段区间判定）。可借的既有形态只有三种：纯文案描述关系
   （`locales/zh-CN/settings.ts:347-349/372-373`）、后端 `Err` → 板面级错误条
   （`settings/context.rs:271-277` + `SettingsSaveRow.vue:5`）、设置页**只读运行时读数块**
   按态换色（`DeveloperSection.vue:246-330`）。

## 验收标准（必须可测：跑什么、看到什么）

- [ ] AC-S3-1 两段索引指针行上界面、旧两块下线：真窗口跑一轮，开发者面板/日志的
      `agent/request` 里 system 带**一个**索引块（含全局段 + 项目段），段里每条指针行
      能在 `~/.Locowrk/memory/` 下指到一个真实文件一行（= specs/01 的 AC-7 可追溯）；
      `<persistent_memory>` 与 `<project_memory_index>` 两个标签在生产面**0 命中**
      （新判据 = 后端标签表 ↔ 扫描面读数，配变异）。
- [ ] AC-S3-2 正文一个字都不注入：夹具里放一条有长正文的事实 ⇒ 注入块只出现它的
      指针行，正文串在 system 里 0 命中；`top_k` 不再管正文条数（其语义要订正或摘除）。
- [ ] AC-S3-3 人改即生效（mtime 接线）：**同一会话内**改 `global/MEMORY.md` 加一行 ⇒
      下一轮 system 带上那一行；同会话**不改** ⇒ 该段逐字节相同（证缓存仍命中、
      前缀不被砸）。两条都要用例，缺一条不算。
- [ ] AC-S3-4 `<repo_context>` 换成读真实交接层：注入内文来自 `<repo>/.Locowrk/MEMORY.md`
      正文（不是静态串），文案不再点名 `handoff.md`；文件不存在时**整段缺席**而不是
      注入一句"还没有"。
- [ ] AC-S3-5 指引改写不自相矛盾：`<memory_guidance>` 新文覆盖「写入时机 / 按需读文件 /
      排除清单 / 空结果 ≠ 没历史」四要点，且**不出现** §D 点名退役的四个工具名与
      "memory tree"承诺；判据 = 退役名清单 ↔ 提示词扫描面（配一发变异）。
- [ ] AC-S3-6 树目录段移除不留死码：`TreeDirectoryInject` 从 `waterfall.rs` 摘除，
      `prompt_layering.rs` 那两条用例按新形状订正（不许为过闸删断言）；被摘掉的
      `recall_relevant` 通路上的 `memory_learn` 族、`terms.rs`、`learnEnabled` 设置格
      **按要你定 ② 的裁定处置**，本仓审计报告那条「存在但从未接线」的判据不许再判到我们头上。
- [ ] AC-S3-7 占用只有一个出处（ⓟ 本体）：界面分子与 `Compactor::used` **同一个函数**
      （新判据钉等值 + 双向变异：任一侧单独改尺即红）；构造"上一轮被超窗拒收"的会话态
      ⇒ 事件里的 `used` ≥ 拒收那颗数（这条就是 ⓟ 说的"失败期间停在旧数"的反证）。
- [ ] AC-S3-8 扣减链显形（ⓟ 的另一半）：式子的真源 = `context_budget.rs:73-82`
      `effective_threshold = min(滑杆值, 窗口 − output_reserve)`（`output_reserve` =
      `min(输出上限 ?? 32K, 21K)`，:55-59；**只扣一次**，AC-S2-2 的靶就是扣两遍）。
      界面要能一眼读出这条链：`窗口 → −输出侧预留 → 与滑杆取小 = 有效阈值 → 已用 / 有效阈值`，
      外加一格「窗口 − 有效阈值 = 留给输出与下一轮净增的空位」——**这颗空位就是 ⓟ 说的那条
      关系**（净增超过它 ⇒ 撞窗）；输出预留那颗数必须是**带出处的读数**（写出"21K 封顶"或
      "按你钉的输出上限 8K"）而不是隐藏的常量。逐态写清各看到什么：窗口取不到（阈值原样吃
      滑杆，:79 那一支）/ 窗口取到且滑杆更小 / 窗口取到且被预留封顶（滑杆说了不算）。
      用户把输出上限钉大钉小 ⇒ 这一格当场跟着变（`min` 的两侧都要有用例反证，
      沿 `reserve_follows_a_small_ceiling` 那条既有纪律）。
- [ ] AC-S3-9 `make check` 全绿；涉界面两刀（刀①④）各自在真实窗口自截图核对后再给你看。

## 非目标

- 不做回合收束的增量抽取（S1 之后单立一批，要 LLM 通道）；不做 embedding 检索
- 不动 organize 管线、不退役 memory-sqlite 插件与树工具族本体、不动 `MemoryPage`
  两个 tab、不建 §E 那三枚行为开关（全归 S4）
- 不动 ⓖ/ⓗ 两枚控件形态（仍等你批形态，本线不借道塞进去）
- 不合并"生效窗口"两条链（specs/09 取证 6 登记的既成事实）；不动账 ⓛ（定时那条链）
- 不做跨设备同步、不做应用内记忆列表视图
- ⓝ（某类会话里深度压缩长期零起步）与 ⓞ（判据口径）**不归本线**：一条是走查读数说明、
  一条已写进 spec

## 计划（四刀，每刀一提交、跑绿再走下一刀）

1. **刀① 索引段替换** ✅ 2026-10-20：新块（全局段 + 项目段，钳 200 行 / 25 000 字符，复用
   `memlayout` 那颗常量与 `memory_index.rs` 的钳）；`<persistent_memory>` 与
   `<project_memory_index>` 两块下线；标签判据 + AC-S3-1/2。**施工记录见下面那节**。
2. **刀② 缓存键与注入源** ✅ 2026-10-20：mtime 进预取缓存键（全局 + 项目 + 交接层三处指纹）；
   `<repo_context>` 从静态串改成读 `.Locowrk/MEMORY.md`；AC-S3-3/4。**施工记录见下面那节**。
3. **刀③ 指引与退役段**：`<memory_guidance>` 改写；`TreeDirectoryInject` 摘除；
   学习档按裁定处置；AC-S3-5/6。
4. **刀④ 占用一嘴到底（ⓟ 归这一刀）**：`record_usage` 的分子改吃 `Compactor::used`
   （或两者共同下沉成一个函数）+ 拒收读数进 `ContextUsage` 载荷 + 扣减链上界面；
   AC-S3-7/8。

## 刀① 施工形状（本线第一刀；与三问无关，点头即照此动码）

- **契约**：`MemorySource` 新增 `global_index(session_id) -> Option<String>`，**缺省 `None`**
  ⇒ 五个测试 fake 与 headless 装配零改动（与 `repo_context` / `project_index` 同一惯例）；
  `project_index` 语义一字不动。内核把两段包成**一个** `<memory_index>` 块，段头
  `[global]` / `[project]`，两段各自可缺席（由 `append_block` 的"add=None 不动"承担，
  不写第二种早退分支）。块名与段头是我定的，可翻案，不为它单开一问。
- **同刀删**：`build_block` / `clip` / `tag_suffix` 三个私有件——摘掉 `recall_relevant`
  那唯一一次调用（`mod.rs:276-280`）之后它们必成 dead code，`clippy -D warnings` 当场红，
  所以删它们是这一刀的必经之路，不是"顺手清"。
- **同刀不动**：`recall` / `recall_relevant` / `MemoryEntry` / `VALUE_INJECT_MAX_CHARS`
  全部原样留着——宿主 `relevance` 信号还在复用那颗常量（`mod.rs:62-67` 写明了"匹配面必须
  等于注入面"这条纪律与它的判据），而 `learnEnabled` 一族的去留是**三问②**，不在本刀偷做。
- **`top_k` 只改一处**：早退条件从 `!cfg.enabled || cfg.top_k == 0`（`mod.rs:215`）改成只看
  `enabled`——索引常驻之后"条数设 0 连索引一起关掉"是说不清的隐藏耦合。但
  `MemoryInjectConfig.top_k` 字段、`settings/memory` 那三处、`PrefetchEntry.top_k`
  **一律不动**：`settings.rs:279-287` 那段注释把"字段集只有 `block`/`top_k`"写成**有意承重**
  （破它 = 有人能把权重塞进失效判据），要改得单独带理由。登记：`memory.topK` 自本刀起
  在注入面**无消费者**，去留随 §E 那一批（S4）。
- **宿主侧**：`global_index()` 读 `~/.Locowrk/memory/global/MEMORY.md` 的**落盘正文**
  （真源 = 文件、不现算排序 = specs/01 的 AC-7）；钳制复用 `memory_index.rs:173 clamp` 与
  `memlayout/index.rs:30` 那颗常量——**同一把钳不许出现第二个出处**，全局段与项目段
  走同一个 `clamp`，只是喂进去的 body 不同。
- **判据**：新 SS 项钉「`<persistent_memory>` 与 `<project_memory_index>` 两标签在生产扫描面
  0 命中」+「`<memory_index>` 恰好 1 处」；变异必须是**在生产文件里**复活旧标签
  （测试夹具里出现旧标签不算红，否则这条判据挡不住"生产还留着、只是没人调"）。
- **用例**：`memory_inject/tests/project_index.rs:89-95` 那条「facts < index < guide」的内部序
  按新形状改写（旧的两块没了）；`crates/kernel-agent/tests/prompt_layering.rs` 两条**不动**
  （环序没变）；`tests/prefetch.rs:80-101` 的「top_k 变 = 重算」仍绿（字段没动）。

## 刀① 施工记录（✅ 2026-10-20 · 22 文件 +622 / −862）

按上面那版形状落地，**四处偏差与连带**都要记（都是"不这么做就当场红"的那种）：

1. `terms.rs`（107 行）与 `tests/learn.rs`（7 条用例）**随唯一消费者同刀离场**：原计划
   只写"摘调用点"，实际不摘这两件，`clippy -D warnings` 的 `dead_code` 当场红。
   learn.rs 里那条 M1 承重（"同会话只取数一次、不许把权重塞进失效判据"）**迁到
   `prefetch.rs`**，道具从"正文条目漂移"换成"索引内文漂移"，断言语义一个字没改；
   `settings.rs::memory_inject_config` 头注点名的是 learn.rs 那条 ⇒ 注释同步订正并留
   "原先住哪"的记号（不改它 = 留下一条指向已删用例的假指针）。
2. `top_k` 早退条件按计划改掉，并**新增一条用例** `zero_top_k_no_longer_switches_the_block_off`
   钉这次语义变更本身（旧形状下 `top_k = 0` 关整块，现在不关）。
3. **观测通道同步**：`llm/diagnose.rs` 的 `SYSTEM_MARKERS` 从 6 枚改 5 枚（旧两块合并），
   顺序跟着新组装序走；它的夹具与断言（`diagnose/tests.rs`）同步换成 `<memory_index>`。
   漏改这一处的症状不是编译错，而是"索引段中途变化被报成 system 变了"——缓存诊断
   从此看不见这一格，而刀② 要接 mtime 时最需要盯的就是它。
4. 内核侧三处假件换通道：`memory_inject/tests/mod.rs`（`Fixed` → `Empty` + 新增 `Sourced`）、
   `kernel/tests/mod.rs` 的 `DriftingMemory`（`recall` 漂移 → `global_index` 漂移）、
   `kernel/tests/prefix.rs` 与 `tree.rs` 的定位标签。
5. **判据 `SS27`**（三段读数：旧标签 0 命中 / `format!("<memory_index>` 恒 1 / 字面量所在
   文件集合 = 内核 + `llm/diagnose.rs` 两处）实测读数 = `0|1|...mod.rs|...diagnose.rs`；
   窄变异 = 在生产面复活 `<persistent_memory>`，`make selftest` 读数 `✓ SS27 变异可见`。
6. **真机数据根核对**（不是假件）：`~/.Locowrk/memory/global/MEMORY.md` 在场，形状
   `# 记忆索引` + 一条 `- [user.pref.techstack-agnostic](user.pref.techstack-agnostic.md) — 技术栈无关 规范设计`
   ⇒ 与 `pointer_lines_from_md` 的解析形状匹配；全局层目录经 `memlayout::layers()` 枚举，
   宿主没有第二处 `global/` 字面量。
7. **门禁**：`make check` 末行 `▶ check 全绿`（宿主 709 条 / 内核 98 条 + 全部集成套件 /
   vitest 193 条 / 两处 clippy 干净）；`make selftest` = `SELFTEST_ALL_CAUGHT`。

**这一刀欠的一发读数**：AC-S3-1 里"真窗口跑一轮看 system"未取——刀① 没动任何界面组件，
而真窗口那趟要看的 `repo_context` 还在刀② 改，所以这一发**随刀② 一起补**（届时一次
起窗口取两刀的证）。登记在册，不算已验。

**新增的挂账**：`MemoryInjectConfig.learn` 那枚字段现在**无读者**（内核不再读它，
宿主 `memory_inject_config` 仍在填）——它随刀③ 的 `learnEnabled` 族一起清，本刀不偷删
（删它要动 settings 面，属刀③ 的改动面）。

## 刀② 施工记录（✅ 2026-10-20）

**键**：预取缓存键从 `(session_id, scope_id)` 变三段 `(…, index_stamp)`。契约新增
`MemorySource::index_stamp()`（缺省空串 = 不参与键），宿主实现交 `g{ms}p{ms}r{ms}`
三档（全局索引 / 项目索引 / 交接正文），**只 stat 不读内容**（它每 step 都跑，排在缓存
查询之前）。三处路径全部复用既有单点算式（`memlayout::layers` / `project_dir` /
`MemoryStore::repo_root` + `repo_context::{DIR, HANDOFF}`），宿主没有再拼一遍 `global/`
或 `MEMORY.md` 字面量；`HANDOFF` 抬成 `pub(crate)` 的理由写在它自己的注释里。
`scope_id` 与 `index_stamp` **分成两枚方法**而不是合成一个串：键位身份与新鲜度是两件
事，混在一个返回值里就没法分别判读（`scope_id` 那条"mtime 退出缓存键"的旧注释同步订正）。

**注入源**：`repo_context::build()` 从静态串改成读 `<repo>/.Locowrk/MEMORY.md` 正文 +
三行维护提醒。三个决定要记：① **读不到 ⇒ 整段 `None`**，不退回旧的静态提醒——留一份
与文件无关的提醒会把"这层没接线"与"接了但暂时没内容"混成同一个形状；② 因此
`ensure_layout` 同刀多写一份 §F 主题分节骨架（否则第一个会话什么都收不到，"注入源切换"
就成了空头话），骨架与 requirements 模板都是"缺则写、有则一个字不碰"；③ 正文按 §F 硬顶
做行 + 字节双钳，**超预算那一行整个吐掉**（半行会被模型读成一条完整事实）。旧静态串里
那三个退役物（`handoff.md` / `notes/` / `memory_project_history` 等三枚树工具）随文本
一起消失。

**一处语义放宽，不许说成"延续 M1 纪律"**：旧形状下模型自己 `memory_write` 之后本会话
看不见自己写了什么（那是刻意的，防逐轮换字节砸前缀）；mtime 进键之后**分不清改的是人
还是模型** ⇒ 模型每写一条事实，下一轮付一次前缀缓存断裂。买到的是"人编辑 `MEMORY.md`
当场生效"（§C/§F 硬要求）+ 新记的那条本会话内可见；付掉的是每次写入后一轮全价 input
（与 S2 刀④ 主层压缩那种一次性前缀重置同量级，不是每轮都付）。这段记在
`memory_inject/mod.rs` 头注里，不在文档里粉饰。

**删掉一条用例**：`repo_context/tests.rs::build_is_static_reminder_independent_of_files`
断言的正是本刀要**反转**的形状（"与文件状态无关"）。删它不是删测试凑绿，替代它的断言
写在同一簇里（"改了正文输出必须跟着变"是同一格的反面证据），删除理由记在该文件头注。

**用例**：内核 1 条正反同体（`a_fresh_stamp_recomputes_while_a_stale_one_freezes`：
不变 ⇒ 逐字节复用且取数次数不涨；变 ⇒ 重算、带新内文、旧份消失、新键下重新钉死）；
宿主 2 条（`the_index_stamp_follows_each_injected_input_and_only_that_one` 钉"只有该动的
那一档动"——防止过度失效砸前缀；`an_unattributed_session_still_carries_the_global_stamp`
挡住"在开头顺手整体早退"）；`repo_context` 3 条（骨架幂等且不覆盖人写的 / 正文进段 +
缺席 + journal 不注入 + 退役名不在场 / 超预算钳 + "没超就不许说超了"的负对照）。

**门禁**：`make check` 末行 `▶ check 全绿`（宿主 711 条 / 内核注入簇 21 条 /
vitest 193 条 / `SELFTEST_ALL_CAUGHT`）。首跑在 clippy 红过两条——
`explicit_counter_loop`（钳制函数拿 `lines` 当计数器）与 `&PathBuf` 该写 `&Path`，
两条都是真信号，改的是写法而不是加 `#[allow]`。

**这一刀新增的挂账**：
- ⓠ 交接层 `MEMORY.md` 的**写侧钳不存在**（S1 只给记忆库两层做了写侧钳）⇒ 今天只有
  读侧钳在挡，§F 那句"超顶 = 逼蒸馏"没有生产者；补它要动收束更新那一侧，不属注入面。
- ⓡ §F 的另两件没跟着落地：`journal/<YYYY-MM-DD>.md` 的**按天封卷**与"轮次收束后模型
  主动更新"的提示词改写。刀② 只切了注入源（本刀的承诺面）。存量 `handoff.md` 归并
  **不需要做**——本机实测本仓 `.Locowrk/` 从未存在过，没有存量可归并（这条不是欠账）。
- 真窗口那一发（AC-S3-1/3/4 的"跑一轮看 system"）仍欠，见下面风险③ 的登记。

## 刀③ 拆成两刀的清点（2026-10-20 读实，这一节决定下一轮从哪儿动码）

清点结果推翻了"刀③ 是一刀"的写法，两处非做不可的拆分理由都记在这里：

1. **`tree_directory` 不只是目录文本**。`TreePlan`（`kernel/services.rs:145`
   `TreeContextSource::plan`）一次交出三样东西：目录文本、**树会话标记**、
   `tail_user_start`（消息切到"未整理尾巴"的起点）。内核组装侧
   `kernel.rs:215-227` 拿它做的是三件事，而生产者在
   `desktop/src-tauri/src/organize/assemble.rs` —— **那是 S4 要整片冻结退役的地方**。
   ⇒ 本线只能摘**监听器**（`middleware/tree_context.rs` + `waterfall.rs:132-135` 那一环
   注册），`RequestPayload.tree_directory` 字段与 `plan()` 通路**一律留着**（S4 随
   organize 一起拔）。刀① 我对 `memory_learn` 的裁定是"不留存在但从未接线"，这一处
   为什么可以留：它的字段仍被消费（marker 与切片），消失的只是"文本进 system"这一档，
   而那一档的**生产者**住在别的阶段。这条边界不写下来，下一刀就会顺手去动 organize。
2. **摘掉监听器会撤掉一条纪律的证明**。`crates/kernel-agent/tests/prompt_layering.rs`
   三条用例（`static_prefix_memory_middle_directory_last` /
   `directory_growth_leaves_prefix_intact` / 空串那条）的对象**就是** `TreeDirectoryInject`：
   它们证的是"append-only 段只动尾部 ⇒ 目录增长不砸前缀"。摘掉注入器之后 system 尾部
   不再有目录，这三条失去对象 ⇒ 同刀删除，但**纪律本身要交代继承者**：静态前缀不被动态
   段改写这件事，此后由 `memory_inject/tests/{prefetch,project_index}.rs` 那几条
   （逐字节复用 + 取数次数）守。这一句必须写进提交正文，否则"删了三条测试"看起来就像
   为了过闸删测试。
3. **学习档删除面实测比裁定时那一行大**：`memory_learn/` 本体 **933 行**，
   `learn_enabled` / `LearnWeights` / `memory_learn` 的触点铺开 **20 个文件**，除后端字段
   与前端那一格外还牵到 `inject_view.rs`（管理页注入视图）、`recall_log` 的 `breakdown`
   埋点面、`memory/schema.rs` 那两列 `hit_count` / `last_hit_at`（影子库的列）。
   ⇒ 拆成 **③a**（指引改写 + 树目录段摘除，约 8 文件）与 **③b**（学习档族删除），
   各自跑绿各自提交；③b 里**影子库那两列不动**（属 S4 的 sqlite 退役面），本刀只摘
   生产引用与设置面，并把"列还在、没人读写"这一格如实登记。

## 开工三问与裁定（2026-10-20 决策人「三问照推荐案，开跑」⇒ 三处全按推荐案落地）

① **刀④ 的可见性用哪种既有形态**（本仓没有跨字段校验先例，取证 9④ 已列三种可借）
   —— **裁定 = 案 A**：
   - **✅ 采纳**：进度条浮层加一行只读扣减链（`窗口 → −输出侧预留 → 与滑杆取小 = 有效阈值`，
     式子照 `context_budget.rs:73-82` 不另造），
     设置页「压缩阈值」那行的说明文案跟着引用它——**复用既有的"只读读数 + 文案"形态**，
     不加新控件、不做保存拒绝（拒绝会让用户改不动，且 §4 要求限制可关）。
     代价：关系仍然是"看得见但不拦"，撞窗还得等真撞。
   - **❌ 否掉的 B**：再加一道后端 `Err`（输出上限 × 阈值 跨字段校验，保存即拒）——最硬，
     但它把"预留 ≥ 一轮增量"这条**没有外部真源**的关系钉成禁令，违背 §4「限制可关」。
   - **❌ 否掉的 C**：只做纯文案（现 `thresholdHint` 那种），不加读数——最省，但 ⓟ 要的正是
     "当场看见这一档动了多少"，纯文案给不了读数，等于没销账。

② **`learnEnabled` 那一档与 `memory_learn` 族**在刀③ 摘掉唯一调用点之后
   —— **裁定 = 同刀退役删除**：md 是唯一真源，加权档是残留的第二真源，跨三刀留着
   就是审计报告 H2 判过的那个形状（"存在但从未接线"），且设置页会留一格拨了没反应的开关。
   删除面：`memory_learn/` 整族 + `terms` 面（刀① 已走）+ `learnEnabled` 后端字段与
   设置格 + `MemoryInjectConfig.learn` + `recall_relevant`（trait 方法与宿主实现）。
   **被否的另一案**：登记到 S4 一起拔（代价如上）。

③ **输出预留那颗封顶常量（21 000）**：按 §4「有不确定性的数值一律做成设置项」要不要
   在本线做成可调项（缺省 = 改前那颗常量，越界整笔拒，照刀⑥ 的形状）？还是刀④ 只把它
   作为**只读读数**显形、可调性等 §E/ⓖ/ⓗ 那一批一起做？
   —— **裁定 = 刀④ 做成可调项**（照 §4 那条硬规矩与刀⑥ 先例：后端字段 + 区间常量 +
   越界整笔拒 + 中英两条文案 + **缺省 = 改前那颗 21 000**，等价性用"既有
   `context_budget` 测试一字未改仍然绿"实证，不靠我说）。

## 影响范围

| 文件 / 目录 | 改动 |
| --- | --- |
| `crates/kernel-agent/src/middleware/memory_inject/{mod,contract}.rs` | 刀①：块形状与标签换成索引段（内核管标签、内容真源在宿主的分工不破）；刀②：缓存键 + mtime；刀③：`GUIDANCE_BLOCK` 改写 |
| `crates/kernel-agent/src/middleware/memory_inject/terms.rs` | 刀① ✅ **已删除**（107 行，随唯一消费者 `recall_relevant` 调用点一起离场；`tests/learn.rs` 那 7 条同刀删，其中的 M1 承重迁进 `prefetch.rs`） |
| `crates/kernel-agent/src/middleware/tree_context.rs` | 刀③：移除（`kernel.rs:216-227` 的 `tree_directory` 带入通路一并订正） |
| `desktop/src-tauri/src/memory/{source,recall}.rs` `memory.rs` | 刀①：读面新增全局索引段；`recall_relevant` 摘除或保留按裁定 ② |
| `desktop/src-tauri/src/memory_index.rs` | 刀①：全局段复用同一把钳（现在只钳项目段） |
| `desktop/src-tauri/src/repo_context.rs` | 刀② ✅：`build()` 从静态串改成读 `.Locowrk/MEMORY.md` 正文 + 提醒行 + §F 双钳；`ensure_layout` 多写一份骨架；`HANDOFF`/`HANDOFF_MAX_*` 成常量；该簇 3 条用例（含反转掉的那条静态断言） |
| `desktop/src-tauri/src/memory_learn/`（+ `settings/memory/*` 的 `learnEnabled`） | 刀③：按裁定 ② 处置 |
| `desktop/src-tauri/src/state/waterfall.rs` | 刀③：摘 `TreeDirectoryInject` 一环（头注那"注册序就是执行序"的段要订正） |
| `desktop/src-tauri/src/runner/mod.rs` `compact/deep.rs` `events.rs` `runner/projection.rs` | 刀④：占用收成一处 + `ContextUsage` 载荷补拒收档 |
| `desktop/src/{components/ContextProgressBar.vue,components/chat/UsageRing.vue,components/chat/ContextUsagePopover.vue,stores/chat/{projection,state}.ts,types/events-agent.ts,lib/api/channels.ts}` | 刀④：分子换源 + 扣减链一行（文案中英成对） |
| `desktop/src-tauri/src/settings/context.rs` + `components/settings/ContextSection.vue` + `locales/{zh-CN,en}/settings.ts` | 刀④（若裁定 ③ 选可调项则多一格字段）：说明文案引用扣减链 |
| `crates/kernel-agent/tests/prompt_layering.rs` `memory_inject/tests/*` `desktop/tests/*` `scripts/check-single-source.mjs` | 全程：形状用例订正 + 新判据（标签等值 / 占用等值 / 退役名清单）+ 各配变异 |

## 风险与回滚

- 风险①：**倒退窗口**（ADR-0007 风险③ 点名的就是 S1–S3 间隙）。刀① 一摘
  `<persistent_memory>` 正文，而"按需读"要靠 `memory_read` / 文件工具真跑通；
  树工具到 S4 才退役 ⇒ 期间指引与实际能力必须**一致**（AC-S3-5 的判据为这一格存在）。
- 风险②：刀② 把 mtime 塞进缓存键 = 砸前缀缓存的通路。命中态必须有用例反证
  （不改文件 ⇒ 逐字节相同），否则等于把 S2 刀③ 花力气守的"被清前缀稳定"从另一头漏掉。
- 风险③：刀④ 改分子 = **改界面读数口径**，不是修 bug。开工前要先读到消费端
  （`ContextProgressBar` 与 `UsageRing` 两处 + 重放链 `runner/projection.rs:106-120`），
  并如实写成"旧行为是只吃上报数"，不许说成"改成拒绝"。
- 回滚：每刀一提交；刀①②③ 有现成门（`memory.enabled=false` / `top_k=0` ⇒ 整块缺席，
  `mod.rs:215-217`），刀④ 是读数口径，回滚 = revert 该提交（不动数据）。
- 挂账：本线不销 ⓝ/ⓞ；ⓖ/ⓗ 仍等你批形态；specs/09 的「主层与 spill 的**真机**长会话一轮」
  与「spill 目录清理通路」两欠与本线无关，仍挂在 roadmap 表上。
