# 05-记忆数据面 S1（把 md 从人读镜像扳成唯一真源）

- 状态：**进行中（刀①②a②b-1②b-2②b-3②c-1②c-2③a③b-1③b-2④a④b④c 已落地，另加刀④d
  「对账重建盘上每一层索引」= 真实窗口走查当场抓出来的；2026-10-19 两轮真窗口实测：第一轮
  当场修三处（列表被压成 0 高 / 删完不刷回收区 / 索引不重建），第二轮补 AC-S1-1 与那三枚开关、
  软删/恢复/取消清空的读数，全部见下面两节走查记录；仍欠三件：① 真删那一下与
  "删掉之后下一会话注入块查不到"那两发（**改走不依赖界面的读法**，见走查节第 3 条）、
  ② AC-S1-2 的库改名那一次、③ ②c-2 学习档那一格（等点头）；界面那一侧一条口径订正——
  `MemoryPage` 那两个 tab（记忆树看板 + 事实库）**整体退役删除**，归 S4 退役面，
  本轮起这一页只读不改，见「界面落点订正」）**——本文件是
  [`specs/01`](../01-memory-context-redesign/spec.md) 阶段表里
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
    **进展订正（刀②c-1）**：`organize/facts/tests.rs` 那几处随 `index_rows` 退役已经挪走
    （改吃 `memlayout::facts_of`，断言语义一字未动）；`organize/tests/fact_cards.rs` 那 6 处
    仍是 `list()` 代理——它们验的本来就是"管理页看得见"这一侧，同源所以不是缺陷，
    但**"库里落了几行"这一维至今没有独立证据**，仍登记待补。
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
- **刀②c-1（写侧真源反转 + 读面摘档）✅（2026-10-18 落地）**：这一刀是本线**语义反转的主体**——
  四条宿主写路径从"先写 sqlite（真源）、成功后同步 md、md 失败只 warn"扳成
  "**先落 md（真源，失败 = 操作失败并上抛）、成功后再更新 sqlite 影子（失败只 warn）**"。
  同一句 `if let Err(e) = …` 换了主语：掉哪一份会导致什么，从"镜像旧了"变成"这条记忆没写进去"。
  七件事一起落地，逐件点名：
  1. **真源不是接线而是结构**：`files: Option<Arc<MemoryFiles>>` 与 `files_on` 两个字段删除，
     `MemoryStore::new(dir, …)` 自己按数据根算出 `<dir>/memory`（`MemoryFiles` 只是路径持有者、
     构造不碰盘）。**这一条形状选择省掉了整批夹具改动**——原先估"约 16 处依赖未接线档的夹具要改"，
     实际一处 `MemoryStore::new` 调用点都没动（只有 `wired(dir, on)` 少了那个已经无意义的 `on` 参数）。
  2. **读面只剩一档**：`list()` 里那条 SQL、`recall_once()` 的 sqlite 支路、`project_index` 的
     "未接线 ⇒ 库里现合成"档全部删除；连带退役 `index_rows` / `memory_index::from_rows` /
     `FactLine`（三件都没有生产者也没有读者了）。
  3. **存在性判据按真源**：`update_value` / `delete` / `clear_all` 的判据从"SQL 影响行数"换成
     "文件在不在"（`delete_fact` 的返回值天生就是这个判据）。新用例
     `a_row_in_the_db_without_a_file_is_not_a_fact` 钉住反面：库里留一行而文件不在 ⇒
     列表读不到、编辑返回 false、删除返回 false、清空不计它。
  4. **顺序有独立证据**：`a_failed_md_write_is_an_error_and_writes_no_shadow` 不只断言
     `write()` 报 Err，还断言**影子里一行都不许落**——只报 Err 的话"先写影子、再写 md、
     md 失败也 Err"那种实现照样绿，而那种顺序里库里已经躺着一条用户以为没写进去的记忆。
  5. **层级故障进账**（订正刀②a 那条"故意不坐实"）：`unreadable` 过去只数文件级，
     `projects/` 或某层 `facts/` 读不动时 `facts` 空而计数 0 = 与"从没记过"逐字同形。
     当时说"没有跨平台夹具"——**这个理由被一条更普通的夹具推翻**：在同一位置放一个普通文件
     冒充目录，两侧 `read_dir` 都失败且都不是 `NotFound`。现在层级也进账 ⇒ `partial` 一档
     盖住"整源不可用"与"部分读不动"两种故障，`RecallOutcome` 因此不再是 `Result`
     （留一条恒不 Err 的 `Result` 就是死 API），`inject_loggable` 从三档收成两档并说破理由。
  6. **层枚举与两个清点名收成一处**：`memlayout::layers()`（按磁盘在场枚举，第二返回值 =
     扫不动的层数）、`facts_of(root, ws)`（按键位读一层）、`clear_facts(root)`（清空真源并报数）。
     三者存在的理由同一条：**`StoredFact::workspace` 在人新建的无 frontmatter 文件上只是目录名兜底，
     不是权威原串**——拿它再去算 `fact_rel_path` 会指到别的目录（`clear_all` 逐条删就会"删不到也不报错"，
     这条坑是实现时撞出来的，不是设想）。`organize/facts.rs::existing_keys` 与
     `memory_index::ages_of_layer` 都改吃 `facts_of`。
  7. **补回一处 ②b-3 静默丢掉的功能**：陈旧标注 `（N days old）`。`from_md` 拿到的是纯文本行，
     里面没有 `updated_at` ⇒ 那一维原先只在已退役的 `from_rows` 里算，切读源时就丢了。
     现在由 `ages_of_layer` 现算并追加到行尾（**必须站在注入侧算**：落盘时写死的"3 天前"
     在没人再写它的日子里永远不会自己变陈旧）。配一条负对照（新写的那条不许带标注）与
     一条"对不上号就不标也不丢行"。
  另两处随这一刀发生：`delete_fact` 里**「层不是目录」与「没有这条」在 Windows 上是同一个错误码**
  （`ERROR_PATH_NOT_FOUND`），不特判就得到一条"Linux 报错、Windows 谎称不存在"的跨平台假性质 ⇒
  现在两侧都 Err；`memlayout/mod.rs` 涨到 416 行 ⇒ 寻址面整块拆进 `paths.rs`（逐字未改），
  于是 `SS1b` 那条写死扫 `mod.rs` 的判据第二次撞同一个洞（刀②a 时 `SS10b` 撞过一次），
  按学到的口径改成吃子树面；`legacy.rs` 涨到 481 行 ⇒ 项目索引面拆进 `memory/tests/index.rs`。
  **判据**：新增 `SS11`（兜底档那五个名字在宿主记忆面里**调用/定义/字段三档形状**恒 0）。
  它为什么必须是结构判据：`files_switch_off_keeps_pure_sqlite` 测的正是被本刀取消的那一档，
  而"未接线的 store"在新形状里**类型上就无法构造** ⇒ 编不出等价的行为测试，删掉那条测试之后
  只剩这块墓碑挡得住有人把兜底档塞回来。这条判据自己错过两次、都写进它的说明了：
  第一版按**名字**数 ⇒ 撞上一堆"解释为什么它没了"的必要留痕，得 8 条假红（SS6 早立过同一口径）；
  第二版把 `memory_files.rs` 里 `spawn_initial_export(files_on=…)` 那个**合法的空转开关参数**
  也数进去 ⇒ 得 1 条假红。窄变异 = CASE 12（往 `crud.rs` 塞回一行兜底档调用，闸必须**点名 SS11**）。
  **证据**：CASE 21–29 九发窄变异（层级不进账 / facts 槽被文件冒充不进账 / 删除不区分两种故障 /
  写序退回影子先落 / partial 恒 false / 陈旧标注不追加 / 清空遇到扫不动的层不中止 /
  编辑存在性判据退回影子 / 清空计数退回影子）各打中一条，全部 sha 复核原样还原；
  既有 CASE 1–4（锚随 `index.rs` 拆分改指）与 9/12/14/16/19（锚随兜底档退役重指）逐条复跑仍红。
  ⚠ 其中 CASE 4 是**按预期打空**并已在脚本里标 `expectNull`：改一份共用常量时两侧一起漂，
  行为测试结构上抓不到它，承重的是 `SS10`——把这种打空算成"零证明力"会把责任推给错的档。
  **测试去向**（按 AGENTS §6.3 逐条交代，不静默删）：删 2 条
  （`files_switch_off_keeps_pure_sqlite`、`project_index_falls_back_to_sqlite_when_the_md_is_an_empty_shell`，
  两条测的都是被本刀取消的档，接管者分别是 `SS11` 与 `a_shell_index_file…`）；改写 9 条
  （观测点从"拿 `list()`/SQL 直插当库的代理"挪回真源或 `export_rows()`：`crud_roundtrip` 拆成
  影子迁移 + 真源往返两条、`limit_is_clamped`、`recall_projects_memory_entries_and_degrades_to_empty`、
  `a_broken_database_degrades_to_no_section…` → `a_broken_shadow_db_changes_nothing…`、
  `project_index_is_the_same_keyside…` 钉 md 字面形、learning 簇 4 条换 `export_rows()`、
  `recall_log` 那条等价判据换 `write_fact` 造数据、`initial_export_skips…` 用 `reconcile_upsert`
  造"库有文件无"的存量形状）；新增 10 条。**断言的语义一条没放宽**。
  具体 passed 读数不落在本篇（每批都会漂的数只写提交正文，唯一出处 = 闸自己的输出行）。
