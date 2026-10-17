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
- [x] **T15** `settings/update.rs` 其余分区（context / memory / llm / embedding…）照 developer 那一段的形状收进各自的 `apply_update`
      · 现状 = 本批只搬了 developer 一段（78 行），其余仍在编排点就地铺开
      · 收完的形状：十一个分区各自持有 `apply_update`（`appearance` / `embedding` / `context` /
        `memory` / `tools` / `plugins` / `chat` / `developer` / `thinking` / `multimodal`，`llm` 自 v2.58 就是），
        编排层只按 settings.json 的字段顺序转发 ⇒ `update.rs` 381 → **111 行**
      · 两个根字段 `locale` / `shortcut` 留在编排层：它们不属于任何分区结构、各自只有 1–2 条拒绝语句，
        为两行代码再造一层是凭空加机制（这条留在头注里，别让下一个人在"为什么它们不一样"上花时间）
      · `chat` 原来是**自由函数** `apply_update(next, u)`，本批改成方法——它是这十一份里的第二种写法，
        也是 `update.rs` 里唯一一处 `super::chat::…` 转发；签名一族现在统一成"分区自己 + 吃消费型值"
      · 签名吃消费型值（`u: XxxUpdate`）而不是 developer/llm 那个 `&u`：十一区里有 12 处 `String` /
        `Vec<String>` 字段是"从 u 里搬出来落值"，改 `&u` 就得给每处补 `.clone()`——那是**新写的代码**
        而不是搬移，等价性从此只能靠人读（理由写进 `memory/update.rs` 头注，其余九处引它）
      · `settings/memory.rs` → `settings/memory/{mod.rs,update.rs}`：并回 `mod.rs` 是 419 行 = 新增超限件，
        三问自检第三问为"否"（300+ 行字段清单里找某一档）⇒ 按纪律拆目录式子模块。`check_length` 名单不动
      · 等价性不靠人读：一次性脚本 `tmp/t15-equivalence.mjs`（**不入库**，性质同 `make ledger`——比的是
        改前原文与当前产物，越用越红），声明四条变换（`next.<分区>.`→`self.`、去一层缩进、
        仅 memory 的 `super::memory::`→`super::`、函数末尾补那一行 `Ok(())`），
        十三条读数全部"逐行相同"：appearance 22 / embedding 9 / context 29 / memory 124 / tools 67 /
        plugins 10 / thinking 10 / multimodal 21 / chat 41 / locale 4 / shortcut 10 / llm 3 / developer 4；
        阳性对照 = 把 memory 段的 `1..=20` 改成 `1..=21`，脚本当场报漂移（打不中就是脚本自己没在比东西）
      · 常驻判据：`check-single-source.mjs` 加两档。SS5 = 编排层 `return Err(` 3 次与 `.apply_update(` 转发
        11 次（写回第三处就地校验就红）；SS5b = 分区面 `fn apply_update` 定义 11 个（转发还在、定义没了就红）。
        两条各配一条窄变异，`make selftest` 八档全中
      · 实测：Windows `make check` 全绿，桌面 584 条（`settings/tests/*` **一字未改**仍然绿 = 等价第二证）；
        前端未动 ⇒ vitest 13 文件 104 条照旧
