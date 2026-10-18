# 05-记忆数据面 S1（把 md 从人读镜像扳成唯一真源）

- 状态：**进行中（刀①②a②b-1②b-2②b-3 已落地 / 刀②c 待做，卡一次存量回填的点头）**——本文件是 [`specs/01`](../01-memory-context-redesign/spec.md) 阶段表里
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

## 四处落点（2026-10-18 决策人已裁定：四条全按推荐案，"准备开刀"）

1. **布局照现状**：方案稿那张图原先画的是"全局事实与 `MEMORY.md` 直接住 `~/.Locowrk/memory/` 根，
   项目层 `projects/<项目-id>/`"；实况多两节（全局在 `global/` 子目录、项目事实再套一层 `facts/`、
   根目录还坐着 `tree.md`）。裁定 = 照现状，索引与事实分层、根目录不混投文件；方案稿 A① 已同步订正。
2. **四类型进已有的 `metadata.type`**（现在恒 `"fact"`），`tags` 保留当自由标签不动 ——
   旧文件读回 `type=fact` 视为"未分类"，不重写存量。不拿 `tags` 当 type：tags 多值、type 单值，混用必漂。
3. **type → 层 的映射做成一个纯函数**（`user` / `feedback` 缺省全局、`project` / `reference` 缺省项目，
   显式 `scope` 参数可覆盖），模型只报 type、不自己算目录。
   **落点订正**：我原先写"放宿主侧"是错的——记忆工具有两条通路（宿主 `MemoryStore` 与
   `plugins-src/memory-sqlite` 插件），插件不依赖宿主，放宿主 = 插件那条路算不出落点。
   放在 `plugin-api/src/memlayout/`（宿主与插件共用同一份布局实现，这正是它存在的原因）。
4. **尺寸硬钳搬到写侧**：现状钳只在注入侧现算、落盘的 `MEMORY.md` 无上限。裁定 = 搬到
   `refresh_index` 写侧（超限即截断 + 末行写 WARNING），人打开看到的就该是模型看到的那份
   （同一物理定律见方案稿 F 节）。代价如实登记在风险③：人改索引 = 直接改注入面。

另有一条不提问、直接按规矩办：**动 `plugin-api` 只往结构体末尾追加字段**，
`FactEntry` 不进 C ABI（Rust 侧结构），`PLUGIN_ABI_VERSION` 不动 —— 刀① 用一个
"追加字段后既有测试一字未改仍绿"的实证来担保这句话，不靠我说。

## 施工拆法（每刀独立可回退、一提交）

- **刀①（✅ 2026-10-18 已落地）**：索引硬钳搬到**写侧** + 预算数字收成一处出处。
  `memlayout` 新增 `MAX_INDEX_LINES` / `MAX_INDEX_CHARS` / `INDEX_TRUNCATED_NOTE` 与 `clamp_index`，
  `refresh_index` 落盘前按预算截（行尾换行与头部都算进字符预算，只丢整行不写半行）；
  宿主 `memory_index.rs` 两份 `const` 换成一条 `pub(crate) use` 再导出。
  判据 = 新增 `SS10` / `SS10b` 两条同源对账（基线与口径的唯一出处 = 闸自己的输出行，本篇不钉）；
  测试 = `memlayout/tests.rs`
  三条新用例，**既有 66 条一字未改仍绿**。sqlite 写路径一行未动 ⇒ 仍是旧形状，随时可停。
