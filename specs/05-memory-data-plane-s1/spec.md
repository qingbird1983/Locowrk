# 05-记忆数据面 S1（把 md 从人读镜像扳成唯一真源）

- 状态：**待确认（未开工）**——本文件是 [`specs/01`](../01-memory-context-redesign/spec.md) 阶段表里
  S1 那一行的施工 spec（01 方案稿 = `docs/design/Memory and Context Redesign v1.md` ⊘ 不入库，
  决策依据 = [`ADR-0007`](../../docs/adr/0007-memory-flat-files-context-llm-compaction-primary.md)）
- 负责人：Qoder（施工） / 青鸟（四处待定项点头）
- 日期：2026-10-18（现状取证同日）
- 一句话：**S1 不新建文件库——文件库已经在了**。要做的是把"sqlite 是真源、md 是镜像"这套
  已经跑着的形状**扳正**成"md 是唯一真源、sqlite 停止被写"。

## 现状画像（2026-10-18 实测，每条带出处）

方案稿 A 节假设"从零建 `~/.Locowrk/memory/` 扁平事实文件制"。实测结果：**四个构件里三个已经
存在并在运行**，缺的是"唯一真源"这条纪律。逐个点名：

| 方案稿要的构件 | 现状（实测） | 差什么 |
| --- | --- | --- |
| ① 布局（md 唯一真源） | 布局已在：`plugin-api/src/memlayout/mod.rs:58` `memory_root()` = `<数据根>/memory`；全局 = `global/`、项目 = `projects/<可读名>-<fnv hex8>/`（`:76` `project_dir_name`）+ `facts/` 子目录、未归属 = `_unaffiliated`（`:41`）；数据根算式 = `plugin-api/src/fsutil.rs:21-31`（`LOCOWRK_DATA_DIR` 优先，否则 `USERPROFILE`/`HOME` + `.Locowrk`），宿主侧 `desktop/src-tauri/src/data_root.rs:19` 同规则另有一份 | 布局**不是**唯一真源：md 只是"真源已落之后的同步"（见下条） |
| ② 一事实一文件 + frontmatter | `render()` 已写 `---\nname / description / metadata{type, source, workspace, updated}\n---\n\n正文`（`memlayout/mod.rs:247-261`），`FactEntry` = workspace/key/value/tags/source/updated_at（`:200-208`） | `metadata.type` **恒为 `"fact"`**（`:249`）⇒ 四类型 `user/feedback/project/reference` 没有；`Why:` / `How to apply:` 正文段没有 |
| ③ 排除清单 | 无（现役抽取 = organize 记录员顺带产事实卡） | 整条要写进提示词与工具说明 |
| ④ 索引常驻、正文按需 | 已落两处：`memlayout::refresh_index()` 产 `# 记忆索引` + `- [stem](stem.md) — desc`（`:292-321`）；注入侧 `desktop/src-tauri/src/memory_index.rs:13-17` **md 投影优先**、缺失才由 sqlite 现合成且不回写，硬钳 200 行 / 25_000 字符 + 截断指路行（`:19-21`/`:30-32`） | 有"sqlite 兜底"这第二条数据源 = 双出处没消灭；`refresh_index` 落盘时**不带**尺寸钳（钳在注入侧现算） |

真源侧（这才是 S1 要扳的东西）：

- **sqlite 是真源，写它才是契约**。宿主 `desktop/src-tauri/src/memory/store.rs:17` 自己
  `rusqlite::Connection`，建表串 `memory/shapes.rs:48-58`（列 = workspace / key / value / tags /
  updated_at / source / hit_count / last_hit_at，主键 `(workspace, key)`）；插件
  `plugins-src/memory-sqlite/src/store.rs` 同一张表另一份实现，两侧逐字节等值由 schema parity
  十档闸钉着（`make gates` 的 A1–E1）。
- **md 写失败只 warn，不拦**：宿主 `memory/store.rs:99-125` 的 `md_write` / `md_delete` 全是
  `if let Err(e) = … { tracing::warn!(…) }`；插件侧 `store.rs:50`/`:73` 用
  `memlayout::files_enabled()`（env `LOCOWRK_MEMORY_FILES`，`memlayout/mod.rs:42-55`）决定要不要同步。