- **刀②c-2（反向通路退役 ✅ 2026-10-18 已落地 / 学习档 ✅ 2026-10-20 由 S3 刀③b 收口）**。
  1. **"库 → 文件"那两条反向写入通路已全部停止**（这条不是可选优化，是遗忘验证的前置）：
     `run_initial_export` 的"行在文件缺 ⇒ 补文件"与 `memory_reconcile` 的 `healed` 支
     "文件未动而索引先行 ⇒ 从索引愈合文件"，在真源 = md 之后的含义是**把已删除的记忆复活**。
     刀②c-1 之后 `delete` 只看文件 ⇒ 文件删了、影子没跟上时，下次启动就会按影子把文件写回来
     ⇒ AC-S1-6 那三条遗忘验证不成立。落地形状：`healed` 那一支改成**一个字节都不写**、
     只 `sum.warnings += 1` + `tracing::warn!` 点名（`rows` 里那份值留在原地不动，刀③ 摘影子后
     这一格连"出现"都不可能）；`run_initial_export` 的事实循环整段删除，签名跟着从
     `(memory, tree_store, files, sessions_dir) -> (usize, usize)` 收成
     `(tree_store, files, sessions_dir) -> usize`——**留下一个恒 0 的 `facts` 返回值就是死 API**，
     日志字段（`facts=`）与两处用例跟着换通道。
     ⚠ **一处对上一轮判断的订正**（写在这里是因为它改变刀④ 的范围）：本节原文说过
     "`memory_reconcile.rs` 在 v1.1 标准里本就整项作废、随这一刀退役、连带命令与按钮"——
     **不退**。理由：插件那六枚工具（刀③ 才改道）仍在读 sqlite，所以"文件 → 影子"的对齐在
     刀③ 落地之前是**必要过渡机制**——否则人在文件夹里改的记忆模型根本读不到，那正是本阶段
     要消灭的"另一种双真源"。于是本刀只拆"副本反写文件"这一个方向，命令与按钮留到刀③ 之后
     评估（影子一旦没有生产者，对账自然无处可对齐）。
     **判据**：新增 `SS12`（对账 / 启动导出面里"影子 → 文件"反写：调用档 0、转发档 2；
     两档一起报，`0|0` 那种"两边一起消失"读法兜不住面瞎）。它与两条行为测试的分工写进了判据
     说明，也写进了变异设计：`CASE 30` **故意长成一条没人调用的反写函数**——行为测试对它全绿
     （不在任何调用路径上），只有秒级档看得见（`make quick` 不跑 cargo），这一档才是 SS12 独有的；
     `CASE 31`（healed 支塞回）与 `CASE 32`（导出多写一条事实文件）各打中一条行为用例，双证。
     **测试去向**（AGENTS §6.3）：`stale_file_is_healed_from_index` **反转**为
     `a_shadow_ahead_of_the_file_is_reported_but_never_written`（正向"文件被愈合"断言换成反向
     "字节相同"断言 + 重跑一轮不许升级 + 一条内联负对照：mtime 推进后走的是"文件为准"那格，
     证明命中的确实是"影子先行"）；`initial_export_skips_existing_files_and_regenerates_trees`
     **改写**为 `initial_export_rebuilds_tree_md_and_writes_no_fact_files`（只验投影半区，
     真源侧断言 = 磁盘上事实文件计数不增；计数按**文件名**数而不猜目录名算法，
     因为事实落点由键位串决定、投影落点由 `TreeStore` 里存的 workspace 决定，两者不保证同串）。
     **一条登记**：`memory_reconcile.rs` 里 `"global"` 字面量仍有两处（第 85 行收集、第 166 行
     删除判定），而算式唯一出口 `GLOBAL_DIR` 在 plugin-api 里是 `pub(crate)` ⇒ 宿主拿不到。
     收口要先把那个常量升面，与本刀无关，留在这里等下一次动寻址面时一起做。
     连带的前端读数（`healed` 从 `ReconcileSummary` 六处一起退役：Rust 字段 / `has_action` /
     启动日志字段 / TS interface / `DeveloperSection.vue` 插值 / zh+en 两枚文案）只删插值、
     不动形态；**开发者分区那两行读数随刀④ 一起截图核对**。
  2. **学习档仍读 sqlite**（`source.rs::recall_relevant`，settings `memory.learnEnabled` 打开时才走）：
     它是宿主读面里最后一条按库寻址的通路，也是 ②b-4 推过来的那一格。**不能顺手改**的理由有两条，
     都写进代码注释留痕：① `hit_count` / `last_hit_at` 这两个学习量**不能进 md frontmatter**——
     文件的 mtime 是"人在整理之后动过它"的唯一信号（`write_tree_md` 的人改保护、AC-S1-8 的
     "人工编辑过的文件 sha 不变"都挂在这一维上），机器每交付一次就写一次文件会永久摧毁它；
     ② 把它改成"候选集取 md + 去掉强化信号"是一次**排序口径变更**（四信号里 frequency 那一维失效），
     属产品决策而不是机械迁移 ⇒ 要决策人点头（要么关掉学习档、要么给学习量另找一个不进人读面的家）。
     在这一格收口之前，"`<persistent_memory>` 停读 sqlite"这句话只在 `learnEnabled=false`（缺省）时成立，
     代码头注与本节都按这个口径写，不许说满。
     > ✅ **本格的裁定结果（2026-10-20 · S3 刀③b）**：决策人选的是**第三条**——整族退役
     > （`specs/10-context-injection-s3` 开工三问②「照推荐案」）。`recall_relevant`、
     > `memory_learn/` 一族、`learnEnabled` 与四枚权重、`breakdown` 回执与解释面板全部删除，
     > 宿主读面至此**一条 SELECT 都不发**；上面 ① 那条"机器写文件会摧毁 mtime 信号"的顾虑
     > 随强化回路一起消失，②那句"排序口径变更要点头"也到此为止。
     > **两件遗物留着**：`memories` 的 `hit_count` / `last_hit_at` 两列与
     > `memory_inject_log.breakdown` 一列——在场、无人读写，去处随 S4 的 sqlite 面一起裁。
     > 复活由判据 `SS29`/`SS29b`/`SS29c` 三档机械挡住。本节上面那些当年的理由**不改写**
     > （那是刀②c-2 当时的事实），订正记号只加在这一条末尾。
- **刀③a（四类型打通 + 工具参数面）✅ 2026-10-18 已落地**，六件事：
  1. `FactEntry` 末尾追加 `ty`（`plugin-api` 的规矩：只往结构体尾部加字段）；`render()` 写
     `metadata.type`，值由 `normalize_type` 收档 ⇒ **没给类型与写 `fact` 同形**，
     所以宿主 598 条与插件既有 28 条用例**一字未改仍绿**（这就是"加档不是改档"的实证）。
  2. 新建类型面 `memlayout/kind.rs`：`MEMORY_TYPES` / `UNCLASSIFIED_TYPE` / `normalize_type`
     （值域外与大小写、空白漂都收进未分类）/ `Layer` / `layer_for`（纯函数算落点）/
     `missing_guidance`（提醒文案点名缺哪一段，三种缺法三条文案）。
  3. `ParsedFact` 加 `ty`（`parse_fact` 认 `  type:` 那一行）、`StoredFact` 加**已收档**的 `ty`
     ⇒ 消费侧可以假定值域只有五种，不必各自再判一次。旧文件没那行 = 读成未分类且
     **读一次不许改写文件**（回填存量 = 摧毁人的 mtime 保护，那条用例直接断言字节相同）。
  4. **工具参数面**（四类型的生产者，必须与枚举同刀——否则挂在那里就是一圈死 API）：
     `memory_write` 加 `type` 枚举参数，描述与 schema 一起改，磁盘 `manifest.json` 由
     `LOCOWRK_MANIFEST_SYNC=1` 那条维护通道对齐（T11 同源对账随后复绿）。
  5. **落点由类型算**：`write_scope` 在"没给 scope"那一档改问 `layer_for(ty, None)`。
     未分类 ⇒ `Project` ⇒ 与四类型之前逐字同形；`user` / `feedback` ⇒ 全局层，于是未归属
     会话多了一条**正当**出路（报 `type="user"` 说的是"这是全局偏好"，不再需要假装成
     `scope` 参数）。值域外的 `scope` 照旧直接 Err，两档判据不许互相掩盖。
     `Why:` / `How to apply:` 只提醒不拦写：先落盘，缺的段数以 `hint` 字段带回（两段齐 = 无 hint）。
  6. 顺手收的一处形状：`MemoryFiles::write_fact` 从六个平铺参数改成收 `&FactEntry`——
     加一个字段就要改一遍每层签名，那次改动在宿主侧的落点本来可以有 `write_md` 一处。
     插件侧的 `mdsync::sync_write` **没有**跟着收（它整个随刀③b 消失），它改的是
     `ty: Option<&str>`：`None` = 调用方不知道分类（undo 的载荷取自库行，库里没这一维）
     ⇒ 沿用现存文件里的 `type`，否则一次 undo 会把四类型抹平成 `fact`。
  判据：本刀不加新闸（四类型是**加档**，没有需要墓碑的退役形状）；AC-S1-9 那条属于刀③b。
  **证据 = 六发窄变异**（`CASE 33–38`，各打一条用例、全部 sha 逐字节还原复核一致）：
  未分类被算成全局层 / 值域外的 type 不收档 / 提醒不挑类型 / 工具不带回提醒 /
  `render` 把 type 写死回 `fact`（四类型没有落盘通路）/ undo 把分类抹平。
  ⚠ `CASE 38` 第一版**打空**过一次，打的不是代码而是脚本自己的过滤串（用例名抄错 ⇒
  cargo 报 0 条仍然 rc=0）——是"命中用例数 = 0 就叫停"那条自检把它拦下来的，
  否则这条会被记成"零证明力"而实际上一行都没跑。刀③b-1 之后它的锚搬了位置
  （`store.rs` 里"载荷没 type 就去读现存文件"那一支随 mdsync 消失 ⇒ 改指
  `store/undo.rs::payload_type`），复跑仍红、命中 1 条。
  测试去向：`memlayout/tests/kind.rs` 新簇 6 条（同形担保 / 四类型逐个往返 / 值域外两侧收档 /
  旧文件无那行且不回填 / `layer_for` 十三档 / 提醒只咬两个类型）+ 插件 `tests/type_layer.rs`
  新簇 7 条（无类型照旧落项目 / user+feedback 落全局且不留第二份 / 显式 scope 两个方向都赢 /
  未归属会话被类型救场 / 提醒不拦写 / 撤销把载荷里的分类一起还原 / 值域外不落进真源文件）。
  ⚠ 本刀踩到的一条**夹具脆弱性**（登记，未修）：插件测试簇共用 `ENV_LOCK`，一条断言失败时
   panic 线程带着锁下去 ⇒ 同批后面每条拿到的都是 `PoisonError` 而不是它自己的结论。
  我自己刚踩了一次（feedback 落全局却断言项目层文件在场 ⇒ 三条红，其中两条是被污染的），
  所以那种"一片红"永远要先读第一条的 panic 行再下结论。