- **刀②a（✅ 2026-10-18 已落地）**：`memlayout` 长出**读回面**——新建 `read.rs`（`StoredFact` /
  `FactsScan` / `read_fact` / `iter_facts`，排序与现役 SQL 逐字同形 `updated_at DESC, key`），
  `tags` 升成 frontmatter 一等字段（`render()` 写、`parse_fact` 读；升级前的旧文件缺这行 = 读回空串，
  不是错，也不得因此造一份"人工编辑备份"）；`mod.rs` 涨到 405 行 ⇒ 索引面拆进 `index.rs`，
  消费点经 `pub use` 再导出**一字未动**。
  **判据随拆分订正（这条红过一次是真红）**：常量搬到 `index.rs` 后 `SS10b` 左值 2→0——写死单文件的
  扫描面在目录式模块面前会瞎。改法是让它吃 **memlayout 子树**（动态取、排 `tests.rs`，再加文件不必改判据），
  宿主"是不是吃再导出"那半独立成 `SS10c`，三档各配一条窄变异；下限另加一条"子树面非空"，
  它在本刀当场抓到过我一次漏 `.map(read)`（面只剩路径串 ⇒ 判据会读成"定义丢了"）。
  sqlite 读写两侧一行未动 ⇒ 这一刀只添面、不改行为，随时可停。
  两处顺带撞出来的事实：**① `tests.rs` 涨到 565 行撞 400 纪律**（三问第 1 问答"否"：它同时管
  寻址/渲染/写删同步/索引预算/读回五件事）⇒ 按面分簇拆成 `tests/`（`layout` / `render_parse` /
  `write_delete` / `index_budget` / `read_back`），用例本体逐字搬、**73 条一条没多没少**（拆前拆后
  同为 73 passed 就是"只搬不改"的判据）；搬完 `tmp/mut-clamp.mjs` 里那些 `memlayout::tests::xxx`
  过滤串当场失效——全名多了一段 `tests::<簇>::`，cargo 打的是 `0 passed; 0 failed` 而 rc=0，
  会被误读成"变异打空"，故给变异器加了"选择器打空 = 对照无效"的自检并**重跑 5–8 四条确认仍红**。
  **② `GLOBAL_DIR` 请客没请到底**：本刀刚给 `mod.rs` 写上"`"global"` 字面量只许一处"，
  同文件 `fact_rel_path:209` 还硬编码着第二处 ⇒ 已改为吃常量；宿主 `memory_reconcile.rs`
  另有两处（该模块在 v1.1 里整项作废、待退役）跟着一起收，本刀不顺手搬。