- **双写点共四处**：宿主 `memory/store.rs:112`、`desktop/src-tauri/src/memory_reconcile.rs:217`
  （+ `:129` 刷索引）、插件 `memory-sqlite/src/store.rs:50/73`、首启导出
  `desktop/src-tauri/src/memory_files.rs:225`（幂等：已有文件一律不动，人的编辑不被覆盖）。
- **工具通路两条并存**：插件 6 枚 `memory_write/read/list/search/delete/undo`
  （`plugins-src/memory-sqlite/src/lib.rs:53-164`，带 `scope: enum[project, global]` 参数，`:74`）
  + 宿主内置树工具族 `memory_expand/collapse/search_titles/project_history`
  （注册在 `crates/kernel-agent/src/kernel/services.rs`）。
- **注入面**：`crates/kernel-agent/src/middleware/memory_inject/`（`mod.rs` 313 行 +
  `contract.rs` 166 + `terms.rs` 107）产 `<persistent_memory>`（`mod.rs:163-181`，取数走
  `recall_relevant(session_id, top_k, query)` = sqlite + `hit_count` 加权，`:277`）、
  `<project_memory_index>`、`<memory_guidance>`、`tree_directory` 段；预取缓存键 =
  `(session_id, scope_id)`（`memory_index.rs:25`）。
- **管理页**：`desktop/src/components/MemoryPage.vue`（341 行）调六枚 api
  （memoryList / Update / Delete / ClearAll / Reconcile / RevealFolder），页内还嵌
  `TreeBoard` + `TreeSection` + `InjectSection` 三处树形状；命令面 =
  `desktop/src-tauri/src/commands/memory.rs` 八枚（list / update / delete / clear_all / reconcile /
  reveal_folder / reveal_project / inject_view），树看板另在 `commands/organize.rs` 八枚 `memory_tree_*`。
- 管理页单次列举上限 `LIST_MAX = 200`（`memory/shapes.rs:16`）—— 与索引 200 行同数，不是巧合。

**结论（改工作量）**：S1 ≈ 扳真源方向 + 补四类型与排除清单 + 加 `obsolete/` + 存量导出改道，
**不是**从零建库。原先估的"布局 + 读写 + 索引 + 工具改道 + 管理页"里，前两项大半已存在。

## 要你定的四处（我没替你猜）

1. **布局照现状还是照方案稿改**：方案稿那张图画的是"全局事实与 `MEMORY.md` 直接住
   `~/.Locowrk/memory/` 根，项目层 `projects/<项目-id>/`"；现状多两节（全局在 `global/` 子目录，
   项目事实再套一层 `facts/`，根目录还坐着 `tree.md`）。
   **建议照现状**（索引与事实分层、根目录不混投文件），把方案稿 A① 那张图订正。
2. **四类型怎么进 frontmatter**：建议新增枚举值填进已有的 `metadata.type`（现在恒 `"fact"`），
   `tags` 保留当自由标签不动 —— 这样旧文件读回 `type=fact` 视为"未分类"即可，不重写存量。
   另一条路是拿 `tags` 当 type，我不选：tags 是多值、type 是单值，混用必漂。
3. **type → 层 的映射放在哪**：建议放在**宿主侧一个纯函数**里（`feedback`/`user` 缺省全局、
   `project`/`reference` 缺省项目，显式 `scope` 参数可覆盖），模型只报 type、不必知道目录。
   放提示词里 = 每条记忆落点都要模型自己算，出错没人拦得住。
4. **`refresh_index` 要不要带上尺寸硬钳**：现状钳只在注入侧现算，落盘的 `MEMORY.md` 无上限。
   建议把钳搬到**写侧**（超限即截断并在索引末行写 WARNING），理由 = 索引文件本身也要能被
   人在资源管理器里看，人看到的就该是模型看到的那份（同一物理定律见方案稿 F 节）。

另有一条我不打算问、直接按规矩办的：**动 `plugin-api` 只往结构体末尾追加字段**，
`FactEntry` 不进 C ABI（Rust 侧结构），`PLUGIN_ABI_VERSION` 不动 —— 开工第一刀会用一个
"追加字段后既有测试一字未改仍绿"的实证来担保这句话，不靠我说。

## 施工拆法（三刀，每刀独立可回退、一提交）

- **刀①（纯新增，行为零变化）**：`memlayout` 补 type 枚举 + `render()` 写 `metadata.type` +
  `Why:` / `How to apply:` 正文段 + `refresh_index` 写侧硬钳与 WARNING + 索引/落点映射的纯函数
  与测试。sqlite 写路径一行不动 ⇒ 做完仍是旧形状，随时可停。