- **刀③b-1（插件六枚工具改道 md）✅ 2026-10-18 已落地**。
  工具面按 `lib.rs` 实况是**六枚**（`memory_write` / `read` / `list` / `search` / `delete` /
  `undo`），本节原文写的"save / read / search / delete 四枚"漏了 `list` 与 `undo`——它们
  同样按库寻址，只改四枚会留下半改状态。名字、参数面与返回字段名一律不动，
  所以**既有 31 条接口用例一字未改全绿**（双层纪律、undo 契约、分页与 total 口径、
  大小写与通配符字面量这些都在里面）——这就是"换存储不换契约"的实证。
  新增 `store/facts.rs`（真源读件 + 键位裁决）承担原来 `rows.rs` 的职责，两处形状值得点名：
  ① `Hit { ws, fact }`——从磁盘读到内存的第一站就把"我问的是哪一层"带在身上，因为
  `StoredFact.workspace` 在人新建的无 frontmatter 文件上是**目录名兜底**（第四处 hint 陷阱），
  拿它当 undo 的还原目标会把记忆写进一个不存在的键位；
  ② 列举的排序仍是 `ORDER BY key` 的**字节序**（大小写敏感），与搜索那条
  `updated_at DESC, key` + 折叠大小写不是同一口径——照抄退役掉的 SQL，不顺手"改对"。
  退役清单（都写在 `store.rs` 头注里，免得下一轮再被问"怎么没了"）：`rusqlite` 依赖 ·
  `store/schema.rs`（建表 + 两份旧库迁移）· `rows.rs` 的 SQL 件 · `mdsync` 那层同步与它
  带回的 `md_warning` 字段 · 宿主两处 env 注入（`state/boot.rs` 与 `commands/settings.rs`）。
  **`memlayout::files_enabled` 本身留着**：它是 `plugin-api` 的既有对外签名，删除属
  AGENTS §5 禁区 ⇒ 只把注释改成"自刀③b 起无人调用"并挂账（要退役得先向人申请）。
  两处连锁按预告落地：① `check-schema-parity.mjs` 从"两侧逐字节相同"改成**宿主单侧形状**
  ——A1/C1/D1/D2 那四档"两侧比"没有对象了，摘掉；留成 H1–H7 七档（学习两列在场 / 两条
  ALTER 原文在场 / 建表字面量一个出处 / 迁移四片段 / open 调一次 / 顺序 workspace 在前 /
  SET 不含学习列），`--selftest` 七档变异全可见。脚本**没改名**：改名要动
  `.github/workflows/ci.yml` 的步骤名，那是禁区。② 学习档 `recall_relevant` 仍读
  `memories` 表，所以本刀的形状是"表留着、由宿主 reconcile 与 shadow 维护"，
  **代价如实登记**：插件写的记忆不再即时进影子，要等下一轮对账 ⇒ `learnEnabled=true`
  时语义召回看到的集合会滞后一次（缺省关，且那一格本来就等你拍板）。
  撞出来的三个真问题（前两条是代码问题，第三条是测试耦合）：
  ① **一条合法 key 写不进去**：工具参数面允许 key 到 512 字节（`KEY_MAX`，写给模型的契约），
  而 Windows 的路径上限 260 会咬长文件名——实测 512 个 `k` 得到 `os error 123`。
  由布局吸收而不是降接口上限：`key_file_stem` 现在按 `STEM_MAX_CHARS = 100` 收口，
  手法与 `project_dir_name` 同源（可读前缀 + 全串 fnv 短哈希，确定性、不撞、按**字符**
  切不按字节切）。红证两条：`layout.rs::an_overlong_key_stem_is_bounded_and_still_distinguishes`
  （有界 + 前缀相同尾部不同 ⇒ 两个文件）与
  `write_delete.rs::a_key_at_the_interface_ceiling_round_trips_through_the_disk`
  （**真的落一次盘**——长度问题住在文件系统，不住在算式里）。
  ② **一条讲排序的用例每隔若干次全量跑假红一次**：`recall_takes_the_global_layer…`
  断言"同秒按 key 升序"，而它的夹具走 `store.write` 取墙钟 ⇒ 两次写入跨过秒边界时前提就没了
  （实测抓到一回，报 `[pref.style, pref.lang]`）。改成经 `memlayout::write_fact` 把
  `updated_at` 钉死，顺手把"库不在场"那半档换成更强的夹具（放一份**非数据库字节**：
  实现里若还剩一处开库拿到的是 `Err`，而 rename 之后 open 会静悄悄新建空库、红只剩"少行"）。
  ③ **一条端到端用例把库路径写进了断言**（`crates/plugin-host/tests/tools_batch3.rs`）：它测的
  本来就是"写进去的记忆真的落了盘"，而那一半档取的是 `memory.sqlite3.is_file()` ⇒ 插件不再建库
  之后必红（同文件另两条是 `ENV_LOCK` 的 `PoisonError` 连锁带红，不是它们自己的结论）。
  订正按"断言语义不动、只换取数通道"：改成在数据根子树里找一个文件名 `user.lang.md`，
  **不写目录名**——落点由布局算，端到端把布局写死就是下一次改布局的债。这一版比改前更强：
  空库也算"库在场"，而现在要求那条 key 真的寻成了一个文件。
  ⚠ 它为什么漏到 `make check` 才撞出来：分面自测跑的是 `-p plugin-api -p memory-sqlite` 加宿主
  `cargo test --lib`，而 `plugin-host` 的**集成测试**（真实 dlopen）两边都不覆盖 ⇒ 改存储面的刀
  分面绿不等于改动面绿，收尾仍要按改动面跑一次 `make check`。
  **判据**：新增 `SS13`——插件记忆面（排 tests、**剥掉行注释**）里 `rusqlite` 引用 /
  `… memories` 表名 SQL / `Connection::open` 三档形状恒 0。它为什么必须是结构判据：
  "把某枚工具退回先写库"这种形状，那 31 条接口用例**一条都不会红**（它们判的是工具契约），
  换存储在行为面上是不可见的。剥注释的原因与 SS11 同一个洞：退役说明在注释里留痕极多，
  按名字数会得一片假红。
  红证 = **磁盘变异 `CASE 39`**（不是 `--selftest` 那种内存变异——它等于让判据吃自己编的数，
  零证明力）：往 `store/tools.rs` 塞一条**没人调用**的 `legacy_shadow_write`，字符串里同时含
  `Connection::open` 与 `INSERT INTO … memories` 两个形状。同一份损坏态跑两遍：① 闸报红且
  点名的就是 `SS13`；② `cargo test -p memory-sqlite` = **31 passed / 0 failed**。
  两条合起来才叫"独有一档"——结构档看得见，行为面照旧全绿。为此给变异器加了 `also` 档
  （在损坏态还原**之前**跑第二个命令，`expectGreen` 不绿就叫停，因为那说明立论要重估）。
  **测试去向**（AGENTS §6.3 逐条交代）：删 4 条——`learning` 簇 3 条（测"宿主与插件两个实现
  谁先访问谁 ALTER"，插件不再开库 ⇒ 没有对象）与 `legacy_key_only_table_migrates_into_unfiled_scope`
  （测插件那两份旧库迁移）；两条都在簇头注里写了"被删的是什么、为什么没有等价形状可写"，
  学习量两列的迁移与不清零判据仍留在宿主那一侧。改写 2 条
  （`write_and_delete_sync_md_human_layer` → `write_and_delete_land_in_the_md_layer`，
  那句 `md_warning.is_none()` 已经没有可失败的对应物，换成断言回显的层标与分类；
  `recall_takes_the_global_layer…` 见上面②）。新增 3 条（上面两条长度红证 + `fact_matches`
  抽成唯一口径后宿主管理页那条过滤用例照旧）。**零断言放宽**。