- **刀②b（读迁移 · 扳真源的证据所在）**：**宿主侧**读路径改**只读 md**，把 sqlite 兜底那档摘掉。
  **边界订正（本刀收尾时定，防两刀抢同一件事）**：插件那六枚消费点（read / list / search / undo）
  **不在本刀**——它们是"工具内部改道"，归刀③；本刀只搬宿主读面。判据落在改动面上：
  刀②b 之后 `desktop/src-tauri/src/memory*/` 里不许再有对 `memories` 表的 `SELECT`
  （写侧仍在，那是刀②c 的事）。
  开工后发现它内部还得分四级（按"改了会牵动谁"分，不是按文件分）：
  - **②b-1 管理页列举 ✅（2026-10-18 落地）**：只有 `commands/memory.rs:23` 一个消费者，
    且**不碰注入字节** ⇒ 聊天面零风险，最适合作为第一块多米诺。`files_face()` 收成一处
    （原先 `(Some(files), true)` 那份判断散在 `md_write` / `md_delete` / `md_index_body` 三处），
    `list()` 先问人类层；新簇 `memory/tests/md_read.rs` 四条 + 五发窄变异各打中一条。
    留着一档兜底（未接线 / 开关关 ⇒ 仍走 SQL）是**有意的暂时状态**，测试里那条
    "墓碑断言"就是它必须被刀②c 删掉的凭据。
    **本刀撞红两条、根因是观测点错位而不是实现**：`memory_reconcile/tests.rs` 有五处把
    `store.list()` 当"库里有哪几行"的代理，读源一翻就失真——一条比出目录反查名
    `proj-a-67ed0df8` 而权威键位是 `F:/ws/proj-a`（md 侧只能给目录名），一条比出 0 行
    而库里明明还留着 1 行（文件删了、行按"目录缺席不删行"保留）。处置 = 把眼睛挪回
    `export_rows()`，**断言语义一字未动**，并把"不许换成 list"写进该文件头注（放宽断言
    或删测试都是错的方向）。同族口径还散在 `organize/facts/tests.rs`（4 处）与
    `organize/tests/fact_cards.rs`（6 处）：它们用的是未接线 store，这刀没红，
    但"验落库却读管理页"迟早同样失真 ⇒ 登记待收，改法与这里一样换 `export_rows()`。
    五发窄变异各打中一条（md 分支撤掉 / 折叠大小写撤掉 / tags 那一维撤掉 /
    总开关档撤掉 / limit 钳撤掉）。第一版那条"md 分支撤掉"写的是 `if false {`，
    红在**编译错误**上、零证明力 ⇒ 改成 `None::<&MemoryFiles>` 才是"读源没换"那个
    缺陷形状的真红（这条教训与本仓"一句话机制描述不许直接翻译成变异代码"同源）。
  - **②b-2 召回正文 ✅（2026-10-18 落地）**：`recall.rs::recall_quiet` 取全局层那条 SQL 换成
    `iter_facts` ⇒ **`<persistent_memory>` 就此停读 sqlite**。三件机械后果一并处理掉：
    ① 两条读源共用一个读数形状 `RecallOutcome{rows, n_candidates, partial}`，埋点口径因此
    只有一个出处；② "不许冒充源是空的"那条纪律抽成纯函数 `inject_loggable` 并扩到
    "读数不完整同样不记"，`warned` 闸门收成 `warn_once` 一处（两类降级各写一份 `swap`
    的话第二类会被第一类永久静音）；③ `StoredFact → MemoryRow` 的装配收进 `shapes.rs`
    的 `From`（原先 `crud::md_rows` 内联一份，召回再来一份就是同一件事两个出处）。
    证据 = 把 `memory.sqlite3` 改名后召回照常出全局层两条、项目层那条依然不注入、
    limit 照样钳；埋点那半**落成磁盘行数**（`memory_inject_log` 先记一行、partial 那轮不记第二行）。
    踩到的两处：Rust **不许在 `impl` 里定义类型**（第一版把 `RecallOutcome` 放进 impl，
    `cargo fmt` 直接拒绝解析）；替换 `.take(limit)` 那行时我把它整行丢了，
    补一条"limit 在召回支路也生效"的断言正是这种丢法的红证。
    **两条"验证器自己错"当场抓到（都写进代码注释留痕）**：① 变异 15（`partial` 恒 false）
    第一次跑**打空**——那组用例两轮都用 `None` 会话，而 `ux_mil_inject` 的唯一键是
    `(session_key, scope_id)`，第二行被 `INSERT OR IGNORE` 吃掉 ⇒ "仍然只有一行"在两种
    实现下都成立。改法 = 两轮各用一个真实会话 id，并加一组**正对照**（同两轮、无扫不动
    条目 ⇒ 必须记到第二行），否则那个"= 1"也可能只是埋点通路整条没接上。
    ② 变异器的"选择器打空"自检第一版判据是"输出里出现 `0 passed; 0 failed`"，而 cargo
    会为 `src/main.rs` 那个零测试 target **恒打**这一行 ⇒ 每条变异都误报；改成把所有
    `test result:` 行的 passed+failed 加总、总和为 0 才叫打空。
  - **②b-3 项目索引 ✅（2026-10-18 落地）**：两档的关系**反过来**而不是删掉一档——
    `project_index` 现在只看"人类层有没有接线"来决定走哪档，md 里没指针行就是**这个项目
    确实没记过**，如实缺席，不再去库里补一份已不再是真源的副本（`index_rows` 那一档
    剩下的唯一资格 = 未接线/降级启动，由刀②c 取消）。
    **原先写的"`TRUNCATED_NOTE` 退役"是错的措辞，这里订正**：它不是消失，而是**值改成
    写侧那一句**（`memlayout::INDEX_TRUNCATED_NOTE`），两支共用一串 ⇒ 文案合一、
    注入侧仍保留预算保护（真源 = 文件，而文件正是人会去改的东西，钳必须站在通路上）。
    动手时抓到一个**先前就存在的静默缺陷**：`pointer_lines_from_md` 只收 `- ` 开头的行，
    写侧那句 WARNING 会被剥掉 ⇒ 索引被截断时**人打开文件夹看到"还有更多没进来"，
    模型拿到的却是一份看起来完整的索引**。现在 from_md 认出那一行、把标记并进取决条件
    （`clamped || truncated_by_file`）。四条窄变异各打中一条（并集撤掉 / md 支预算撤掉 /
    档序没反过来 / 文案退回自造一份）。
  - **②b-4 语义召回**：`source.rs::recall_relevant` 自己开连接取行做向量匹配，还带学习列
    （`hit_count` / `last_hit_at`）⇒ 与"学习列停读"是同一件事，**推给刀②c**，
    不在本刀偷偷做（那要连 `memory_learn` / `recall_log` 一起改，混进来就回滚不掉）。
  **顺带消掉一处暂时状态**：注入侧现在还在自己算截断（算法与写侧的 `clamp_index` 各一份），
  它改读落盘的 `MEMORY.md` 之后这两份合一，数字与文案都不可能漂——文案也是两份
  （注入侧 `memory_index.rs` 的 `TRUNCATED_NOTE` 与写侧 `index.rs` 的 `INDEX_TRUNCATED_NOTE`），
  读迁移那一步前者应当退役而不是留着各说各话。