- **刀②（扳真源）**：`md_write` / `md_delete` 的 `warn` 改成 `Err` 上抛；读路径（`memory/crud.rs`
  列举、`recall.rs` 取数、`memory_index.rs` 兜底档）改**只读 md**，把 sqlite 兜底那档摘掉；
  `hit_count` / `last_hit_at` 学习列停止读写（文件制没有时间信号以外的权重，v2.136 那条教训）。
  这一刀才是 AC-S1-2 的证据所在。
- **刀③（工具与管理页改道）**：save / read / search / delete 四枚工具的内部实现从 sqlite 改 md
  （名字与参数面不动，提示词与审批面才不会跟着漂）；`obsolete/` 软删 + 清空真删（清空要确认）；
  管理页六枚命令读源换 md、加"三枚行为开关 + 每层一行"。**TreeBoard / TreeSection /
  InjectSection 三处嵌入这一刀不碰**（看板整体删除是 S4 的事，混在一刀里回滚不掉）。

工具归属那条（开放决策 5 说记忆工具归宿主内置）在刀③ 只做到"内部改调 md"，**不把插件那 6 枚
搬进宿主**——搬 = 摘插件注册面 + 动 SS7 十档对账，按禁区相邻规矩单独说明、单独点头。

## 验收标准（跑什么、看到什么）

- [ ] AC-S1-1 写路径唯一：工具或管理页写一条记忆 → 数据根只新增 1 个 `.md` + 索引多 1 行；
      `SELECT COUNT(*) FROM memories` 前后**不变**（sqlite 不再是落点）
- [ ] AC-S1-2 读不碰 sqlite：把 `memories.db` 改名造损坏态 → 管理页列表、`<project_memory_index>`
      注入、`memory_read/search` 全部照常出数（这条是"真源翻转"最硬的证据，也是刀②的收口判据）
- [ ] AC-S1-3 四类型落位：`type=feedback` 缺省落 `global/`、`type=project` 缺省落
      `projects/<可读名>-<hash>/facts/`；显式给 `scope` 时按覆盖落，frontmatter 回读一致
- [ ] AC-S1-4 排除清单生效：一句"我测试过了 / 我改了 store.rs"收束后**不产文件**；
      同一句里"以后都用 pnpm"产 1 个 `type=user` 文件（正反对照同批跑）
- [ ] AC-S1-5 索引硬钳在写侧：造 250 条 → 落盘的 `MEMORY.md` ≤ 200 行且末行带 WARNING 指路，
      注入侧读到的就是这份（不再现算）
- [ ] AC-S1-6 遗忘三条全过：删一条 → grep 数据根该 stem 零命中 → 该层 `MEMORY.md` 零命中 →
      下一会话注入块零命中
- [ ] AC-S1-7 软删可恢复、清空才算数：管理页删除 → 文件进 `obsolete/`、注入零命中、能恢复；
      清空 `obsolete/` 弹确认，取消 = 文件还在
- [ ] AC-S1-8 存量导出幂等：导出脚本跑两次 → 第二次零新增、**人工编辑过的文件 sha 不变**
      （复用 `memory_files.rs:225` 首启导出那套"已有文件一律不动"）
- [ ] AC-S1-9 新判据要报红：给"记忆写路径只许一个出处"这条判据做窄变异（把 md 写退回
      sqlite 权威那侧）必须红；红不出来 = 这条判据零证明力，不许留着
- [ ] AC-S1-10 `make check` 全绿；涉界面那半（三枚开关 + 每层一行）在真实窗口自截图核对后再给你看

## 非目标

- 不动注入面形状（`<persistent_memory>` 换成两段索引指针行 = S3 的事，这一刀只把它的**数据源**
  从 sqlite 摘掉）
- 不删 organize 管线、不退役 memory-sqlite 插件、不动树（S4）；`memory_tree_*` 八枚命令不动
- 不做回合收束的增量抽取（它要 LLM 通道，与压缩同层，排在 S2 之后单立一批）
- 不做 embedding 检索、不做应用内记忆详情视图（看文件 = 「打开文件夹」）
- 不为旧数据做兼容承诺：sqlite 存量一次性导出，之后原地保留不读不删；
  删除 / 覆盖 `~/.Locowrk` 任何目录前**单独点头**（AGENTS §5）