- **刀③b-2（宿主影子写退役）✅ 2026-10-19 已落地**。
  `crud.rs` 那四条 `shadow_*`（`shadow_upsert` / `shadow_update_value` / `shadow_delete` /
  `shadow_clear`）连同它们共用的 `note_shadow` 一起摘掉——它们留在最后的唯一理由"那六枚工具
  仍按库寻址"随刀③b-1 消失。**AC-S1-9 那句"记忆写路径只许一个出处"从此在两侧同时成立**：
  插件面三形状恒 0（`SS13`），宿主面"写 `memories` 内容"的三种形状各自只许出现在白名单文件里
  （`SS14a/b/c`）。`delete` / `clear_all` 现在就是 `delete_md` / `clear_facts` 的直通，
  `write` / `update_value` 只落真源——**宿主写路径一条 SQL 都不发**。
  判据为什么是**文件集合**而不是计数：计数只挡"多一处"，挡不住"白名单那两处搬到了别的文件"
  与"某处消失了"；集合形状一变就红，红得能直接看见是谁。第二档（按列 UPDATE 内容）期望 `none`，
  而 `memory_learn/mod.rs` 那条 `UPDATE memories SET hit_count = hit_count + 1` 合法在场 ⇒
  口径**按列名收窄**到 `value|tags|source|updated_at`，不收窄一上手就假红。面 =
  `desktop/src-tauri/src` 整根、排 tests、剥行注释：排 tests 是因为夹具手写 `INSERT INTO memories`
  是"造损坏库"的唯一办法，剥注释与 SS11/SS13 同一个洞。`none` 这一档会被"面瞎了"读成绿 ⇒
  FLOORS 加两条下限（生产文件数 + 白名单那两处 INSERT 本体必须在面上）。
  磁盘红证 = `CASE 40`：往 `crud.rs` 塞一条没人调用的 `legacy_shadow_upsert`，闸点名 `SS14a` 报红，
  而同一份损坏态下宿主 **598 条全绿**（`also` 档二次取证）——"宿主还写不写库"这件事在行为面上
  同样不可见，所以它必须由结构档守。
  **撞出来的真 bug（一处，已修）**：`memory_reconcile::process_file` 的 `created` 支把 tags
  **写死空串**（`reconcile_upsert(&workspace, &key, &parsed.body, "", source, at)`）。在旧世界这
  只是"少写一维"——`shadow_upsert` 随后会用真 tags 覆盖同一行，所以**没有任何用例盯它**；
  影子写一退役，这一支就成了影子行 tags 的**唯一来源**，后果是人打开管理页看见"标签集体消失、
  搜索也搜不到"。改成取 `parsed.tags`（[`ParsedFact.tags`] 的注释本来就写着它是"读侧的唯一标签
  来源"），无那行 = 存量形状按空串读、不回填。红证 = `CASE 41`（改回空串 ⇒
  `human_created_file_is_ingested_as_new_row` 那条新加的 tags 断言红）。
  ⚠ 同族留一处挂账：`updated` 支沿的是**行里的** tags（"文件为准"只落到正文那一维）⇒ 人改了
  frontmatter 的 `tags:` 行不会回灌影子。这是既有的语义、不是本刀改坏的，改它要一并裁"谁赢"，
  登记待裁。
  测试去向（AGENTS §6.3）：造"库里有一行"的夹具统一换到生产里唯一还写内容列的入口
  `reconcile_upsert`（`md_read.rs` 两条 / `legacy.rs` 两条 / `learning.rs` 两条 / `recall_log`
  的共用夹具 `store_with_log`）——手法跟着真源形状走，不跟着方便走，`md_read.rs` 那段
  "造法换了一次"的注释现在如实记成换了两次。两条 reconcile 用例（`human_edit_flows_back_into_index`
  / `deleted_file_removes_row_but_missing_dir_keeps_rows`）补"先跑一轮对账"，否则它们断言的
  "更新一行 / 删一行"没有前提。反转 1 条：`host_writes_land_in_md_and_the_db_shadows_them` →
  `host_writes_land_in_md_and_never_touch_the_shadow`，判据从"影子跟到同一归一键位"改成
  **影子恒空**（键位同源那一维的家随之搬去 md 目录名那条 + `memory_reconcile/tests.rs`）。
  改通路 1 条：`deleting_a_row_takes_its_learning_with_it` 走 `reconcile_delete`（现在删影子
  只有这一条路）。订正注释 3 处（`learning_columns_survive_every_write_path` 说破那两段现在
  防的是什么、`a_broken_shadow_db_changes_nothing_on_the_read_or_write_path` 的成因从"只 warn"
  变成"根本不开库"、`a_failed_md_write_is_an_error_and_writes_no_shadow` 现在结构上恒真、
  留着防"加回补偿写"）。**零断言放宽**。
  还有一处**按面拆簇**：这批夹具注释把 `md_read.rs` 抬到 408 行越过 400 闸，而它那时已经同时
  管着两件事（读源 / 注入记账与埋点）。没有抬基线、没有压注释，把"降级照常交付 + 埋点宁缺
  毋伪"那一族四条逐字搬进新簇 `memory/tests/inject_row.rs`（拆完 264 + 161 行）。
  行守恒的证据 = 拆前拆后**用例总数一字未动仍是 598 条全绿**；要改语义的话这个数不会刚好对上。
  代价如实登记（与 ③b-1 那条同族，现在覆盖到宿主自己）：任何一侧写完记忆，`memories` 表里的
  内容都要等下一轮 reconcile 才更新 ⇒ `learnEnabled=true` 时语义召回看到的集合滞后一轮。
  `settings.memory.files` 至此**彻底没有副作用**（env 注入已摘、影子写已摘），去处 = 刀④ 定改义或退役。
- **刀④a（`obsolete/` 软删数据面）✅ 2026-10-19 已落地**。
  `memlayout` 长第六片 `obsolete.rs`：`soft_delete_fact` / `soft_clear_facts` / `list_obsolete` /
  `restore_obsolete` / `purge_obsolete`（+ 防撞算法 `slot_stamp` 纯函数）。删除从此是**搬移**：
  文件进 `<根>/memory/obsolete/`，**镜像原 rel**（`global/x.md`、`projects/<dir>/facts/x.md`）
  并把主干加上 `~<删除毫秒>`——人打开回收区就看得见"它原来住在哪一层、什么时候删的"，
  恢复 = 原路搬回，真删只发生在 `purge_obsolete`（界面上的清空回收区，要二次确认）。
  三条布局纪律，各自都有机器判据或用例：
  ① 恢复按**镜像路径**搬回，不拿 frontmatter 的 `workspace` 寻址（那是本项目**第五处 hint
     陷阱**：人自建的文件那一维可能整个缺失，缺了就恢复不回它来的地方）；
  ② 读面（`layers` / `iter_facts` / `facts_of` / `read_fact` / `clear_facts`）天然看不见回收区
     ——它是那些目录的**兄弟**不是子目录，所以不需要额外判据；
  ③ 软删**不动 `.sync/` 标记**：留着它，reconcile 才会走"标记在 + 行在 + 文件缺 ⇒ 删行"那一支；
     连标记一起删 = 影子行成为没有主人的幽灵（红证 = `CASE 44`）。
  接线四处全改软删：宿主 `MemoryStore::delete`（经 `delete_md`）、`clear_all`（`soft_clear_facts`）、
  `MemoryFiles::delete_fact` → `soft_delete_fact`、插件 `memory_delete` 两个分支。
  **唯一容许的硬删点** = `memory_undo` 撤销一次**新建**（那条用户从没见过，进回收区会把回收区
  变成补偿日志）；`delete_fact` / `clear_facts` 两个既有签名**不删不改名**（`plugin-api` 的对外
  签名属 AGENTS §5 禁区），改为在 doc 上把用法钉死 + 判据看着调用点。
  新判据 `SS15a/SS15b`：硬删调用点的**文件集合**——`SS15a` 期望恰好那一个 undo 文件，
  `SS15b`（硬删整层）期望 `none`。两处形状都带 `(?<!soft_)` 那段 lookbehind，它是**判据的
  承重件**：`soft_delete_fact(` 里含着 `delete_fact(`，不剥前缀每条合法软删都会被打红
  （第一版就错在这儿，一上手全红）。面排掉 `memlayout` 自身（那是定义处，`purge_obsolete`
  在里面删回收区的文件是它的职责）。`SS12` 的转发档跟着换形态：第二条出口从
  `memlayout::delete_fact` 变成 `memlayout::soft_delete_fact`，**条数仍是 2**——这一档报的是
  "对真源落笔的出口有几个"，不是它们叫什么名字。
  **撞出来的一处测试设计错误**（登记，因为它改了实现形状）：防撞逻辑第一版拿集成用例
  `two_soft_deletes…` 做红证，变异**打空**——要撞出名得靠两次 `now_ms()` 落在同一毫秒里，
  那是运气，跑一次绿一次等于零证明力。于是把"槽被占就 +1ms"抽成收 `taken` 闭包的纯函数
  `slot_stamp`，测试可以确定性地造"前 100 格占满"（撞满 = `Err`，绝不静默覆盖）。
  挪的是**戳**不是名字：`strip_stamp` 只认 `~<全数字>`，加 `-2` 那种后缀会让恢复把文件
  放回一个名叫 `x~1728-2` 的 key 上。
  红证四条：`CASE 42`（撤防撞 ⇒ 两条纯函数用例红）/ `CASE 43`（恢复改成静默覆盖 ⇒ 拒绝那条红）
  / `CASE 44`（软删连标记一起清 ⇒ marker 那条红）/ `CASE 45`（往 `crud.rs` 塞一条**没人调用**的
  硬删 ⇒ 闸点名 `SS15a` 报红，而宿主 598 条全绿——这一档同样是行为面看不见的）。
  测试去向：新增一簇 10 条（`memlayout/tests/obsolete.rs`）；**既有 31 条插件接口用例与 598 条
  宿主用例一字未改全绿** = "删除的实现换了、契约没换"的实证（工具的 `deleted` / `previous_entry`
  字段与管理页的 no-op 判据都不感知"搬移 vs 删除"）。
  ⚠ **AC-S1-6 与 AC-S1-7 在这一步撞上了，判读要登记**：AC-S1-6 第一条写的是"删一条 → grep
  数据根该 stem 零命中"，而软删之后那句话在字节层面**不成立**——回收区里正躺着那份内容，
  全文 grep 会命中。两条 AC 的本意是不同的：AC-S1-7 的标题就叫"软删可恢复、**清空才算数**"。
  所以走查时按这个口径验：AC-S1-6 的三条落点是"**读面 + 该层索引 + 下一会话注入块**三处零命中"，
  而"字节也从磁盘上消失"属于 AC-S1-7 的清空动作（要确认）。界面上这一维不能藏：④c 的删除
  文案必须写清"先进回收区、可恢复；要真正抹掉请清空回收区"——用户删一条可能是为了删掉一句
  含隐私的话，那他要的是不可逆那一档。