- **刀②c（存储退役）**：`md_write` / `md_delete` 的 `warn` 改成 `Err` 上抛（含取消
  `files: Option` 与 `files_on` 的"未接线 = 纯 sqlite"兜底档），`hit_count` / `last_hit_at`
  学习列停止读写（文件制没有时间信号以外的权重，v2.136 那条教训；牵动 `memory_learn` / `recall_log`，
  可能得推给 S4），`md_delete` 的 no-op 判据按现状画像第 4 条反过来。
- **刀③（四类型打通 + 工具内部改道）**：`FactEntry` 末尾追加 `ty`、`render()` 写
  `metadata.type`、`ParsedFact` 读回 type（未知值 = 未分类，存量不重写）、type→层映射纯函数、
  `feedback`/`project` 缺 `Why:` / `How to apply:` 时提醒不拦写；这几件必须与工具参数**同刀**，
  否则枚举值没有任何生产者。save / read / search / delete 四枚工具内部实现从 sqlite 改 md
  （名字与参数面不动，提示词与审批面才不会跟着漂）。
- **刀④（`obsolete/` 与管理页）**：软删 + 清空真删（清空要确认）；管理页六枚命令读源换 md、
  加"三枚行为开关 + 每层一行"。**TreeBoard / TreeSection / InjectSection 三处嵌入不在本阶段动**
  （看板整体删除是 S4 的事，混在一刀里回滚不掉）。

拆刀订正的理由留在仓里：**原先那份刀① 想"先补 type 枚举 + frontmatter"，但 type 的值只能从
工具参数来——只加枚举没有任何调用能写出 `user` / `feedback`，挂在那里就是一圈死 API**。
所以四类型挪到刀③ 与工具参数一起做，刀① 只留有真实消费者的那块。

工具归属那条（开放决策 5 说记忆工具归宿主内置）在刀③ 只做到"内部改调 md"，**不把插件那 6 枚
搬进宿主**——搬 = 摘插件注册面 + 动 SS7 十档对账，按禁区相邻规矩单独说明、单独点头。

## 刀② 前置取证（2026-10-18 已读到消费端，动手前的事实）

风险①要求"动手前把 `md_write` 各调用点的失败后果读一遍"，读完的五条：

1. **两条写路径的失败语义本来就不一样**（这条决定刀② 要动几处）：宿主管理页通路的
   `md_write` / `md_delete` 返回 `()`、内部 `if let Err(e) = … { tracing::warn!(…) }`
   （`memory/store.rs:99-125`），四个调用点（`memory/crud.rs:98`/`:145`/`:159`/`:182`）**看不见失败**；
   插件工具通路的 `sync_write` / `sync_delete` 返回 `Option<String>`，六个消费点
   （`store/tools.rs:63`/`:166`/`:189`、`store/undo.rs:36`/`:57`/`:86`）把它塞进工具输出的
   `md_warning` 字段 ⇒ 旧形状是"部分成功 + 如实带回警告"，**不是静默**。扳真源后两边收敛成
   "md 落盘失败 = 操作失败"，`md_warning` 这个字段从此不该再出现。