- [x] **T16** 三份"即时生效"的写法合一：`setLocale` / `setTheme` / `setUiFontSize` 同一件事三种实现
      · 合之前要先补"失败时回滚时序"的用例，否则两条现网通路没有兜底
      · 先出题后动刀：`desktop/tests/instant-pref.spec.ts` 九条（三通路 × 三态）先落在**改前实现**上跑，
        读数是 `3 failed | 6 passed (9)` —— 红的正是 ③「呈现层自己抛错」那三条。这不是测试写坏了：
        三份都写 `catch { apply(this.view…) }`，若 `setSettings` 已成功而 `apply` 抛错，
        `this.view` 此刻已是**新**投影 ⇒ "回滚"回到新值 = 等于没回滚。界面表现为"设置项没改成、
        但呈现卡在一个既不是旧值也不是磁盘值的中间态"。
      · 合一 = 新增 `applyInstant(patch, apply)`（与批 F-19 收十份「保存类」的 `patchSettings` 同款做法），
        三个 action 退化成"带自己那个 patch 键与呈现闭包的委托"。**不直接复用 `patchSettings`** 的理由
        是可观测差异：即时生效不吃防连点门、不闪「已保存」（下拉挡一挡就闪"已保存"是噪声），
        但它多一条责任——失败时把**呈现与投影一起**退回进来那一份。
      · 三态判据各配一发变异：删 `this.view = before` ⇒ ③ 三条红；删 `if (before) apply(before)` ⇒
        ② ③ 六条红；把语言那路的呈现闭包改成吃入参 ⇒ `setLocale` 三条红。还原逐字节 + `cmp` + sha 复核
      · 常驻判据：`check-single-source.mjs` 加档 SS6（`this.applyInstant(` 3 / `const before = this.view;` 1 /
        `this.view = before;` 1 / `if (before) apply(before);` 1）。**第一版写成"数反面字面缺席"当场假红**——
        我的注释里正引用着旧形状那句 `apply(this.view…)`，被数成了 2。判据改数"正形状成对在场"；
        这条教训写进档位文案里（假红教人绕闸，比漏红更贵）
      · 实测：前端 14 文件 113 条（+1 文件 / +9 条），`make selftest` 九档全中；
        `appearance-store.spec.ts` 那四条真 DOM 面一字未改仍然绿（合一没动呈现层的实际行为）

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
- [x] **T17** `docs/design/authority/DESIGN.md` 文档版本行里那处指向 `docs/history/` 的链接是裸链接
      · 本机层不入库之后（[`ADR-0003`](../../docs/adr/0003-private-doc-layers.md)），别人 clone 过去点不开；
        改它要动 `[LOCK]` 文件 ⇒ **等你点头**（`check-docs.mjs` 的 R10 对这两处网开，所以闸不报）
      · 2026-10-18 点头（原话"这 10 条可以收账，同意"），本条结清。动的不是规范正文，是指针标注 + 豁免面
      · ★ 台账原先那句"对这两处网开"**低估了一处**：豁免写的是 `r === 'docs/design/authority/DESIGN.md'`
        ——**整个文件免检**，不是两处。所以撤掉豁免的同一秒闸当场报出 2 条从没被看过的：
        `DESIGN.md:2809` 与 `:2810` 指向 `docs/references/{cordis-rs,rutis}-notes.md`（同属不入库层）。
        登记时只数到版本行那一处，因为那一处是"人看出来的"，另两处藏在免检面里根本没有入口
      · 处置 = 三处各补「⊘ 不入库，只在作者本机」+ **豁免只剩 `docs/adr/`**（那条理由不同且成立：
        ADR 只增不改，老链接是当时的事实）。长行读数不变（`LONG=9处在3个文件/名单3个`）
      · 变异承重：把 2809 那行的标注删掉 ⇒ `DOC_FAILED=1` 且点名 `DESIGN.md:2809`，不是靠下面那条
        "命中数为 0 判红"的下限兜底；还原逐字节写回 + `cmp` + `sha256sum`（sha 前后一致 8e35225f…）
      · 副作用如实记：`--selftest` 的 ST2 样本是**动态取现网真实指针**的，撤豁免后它从
        `Front-end Design Guidelines.md:148` 变成 `DESIGN.md:2809`，三档仍全中
      · 版本行 **不升号**（仍 v2.58）：那一行自己写着"只随批次改规范正文与本行版本号"，本刀没动规范正文；
        先例是同一行在 G6（`e0739c9`）改指针时也没升
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
- [x] **T19** `check-docs.mjs` 的扫描面比它自称的口径小 ⇒ 对 `specs/` 层的断链与旧用词**零证明力**（2026-10-17 实测抓到）
      · 根因一句话：`mdAll` 只从 `docs/` 一棵根往下走，可 R6 的注释写着"扫描面含 specs/**"、
        R8 与 R10 也都在 `live` 上跑 —— `live` 里根本没有 specs 文件，那句 `startsWith('specs/')` 恒假
      · 修法 = 扫描根并成 `[walk(docs), walk(specs)]`；**并进来之后当场报了 7 条**，逐条分开处置：
        ① 5 条"STRUCTURE.md 没点名在役目录 `specs/<编号-阶段>/`" = **地图判据的口径缺口**，不是文档漏写。
          阶段目录是**实例**（一阶段一目录、随阶段开结，STRUCTURE.md 自己就写着"开工才建 specs/04-…"），
          要求逐一点名 = 每开一阶段都要改地图，那种红只会教人绕地图 ⇒ R2 改成"点 `specs/` 这一层、
          不点里面的实例"（排除条件写理由，`specs/` 仍由 `dirs.add('specs')` 承重）
        ② 1 条"断链 `docs/design/Conversation Flow Redesign (ZCode Alignment`" = **判据自己截断了链接**。
          R8 的字符类写成 `[^)\s]+`，遇到目标里的第一个 `)` 就断，截出来的半截路径既不匹配
          `/docs/design/*.md` 也不是任何目录 ⇒ 一条本机点得开的不入库指针被判成**真断链**。
          本仓 `docs/design/` 根上那批方案稿**文件名里真的带括号**，改成按 CommonMark 取"一层平衡括号"
        ③ 2 条"指向不入库层却没写「不入库」"（`specs/02-tool-cards/spec.md:6` 与 `:8`）= 文档的账，
          已在两处链接所在行各补 `⊘ 不入库`（原来那句总说明写在第 9 行，R10 判的是**同行**）
      · 防回潮下限（这条最重要）：并面之后若有人把 `walk(specs)` 删掉，**现有任何一条判据都不会变红**
        ——少扫一层是静默失效。故加一条点名在场的读数：在役面里必须至少有一份 `specs/` 文档，
        没有就判红"扫描根又只剩 docs/（T19 的账原样回来）"
      · 变异自证 = 本闸新加 `--selftest`（R8/R10 的判体抽成 `judgeBroken` / `judgePointers`，
        变异只喂**内存里的链接数组**，一个磁盘文件都不碰；两条下限留在主流程，否则自测那一条
        变异会自己触发下限、报出来的红说不清是哪件事）：
        ST1 specs 层插一条断链 ⇒ R8 红；ST2 把一条**真实存在**的不入库指针去掉「不入库」三字 ⇒ R10 红
        （样本实测取的是 `Front-end Design Guidelines.md:148`——用现网真实指针是因为 `ignored` 集合来自
        问 `git check-ignore`，现造路径不在集合里，那种对照打空之后看着像判据红、其实是自测自己错）；
        ST3 括号文件名必须整条解析 ⇒ 证 ② 不再退回去。三条全中，已接进 `make selftest`（现三档）与 `make doc`
      · 读数：`DOC_FILES=73 → 78`（+5 份 specs 在役文档）、`LIVE=36 → 41`、R8 链接 90 条、
        R10 可判 58 条 / 命中不入库 16 条、`DOC_FAILED=0`；`specs/01`（用户同期在写的那份）并入后零红
      · STRUCTURE.md 那句"十道闸与两支自证"顺手改成不抄计数（加一档自证它就漂，正是本仓"会漂的数不进文档"那条）