- **刀④b（三枚行为开关）✅ 2026-10-19 已落地**。三枚 = 全局层 / 项目层 / 交接层（ADR-0007 §18）；
  交接层那枚 `memory.repoContext` 早已存在 ⇒ 本刀只新增前两枚，并把三枚收到**同一个界面分区**
  （`components/memory/LayerSwitches.vue`；设置页那行 `repoContext` 撤掉——同一属性在两页各放一枚
  开关就是两处真相，字段仍随 view 回填、原样保存）。
  ⚠ **对原计划的订正**：这里原先写的是"关 = 该层**不读、不更新、不注入**"，实做的裁定是
  **只停注入**——一个文件都不删、管理页照旧全量、模型主动查（`memory_read` / `memory_search`）
  不受影响。理由不是偷懒：那两枚工具在插件侧、不吃宿主 settings，拦它们要跨冻结 ABI 传新形状。
  接线点 = `recall_quiet` 与 `recall_relevant` 共用同一枚 `global_on`（**早退**而不是 filter，
  否则账本上留下一行"候选 0 条"，把"用户关了开关"冒充成"记忆是空的"）、`project_index` 用
  `project_on`（**不**管 `<environment>` 那行：那里面是项目根与日期，不是记忆内容）。
  证据 = 宿主 598 → **603** 条（`memory/tests/layer_switches.rs` 4 条 + `settings/tests/memory.rs`
  1 条），红证 = `tmp/mut-rust.mjs` CASE 1–6（六发各打中一条）。
  ⚠ 两笔自纠都要留在仓里：① **CASE 3 第一版打空**——`memory_inject_log` 的唯一键含 session_id，
  测试全程用同一个会话（或都用 `None`）就数不出"关档那轮多记了一行"，改成每轮换 session 才有证明力；
  ② **一处说谎的注释已订正**——`GLOBAL_LAYER_DEFAULT` 上方原先写"改 false 会当场红在既有注入用例上"，
  实测那些用例一条都不红（它们拿的是 `MemoryStore::new` 里那枚未接线缺省，与 settings 常量不同源）
  ⇒ 这套东西有**两处缺省要同向**，各自都得有守卫：settings 簇守常量那枚，
  `both_switches_default_to_open_on_an_unwired_store` 守未接线那枚。
- **刀④c（每层一行 + 分类那一列 + 回收区入口 + 文案订正）✅ 2026-10-19 已落地**。
  界面 = `components/memory/LayerSection.vue`（刀④b 那三枚开关与「每层一行」合成一块——§18 那句
  本来就是一件事：这一层的身份、它进不进上下文、它对人的磁盘动作要在同一处对齐）+
  `components/memory/ObsoleteSection.vue` + `components/TypeBadge.vue`（与 `SourceBadge`
  **同形态**那颗，不裸写 span、不另造形状）。后端 = 六枚新命令（`memory_layers` /
  `memory_clear_layer` / `memory_obsolete_list` / `memory_obsolete_restore` /
  `memory_obsolete_purge` / `memory_reveal_layer`）与前端 invoke **同批落**——`check-commands`
  那条「后端有、前端无要进 RESERVED」的债没欠，三向闭合直接绿。
  ⚠ 三条实现裁定：① 恢复的 `rel` 必须过**越界闸** `is_trash_relative`——这枚按钮把它暴露到
  IPC 面之后，一条 `../../x.md` 就能把数据根外的文件搬进真源，那已经不是删除动作而是任意路径
  读写；闸在 memlayout 里而不是命令壳里，因为薄壳拦不住下一个调用者。② 「打开该层」「删除该层」
  都按 `hint` 回到 `layers()` 的**同一个** `LayerDir`，不复用 `memory_reveal_project`（那颗吃
  权威原串，而原串在真源 = 文件之后已经读不回来 = 第五处 hint 陷阱）。③ 两个「没能读出来」的数
  （`blocked_layers` / `unreadable_files`）必须在载荷里并显示出来：扫不动那一层**根本不进清单**
  （`layers()` 只枚举读得动的目录），不交代它就会把"读不到"演成"没记过"——这一条是实测撞出来的：
  我原先以为清单里会留一行标着 `blocked`，断言当场红，那个字段当时根本没有生产者。
  `memory.files` 的去处 = **保留字段 + 如实文案**（不改义、不退役）：它剩下的作用面是"启动那一轮
  reconcile 与首启导出跑不跑"，那仍然是一句真话；`settings.memoryFilesHint`（刀④b 已改）与
  `developer.filesHint` / `developer.reconcileHint` 三句镜像时代说法一并订正成"md 就是真源、
  sqlite 只是影子"。退役它等于删一个用户已经习惯看得见的设置项，那要单独点头。
  证据 = plugin-api 96 → **101**（`obsolete.rs` 长成目录式：身份与镜像留 `obsolete.rs`、四个磁盘
  动作进 `obsolete/shift.rs`，加单层清空 / 越界拒 / 层读数三族）、宿主 603 → **608**（新簇 `memory/tests/layers.rs` 5 条）、
  前端 151 → **153**（`tests/memory-type.spec.ts`）；红证 = `tmp/mut-rust.mjs` CASE 7–12
  （越界闸 / hint 选层 / unreadable 混进 count / 两个汇总数各一 / 宿主转发目标）。
  界面上「先进回收区、可恢复；要真正抹掉请清空回收区」这句话已经写进 `layerRowsHint` 与
  `trashHint`——AC-S1-6 与 AC-S1-7 的分界不能只留在注释里（隐私那一档要的是不可逆）。
- **刀④d（对账重建盘上每一层的索引）✅ 2026-10-19 已落地**。这一刀不在刀序里，是走查
  AC-S1-5 那一格**当场不通过**抓出来的：往 `global/` 手工丢 250 个 `.md` 之后，管理页层区
  那条读数涨到 255（`layer_stats` 扫盘算的，没问题），而 `MEMORY.md` 一行不长（实测仍
  **8 行 / 427 字节**）⇒ 注入侧读的正是那份索引，这 247 条对模型根本不存在。
  改前 `run()` 只刷"这一轮删过幽灵行的那几层"（那份 `dirty_scopes`），而 `MemoryPage` 的
  `onMounted` 注释早就写着"人改 .md 打开本页即生效——文件为准回写索引"——**这句话没有实现者**。
  实现 = `memlayout::refresh_all_indexes(root) -> (刷成功的层数, 没刷到的层数)`（住 `index.rs`，
  经 `layers()` 枚举）。为什么不是"按 `scopes` 逐层调 `refresh_index`"：`scopes` 来自影子表与
  已知工作区，而人手工建出来的那一层**没有权威 workspace 原串**，拿目录名反推就是第五处
  hint 陷阱（算出来的目录指到别处，既不报错也不生效）——能回答"盘上有几层"的只有磁盘本身。
  没刷到的层数并进 `sum.warnings`（`layers()` 把扫不动的层**排除在清单之外**，不单独交代
  就会被读成"这里没东西"，与刀④c 裁定③同一件事）。
  ⚠ 一处夹具自纠要留在仓里：宿主那条测试第一版红在 `fs::write(&index, …).unwrap()` 的
  `NotFound` 上——不是实现坏，是测试拿**未归一**的 `"F:/ws/proj-x"` 去算 `fact_rel_path`，
  而 `MemoryStore::write` 内部先过 `workspace::scope_key` 归一（`memory/crud.rs:130`）
  ⇒ 两边算出**两个目录名**。改成先归一再算路径，那条红变成绿。
  证据 = plugin-api 102 → **104**（`refresh_all_indexes_rebuilds_every_layer_on_disk` /
  `..._reports_layers_it_could_not_reach`）、宿主 608 → **609**
  （`a_layer_nobody_declared_still_gets_its_index_rebuilt`）；红证 = `tmp/mut-rust.mjs`
  CASE 13–15（不接 `blocked` / 只刷清单第一层 / 对账干脆不调它——三发各打中一条，无一打空）。
- **刀④ 不动的面**：**TreeBoard / TreeSection / InjectSection 三处嵌入**（看板整体删除是 S4 的事，
  混在一刀里回滚不掉）；应用内记忆列表/详情视图按 ADR §18 是**目标态退役**，但删它 = 移除用户
  在用的能力，未列入本轮 ⇒ 走查之后单独点头。

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
   **→ 2026-10-18 决策人裁定：不回填**（原话"库里的不回填了 免得污染 我到时会清理掉的"）。
   这条裁定把上面那句"必须先跑一次带 tags 的存量回填"的前置**取消**了，代价如实落地成一条
   新语义并已有用例钉住：存量里"只在库里"的那些行从此按**不存在**处理
   （`a_row_in_the_db_without_a_file_is_not_a_fact`），管理页标签列对那 10 条也是空的
   ——读的就是文件，文件里没那一行。他清理之后两边一起消失，不留"看起来丢数据"的中间态。
4. **`md_delete` 挂在 `if n > 0` 后面**（`crud.rs:158-160`）⇒ "库里没这行" 现在直接当 no-op，
   连文件都不碰。真源翻转后这个判据必须**反过来**：以文件是否存在为准（库里没行、文件有 = 该删掉
   文件）。AC-S1-6 那三条遗忘验证能不能成立，正卡在这一步。
5. 一条实测纠正了我的推断：我原先以为"决策人开了 `.worktrees` 工作树会把结构闸的扫描面污染成
   双份读数"，实测否掉了——`check-single-source.mjs:29-43` 的 `prodRustFiles()` 写死从
   `['crates','desktop','plugins-src']` 三个根 walk，工作树副本不在面内（工作树存在时"生产 .rs
   文件数"读数与工作树之前一致、SS1 仍 `1`、gates rc=0；具体数不落进本篇——那是每刀都会漂的数，
   唯一出处是闸自己的输出行）。残余风险登记在此：那个 walk 只排
   `target|node_modules|dist`，**不排 `.worktrees`**，哪天把扫描根扩到仓库根就会重算一遍。

## 真实窗口走查（2026-10-19 · 真进程 `locowrk-desktop.exe`，逐态截图核对）

数据面 = 本机 `~/.Locowrk/memory/`（三层：`global` + 两个历史项目层）。走查期间往里丢过
250 条 `probe.*` 与 12 条 `walktyped.*`（四分类各一条，`metadata.workspace` 那行**照同层
真实文件逐字节抄**），读数取完**已全部删除**：盘面回到 1 条真实条目，回收区空。