2. **旧形状有测试锁着**：`plugins-src/memory-sqlite/src/tests/human_md_layer.rs:23` 断
   `out.get("md_warning").is_none()`（"双写成功不带回警告"）。新语义下它照样绿，但**成立的理由变了**
   （字段根本不存在）⇒ 不许一删了事，要补一条反向用例"故意让 md 写失败 → 工具返回 Err 且输出里
   没有成功标记"，让这条判据继续承重（AGENTS §6.3：不许为了过闸删测试）。
3. **两段 SQL 写的列集合不一致**（真源翻转后会以"文件与库不一致"的形式暴露）：宿主 `update_value`
   走 `UPDATE memories SET value, updated_at`（`crud.rs:75-77`，不动 tags / source）；插件
   `memory_write` 走 `INSERT … ON CONFLICT DO UPDATE SET value, tags, updated_at`
   （`tools.rs:53-56`，**不动 source**）。刀② 之后这两段一起下线、分歧自然消失，但
   **导出脚本必须按文件侧字段并集取数**（value / tags / source / updated_at 四件都要进
   frontmatter 或正文），否则导完就丢信息。**这条已由真实数据根坐实（2026-10-18 只读探针）**：
   库里 10 行、md 层 10 条事实，键位集合两侧**完全一致**（只在库里 = 空、只在 md = 空），
   `source` 与 `updated` 逐条相等 ⇒ 双写一直有效；但 **10/10 条 md 都没有 `tags:` 行**
   （那一行是刀②a 才进渲染的），而库里 10 条全带标签 ⇒ 现在就切读源，管理页标签列整列变空、
   按标签搜索全部落空。所以②b-3 之后必须先跑一次**带 tags 的存量回填**（= AC-S1-8 那条导出脚本），
   而动的是他的数据文件 ⇒ 要他点头，不许我顺手写。
4. **`md_delete` 挂在 `if n > 0` 后面**（`crud.rs:158-160`）⇒ "库里没这行" 现在直接当 no-op，
   连文件都不碰。真源翻转后这个判据必须**反过来**：以文件是否存在为准（库里没行、文件有 = 该删掉
   文件）。AC-S1-6 那三条遗忘验证能不能成立，正卡在这一步。
5. 一条实测纠正了我的推断：我原先以为"决策人开了 `.worktrees` 工作树会把结构闸的扫描面污染成
   双份读数"，实测否掉了——`check-single-source.mjs:29-43` 的 `prodRustFiles()` 写死从
   `['crates','desktop','plugins-src']` 三个根 walk，工作树副本不在面内（工作树存在时"生产 .rs
   文件数"读数与工作树之前一致、SS1 仍 `1`、gates rc=0；具体数不落进本篇——那是每刀都会漂的数，
   唯一出处是闸自己的输出行）。残余风险登记在此：那个 walk 只排
   `target|node_modules|dist`，**不排 `.worktrees`**，哪天把扫描根扩到仓库根就会重算一遍。

## 验收标准（跑什么、看到什么）

- [ ] AC-S1-1 写路径唯一：工具或管理页写一条记忆 → 数据根只新增 1 个 `.md` + 索引多 1 行；
      `SELECT COUNT(*) FROM memories` 前后**不变**（sqlite 不再是落点）
- [ ] AC-S1-2 读不碰 sqlite：把 `memories.db` 改名造损坏态 → 管理页列表、`<project_memory_index>`
      注入、`memory_read/search` 全部照常出数（这条是"真源翻转"最硬的证据，也是刀②的收口判据）。
      **已证三格**：管理页列表（②b-1 `listing_survives_the_db_being_renamed_away`）、
      全局层注入（②b-2 `recall_takes_the_global_layer_from_md_with_the_db_gone`）、
      项目索引（②b-3 `a_shell_index_file_means_the_section_is_absent_even_if_the_db_has_rows`
      走的是同一批夹具形状：库里留着行而 md 是空壳 ⇒ 如实缺席）；
      只剩插件那两枚工具（`memory_read` / `memory_search`，归刀③）⇒ 整条暂不勾
- [ ] AC-S1-3 四类型落位：`type=feedback` 缺省落 `global/`、`type=project` 缺省落
      `projects/<可读名>-<hash>/facts/`；显式给 `scope` 时按覆盖落，frontmatter 回读一致