- [x] **T20** 符号链接那一面在两个平台上都还没有证据（T18 收口时留下，不许算进那一条的绿）（2026-10-18 结清）
      · 先把分歧钉成读数（本机 WSL，夹具 `<base>/root/link → <base>/elsewhere`，三条并排跑）：
        `/tmp/t20v/root/link/../b` 的 `realpath` 与**内核 open 后的真实落点**都是 `/tmp/t20v/b`
        （读出的内容 `KERNEL_HITS_THIS` 证明确实打开了那个文件），而 `posixpath.normpath`
        （= 纯词法折叠）给 `/tmp/t20v/root/b` —— 两个不同的文件；同一形状只换尾巴（`../c`）
        内核直接 `ENOENT`，词法折叠照样给一个"看起来正常"的串 ⇒ 词法答的根本不是内核答的那件事
      · Windows 侧同形（junction：`mklink /J`，不需要管理员权限）。把两处算式临时换回**改前实现**
        跑出来的红串就是证据：`normalize` 给 `c:\…\lw-t20-2176\root\b`（词法答复）而内核落点在
        `c:\…\lw-t20-2176\b`；围栏那条更硬——`resolve_for_write` 当时返回 **Ok**，即越界写入被放行
      · 出题在先（两条，改前实现下**各红**，红法实抄）：
        `wskey::tests::dotdot_after_a_directory_link_lands_where_the_kernel_opens_it` 红在 `tests.rs:349`
        的 `assert_eq!`（左边词法答复、右边内核落点）；
        `fsutil::tests::a_middle_directory_link_is_judged_where_the_kernel_writes` 红在 `tests.rs:259`
        的 `unwrap_err()` 拿到 `Ok` 值。键位那条自带 `assert_ne!` **防空转对照**（若两个串本来就相等，
        `assert_eq!` 恒真、这条判据什么也没盯）；围栏那条同题配一条"根内的 `..` 回退必须放行"的
        阴性对照，防"改成什么都拒"也能绿
      · 实现 = `wskey::resolve_partial`，口径是「**最深存在的祖先做真实解析，剩下的尾巴按字面走**」。
        改前的形状「整条 `canonicalize` 成功就用，失败就当纯词法」为什么必错：记忆键位与写入目标
        **本来就常常还不存在**（第一次写进某个项目目录），这类输入一律掉进词法兜底，而词法不认链接。
        `normalize` 与 `fsutil::resolve_for_write` **同吃这一个算式**（两处各写一份就必然漂，
        它们判的是同一件事）
      · 两条踩坑都不是"想当然能过关"的形状，记下因为它们各自骗过我一次：
        ① 第一版用 `Path::ancestors()` + `components()`，实测红——std 的 `Components` **自己会做词法
          抵消**，`<根>\link\..\b` 交给它回来就是 `<根>\b`，`..` 根本没机会落到链接目标上，而本函数
          要算的恰恰是"内核消不消、消在哪一步"；于是改成按分隔符手工切原始串逐段推进
        ② 夹具原本用 `canonicalize` 后的 base ⇒ 输入带着 `\\?\`（生产中不存在该形状）又被判成 UNC
          回落；夹具改回不 canonicalize，同时把 `strip_verbatim` 从"成功分支里剥"挪到**统一出口剥**——
          祖先解析必然带前缀回来，只在成功分支剥会让"路径不存在"那条路的键位留着 `\\?\`
          （实测 `\\?\c:\…\b` 与 `c:\…\b` 两个串 = 同一个项目两个主键，正是本模块要治的病）
      · 射程如实收窄并写进文档：相对串、UNC（共享名不许被 `..` 弹掉）、出现连续分隔符 ⇒ 一律回落
        `lexical_fold`；`canonicalize` 自身失败（权限等）⇒ 用原样段继续**不报错**——键位是标识符
        不是路径访问，"打不开"不该让记忆读写拒落，更不该归成 `""`（那等于污染进全局偏好层）
      · 夹具建不起链接就 **panic 不 `return`**（两处 `link_dir`/`make_dir_link` 同形）：跨平台的性质
        不许因为夹具难造就单侧跳过——跳过之后测试显示绿，而这条性质在该机根本没有证据
      · 两侧读数（同一份代码）：Windows 本机 `sh scripts/tasks.sh check` 末行"check 全绿"，
        `cargo test --workspace` **519**（= T18 的 517 + 本刀新增 2）、`desktop/src-tauri` **584**、
        vitest **113**、`check_length` 超限 21 · 名单 21 · 零新增；Linux（本机 WSL）
        `cargo test --workspace` **518**（= 516 + 2）、plugin-api 单面 60 全绿，两条链接用例两侧各 ok。
        两侧差的那 1 条仍只是 `wskey` 里那条正当的 `#[cfg(windows)]` 大小写折叠档（T18 已记）
      · 变异与对照的还原一律逐字节写回 + `cmp` + `sha256sum` 复核（禁用 `git checkout --`：
        它会把这两个文件其余未提交的改写一起冲掉）
      · **口径更新**：T18 末尾"仍欠的一面"到此结清。Linux 现在可以说"路径语义（含链接中段的 `..`）
        两侧都有实证"，仍**不等于**"跨平台完全对齐"——记忆树 scope 目录名与 `fs_roots` 下发串的口径
        一个字没动（`wskey.rs` 头注「不做的事」那两条依旧是边界）