| 那一格 | 实测看到什么 |
| --- | --- |
| 三枚行为开关（刀④b） | 「记忆层」三枚都在位、缺省全开。`settings.json` 里根本没有 `globalLayer` / `projectLayer` 两键 ⇒ 吃的就是缺省那一档（两处缺省同向那条判据在真实进程里对上了）；"关掉只停注入、文件一个都不动"写在 hint 里 |
| 每层一行（刀④c） | 全局偏好 **255 条** · `timo-9291bc5e` **7 条** · `tity-9242b25e` **10 条**，与 `find` 逐层数的文件数一致 |
| 分类那一列 | 五档徽章全见：用户信息 / 反馈纠正 / 项目约定 / 参考资源 / **未分类**（旧 `fact` 文件那一档）；同一条上来源徽章「整理记录」也第一次露出来 |
| 回收区三枚入口 | 列表里点删除 → 文案变「确认删除」→ 再点 → **回收区当场**列出「1 条待处理」+ `walktyped.project · timo · 删于刚刚 · 恢复`；点恢复 → 条目回列表且 `obsolete/` 清空（磁盘核对）；「清空回收区」→ 文案变红「再点一次确认真删」，没点第二下 ⇒ 一条也没真删 |
| AC-S1-5 索引硬钳 | 盘上 255 条 ⇒ `global/MEMORY.md` = **204 行 / 10989 字节**，其中指针行**恰好 200**（= `MAX_INDEX_LINES`），末行 = `… index truncated — open this folder to see every file, or use memory_search` |

走查**当场抓出三个缺陷并当场修掉**（都长在这条线自己的代码上）：

1. **事实列表在 768px 窗口里被压成 0 高**：刀④b/④c 那块「记忆层」吃掉约 500px，而
   `ListPage` 根上的 `min-h-0` 允许它被压扁 ⇒ 打开管理页**一行记忆都看不见**（搜索框也
   被裁掉），下面两块只读区同样够不着。修 = 「事实库」那一支整支可滚 + 列表区下限走
   `.lw-list-floor`（唯一落点 `style.css`，与 `.lw-expand` / `.lw-tabhead` 同族；不在组件里
   各写一份 `min-h-64`）。根容器仍 `overflow-hidden` 是「看板」那一支的定高画布要的，没动。
2. **删完回收区不刷新**：`remove()` / `clearAll()` 只调 `load()` ⇒ 文件已经搬进 `obsolete/`
   （磁盘核对过），界面上仍写"回收区是空的"。修 = 改走 `onMemoryChanged()`（列表 + 层区 +
   回收区一起刷）。这一格恰恰是刀④c 提交信息里那句"软删这条线做到界面上才有人认得"的
   第一步——删完看不到"还能恢复"，那句话就是空的。
3. **对账不重建索引** = 上面一整段刀④d（AC-S1-5 那一格当场不通过的根因）。

两处**登记、不当场改**：

- 「再点一次」这类确认态**没有外点或超时复位**（实测：点页面空白处它仍然挂着）。
  `MemoryPage` 那句注释原本写着"点其他处会复位"——那是实现里没有的事，注释已订正。
  ~~要不要给三枚「再点一次」统一加超时复位 = 新形态 ⇒ 等你点头~~ ⇒ **这条待点头已撤销**
  （同日第二轮）：那三枚按钮长在按方案稿 §E 要换骨拔除的这页上，给它加形态正是"给要拆的壳
  雕花"。留下来的只有注释订正那半件——它是事实，不随页面退役而失效。
- 项目层里**没有 `metadata.workspace` 这一行**的文件，管理页删不掉：读面兜底给的是目录名
  （第五处 hint 陷阱），而 `memory_delete(workspace, key)` 拿它再去算一次目录 ⇒ 界面报
  「记忆 `walktyped.feedback` 不存在」。这是本轮探针夹具漏字段撞出来的，但真实用户手工往
  项目层丢一个裸 `.md` 就是同一个形状 ⇒ 等你点头（写侧补默认，还是界面上给一句能懂的话）。

~~还欠的一格：关掉开关之后注入块真的少了那一层 ⇒ 交给你动手那一条~~ ⇒ **这一格同日第二轮
由我自己打掉了**：对话发得出去（上一轮那条"发不出去"是我的取证方式错了，见下面第二轮那一节），
关掉开关与不关的两个新会话 `llm_usage.input` = 5582 / 5732，差的 150 token 就是那一层。

### 推上去之后 CI 报的两条红（同日订正 · 跨平台性质）

`origin/main` 推到 `7b33f5d` 之后，CI run 39187445795：**hygiene ✓**（上一轮红的那条
R10 已消，干净 clone 面本地也复现过）、**desktop ✓**，**kernel ✗** 两条：
`memlayout::tests::obsolete::a_blocked_layer_is_an_error_not_a_missing_fact` 与
`memlayout::tests::write_delete::deleting_into_a_blocked_layer_errors_instead_of_claiming_absence`。

两条都红在**断言文案那一行**（`e.contains("层不是目录")`），不是红在"没报错"——报错发生了，
只是 Linux 报的是另一句。根因：这两条性质在两个平台上读的不是同一个码，POSIX 对"路径中间有
一段是普通文件"给 `ENOTDIR`，Windows 把它折叠成 `ERROR_PATH_NOT_FOUND`，而后者与"文件不存在"
**同一个码**；实现原先只在 `is_not_found` 那一条支里顺手看一眼父目录 ⇒ Windows 命中、
Linux 落到通用 Err 支。**测试头注里写着要防的正是这件事，但防的方向对了、落点选错了**：
拿错误码当判据，就注定两侧不一样。

修法（`7cbc1b7`）：不是给测试加平台标记（AGENTS §7 明令跨平台性质不许 `#[cfg]` 单侧跳过），
也不是放宽那句 `contains`（那是把差异塞进测试）——让判定**不读错误码、读磁盘事实**：
`remove_file` / `rename` 一失败，先看父路径在场且不是目录 ⇒ 同一句「层不是目录」，两平台
同文案；不是 ⇒ 再按"确实不存在"（幂等 `Ok(false)`）或真 IO 错误分流。**顺序是承重的**：
层级故障那一条必须先判，否则 Windows 上它会被"不存在"那条吞掉（那正是改前的形状）。

| 取证面 | 命令 | 读数 |
| --- | --- | --- |
| Windows（本机） | `cargo test -p plugin-api` | **103 passed**，含那两条 |
| Windows 全量 | `sh scripts/tasks.sh check` | 全绿：43 套件 **1194 条** + vitest 24 文件 181 条 |
| Linux（本机 WSL Ubuntu-24.04，新 clone `7cbc1b7`） | 先 `build_plugins.sh`（10 个 `.so`）+ `build_market.sh`（2 个），再 `cargo test --workspace` | **rc=0** · 40 套件 · **581 passed** · `test result: FAILED` 0 处；那两条定点重跑都是 `ok` |

⚠ 两轮自纠都留在这儿，因为它们都是**取证方式**的错而不是代码的错：
① 前一轮脚本写成 `cargo test --workspace | tail -40`，那个 `RC=0` 是 `tail` 的退出码
（AGENTS §2 纪律第 1 条在本仓第三次咬人）——它从来没有证明过任何事；这一轮改成完整落盘、
退出码单独取。② 两侧 `--list` 用例清单逐行比过：差的 3 条是 `wskey` 那族平台专属用例
（Linux 有 `posix_never_folds_case…`，Windows 有 `windows_collapses_case…` 与
`missing_and_existing_projects_agree…`）加一处路径分隔符的显示差异，**不是漏跑**；
581 与 1194 的差 = Linux 侧不含 `desktop/src-tauri` 那个独立包（CI 同口径，见 `ci.yml` 作业注释）。

⚠ 这一格的红字是**第二轮才发现的**：上一轮 CI 红在 hygiene（16 秒就退），kernel 作业当时是
绿的——因为那两条测试是刀④a/④c 写的，而那三刀从没 push 过。"CI 绿"在这仓里一直是
**上一次推的东西**绿，不是当前 HEAD 绿；本地领先 45 个提交时，这个差别就是这一格。

## 第二轮实测（2026-10-19 · AC-S1-1 / 6 / 7 与那三枚开关）

**先订正上一轮写错的一条**：上面那句"发不出去对话"是**我的取证方式错了**，不是应用缺陷。
这一轮同一条键序在欢迎页与历史会话里都发了出去（`sessions/` 当场多出
`desktop-1792382682441.jsonl` / `desktop-1792382807714.jsonl`，`turn_start` → `turn_end` 齐全）。
差别在两处：① 上一轮点的是发送按钮，而它在输入为空时是 disabled 态；② 坐标是估的。
⇒ 这一轮跑通的配方（三条都要）：**一个 cell 只取 observation 并 emit，下一个 cell 才用那份
截图的 id 点**（同 cell 内刚取的 id 一律 `unknown screenshotId`）；**提交用单独一个 cell 的
`Return`**（前面只要夹过一次 click，焦点就不在输入框里了）；**坐标按当帧截图逐次量**——
「确认删除」那一下我连打三次空，估的 x 在 1155–1189，量完是 1113，差 40px 就落进气口。

### 写路径（AC-S1-1）：一次工具调用，四张读数

在会话里让模型调 `memory_write`（`key=acs1.probe.pnpm` `type=user` `scope=global`），界面回显
「记忆 acs1.probe.pnpm」+ `created`。盘上/库里逐点核对：

| 观测点 | 写之前 | 写之后（同一时刻） |
| --- | --- | --- |
| `memory/global/` 事实文件 | 1 | **2**（`acs1.probe.pnpm.md`，frontmatter `type: "user"` / `workspace: ""` 齐） |
| `global/MEMORY.md` | 3 行 / 115 B | **4 行 / 164 B**（指针行 +1） |
| `.sync/global/` 标记 | 272 | **273**（写侧照落 mtime 基线） |
| `SELECT COUNT(*) FROM memories` | 10 | **10** ← 这一行就是 AC-S1-1 要的那句 |
| 库里 `key LIKE 'acs1%'` | 0 | **0** |

⚠ 但**别把这条读成"库里永远不会有这一行"**：随后打开一次记忆页（挂载触发 reconcile），
影子表 10 → **11**、`acs1` 命中 0 → 1。写路径与对账是两件事——"落点只有 md"归本条，
"影子按文件补齐"归 reconcile（单向、派生）。这条差别不写清，下一轮看到 11 的人会以为真源翻转没做成。