- [ ] AC-S1-4 排除清单生效：一句"我测试过了 / 我改了 store.rs"收束后**不产文件**；
      同一句里"以后都用 pnpm"产 1 个 `type=user` 文件（正反对照同批跑）
- [ ] AC-S1-5 索引硬钳在写侧：造 250 条 → 落盘的 `MEMORY.md` ≤ 200 行且末行带 WARNING 指路，
      注入侧读到的就是这份（不再现算）。**前半已由刀① 实证**（`refresh_index_clamps_at_the_shared_budget`
      等三条 + `SS10`/`SS10b`/`SS10c` 三道判据，四条磁盘变异里撤钳/只钳行数/不写截断行三档都报红）；
      后半由刀②b-3 落地：注入侧与写侧共用**同一份预算常量与同一句截断文案**
      （`MAX_INDEX_LINES` / `MAX_INDEX_CHARS` / `INDEX_TRUNCATED_NOTE`，出处判据 = SS10 三档），
      且文件里那句 WARNING 现在会**透传给模型**（`pointer_lines_from_md` 只收指针行，
      先前会把这句剥掉 ⇒ 截断对模型静默）。残留一条：注入侧仍保留一次同预算的钳制，
      那不是"另算一份口径"，而是真源 = 可被人改写的文件时的保护网（`the_budget_still_bites…`）。
      **整条仍不勾**：两半都只有单测证据，还差真实窗口那一次——造 250 条、打开管理页与
      新会话的注入块亲眼核对
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
| `crates/plugin-api/src/memlayout/{mod,sync,index,read}.rs` | 刀①：索引写侧硬钳 + 预算常量收一处（`index.rs`）；刀②a：读回面 `read.rs` + `render()`/`parse_fact` 补 `tags`；`FactEntry` 只末尾追加字段、既有签名不动 |
| `crates/plugin-api/src/memlayout/tests.rs` | 四类型落位、钳、Why 段的用例 |
| `desktop/src-tauri/src/memory_files.rs` | 刀①③：写语义从"同步"改"落盘"；`obsolete/` 移入与清空 |
| `desktop/src-tauri/src/memory/{store,crud,recall,reconcile}.rs` | 刀②：`warn` → `Err`；读路径改 md；学习列停读 |
| `desktop/src-tauri/src/memory_index.rs` | 刀②：摘掉 sqlite 兜底档（md 用不上 = 该层就是空，如实空） |
| `desktop/src-tauri/src/commands/memory.rs` | 刀③：列举/删除/清空读源换 md；新增层读数与 `obsolete` 清空两枚薄命令 |
| `plugins-src/memory-sqlite/src/{lib,store}.rs` | 刀③：内部改调 md；**工具名与参数面不动**；插件注册面不摘 |
| `desktop/src/components/MemoryPage.vue` + `lib/api/memory*.ts` | 刀③：三枚行为开关 + 每层一行；文案中英成对 |
| `docs/design/authority/Memory File-ization Design.md`（**入库件**） | ✅ 本刀前已改：v1.0 草案升 **v1.1 生效规范**（md 唯一真源、四类型 frontmatter、写侧索引硬钳、§8 决策点 1 撤销双层双写、reconcile 整项作废，带逐条勘误表）；三处既有《v1.1》代码引用就此对齐 |
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
- 回滚：**每刀一提交**（决策人 2026-10-18 定"每轮收尾落一个提交"；提交号只认 `git log`，本篇不抄）；
  `memory.files=false`（→ env `LOCOWRK_MEMORY_FILES=0`）是现成开关，
  刀② 之后关它 = 只剩 md、不写 sqlite（语义要写清，别让"关"字含糊）。
- 挂账：方案稿 A① 那张布局图按上面待定项 1 的结论订正；`data_root` 两份实现（plugin-api 权威 +
  宿主一份）是否并成一个出处，另立条目，不塞进 S1。
