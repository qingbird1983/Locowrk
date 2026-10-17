# 03 · 结构性欠账清零

## 这个阶段是什么

审计线（批 A–F）在 v2.181 收口时留下的**没结的小事**，逐条登记在下面的计划里（编号只增，新账往后加）。
每条都不大：拆一个文件、去掉一次重复读盘、改一处扫描方式。它们没在这批做，全都有具体原因——
要么缺前置条件，要么需要你点头，要么当时判过"不值得为它单独开一批"。

现在它们只在这里是**在册的**：勾掉一条就少一条，做完就从这张表删掉。
原文在 [`docs/history/audit/AUDIT-FIX-PLAN-v1.md`](../../docs/history/audit/AUDIT-FIX-PLAN-v1.md) §7（⊘ 不入库，只在作者本机），
那里的措辞不改写（历史只读），要更新的东西写在这里。

## 现在到哪了

进度**只在下面的勾选框里**，这一行不抄数——抄一次就会漂，它曾经写着"十六条一条没结"，
而那时 T8 与 T18 都已经结了。要剩多少条就现查：`grep -c '^- \[ \]' specs/03-structural-debt/spec.md`。
"可直接做"那一组是纯结构收敛、零行为变化，不需要前置；"卡在人的一侧"那组要点头或要裁定。

## 计划

### 一、可直接做（不需要你点头，零行为变化）

- [x] **T1** 把 `approval/mod.rs`（470 行）里"决策"那半拆成 `approval/decide.rs`
      · 落点 `desktop/src-tauri/src/approval/` · `check_length` 名单 22 → **21**（实测，不是估计）
      · 拆分口径 = 纯搬移：等价证明用"HEAD 版逐行必须活在新 mod ∪ decide 里"，
        缺失只剩 8 行且全是文档注释与一行 `use` 的合并，**实现体一行未改**；
        阳性对照（把 reason 改一个字母）实测把缺失从 8 抬到 9 ⇒ 这套判据真的在盯代码
      · 测试面一字未改（579 条桌面用例全绿）：trait 实现不产生调用点，`Pending` 等私有项
        靠"子模块看得见祖先模块的私有项"这条可见性规则直接引用，没有为了拆分放宽到 `pub`
- [x] **T5** 去掉 `llm/app_llm.rs::prepare` 里剩下的那一处重复 `llm_config()` 取用
      · 复核发现台账原先那句"它有一条既有判据钉着'恰好 1 次'"**是假的**：`tests_hotpath.rs`
        注释点名的门禁 `no_llm_clone_hotpath` 全仓查无此文（那个名字只出现在注释里），
        所以次数从来没人盯，那次多余的克隆才活到现在
      · 现在真有一条：`check-single-source.mjs` 新增 **SS3**（`with_llm_config(` 1 次、
        `.llm_config()` / `.llm()` 各 0 次），并配一条内存变异（把克隆塞回去 ⇒ SS3 报红），
        `make selftest` 五档全中；注释同步改成指向 SS3 并写明"当初那条不存在"
      · 实现侧：`effective_cfg_facts` 在同一次快照里顺手选出"最终生效的那个档位"并读三个钉死值，
        `prepare` 不再第二次取锁、不再第二次克隆含各档明文 Key 的档位区；
        `effective_cfg` 收成 `#[cfg(test)]` 的解析档（那 4 条 dispatch 用例钉的是**解析次序**，
        不该被钉死值干扰），既有测试一字未改仍绿
- [x] **T6** 会话命令两处取数方式改正：`session_export` 里为了拿标题扫了整个目录（正解 = 读旁车 `read_meta`）；
      `session_list` 没包 `spawn_blocking`（会卡住异步线程）
      · 落点 `desktop/src-tauri/src/commands/sessions.rs` + `commands.rs`
      · 导出标题三档（旁车重命名 → 本会话首条用户消息截 `SESSION_TITLE_CHARS` → 会话 id）抽成纯函数
        `export_title` 并被新用例 `tests_session_log/export_title.rs` 钉住：以前这三档藏在
        "扫全目录再找自己"的实现里、导出通路从来没有判据。截断宽度用共享常量，不在测试里重抄 40
      · `session_list` 走 `spawn_blocking`（与 assets / llm_import 同一族先例），不再占住 tokio worker
      · 窄变异实测：把三档次序换成"自动标题压过旁车" ⇒ 用例红在那条断言上，另一条仍绿（各打一处）