**注入侧的闭环顺带也有了**：同一个会话里让模型逐字复述 `<persistent_memory>`，它给出的正是

```
[global] user.pref.techstack-agnostic = 用户明确要求：… [tags: 技术栈无关 规范设计]
[global] acs1.probe.pnpm = always use pnpm for acs1 probes [tags: acs1]
```

⇒ 刚写进去的那一条，下一句话就被模型看见了（这条同时是 AC-S1-3 那一档的界面外读数：
`type=user` 落全局层 + 管理页分类徽章「用户信息」、层列「全局偏好」）。

### 三枚开关（刀④b 那一格）：不靠模型自述，靠 token 差

模型**不可信的那一半**要写下来：同一句问句，会话 A 老老实实复述了整节，会话 C 回"这一节不属于
系统提示文本"——所以"它没复述"永远不能当"没注入"的证据。换成不依赖它配合的量法：
同一条问句、两个**都无工作区**的新会话，只差那一枚开关：

| 会话 | `settings.memory.globalLayer` | `llm_usage.input` | `tools_est_tokens` |
| --- | --- | --- | --- |
| `…1792382682441` | `false` | **5582** | 8860 |
| `…1792382807714` | `true` | **5732** | 8860 |

工具面同一个数（8860）⇒ 那 **150 token** 只能来自 system 文本，而它正是全局层那一块
（两条 `[global]`，其中一条是长句）。开关写盘也核过：关掉之后 `settings.json` 里
`globalLayer: false`，重进管理页仍显示关 ⇒ 界面回读、持久化、注入生效三件事一条链上都在。
**这一格从此不再欠。**

### 软删 / 恢复 / 清空（AC-S1-6 与 AC-S1-7）

管理页删掉那一条（二次确认「确认删除」）之后：`global/` 2 → **1**、`MEMORY.md` 4 → **3 行**
（指针行没了）、层区「全局偏好」2 条 → **1 条**、列表按 `acs1` 搜索 = **没有匹配的记忆条目**、
回收区当场列「1 条待处理 · acs1.probe.pnpm · 全局偏好 · 删于刚刚 · 恢复」。磁盘侧：

```
obsolete/global/acs1.probe.pnpm~1792383087234.md   ← 镜像原 rel + 删除戳，内容逐字节 = 原文件
.sync/global/acs1.probe.pnpm.ts                     ← 标记仍在（布局纪律③：软删不动 .sync）
库里 acs1 命中 1                                     ← 影子滞后，等 reconcile 收
```

⇒ AC-S1-6 那句原文"grep 数据根该 stem 零命中"现在有**三处**反例（`obsolete/` 镜像、
`.sync/` 标记、影子行），判据落点必须按重述后的三处读面来判，这一节就是那条重述的实测依据。

点「恢复」：文件回到 `global/acs1.probe.pnpm.md`、`updated: 1792382020` **没变**（rename 带自己的
mtime，不伪造时间）、`MEMORY.md` 3 → 4 行、`obsolete/` 归零、层区回到 2 条。
点「清空回收区」第一下：文案变红「再点一次确认真删」，**镜像文件仍在**（当场 `find` 过）⇒
取消 = 一条都不动这一格成立。第二下我没能点中（右缘那 40px 的坐标问题），
所以"字节真的从磁盘上消失"这一格仍只有数据面测试（`obsolete.rs` 的 `purge_obsolete` + `CASE 42–45`）。

顺手补一条**幽灵行判据的实况**（本轮自己撞出来的，之前只在代码里读过）：我把回收区那条镜像
手工 `rm` 掉、再重进记忆页 ⇒ 库里 11 → **10**、`acs1` 命中 → 0。`memory_reconcile.rs:129` 那句
`!文件在场 && read_marker(...).is_some()` 判的就是这个形状——**`.sync` 标记在这里是凭证不是垃圾**：
它区分"本程序写过、后来文件没了"（⇒ 删行）与"存量库里本来就没落过文件的行"（⇒ 不动）。
启动日志里那条 `deleted=262` 是同一件事（上一轮手工删的 262 个探针文件 ⇒ 262 行被收掉）。
⇒ 结论：**别顺手清 `.sync/`**，清了那些行就再也收不掉。本轮留下的 273 个标记里 263 个是孤儿，
无害（`stash_human_edit` 在文件不在时直接返回 None），登记为已知形状，不做回收。

### 界面落点订正（决策人 2026-10-19 当场叫停）

上面这些读数全是在**左边栏那页 `MemoryPage`** 上取的，而方案稿 §E 早就写死了它的归宿：
**「记忆管理页（Qoder 设置页同款；看板退役）」——落点 = 设置应用的「记忆」页重建
（`MemoryPage.vue` 换骨、`TreeBoard` 族拔除）**，四条清单里第 3 条更是明写
**不做应用内记忆列表/详情视图**（"看某条记忆 = 点『打开文件夹』进目录人自己看"）。

⚠ 那份方案稿 **⊘ 不入库**（只在作者本机），所以这两句判据**以这里这份抄件为准**——
一条要机器判、要下刀引用的口径不能挂在一个不进 git 的文件上（同一族错误本仓登记过：
引用不入库的本机文件 = 假证据）。§E 那四条里对本条线有约束力的就是①②③三条，抄全在这里；
第 4 条（压缩设置归上下文侧那页）不属于 S1 范围。

⇒ 三条后果，按这个口径执行：

1. **这一页整体退役，不只是换骨**（决策人 2026-10-19 补口径，原话"这个页面的记忆树看板和
   事实库看板全部都要退役删除的，所以没有必要打磨"）：`MemoryPage` 那两个 tab（看板 + 事实库）
   **两块都删**，归 [`specs/01`](../01-memory-context-redesign/spec.md) 阶段表里的 **S4 退役面**
   （organize 管线 / 树工具族 / sqlite 读路径下线那一条）。本轮起这一页**只读不改**：
   不加形态、不修视觉与几何、不为它补界面测试。本轮"确认删除按钮贴右缘"那件事因此
   **不登记成缺陷**——它是我坐标估错，且落点是一个要删的壳。
2. **刀④c 做进去的三块分开算**：「每层一行」与「三枚行为开关」是 §E①② 点名要在**设置页**
   重建的零件（搬过去，不是在这页雕）；「应用内事实列表 + 行内编辑/删除 + 回收区列表」这一整块
   按 §E③ **本来就不做**，随本页一起退役——所以它的读数到此为止，缺的那两发界面读数
   （清空第二下 / 删除后下一会话）**不再补**，改由下面第 3 条那两发数据面读数承担。
3. **验收口径不变**：AC-S1-6/7 要的是数据面与注入面。本轮已拿到的（软删三处落点、恢复逐字节、
   取消清空、关掉一层少 150 token）就是这一条 AC 的证据；剩下两发改成**不依赖界面**的读法：
   真删 = 直接对 `purge_obsolete` 走一次真实数据根（或复用 `CASE 42–45` 那条红证），
   "删掉之后下一会话看不见" = 删一条之后开新会话读 `llm_usage.input` 的差值。

仍欠三件（都不靠这一页的界面读数，见上面第 3 条）：① 真删那一下 = 对真实数据根走一次
`purge_obsolete`（或引 `CASE 42–45` 那条红证），不再等界面上那颗按钮；
② AC-S1-6 第三格 = 删一条之后开新会话读 `llm_usage.input` 的差值（与"关掉一层少 150 token"
同一把尺，那把尺已经验过可用）；③ AC-S1-2 的库改名那一格（既有四格测试在，
真实窗口那一次要重启应用，本轮没做）。

## 验收标准（跑什么、看到什么）

- [x] AC-S1-1 写路径唯一：工具或管理页写一条记忆 → 数据根只新增 1 个 `.md` + 索引多 1 行；
      `SELECT COUNT(*) FROM memories` 前后**不变**（sqlite 不再是落点）
      ✅ 2026-10-19 第二轮实测（真窗口 + 真工具调用）：`memory_write` 一次 ⇒ `global/` 事实
      1 → 2、`global/MEMORY.md` 3 → 4 行、`.sync` 272 → 273，而**库里 COUNT 恒为 10、
      `key LIKE 'acs1%'` 命中 0**；随后开一次记忆页（reconcile）影子才 10 → 11——
      写路径与对账是两件事，这条差别在走查节里单独写着，免得下一轮看到 11 误判真源没翻转。
      注入侧同轮闭环：下一个会话的 `<persistent_memory>` 里就有这一条 `[global]`
- [ ] AC-S1-2 读不碰 sqlite：把 `memories.db` 改名造损坏态 → 管理页列表、`<project_memory_index>`
      注入、`memory_read/search` 全部照常出数（这条是"真源翻转"最硬的证据，也是刀②的收口判据）。
      **刀②c-1 之后这四格都有证据且读面已无兜底档**：管理页列表
      （`listing_survives_the_db_being_renamed_away`）、全局层注入
      （`recall_takes_the_global_layer_from_md_with_the_db_gone`）、项目索引
      （`a_shell_index_file_means_the_section_is_absent_even_if_the_db_has_rows`，库里留着行而
      md 是空壳 ⇒ 如实缺席）、**影子库本身坏掉**（`a_broken_shadow_db_changes_nothing_on_the_read_or_write_path`，
      `memory.sqlite3` 写成非数据库字节之后读写两面一个字节都不变）。整条仍不勾，两个缺口：
      ① 插件那六枚工具（`memory_write` / `read` / `list` / `search` / `delete` / `undo`）仍按库寻址
      ⇒ 归刀③b；
      ② `learnEnabled=true` 时语义召回档（`recall_relevant`）仍读库 ⇒ 见刀②c-2 第 2 件，
      ✅ 已于 2026-10-20 由 S3 刀③b 整族退役收口