- 挂账（刀① 收口时撞见的并发施工，2026-10-18）：本仓**同期有另一条线在写**——
  `scripts/check-i18n.mjs`（+51 行，新增「设置说明文案一行封顶」判据：zh ≤32 字 / en ≤80 字符，
  口径 = 全部 `*Hint` 键）、`desktop/src/components/settings/SettingsRow.vue:49`（`line-clamp-1` 兜底）、
  `docs/design/authority/Front-end Design Guidelines.md` ② 那节（补条款 + 新实测读数，明写"取代原
  51 条 / 最长 84 字"）三件一套自洽，**不是本刀产出、本刀不碰**。当时的僵局是"本刀全绿但不敢提"
  ——一提就把对方当时的闸状态抄进我的提交正文。现已按决策人的"每轮收尾落一个提交"解开：提交正文
  只写本刀改动面内的事实，不抄任何非本刀的读数。
  这正是 `specs/01` 那道开工闸当初写的理由（同仓并发施工会冲突），它被 2026-10-18 的转线令顶掉之后，
  并发本身并没有消失。
- 挂账（一处真隐患）：仓根出现未跟踪 `.pnpm-store/`（实测 162M，`make check` 跑 pnpm 时建在仓内），
  `.gitignore` 没覆盖它 ⇒ 谁执行一次 `git add -A` 就把 162M 提进库。加一行 ignore 即可，
  但那是改配置，等决策人点头；本刀不动。复查（刀②a 收尾）：工作区 `.gitignore` 里
  **已有 `+.pnpm-store/` 这一行**（非本刀产出、仍未提交 ⇒ 本刀不认领也不 `git add` 它）；
  剩下的不是"要不要加"而是"谁来提这一行"。
- 挂账（刀②b-2 复查后**故意不坐实**的一档）：`iter_facts` 的 `unreadable` 只数
  "读得到目录、读不动文件"，某层目录本身读不动时不进账 ⇒ 那种故障下会是"空 + 零计数"，
  看起来像"从没记过"。补法三行（失败后 `metadata().is_dir()` 分类），但**没有任何跨平台
  可造的夹具能让它报红**：POSIX `chmod 000` 造得出，Windows 要动 ACL（`icacls`），
  不可移植。写一段没人盯的守卫比不写更糟——它会让人以为那条纪律已经闭环。
  现实侧的兜底：真出现整层不可用，管理页是**整体空白**而不是少几条，看得见。
- 挂账（刀②a 取证时抓到的保留名盲区）：读侧跳过索引与投影用的是**文件名**
  （`read.rs::is_fact_file` 排 `MEMORY` / `tree` 两个 stem），而写侧的 `key_file_stem`
  只安全化路径字符与 Windows 保留名，**没把 `MEMORY` / `tree` 列成保留键** ⇒
  模型或人真写一条 key=`MEMORY` 的事实，文件落得名正言顺，却永远读不回来（静默失踪）。
  两侧对"哪些名字不许当键位"必须同源：要收的是把这两个 stem 挪进 `key_file_stem`
  的保留名单（写侧就拒绝或加前缀），而不是读侧继续按名字猜。属改写侧行为，本刀只做登记。
- 挂账（取证时抓到的版本号漂移，**本刀已消**）：`desktop/src-tauri/src/memory_files.rs:1` 与
  `crates/plugin-api/src/memlayout/mod.rs:1` 两处模块注释都引《Memory File-ization Design **v1.1**》，
  而 `docs/design/authority/Memory File-ization Design.md` 正文版本行停在「**v1.0 草案**
  （2026-10-05 方向口述 → 设计稿，待用户裁定后生效为规范）」，全篇 grep 不到 `v1.1` 字样
  ⇒ 代码引了一个文档里不存在的版本号。处置 = 把标准升到 v1.1（带逐条勘误表）而不是把注释改回
  v1.0，理由是 `docs/design/Memory-KV Roadmap.md:9` 与 `:174` 也按 v1.1 引它，四处一次对齐；
  根因是「实现先于标准升版」，往后版本只许有一个出处。

## 参考

- [`docs/design/Memory and Context Redesign v1.md`](../../docs/design/Memory%20and%20Context%20Redesign%20v1.md)（方案稿 ⊘ 不入库）
- [`docs/adr/0007-…llm-compaction-primary.md`](../../docs/adr/0007-memory-flat-files-context-llm-compaction-primary.md)
- [`specs/01-memory-context-redesign/spec.md`](../01-memory-context-redesign/spec.md)（阶段表与 AC-4/5/6/7/9）