- [x] **T7** 审批测试里的挂起脆弱性做同形状排查：超时给 0 + 决断者无界轮询
      · 落点 `desktop/src-tauri/src/tests_permission.rs` 及其余审批测试
      · 危害：挂起的测试没有证明力，还会把整轮门禁堵住
      · **同形排查的实数**（改前 9 处各写一份 `loop { 有卡 → decide → break ; 否则 sleep }`，全仓再无第二族）：
        `tests_permission` 3 / `hub_basic` 1 / `keyside` 1 / `project_rules` 2 / `session_timeout` 2 / `tool_status` 1。
        两类挂起分开数：① 被调用方 `worker.await.unwrap()` 的 3 处（`keyside` + `project_rules` 两条）卡片不出现就永挂；
        ② hub 超时给 `ZERO` / `from_secs(0)` 的 4 处（`tests_permission` 3 + `unlimited_timeout…`）没人回填 oneshot 就永等。
        另外 2 处（`hub_basic` / `tool_status`）靠测试缺省 100s 守夜兜底——不永挂，但要等满 100 秒才失败，
        且失败原因被守夜伪装成"超时拒绝"，看不出真正退化的是弹卡那一步。
      · 收口 = 一个共享形状 + 两个上界（`tests_approval/mod.rs` 末尾）：`Decider`（builder：`deny` / `reason` /
        `scope` / `after` / `within`，轮询到 `DECIDER_BUDGET = 10s` 仍没卡片就 **panic 点名"问人通路没弹卡"**）
        与 `bounded(名称, 上界, future)`（给"要靠别人回填才收束"的 `resolve` / `run_turn` 加上界——
        光把决断者收住不够，不限档那 4 处等的是另一侧）。9 处调用点一律改成这一个形状。
      · 撤销一处局部补丁：批 F-24a 曾把 `external_path_forces_card_even_in_auto` 的 `ZERO` 改成 500ms 来躲挂起
        （注释里写的就是这个理由）。那是把通用缺陷摊成一条用例的私人魔法数 ⇒ 本批**恢复 `ZERO`**（= 生产那一档），
        挂起风险改由 `Decider` + `bounded` 承担；超时语义本身仍由 `session_timeout.rs` 专责覆盖，没有动它。
      · 判据两族都新立：行为侧 `tests_approval/hang_guard.rs` 三条（没卡片必须**在预算内**失败 / `bounded` 喂
        永不完成的 future 必须报出来 / 有卡片时照旧正常决议——最后这条是阴性对照，防前两条退化成"永远 panic"）；
        静态侧 `check-single-source.mjs` 加档 **SS4**：审批测试面里 `.pending().first()` 只许 1 个出处
        （= `Decider::spawn` 内部），扫描面 = `tests_permission.rs` + `tests_approval/` 整个目录动态取，
        下限 8 个文件（现值 11）。`make selftest` 六档全中。
      · 变异四发各自打中不同读数（还原逐字节写回 + `cmp` + sha 复核）：删掉上界 `assert` ⇒ 红在保护网
        "又退化成无界轮询了"；把上界改成静默 `return` ⇒ 红在 `is_err`（实测 `Ok(())`）；
        `bounded` 去掉 `timeout` ⇒ 红在第二条；SS4 塞回第二处手写轮询 ⇒ 闸红。
        hang_guard 的外层保护网**故意不用 `bounded`** 而用 `tokio::time::timeout`——自测的兜底不能和被测机制同源，
        否则 `bounded` 自己失去上界时那条自测会和它一起挂住（同义反复 = 零证明力，这是 M3 第一版踩到的）。
      · 耗时读数：`tests_approval` 面 25 条 1.25 秒、`tests_permission` 面 8 条 0.02 秒 ⇒ 上界只兜退化，不拖正常路径
- [x] **T8** 把散在 `tmp/` 的历史判据**收口**：该常驻的升进 `scripts/`，其余明确作废
      · 升格一条：`tmp/q5-parity.mjs` → `scripts/check-schema-parity.mjs`（`memories` 表在宿主与插件
        **各有一份实现**、开同一个库文件，只改一侧时 cargo 与 vitest 全绿、运行时才炸）；十档每档
        都有独立的内存变异对照，已接进 `make quick` 与 `make check`
      · **明确作废且没有升格的两族**（记在这儿，别以为还有闸在盯）：Rust↔TS 字段逐条镜像（原
        `q14b`/`q14d`，随记忆与上下文重构时重立）；测试断言不许锁中文文案（原 `q11-shape` S5，
        改由「断言实参引用夹具常量」这个代码形状承重）
      · `pre-commit` 的暂存面触发**没加**（`.githooks/` 属禁区）⇒ 动 `memory.rs` / `store.rs` 时靠 `make quick` 兜
- [ ] **T15** `settings/update.rs` 其余分区（context / memory / llm / embedding…）照 developer 那一段的形状收进各自的 `apply_update`
      · 现状 = 本批只搬了 developer 一段（78 行），其余仍在编排点就地铺开
- [ ] **T16** 三份"即时生效"的写法合一：`setLocale` / `setTheme` / `setUiFontSize` 同一件事三种实现
      · 合之前要先补"失败时回滚时序"的用例，否则两条现网通路没有兜底