- [x] **T21** 验收叙事 `demo4` 自己打出的读数与它写的"预期"不一致（2026-10-17 两侧同现）
      · 现场：`crates/kernel-bin/src/demo4.rs` 末行打印"danger 工具实际执行 2 次（**预期 1**：首次审批拒绝未执行，
        复核放行后执行）"，而 Windows 本机与本机 WSL 跑出来的都是 2 次、退出码都是 0 ⇒ 不是 Linux 特有问题
      · 两种可能都要查：要么"预期"那半句是旧的（两个作用域各执行一次才是真相，文案该改），
        要么复核放行真的多执行了一次（那就是幂等/审批门的实质缺陷）——**别默认它只是文案**
      · 查完是**第三种**：既没有多执行，也不只是文案旧——是这条叙事根本没演它自称的那件事。
        审批替身 `ApprovalPlugin` 的规则是"**全局**第一次询问拒绝、之后一律放行"，而 `echo` 也声明不可逆
        ⇒ 那一次"首次拒绝"落在 echo 身上（会话日志 `[3] approval_decided a-call-1 allowed=false`），
        `danger` 两次都被批准 ⇒ 执行 2 次是**正确行为**，写"预期 1"的那句才对不上。
        连带第二条：echo 被拒 ⇒ 从未执行 ⇒ "request 决策环改写参数真的被工具吃到"这条验收点
        在磁盘上的会话日志里**没有任何结果承重**（只剩一行 stdout）
      · 修法 = 把脚本顺序排成 `danger → danger → echo`（不动审批替身那条规则：它是夹具，"全局首次拒"
        本身没错，错的是叙事没有按它的语义安排出场次序）。改后 danger 第一次被拒未执行、
        复核放行执行 1 次，echo 落在被批准那一步 ⇒ 两条验收点各有会话事件
      · ★ 顺带查出更严重的一条：`main()` 原来是 `fn main() { cli::run(); }`——`cli::run()` 明明返回
        `ExitCode::FAILURE`，被这个函数**丢掉** ⇒ `demo` / `demo4` 无论判定成不成立，进程一律退出 0。
        AGENTS §2 把这两条写成"改内核后必跑"的验收叙事，而一条恒零的叙事等于没有叙事：
        坏掉的形状照样"跑通"，下一个改内核的人看见的还是那句绿话。改成 `fn main() -> ExitCode { cli::run() }`
      · 自判三条（各锁文案里一句承诺，任何一条不成立就 Err ⇒ 非零退出）：danger 执行次数 = 1 /
        以 `denied by approval` 闭合的调用 = 1 条 / 会话里必须有 upper 档的结果
        （实测形状 `{"echo":"HELLO LOCOWRK","plugin_version":"0.2.0"}`——照生产造出来的结果写，不照想象编）
      · 变异两发各自打中不同读数：把审批替身改成"永不拒绝" ⇒ rc=1 并报"danger 执行 2 次"；
        把 echo 的 `mode` 写死 `plain`（不吃改写后的 params）⇒ rc=1 并报"会话日志里没有 upper 档的结果"。
        两发都实测过"改之前 rc 恒 0"这件事——第一发变异在修 `main` 之前跑，错误打出来了、退出码仍是 0，
        这正是本条要结的账
      · 两侧读数（同一份代码）：Windows `demo4` rc=0 / `demo` rc=0；本机 WSL `cargo test --workspace`
        516 通过 0 失败、`demo4` rc=0 且三条读数与 Windows 逐字相同、`demo` rc=0