- 不顺手把工具从插件搬进宿主（理由见刀③ 末段）

## 影响范围

| 文件 / 目录 | 改动 |
| --- | --- |
| `crates/plugin-api/src/memlayout/{mod,sync}.rs` | 刀①：type 枚举、render 加字段、索引写侧硬钳（末尾追加、不动既有签名） |
| `crates/plugin-api/src/memlayout/tests.rs` | 四类型落位、钳、Why 段的用例 |
| `desktop/src-tauri/src/memory_files.rs` | 刀①③：写语义从"同步"改"落盘"；`obsolete/` 移入与清空 |
| `desktop/src-tauri/src/memory/{store,crud,recall,reconcile}.rs` | 刀②：`warn` → `Err`；读路径改 md；学习列停读 |
| `desktop/src-tauri/src/memory_index.rs` | 刀②：摘掉 sqlite 兜底档（md 用不上 = 该层就是空，如实空） |
| `desktop/src-tauri/src/commands/memory.rs` | 刀③：列举/删除/清空读源换 md；新增层读数与 `obsolete` 清空两枚薄命令 |
| `plugins-src/memory-sqlite/src/{lib,store}.rs` | 刀③：内部改调 md；**工具名与参数面不动**；插件注册面不摘 |
| `desktop/src/components/MemoryPage.vue` + `lib/api/memory*.ts` | 刀③：三枚行为开关 + 每层一行；文案中英成对 |
| `docs/design/authority/Memory File-ization Design.md`（**入库件**） | 它现在的身份是「v1.0 草案 · 待用户裁定后生效为规范」，而 §8 推荐的正是双层双写 = 被 ADR-0007 否掉的那条。刀① 前先把「md = 唯一真源」写进这篇并升版（先改标准再写码） |
| `scripts/check-single-source.mjs` + `scripts/tasks.sh` | 新判据一条（写路径唯一出处）+ 它的窄变异 |

## 风险与回滚

- 风险①：刀② 扳真源是**语义反转**，不是加机制——旧形状"md 写失败只 warn"可能从来没被依赖过，
  也可能有（导出/人改保护/对账三处都读 md）。动手前先把 `md_write` 的三个调用点各自的失败后果
  读一遍，别把"改成拒绝"说成"改成允许"（这仓踩过的那条）。
- 风险②：动用户数据根。开工前置 = 把 `~/.Locowrk/memory` 与 `memories.db` 复制进备份目录；
  导出脚本对已有文件不动（现状已是幂等，沿用它）。
- 风险③：`MEMORY.md` 从"注入侧现算"改成"写侧落盘"后，人改索引 = 直接改注入面。这正是要的
  （人编辑权威），但 mtime 进预取缓存键这条得实测一次（现有关键 `(session_id, scope_id)` 不含
  mtime ⇒ 大概率要加，属 S3 的接线，刀② 先只登记不偷偷改）。
- 回滚：三刀各一提交；`memory.files=false`（→ env `LOCOWRK_MEMORY_FILES=0`）是现成开关，
  刀② 之后关它 = 只剩 md、不写 sqlite（语义要写清，别让"关"字含糊）。
- 挂账：方案稿 A① 那张布局图按上面待定项 1 的结论订正；`data_root` 两份实现（plugin-api 权威 +
  宿主一份）是否并成一个出处，另立条目，不塞进 S1。
- 挂账（取证时抓到的版本号漂移）：`desktop/src-tauri/src/memory_files.rs:1` 的模块注释引
  《Memory File-ization Design **v1.1**》，而 `docs/design/authority/Memory File-ization Design.md`
  正文版本行写的是「**v1.0 草案**（2026-05 口述 → 设计稿，待用户裁定后生效为规范）」，全篇 grep
  不到 `v1.1` 字样 ⇒ 代码引了一个文档里不存在的版本号。刀① 改那篇标准时一并把注释的版本指回
  真实文件与真实版本号，之后版本只许有一个出处。

## 参考

- [`docs/design/Memory and Context Redesign v1.md`](../../docs/design/Memory%20and%20Context%20Redesign%20v1.md)（方案稿 ⊘ 不入库）
- [`docs/adr/0007-…llm-compaction-primary.md`](../../docs/adr/0007-memory-flat-files-context-llm-compaction-primary.md)
- [`specs/01-memory-context-redesign/spec.md`](../01-memory-context-redesign/spec.md)（阶段表与 AC-4/5/6/7/9）