### 二、卡在人的一侧（要点头或要裁定，我不会自己动）

- [ ] **T2** 七个 `agent/request` 监听器各自把整包参数复制一遍再解析
      · 要你三选一：① 监听器改吃消费型值；② 链位之间传类型化的可变引用；③ 认可现状并登记为特例
- [ ] **T3** `stores/chat.ts` 的事件投影改成可单测的纯函数
      · 原前置"前端零测试基建"**已由 Q11（v2.202）解除** ⇒ 现在没有前置，等你排期
- [ ] **T4** 把演示夹具 `lab/BuiApprovalCard.vue`（635 行）移出生产目录
      · 前置 = 那 11 个 `bui.*` 文案键的去留要你定（移出会让它们变成孤儿键，i18n 闸当场报）
- [ ] **T9** 十二条性能优化只有"消除了什么"，没有"快了多少"
      · 立基准夹具本身就是一批 ⇒ 已单列为路线图的「度量基线」阶段
- [ ] **T10** 弹层行为族 B/族 C 的处置（族 A 四处已收进 `lib/use-popover.ts`）
      · 我的推荐 = 认可现状，登记进《Front-end Design Guidelines》§七 特例清单
- [ ] **T11** 插件展示元数据以谁为准：代码里的 `describe` 还是 `manifest.json` 的徽章
- [ ] **T12** 编译产物清不清（两份 `target/`，约 57 GB）
      · 不可逆的时间成本（清完下次全量重编很久），**需你明确同意**，Q8 已二次确认推迟
- [ ] **T13** 工具级取消与 deadline 真接线（审计 H5）
      · 前置 = 设计稿先行；现状 = `CallCtx.cancel/deadline` 两端都没接，取消信号过不去 ABI 边界
- [ ] **T14** 逐批验收与证据核对
      · 需要的是"按批出一份验收清单"，不是新改动 ⇒ 你要抽查哪一批，说编号我出清单
- [ ] **T17** `docs/design/authority/DESIGN.md` 文档版本行里那处指向 `docs/history/` 的链接是裸链接
      · 本机层不入库之后（[`ADR-0003`](../../docs/adr/0003-private-doc-layers.md)），别人 clone 过去点不开；
        改它要动 `[LOCK]` 文件 ⇒ **等你点头**（`check-docs.mjs` 的 R10 对这两处网开，所以闸不报）
- [x] **T18** 内核在 Linux 上的账**正面修**（用户 2026-10-17 推翻"选二"，原话：这样不行，把 Linux 这块账彻底修复，包括前面禁止绕开的）
      · 四个 `#[cfg(windows)]` 绕开标记**已全部撤掉**；[`ADR-0004`](../../docs/adr/0004-ci-paths-windows-only.md)
        标为被取代，新口径写在 [`ADR-0005`](../../docs/adr/0005-linux-verified-on-both-platforms.md)；AGENTS §1 改回"Linux 在范围内"
      · 复核结论与当初的判断**不一样**：这四条里两条根本不是"平台语义不适用"——`fsutil` 那条是围栏没先做词法折叠
        （带尾 `..` 的串在 POSIX 上 `file_name()` 为 None，于是越界判定提前撞墙），`memlayout` 那两条是目录名算式
        与同模块 `key_file_stem` 的口径不一致；而那条 SIGSEGV 前面红着的断言，Windows 上是靠"覆盖正在使用的 dll 被
        系统拒绝"蒙过关的
      · 落地的修法五件：词法折叠收成一处（`wskey::lexical_fold`，`win` 作显式参数）并被围栏共用；目录名把 `\` 与 `/`
        同等当边界并再压一道连续点；`PluginRegistry::load` 的幂等闸门同时看 graveyard（判据 = `Arc::ptr_eq`）；
        `NativeBackend::_lib` 改 `ManuallyDrop<Library>`，"永不 dlclose"从注释变成类型事实；update 先认已安装的产物
      · 两侧读数（同一份代码）：Windows 本机 `cargo test --workspace` 517 条全绿；Linux（本机 WSL）516 条全绿，
        差的那一条是 `wskey` 里正当的 `#[cfg(windows)]` 大小写折叠档（POSIX 侧另有 `posix_never_folds_case…` 反向档）
      · 变异验证：去掉 graveyard 复用 ⇒ 新测试 `a_deactivated_version_revives…` 红；把 `..` 抵消改成原样留着 ⇒
        `lexical_fold…` 与 `dotdot_inside_the_path…` **在 Linux 上**红（Windows 上后者不红，因为它走 canonicalize 成功分支）；
        还原用逐字节写回 + `cmp` 复核
      · **仍欠的一面（不许混进本条的收口）**：词法折叠不解析符号链接——`x` 若真是符号链接，`<根>/x/../b` 与
        `<根>/b` 仍可能落成两个键位。这一面要另出题，见 T20