## 验收标准

- [x] 第一节七条全部做完或明确转入第二节（附理由），没有第三条状态
      · 实测（2026-10-18 本刀收尾）：第一节区间（行 21–128）`- [ ]` **0 条** / `- [x]` **7 条**；
        第二节区间（129–267）`- [ ]` **10 条** / `- [x]` **4 条**（T18/T19/T20/T21 登记在第二节之后，
        它们是"不需要你点头的那一组"，只是往后加编号时排在了一起）
- [x] 每条做完后，路线图「结构性欠账清零」那行的"现在到哪了"由**实测**给剩余条数（不手写计数）
      · 路线图那一行已改成**不给数、只给现查命令**（`grep -c '^- \[ \]' specs/03-structural-debt/spec.md`）：
        上一版写着"T8 与 T18 已结"，那是**手写清单**，结到第六条时它就已经是假信息了 —— 与"会漂的数不进文档"同族
- [x] T1/T15/T16 做完后 `check_length` 的名单**只减不增**，且 i18n 键集读数不变（纯结构收敛不该动文案）
      · 实测：名单 22 → **21**（T1 那刀撤掉 `approval/mod.rs` 一行，此后 T15/T16 两批 `超限 21 · 名单 21 · 零新增`）；
        i18n 三批同数 `zh=970 en=970`、`引用字面量键=793 + 索引键=50`