- [x] AC-S1-3 四类型落位：`type=feedback` 缺省落 `global/`、`type=project` 缺省落
      `projects/<可读名>-<hash>/facts/`；显式给 `scope` 时按覆盖落，frontmatter 回读一致。
      **刀③a 落地**：落位与覆盖由 `plugins-src/memory-sqlite/src/tests/type_layer.rs` 六条钉
      （含"未分类照旧落项目 = 与四类型之前同形"与"值域外的 `scope` 照旧报错"两条负对照），
      frontmatter 往返由 `memlayout/tests/kind.rs` 的逐个类型回读钉。**一句残留**：这四档说的都是
      "真源文件里落对了"，而工具侧 `memory_read` 目前仍从库里读、回显不出 `type`（那一维在
      刀③b 接上；界面那一列在刀④）⇒ 见挂账"类型写进去了但读不回来"
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
      **2026-10-19 真实窗口已过前半**：造 250 条 → 打开管理页 → `global/MEMORY.md` 实测
      204 行、指针行**恰好 200**、末行那句截断提示在场（读数与经过见上面「真实窗口走查」
      那一节——同一次走查当场抓出"对账不重建索引"，那就是刀④d）。
      **后半也在第二轮过了**：让模型逐字复述 `<project_memory_index>`，它给出的 6 行指针与
      `projects/tity-9242b25e/MEMORY.md` 里那 6 行（`project.stress.auto_compact_result` /
      `entropy_fuel` / `probe_alpha` / `probe_delta` / `probes` / `tool_result_budget`）
      **逐条同序**，导语与 `（5 days old）` 陈旧标注都在 ⇒ "注入侧读到的就是盘上这份"
      不再是推断。同一条链上另有一格也过了：关掉「全局记忆层」开关后，两个只差这一枚开关的
      新会话 `llm_usage.input` = **5582 vs 5732**（工具面同为 8860）⇒ 少的那 150 token 就是那一层。
      **仍不勾**：只剩"删掉一条之后，下一个会话的注入块里查不到它"那一发（走查节末尾列着）
- [ ] AC-S1-6 遗忘三条全过：删一条 → 该 stem 在**读面**零命中（`iter_facts` / `facts_of` /
      `read_fact` 三路）→ 该层 `MEMORY.md` 零命中 → 下一会话注入块零命中。
      ⚠ 这条的判据落点自刀④a 起**重述过**：原文写的是"grep 数据根该 stem 零命中"，而删除
      改成软删之后那句话在字节层面必然不成立（回收区里正躺着那份内容）。与 AC-S1-7 的分工
      是："不再被模型看见"归本条，"字节也从磁盘上消失"归 S1-7 的清空动作。
      数据面三处零命中已由 `obsolete.rs` 那簇第一条钉住；**2026-10-19 第二轮在真实窗口过了前两处**：
      管理页删一条 ⇒ 列表按该 key 搜索 = 没有匹配、`global/MEMORY.md` 4 → 3 行（指针行消失）、
      层区 2 条 → 1 条。第三处（下一会话注入块）仍欠那一发。
      ⚠ 同轮把"原文那句 grep 零命中为什么不成立"做实了：软删之后 `obsolete/` 里有逐字节镜像、
      `.sync/global/acs1.probe.pnpm.ts` 标记**照旧在场**（布局纪律③）、影子行也还在库里等
      reconcile 收——三处反例，判据只能按重述后的落点读
- [ ] AC-S1-7 软删可恢复、清空才算数：管理页删除 → 文件进 `obsolete/`、注入零命中、能恢复；
      清空 `obsolete/` 弹确认，取消 = 文件还在。
      **数据面已落地**（刀④a：搬移 / 镜像路径 / 逐字节恢复 / 拒绝覆盖 / `purge_obsolete`，
      红证 = `CASE 42`–`CASE 45`）；界面上的"恢复"与"清空要弹确认"两格在刀④c
      **2026-10-19 第二轮实测**：恢复 = 文件回原位且 `updated: 1792382020` 一字未改、
      `MEMORY.md` 3 → 4 行、`obsolete/` 归零、层区回 2 条；「清空回收区」第一下 = 文案变红
      「再点一次确认真删」而**镜像文件当场仍在**（取消 = 一条都不动成立）。
      只剩第二下（真删）那一次界面读数没拿到——我的坐标没打中那颗贴右缘的按钮，
      不是应用报障；字节消失那一半另有数据面测试兜着
- [ ] AC-S1-8 存量导出幂等：导出脚本跑两次 → 第二次零新增、**人工编辑过的文件 sha 不变**
      （复用 `memory_files.rs:225` 首启导出那套"已有文件一律不动"）
- [x] AC-S1-9 新判据要报红：给"记忆写路径只许一个出处"这条判据做窄变异（把 md 写退回
      sqlite 权威那侧）必须红；红不出来 = 这条判据零证明力，不许留着
      ✅ 2026-10-18（刀③b-1 + ③b-2 两批合上）：判据 = `SS13`（插件面三形状恒 0）+
      `SS14a/b/c`（宿主面"写 memories 内容"的三种形状各自只许出现在白名单文件里）；
      红证 = 磁盘变异 `CASE 39` / `CASE 40`——两条都故意长成**没人调用**的函数，
      闸点名报红、cargo 全绿（同一份损坏态二次取证），"独有一档"因此是实测而不是论证
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
| `crates/plugin-api/src/memlayout/{mod,paths,index,read,sync,kind,obsolete}.rs` | 刀①：索引写侧硬钳 + 预算常量收一处（`index.rs`）；刀②a：读回面 `read.rs` + `render()`/`parse_fact` 补 `tags`；`FactEntry` 只末尾追加字段、既有签名不动；刀③a：四类型面 `kind.rs`；刀④a：**软删面 `obsolete.rs`**（新片，六件对外 + 一个防撞纯函数） |
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
- ~~挂账（刀②b-2 复查后**故意不坐实**的一档）~~ **刀②c-1 已坐实**：原先登记的理由是
  "层级读不动没有跨平台夹具"（POSIX `chmod 000` 造得出、Windows 要动 ACL）。那条理由被一个
  更普通的形态推翻：**在同一位置放一个普通文件冒充目录**，两侧 `read_dir` 都失败且都不是
  `NotFound` ⇒ 不需要权限、不需要 `#[cfg]`。现在 `memlayout::layers()` 把这类故障数进
  `unreadable`（负对照同样在场：层不存在仍不是故障），红证两条在
  `memlayout/tests/read_back.rs`，宿主端到端那一条是 `a_blocked_layer_also_suppresses_the_inject_row`。
  ⚠ 教训留在这里：**"证不了"要区分"想不到夹具"与"真造不出"**，我把前者写成了后者并据此
  决定不写守卫——那是一段纪律被一句未经复查的判断挂起来，本仓不接受这种挂法。
- 挂账（刀②c-1 撞出来的第三处 `hint` 陷阱）：**人自己在项目层新建、没写 frontmatter 的文件**
  读回的 `workspace` 是目录名兜底（`proj-a-67ed0df8` 这种），而目录名反推不出权威原串 ⇒
  拿它再算一次 `fact_rel_path` 就指到别的目录。后果分两半：清点名已经绕开它
  （`clear_facts` 按扫到的 `path` 删，不按键位反推）；但**管理页对这种行的编辑与删除仍会失败**
  （`MemoryRow.workspace` 给的就是那个目录名，前端把它原样传回来 ⇒ 报"记忆不存在"）。
  收口办法 = 刀②c-2 退役反向通路时，把"吸纳人新建文件"改成**回写 frontmatter**（让文件
  自己带上权威键位），而不是在宿主侧再猜一次目录。现状登记，本刀不顺手改。
- 挂账（刀②c-1 之后 `memory.files` 这个设置项**管不到宿主读写面**）：真源没有可退的第二处，
  所以"要不要文件化"这个问题在宿主这边已经没有可开关的东西。它现在剩下的作用面是两处——
  ① 插件侧 env `LOCOWRK_MEMORY_FILES`（刀③b 随工具改道一起摘），② 对账/启动导出的空转开关
  （②c-2 裁定 `memory_reconcile` **不整项退役** ⇒ 它要留到刀③b 之后影子没有读者时一起评估）。
  两处都摘完之后这一项要么**改义**
  （建议：关掉 = 记忆面停止写入并如实报错，仍是一个真的开关，符合 AGENTS §4 可配置原则），
  要么整项退役 ⇒ 归刀④ 与界面文案一起定。⚠ 现在设置页那句 hint 写的是"把记忆**同步存为**
  Markdown 文件"——那是镜像时代的说法，与实况不符 ⇒ 刀④ 一起订正（同类"文字与实况不符"）。
- ~~挂账（刀②c-1 留下的**影子写**）~~ **刀③b-2 已消**：`crud.rs` 那四条 `shadow_*` 与
  `note_shadow` 一起退役，宿主写路径从此一条 SQL 都不发；判据 = `SS14a/b/c`。
  这里保留原话的一句作为理由存档："不在刀②c-1 顺手停的原因 = 停掉之后管理页写的记忆模型读不到，
  而插件写的记忆管理页看不见，中间态用户立刻能发现"——那两个"看不见"分别由刀③b-1（插件改道 md）
  与刀②b-1（管理页读源换 md）接上，所以本刀落地时中间态已经不存在。`CREATE_TABLE` 与两份迁移
  **留着**：表还在（影子 + 学习量），只是它的内容列从此只由 reconcile 写。
- 挂账（刀③a 留下的**中间态：类型写进去了但读不回来**）：`metadata.type` 从刀③a 起由
  `memory_write` 的 `type` 参数生产并落在真源文件里，人打开文件夹看得见。**① 已随刀③b-1 消**：
  工具侧 `memory_read` / `memory_list` / `memory_search` 现在从真源读，那一维由
  `StoredFact.ty` 出来、经 `Hit::json()` 连同权威的 `type` / `scope` / `workspace` 一起回给模型
  （回读一致由此成立，"本次调用的回显"那句旧话不再适用）。**只剩 ②**：宿主管理页的 `MemoryRow`
  还没有这一字段（`md_rows` 把 `StoredFact` 投影成行时丢掉了它），所以界面上看不出这条是 `user`
  还是 `project` ⇒ 归刀④ 那一列。
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