- [ ] **T19** `check-docs.mjs` 的扫描面比它自称的口径小 ⇒ 对 `specs/` 层的断链与旧用词**零证明力**（2026-10-17 实测抓到）
      · 根因一句话：`mdAll` 只从 `docs/` 一棵根往下走，可 R6 的注释写着"扫描面含 specs/**"、
        R8 与 R10 也都在 `live` 上跑 —— `live` 里根本没有 specs 文件，那句 `startsWith('specs/')` 恒假
      · 证据：T17 原写法 `../adr/0003-private-doc-layers.md` 从 `specs/03-structural-debt/` 出发少一级，
        `fs.existsSync` 实测 `false`，而 `make doc` 当场报 OK —— 已在本条同批把这处链接改成 `../../docs/adr/…`
      · 怎么修才算修好：把 specs 并进扫描根之后，必须给"specs 里插一条断链"**补一条窄变异**
        （`make selftest` 里要报红），否则只是换个目录名走一遍，下次照样漏
      · 为什么不在 T18 那一刀里顺手做：这是判据自己的账，动它会改变全体文档的通过线，与 Linux 语义无关
- [ ] **T20** 符号链接那一面在两个平台上都还没有证据（T18 收口时留下，不许算进那一条的绿）
      · 现状：`canonicalize` 成功时解析链接，失败时走 `lexical_fold` 纯词法折叠 ⇒ `<根>/link/../b`
        （`link` 是指向别处的符号链接）与 `<根>/b` 仍可能落成两个键位，围栏同理
      · 要做的：出题在先（Linux 建 symlink、Windows 建 junction，同一性质各一条断言），
        再决定实现是"折叠前先解析已有祖先"还是"键位一律以解析结果为准"
      · 判据落点：这条做完之前，Linux 口径只到"已撤平台标记的那些性质成立"，别写成"跨平台完全对齐"

- [ ] **T21** 验收叙事 `demo4` 自己打出的读数与它写的"预期"不一致（2026-10-17 两侧同现）
      · 现场：`crates/kernel-bin/src/demo4.rs` 末行打印"danger 工具实际执行 2 次（**预期 1**：首次审批拒绝未执行，
        复核放行后执行）"，而 Windows 本机与本机 WSL 跑出来的都是 2 次、退出码都是 0 ⇒ 不是 Linux 特有问题
      · 两种可能都要查：要么"预期"那半句是旧的（两个作用域各执行一次才是真相，文案该改），
        要么复核放行真的多执行了一次（那就是幂等/审批门的实质缺陷）——**别默认它只是文案**
      · 判据落点：改完让 demo4 自己判定（不一致就非零退出），而不是只把数打出来给人看

## 验收标准

- [ ] 第一节七条全部做完或明确转入第二节（附理由），没有第三条状态
- [ ] 每条做完后，路线图「结构性欠账清零」那行的"现在到哪了"由**实测**给剩余条数（不手写计数）
- [ ] T1/T15/T16 做完后 `check_length` 的名单**只减不增**，且 i18n 键集读数不变（纯结构收敛不该动文案）
- [ ] T6/T7 做完各自有一条能红的行为用例（不是只改代码不看结果）
- [x] T8 收口后，仓库里"判据脚本"只有一个地方：`scripts/`；`tmp/` 里的历史脚本整体处置完毕（删或明确作废）。
      加对账仍然**不新建闸**，只分两支走：同一个数字/名字在几处必须相等 → 往 `check-single-source.mjs` 的
      CHECKS 加一项；两侧各有一份实现、要比整段代码块同源 → `check-schema-parity.mjs`（本次升格就是这类，
      它的顺序档与 SET 白名单档塞进 CHECKS 会变成把三种形状硬压成一个读数，故独立成支）。这条由 `STRUCTURE.md` 的口径承重
- [ ] 第二节的九条**没有任何一条被我自作主张做掉**（尤其 T4/T12 属删除与不可逆类）

## 非目标

- 不在这里重开审计线：审计已收口，本阶段只做"当时没结的小事"，不新增审计条目
- 不做性能优化：想量化收益属于「度量基线」阶段（T9），先立夹具再谈快慢
- 不为了压行数而改注释、搬字段、拆无辜文件 —— 那是把建议值当硬闸，规范明令禁止
- 不重写历史：台账原文只读，结项状态只写在这一页

## 风险与边界

这些条目彼此独立，可以一条条做、每条单独提交；**不要**把三条不同域的欠账塞进一个提交里。
T1/T3/T4/T15/T16 都动前端与审批面，属用户可感知区域 —— 纯结构收敛一旦改动行为就要重新走界面自检，
所以每条做完要跑 `make quick`，改到 `desktop/src/**` 的还要跑前端测试面。