- [x] T6/T7 做完各自有一条能红的行为用例（不是只改代码不看结果）
      · T6 = `tests_session_log/export_title.rs` 两条（旁车真读盘的三档优先 / 截断宽度吃共享常量），
        窄变异两发各打一条
      · T7 = `tests_approval/hang_guard.rs` 三条（没卡片必须在预算内失败 / `bounded` 喂永不完成的
        future 必须报出来 / 有卡片时照旧正常决议 = 阴性对照），变异三发各打不同读数
- [x] T8 收口后，仓库里"判据脚本"只有一个地方：`scripts/`；`tmp/` 里的历史脚本整体处置完毕（删或明确作废）。
      加对账仍然**不新建闸**，只分两支走：同一个数字/名字在几处必须相等 → 往 `check-single-source.mjs` 的
      CHECKS 加一项；两侧各有一份实现、要比整段代码块同源 → `check-schema-parity.mjs`（本次升格就是这类，
      它的顺序档与 SET 白名单档塞进 CHECKS 会变成把三种形状硬压成一个读数，故独立成支）。这条由 `STRUCTURE.md` 的口径承重
- [ ] 第二节那一组**没有任何一条被我自作主张做掉**（尤其 T4/T12 属删除与不可逆类）
      · 条数不写在这里（原句写着"九条"，登记 T17 之后就漂成十条 —— 正是本页禁止的那种数）；
        现查 = 本节区间内 `- [ ]` 的个数。本条要等这一节全部裁定完才勾，不是现在勾

## 非目标

- 不在这里重开审计线：审计已收口，本阶段只做"当时没结的小事"，不新增审计条目
- 不做性能优化：想量化收益属于「度量基线」阶段（T9），先立夹具再谈快慢
- 不为了压行数而改注释、搬字段、拆无辜文件 —— 那是把建议值当硬闸，规范明令禁止
- 不重写历史：台账原文只读，结项状态只写在这一页

## 风险与边界

这些条目彼此独立，可以一条条做、每条单独提交；**不要**把三条不同域的欠账塞进一个提交里。
T1/T3/T4/T15/T16 都动前端与审批面，属用户可感知区域 —— 纯结构收敛一旦改动行为就要重新走界面自检，
所以每条做完要跑 `make quick`，改到 `desktop/src/**` 的还要跑前端测试面。
