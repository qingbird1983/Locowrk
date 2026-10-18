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
"可直接做"那一组是纯结构收敛、零行为变化，不需要前置；"卡在人的一侧"那组要点头或要裁定 ——
2026-10-18 三次点头（"这10条可以收账"、"剩下四条也收账"、"能拆肯定是拆啊"）之后第二组**已全结**，
其中两处原本是写给他的选择由 Qoder 代裁定（T10 的 (a)/(b)、specs/04 的三问），各自条目顶部都标了
"可翻案"与翻案代价。不可逆动作做过一件，正是他点名的那件（T4 那两块实验室夹具连名下 15 条文案键
已删净）；**未点名的删除一条没做**，[LOCK] 与 push 未动。第三组（超限名单 20 件的逐件三问判定）
是同日按同一口径新加的，那张表已由用户点头（「认了，从 T23 开始按你排的顺拆」）⇒ 判定里三笔新登记的
欠账，T23 已落地；表里判拆的件从这一日起**按上面那个顺序一件一刀连着走**（做没做完看第三节的勾，
不在这里逐把点名 —— 点名 = 每做完一刀就得改一次这段话，迟早变成假信息）。

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

### 二、卡在人的一侧（要点头或要裁定；2026-10-18 两次点头后已收账，两处代裁定标明可翻案）

- [x] **T2** 七个 `agent/request` 监听器各自把整包参数复制一遍再解析（2026-10-18 用户点头，按取舍清单选②）
      · 落地形状 = `WaterfallListener::handle` 收 `&mut Value`，加一个共用取还件
        `take_request(payload, 环名)` 返回 RAII 守卫（`RequestGuard::req()` 取一次裸引用、`Drop` 里写回）。
        没用手写 `take` + `put`：这些环各有十几个早退分支（legacy 会话零参与 / 无指令文件 / 无碎片 /
        消息列为空 / 检索为空），漏写一次 `put` 就是把 `Value::Null` 交给下一环——那种错只在真实会话里出现，
        单测穷举不了（先例 = `desktop/src-tauri/src/stop.rs` 的注销守卫）
      · 一并收掉的：`replace_decision` 调用数归零后删除（不留死码）；`system` 追加的**四份**实现
        （memory_inject 私有 `append_block` + base_system / tree_context / thinking_language 三份手写
        `match req.system.take()`）合成 `waterfall::append_block` 一处。这不是顺手打扫：环握
        `&mut RequestPayload` 之后"读同一字段再写同一字段"那个形状撞 borrowck，每处本来就必须改写，
        各留一种绕法必然漂
      · 口径如实说清：坏载荷**不是"放行改成拒绝"**。出环本来就是 `from_value(out.payload)?`，
        原 `else { Continue }` 放行的是一份注定在下一行失败的载荷，只多白跑几环且不说是谁先发现的；
        而且载荷被 move 走之后"原样放行"结构上已不可能——剩下只有当场响与把 `Null` 往下传两种结局
      · 判据 `SS8 / SS8b / SS8c`（接 T11 那族，加在 `check-single-source.mjs`）：接上取还形状的环数 =
        实现块数 / 环文件里 `payload.clone()` = 0 / 七环点名。分工用**真实磁盘变异**各验一发：
        插一次 clone 只有 SS8b 红（SS8 仍 7|7）、摘一次 `take_request` 只有 SS8 红（6|7）、
        把一个环整体改名只有 SS8c 红（6≠7 —— impl 块数下限抓不到，这个盲区已写进判据说明，点名就是为它存在的）；
        变异全部逐字节写回 + `sha256sum -c` OK
      · 测试面：`prompt_layering` 的链跑改成逐环断言 `Continue` 并回读 payload——哪一环漏还，
        下一环 `take_request` 就返回 `Reject`，所以这条断言同时是"守卫还没还回载荷"的探针。
        新增两条地基用例：空改写必须逐字节原样还（撑着前缀缓存与所有"零参与"断言）、
        漏还被下一环当场点名。把守卫改成不写回 ⇒ 内核侧 47 条用例红；把点名改成固定串 ⇒ 3 条点名断言红
      · 实测：`make check` 全绿 · `make selftest` 全档 `ALL_CAUGHT` · `demo` 与 `demo4` 各 exit 0；
        demo4 里"审批插件经 `Replace` 注入的 `mode=upper` 被 echo 吃到"= 外部改写环与七个就地环混跑的
        端到端证据（那条 `Replace` 通路由 `kernel-bin/approval_plugin.rs` 故意留作活样本，
        否则 `Replace` 在本仓只剩测试在用）
      · Linux 侧同 commit 全新 clone 复跑：`cargo test --workspace` = **39 套件 ok / 537 条通过 / 0 失败**
        （`uname` = Linux 6.18.54.1-microsoft-standard-WSL2）
      · 取证路上撞到一个**流程缺口，已当场补进 AGENTS §7**（不另立欠账）：新 clone 照 §7 字面直接跑会红
        7 条，全红在 `plugin-service/src/service/tests/mod.rs:67` 那句拷市场产物的 `unwrap()`——开发态
        `plugins/` 与市场态 `market/` 两份产物都不入库，而 §7 只写了 `cargo test`。第一次我按名字猜、
        补跑 `build_plugins.sh`：它 rc=0 却一个 `.so` 也没落进 `market/`（那脚本 stage 到 `plugins/`），
        市场侧归 `build_market.sh`。补完 plugin-service 31/31、全面 0 失败。两侧口径本来就一致
        （`artifact_file_name` 非 Windows 返回 `libtool_shout.so`，脚本找的也是这个名字）
      · ★ 遗留（不在本条里清）：链上传的仍是 JSON ⇒ 每环还剩 `from_value` + `to_value` 两次整包转换
        （改前是 clone + from_value + to_value 三次）。要归零得把链的载体换成类型化
        （`&mut RequestPayload` 一路传下去、只在进出链各转一次），那要动"决策点 = JSON 契约"这条与
        C ABI 同源的纪律 ⇒ 另开 spec，不是本刀能顺手收的
- [x] **T3** `stores/chat.ts` 的事件投影改成可单测的纯函数（2026-10-18 用户点头收账；**交付的是"可单测"，"纯函数"那半被我推翻**）
      · 立项论断"投影不可单测"**已经不是事实**，本轮实测推翻：Q11（v2.202）把 store 层测起来之后，
        `onEvent` 的 21 个事件变体**全部在不改一行生产码、不 mock Tauri 的前提下测得动** —— 新增
        `desktop/tests/chat-projection.spec.ts` 27 条，用的就是 `chat-row-semantics.spec.ts` 那套
        `createPinia + setActivePinia` 原生档
      · 为什么测得动（三条都是查过的，不是推断）：`turn_done` 里的 `dispatchQueue()` 在无打断、队列空时
        两道门都早退 ⇒ 不发请求；`ask_user` / `ask_resolved` 转发的 asks store 与 chat 同在一个 pinia
        实例里，直接断 `pending` 就够；`init()` 才是唯一碰 Tauri 的入口，而投影路径不经过它
      · 所以**不做那次抽取**：纯函数化能带来的差异只有"可测性"，而可测性已经有了。按本仓口径
        （加机制前先问它带来哪种可观测的不同，带不动就砍），那是一次没有买主的抽取，还会把
        `onEvent` 的 21 个分支与 11 个投影 action 全搬一遍
      · 拿到的是分发表的机器证明。六发变异各打中一条守卫（**每发只红那一条**，证明断言不重叠也不恒真）：
        `tool_status` 的"已收束不许退回等待态" / `turn_header` 的"`active_ms` 缺 ≠ 0"（另配一档"0 是真 0"）/
        `llm_retry` 的重放门 / `file_changes` 不另造独立行 / `turn_done` 的 `endTs` 只写第一次 /
        调度轮没有宿主行时不虚构
      · 写测试时自己踩的坑如实记：**同一 pinia 实例里第二次 `useChatStore()` 拿到的是同一份 state**，
        第一版就这么红了三条（断言读到上一场景残留的行）——是测试的错不是生产的错；处置 = 夹具里加
        显式的 `fresh(replaying)`（`$reset()` 再设档）并写下告诫
      · 新文件 456 行被 `check_length` 按"名单外新增超限"判红过。按三问判定（只管一张表 / 名字概括得住 /
        加新事件不牵动别处）**留在名单**并加一行 = 一次显式决策，没有为过闸删断言或压注释；
        它钉的是"这张表覆盖到哪了"，拆成三份会让 `rowOf / fresh / kinds` 变成三个出处
      · 实测：前端 `test:run` **16 文件 / 144 条全过**（原 117 + 本批 27）· `pnpm build`（含 `vue-tsc`）
        0 error（第一版红在一条未使用的 `ToolItem` import，删行而非加断言）· `make quick` 全绿
        （`超限 22 · 名单 22 · 零新增`）
      · ★ 顺带数出该单独立账的一条 ⇒ 当场登记 **T22**：`chat.ts` 名单那一行的实质欠账是**拆**，不是测
- [x] **T4** 把演示夹具 `lab/BuiApprovalCard.vue`（635 行）移出生产目录（2026-10-18 用户点头"剩下四条也收账"⇒ 按键的去留口径我自己定的那条执行：**一个键都不删**；**同日傍晚用户点名"那 2 个可以删除干净了"⇒ 该口径作废，终态见本条末尾 ★**）
      · 台账原文那句「前置 = 那 11 个 `bui.*` 文案键的去留要你定」**订正为 15 个**：实测 zh/en 各 15 条
        （`bui.rc.*` 那 4 条在子对象里，当初按顶层数漏了）；现查
        `grep -n -A 20 "^  bui: {" desktop/src/locales/zh-CN/chat.ts`
      · 做法 = **搬出 `desktop/src/`，不是搬出仓库**：三件夹具 `git mv` 到 `desktop/lab/`，入口照留在
        `settings/DeveloperSection.vue`，但那行的挂载改成 DEV 门控的动态 import ⇒ 生产构建里整条被
        Rollup 摇掉；文案与 15 个键一个不删（用户口径：未点名的删除一律保留），引用的承重面靠
        `check-i18n.mjs` 新增的第二个根 `REF_ROOTS = [src, lab]` 继续算数
      · 产物面实测（同一份代码，改前先 build 一次留基线）：`dist/assets/index-*.js` 里
        `bui.rc.defaultTitle` / 「形态库实验室」/「ApprovalCard（审批卡·多题向导）」三串**改前命中、
        改后零命中**；`dist/assets` 的 .js 总量 **10,873,554 → 10,864,299 字节（−9,255）**
        （探针 = `tmp/t4-probe.mjs`，改前后各跑一次）
      · 开发面没搬丢（这是这条债最容易做坏的方向）：起一次 vite dev 取两个模块 URL ⇒
        ① `DeveloperSection.vue` 的转译产物里指向 `/lab/LabPanel.vue` = 是
        ② `/lab/LabPanel.vue` 本身 rc=200 且编译产物含 `lab-panel` = 是（`T4_DEV_FACE_OK`）；
        只改命令行端口、没动 `vite.config.ts` 里那条 `host: "::1"` 结论
      · 判据自己也要证：**负对照**把 `REF_ROOTS` 退回只有 `src` 一根 ⇒ 无静态引用键从 **0 变 15**、
        引用字面量键从 **793 变 778** ⇒ 证明那根确实在替这 15 个键承重，而不是"加了也没差别"；
        还原逐字节 `cmp` + `sha256sum`（07e6942cc7d8）
      · 跟着搬的三处口径（不搬就是骗闸）：`check_length.sh` 名单里那行路径改成
        `desktop/lab/BuiApprovalCard.vue`（635 行仍超限、仍按三问保留：它只做一件事 = 演示那张卡）；
        `STRUCTURE.md` 的 desktop 树加 `lab/` 一行写明"在 src 之外 ⇒ 不进产物、不被生产形态闸扫"；
        前端 `tsconfig.json` 的 include 加 `lab` ⇒ **类型面没跟着搬出去**，`vue-tsc` 照样编它
        （搬出去的是产物归属，不是"从此没人管"）
      · 有意不收的一侧：stylelint 与 `check-style.mjs` 都按 `desktop/src/**/*.vue` 扫 ⇒ 文件数
        99 → **96**，夹具从此不受「同类只许一种形态」管——这正是"移出生产目录"的语义之一
        （待裁决的形态样本本来就该住在标准管不到的地方）；哪天采纳哪一等形态，是把它搬回
        `src/components/` 并当场过形态闸，而不是在 lab 里直接接业务
      · 两条踩坑记录（都写进代码注释了）：① 第一次 build 红在 `vue-tsc`，因为我在块注释里写了
        glob `lab/**/*.vue` —— 里面的 `*/` **提前闭合了注释**，注释后半段变成代码；② 我注释里带引号的
        点分字面量（DEV 那个常量名、`tsconfig.json`）会被 check-i18n 的"间接豁免"计数吸进去，
        读数从 1148 漂到 1149 ⇒ 改掉写法把读数还原：**prose 不该动闸的读数**（与"判断读数只许一个出处"同族）
      · 门禁：`make quick` 全绿（含文档闸与 ST1–ST3 自证）、`vue-tsc --noEmit` + `vite build` rc=0、
        vitest **151 条全绿**、`check-i18n` 无静态引用键 0、`check_length` 超限 22 · 名单 22 · 零新增
      · ★ **同日傍晚翻案**（用户 2026-10-18 验收时点名「实验室 T4 那 2 个可以删除干净了。不需要那
        2 个了」）⇒ 上面整段是"搬出去留着"那一步的实测，它**不再描述当前的树**，只留作那一步真做过
        的证据。终态 = 一刀删净，七处同时收：`desktop/lab/` 三件（两张卡 635 + 354 行、只 hosting
        它们的 `LabPanel.vue` 66 行）· `DeveloperSection.vue` 里那条 DEV 门控入口与它唯一的
        `defineAsyncComponent` import · 两份语言包 `chat.ts` 里整个 `bui` 域（`git diff` 实测
        zh/en **各删 15 条键行**，与本条前面"订正为 15 个"对上）· `check-i18n.mjs` 为 lab 加的第二个
        根（`REF_ROOTS` 与 `walkAll` 一并退回 `walk(SRC)`：服务的需求没了就撤，不留"以后也许还要"
        的死面）· `check_length.sh` 名单那行 · `tsconfig.json` 的 include 项 · `STRUCTURE.md` 那 5 行
      · 删净的证明不是"我说删了"，是**闸里再没人引用它**：`check-i18n` 无静态引用键 = **0** ——
        只有"键与引用同批删"才可能是 0（只删组件不删键 ⇒ 报 15 条孤儿，这一形状由上面的负对照
        实测过）；今天实测闸读数 zh=**955** en=**955** · 引用字面量键=**778** · 间接豁免=**1129**
      · 产物面：`dist/assets` 的 .js 总量 **10,864,299 → 10,863,411**（本刀再 −**888** 字节；相对
        最初改前基线 10,873,554 累计 −**10,143**）。两个数可比的前提说破：T4 之后前端只动过
        `src-tauri` 的 Rust，vite 产物面没被别的刀碰过。四串 `bui.rc.defaultTitle` /「形态库实验室」/
        「ApprovalCard（审批卡·多题向导）」/ `lab/LabPanel` 全部**零命中**（探针 `tmp/t4-probe.mjs`）
      · 门禁终态读数：`make quick` 全绿、`check_length` 超限 **21 · 名单 21 · 零新增**（22 → 21 =
        那条名单行随文件一起消失，不是把基线调小）、前端形态 **96 文件**干净、`pnpm build`（含
        `vue-tsc`）rc=0、vitest **17 文件 / 151 条**全绿
      · 开发面反向取证（探针 `tmp/t4b-devcheck.mjs`，起一次 vite dev、只改端口，没动
        `vite.config.ts` 里那条 `host: "::1"` 结论）：① `DeveloperSection.vue` 的转译产物里不再指
        夹具 ② `/lab/LabPanel.vue` 取不到 ③ **同一份产物里记忆树面板还在**（它与夹具住同一层，
        删多了就在这里露）。**判据自己先错过一次**：第一版拿 status 当判据，读出来是"夹具还在"——
        vite 对不存在的模块走 SPA fallback，**status 恒 200**，只有 content-type 掉回 text/html；
        补一条从未存在过的路径当负对照（两条形同）才让"取不到"成为一条判读。被测文件按行尾归一后
        与 `git show HEAD:` 同 sha（e13279bd…）⇒ 取证对象就是刚提交那份，不是工作区旧态

- [x] **T9** 十二条性能优化只有"消除了什么"，没有"快了多少"（2026-10-18 用户点头收账：明确转入，不留第三条状态）
      · 立基准夹具本身就是一批 ⇒ 已单列为路线图的「度量基线」阶段（[`docs/roadmap/README.md`](../../docs/roadmap/README.md)
        那一行现在就写着"⬜ 没开始 …（= 台账 T9）"，转入是有落点的，不是把账撕掉）
      · 为什么不在本台账里顺手做：夹具要立的是"同一份负载改前改后各跑 N 次"，那需要**先冻结负载**，
        而本台账其余条目全在动前端与路径语义 —— 边改边量出来的差值没有出处，量了等于没量
      · 本条从台账移出后，欠账没有消失，只是换页记：做「度量基线」阶段时那十二条的"快了多少"要在那里补
- [x] **T10** 弹层行为族 B/族 C 的处置（族 A 四处已收进 `lib/use-popover.ts`）（2026-10-18 用户点头"剩下四条也收账"⇒ 二选一由我代裁定 **(a)** 并落地）
      · 上一批已做的（无争议那半）：`escHandler` 收成 Esc 判定的唯一出处 + `useDismissOnEsc` 注册面 +
        `DefaultModelMenu` 外点从 `mousedown` 归一到 `pointerdown`；两发变异各打中一条（还原逐字节写回 +
        `cmp` + `sha256sum` b3684c47…）
      · **为什么代裁定选 (a)**：(b) 的代价是"三处按 Esc 不关"成为定面 —— 那是标准自己欠的债被洗成特例，
        不是取舍而是赖账。而 (a) 的"新增一条路由规则"其实**不是新机制**：`Composer` 那份
        `if (showModels.value) return;` 已经是同一个形状的第 1 个实例（浮层自持 Esc + 链点名让位），
        (a) 只是把它从"点名认识 1 个"收成"问 1 个数"⇒ 出处从 N 个减到 1 个，符合标准 §零.2「不得同类多套」
      · 落地形状（`lib/use-popover.ts`）：模块级计数器 `openLayers` + `anyLayerOpen()`，占位与归还全在
        注册面内部（`useDismissOnEsc(close, open?)` / `useDismissOnOutside(root, open)` 自动登记，
        **站点没有"忘记归还"这个可犯的错误**：`onBeforeUnmount` 无条件归还，卸载时开着也还得掉）。
        省略 `open` = 父 `v-if` 渲染、挂载即打开那一族；有本地开关必须传——不传的后果我自己在上一批
        写进了本条（常驻组件长期占位 = 输入框整条 Esc 链哑掉），所以 `escHandler` 也加了 `isOpen` 门控
      · 输入框那条链改问 `anyLayerOpen()`（不再点名 `showModels`）⇒ 链上第 2 个"浮层优先"实例消失，
        浮层加多少个都不必再动这条链
      · 补齐的站点：**`chat/PermissionCard.vue` 的 scope 下拉**（它那份外点与族 A 逐字同形 ⇒ 整段换成
        `useDismissOnOutside(scopeAnchor, showScope)`，删掉自写的 7 行）、`shell/SubagentsMenu.vue`、
        `shell/NotificationPopover.vue`；另 `chat/ModelSelector.vue` 自写的那份 document keydown 换共享件
        （同一判定的第 2 个出处消失）
      · ★ 逐站补做取证时发现**第 4 处**、不在原三条里：`shell/RowMenu.vue` 的 Esc 原本只挂在菜单元素上，
        而**开层时焦点不进层**（`focusActive()` 只在按过方向键后才调）⇒ 右键或点 `…` 开层后按 Esc 谁也不吃，
        焦点若还在输入框那一键就直接落到拒审批上。补 document 级一份（元素那份连 `stopPropagation` 一字未动，
        焦点在层内时仍由它先吃）。`SettingsSelect` **不补**：实测它的 `openList()` 在 nextTick 里
        `pop.focus()`，焦点在层内 ⇒ 元素级那份真能接到键 ⇒ 补了才是同一件事两个出处
      · 防回潮 = `check-single-source.mjs` 新增 **SS9**（挂了 document/window 级 `pointerdown` 外点的组件
        必须同文件带 Esc，左=右）/ **SS9b**（让位判定还在：`1|1` = 计数器 1 个定义 + 1 个消费者）/
        **SS9c**（计数器只许一个出处）+ 三条扫描面下限（组件 .vue 数、外点站点数、前端 src 文件数）。
        `make selftest` **17 档全中**；变异三发各打中一条：SS9 = 塞一个"只挂外点没挂 Esc"的浮层、
        SS9b = 把 Composer 那行退回 `showModels`、SS9c = 在别处再写一份 `let openLayers`
      · 行为面 = `desktop/tests/popover-esc.spec.ts` 从 4 条扩到 **9 条**（新增 ⑤ `isOpen` 门控 +
        ⑥⑦⑧⑨ 优先级登记/归还/两层叠开），四发窄变异各打中它自己那一条（摘掉卸载归还 ⇒ 红 4 条；
        挂载时无视 `open` ⇒ 首红在 ⑦ 并级联 ⑧⑨；计数写成布尔 ⇒ 独红在 ⑨；`escHandler` 丢门控 ⇒ 独红在 ⑤）；
        新写 `desktop/tests/esc-routing.spec.ts` **2 条** = mount 真实 `Composer`，同一条 textarea 上按 Esc
        跑三段（无浮层 ⇒ 拒审批 1 次；浮层开着 ⇒ 0 次且关的是浮层；浮层关掉后 ⇒ 又 1 次）——
        这三段互相兜，任何一段假绿都会把另一段顶红；第 4 条打在 `PermissionCard` 的下拉上（Esc 关下拉、
        `decide` 零调用、优先级归还）。变异三发：让位判定写恒真 ⇒ 红在①③；删掉让位 ⇒ 红在②（正是本刀修的
        那个缺陷形状）；PermissionCard 退回"只挂外点没 Esc" ⇒ 红在第四条。三次还原均逐字节 `cmp` +
        `sha256sum` 复核（Composer 3698ebd01613 / PermissionCard 197daa4753fe）
      · 标准同步：《Front-end Design Guidelines》升 **v2.4** —— §二③ 收口那行补上缺的半句规矩
        （「一次 Esc 只许有一个消费者 = 开着的那层吃掉它」+ 两种归属怎么写 + 现查命令 + 实测读数），
        §七 原那两行**订正**：豁免只覆盖"整套方向键链"、不覆盖 Esc（`RowMenu` 那条按实况改写法）
      · 读数（本刀口径，现查命令写进标准 §二③）：自己挂了 document/window 级 `pointerdown` 的浮层组件
        **6** 个，带 Esc **6** 个（改前 3 缺）；引用 `lib/use-popover` 的组件 **11** 个 = 收口侧 10 +
        让位侧 1（`Composer.vue`）；`mousedown` 残留 0
      · 未动的一侧（有意不收）：`ImageViewer` / `CommandPalette` / `ToastStack` / 两份对话框 / 画布那五处
        Esc 消费者不在这条债的射程内——它们不是"外点自持的小浮层"，各自的条件（`saving` 门控、清 toast、
        退出画布）是真语义；标准 §二③ 那句"一律 Esc"管的仍是 §五 那族
      · 门禁：`make quick` 全绿（含 17 档自证 + stylelint + check-style 99 文件干净）、`vue-tsc --noEmit` rc=0、
        vitest **151 条全绿**（改前 144 = +5 优先级 +2 路由）
- [x] **T11** 插件展示元数据以谁为准：代码里的 `describe` 还是 `manifest.json` 的徽章（2026-10-18 用户点头收账）
      · 方向由实测决定，而**两个候选谁都不能全盘胜出**。逐插件比对十份，重叠区漂的是：工具描述 15/30、
        schema 6 处、`exposure` 4 处，`side_effect` 0 处（它历史漂过一次 = 审计 H8，清单写 `Pure`
        而代码早已 `Irreversible` ⇒ 用户看到的徽章与内核真实的审批行为不一致）。决定性的那条是
        **磁盘那份在能力面上是旧的**：`fs_write`/`fs_delete` 与三个 `edit_*` 缺 `expected_revision`
        （模型目录里那份并发写保护的说明与 required 项）、`memory_write` 缺 `scope`（项目层/全局层）、
        四处 `exposure: always` 缺失 ⇒ "以 manifest.json 为准"会把这些保护抹掉，那是行为退化不是收敛
      · 落定的划分已写成 [`ADR-0006`](../../docs/adr/0006-plugin-metadata-authority.md)：
        **能力字段以代码 `describe` 为权威、展示字段以磁盘 `manifest.json` 为权威、重叠区必须逐字段相等**；
        等值由每个插件自带的**一条 `cargo test`** 承重（插件 crate 都在根 workspace 里 ⇒ `make check` 自动跑到）
      · 载体三件都在 `plugin_api::meta`（非 ABI 面、插件源码级依赖，`PLUGIN_ABI_VERSION` 不动）：
        `manifest_drift`（逐字段差异清单，报错就能照抄改对）、`drift_of_describe`（插件侧唯一入口，
        十份测试同形状）、`sync_manifest_file`（**只在 `LOCOWRK_MANIFEST_SYNC=1` 下动文件**的维护通道——
        判据不许顺手改受版本控制的文件，且只回写**静态前缀**：`tool-mcp` 运行期发现的远端工具
        不能被钉进静态清单）
      · 为什么比对器放基座而不是十个插件各写一遍：与 `official_meta` 同一条理由——让十个插件各写
        "怎么读磁盘、比哪些字段"，第九个一定漏比某个字段，而漏掉的那个正是漂移藏身的地方
        （本轮实测就藏在 `expected_revision` 与 `exposure` 上）
      · 防回潮 = `check-single-source.mjs` 新增 **SS7 / SS7b**（用例条数与清单在场数的期望都取
        **插件目录数**，不是写死条数 ⇒ 加插件不结对账当场红）+ 两条下限（内置插件数、插件 src 面文件数：
        `plugins-src` 整目录改名时两边会一起数到 0 而"相等"，这种"一起消失"只能由下限兜）。
        `make selftest` 现 **11 档全中**
      · ★ SS7 的第一版变异**打空**：期望"把一个用例改名 ⇒ SS7 红"，实测仍 `10|10`——因为我数的正则
        `fn manifest_json_and_describe_agree` 连改名后的 `…_retired` 照样命中（子串包含）。
        判据自己太宽就盯不住任何东西 ⇒ 收紧成 `fn …(` 这个**定义形状**，并把这次打空写进 SS7 的说明里
      · 顺带改四处代码侧文案（规则 = 哪份信息更全就把它升为权威，而不是机械"一律取代码"）：
        tool-echo 的工具描述补回 modes 列举、tool-shout 补"repeat 模式自 1.1.0 起提供"、tool-shell
        取清单那份更细的两态文案、**tool-mcp 去掉运行期计数**（原描述用 `&format!` 拼"本次发现 N 个"，
        审计批 F-21c 把它记成"设计使然的例外"，其前提是展示文案有消费者——实测**展示面零消费者**：
        UI 读磁盘清单、`plugin-host` 只用 `.tools`，而"发现了几个"本来就由 `mcp_status` 报）
      · 变异两发各打中不同面：① 把 tool-fs 代码里 `fs_read` 描述改一个词（filesystem → file system）
        ⇒ 红在 tool-fs 那条对账，报的就是 `fs_read.description`；② 把 memory-sqlite **清单**里
        `memory_write` 的 `exposure` 从 `always` 改成 `on_demand` ⇒ 红在那一条并解释后果
        （"少了 `always` 就是把它踢回候选池，可能被 Top-N 挤出目录"）。两发都逐字节写回 + `cmp` + `sha256sum`
      · 界面侧：`read_display` 与前端**一行未动**，变的只是数据（清单里 `side_effect` 从裸串 `"Pure"`
        变成 tagged 对象、`exposure` 显式写出、工具描述变长）——新形态能被同一份
        `PluginManifest::from_json` 解析，已由十条对账用例实证；市场页徽章要不要改成运行期来源
        留在 ADR-0006 的后续里等你定
      · 两侧读数（同一份代码）：Windows `sh scripts/tasks.sh check` 末行"check 全绿"——
        workspace **534**（= T20 的 519 + 基座 5 条 + 对账 10 条）、`desktop/src-tauri` **584**、
        vitest **113**、`check_length` 超限 21 · 名单 21 · 零新增；产物重跑 `build_plugins.sh` rc=0，
        暂存面 `plugins/tool-fs/0.1.6/manifest.json` 里 `expected_revision` 已在（与源文件只差
        `version` 那一行 = 脚本 sed 注入，`cmp` 定位到 byte 40 line 3）；`demo` 与 `demo4` rc=0、
        demo4 三条自判读数不变。Linux（本机 WSL）`cargo test --workspace --no-fail-fast` **rc=0**、
        **533** 条通过（= 518 + 15）、39 个 test target 全 ok、十条对账在 Linux 也各 ok；
        两侧差的仍是那一条正当的 `#[cfg(windows)]` 大小写折叠档
- [x] **T12** 编译产物清不清（两份 `target/`）（2026-10-18 用户点头收账：给取舍依据并判定**不清**）
      · 不可逆的时间成本（清完下次全量重编很久），**需你明确同意**，Q8 已二次确认推迟
      · 实测（本机 `du -sh` + `df -h /f`，2026-10-18）：`target` = **25G**、`desktop/src-tauri/target` = **42G**，
        合计 **67G**；盘是 1.9T、已用 546G、**可用 1.3T（使用率 30%）**。
        顺带证伪本页自己：条目原写"约 57 GB"，那是登记时的数，五个批次之后已经漂成 67G ——
        会漂的数钉进文档就是会漂，这条也顺手改成现查
      · **判定 = 不清**，理由用上面那三个数：在还剩 1.3T 的盘上腾 67G（≈4% 容量），换来的是
        下一次**全量**重编的净时间损失，而本仓每次收口要在 Windows 与 WSL 两侧各跑一面 500+ 条
        用例的 `cargo test` —— 空间不是瓶颈的时候，清产物只把成本从"磁盘"搬到"每个批次的等待"
      · 这不是把账撕掉：要清随时一句"清"，动作 = 两个目录分别 `cargo clean`（我**没有**执行，
        也不会在没单独点头时执行 —— AGENTS §5 把删除类列在"先向人确认"里，总授权不等于点名授权）
      · 取证方式记一句免得下次重踩：`du -sh` 在 67G 上要跑 **7 分钟以上**（本仓第一次跑它超时被转后台
        才出结果），要量的话直接放后台，别把它接在一条要判退出码的门禁链里
- [x] **T13** 工具级取消与 deadline 真接线（审计 H5）（2026-10-18 用户点头"剩下四条也收账"⇒ 三问代答 + 第一刀落地；**欠账换页记到 specs/04 那一页**，与 T9 同一条处理方式）
      · 本条在台账里的欠账原本是两件：**设计稿先行** + **三问等他答**。两件都清了：
        设计稿 = [`specs/04-tool-cancel-deadline`](../04-tool-cancel-deadline/spec.md)（上一批已成文），
        三问 2026-10-18 由我代裁定并把答案与**各自代价**写回该稿（(a) 复用 error 通道 /
        UI 要加"已取消"但实施排 C5 / 停止后显示"取消中…"直到真结束）——**他随时可以翻案，
        翻案只改 C2–C5 的走向，C1 已做完的半刀与三答无关**
      · 第一刀 C1 落地（不碰 ABI、不碰界面的那半）：新模块 `crates/kernel-agent/src/active_calls.rs`
        （登记表 + RAII 守卫），登记点 = `kernel.rs` 进链调用之前，置位点 = 宿主
        `commands.rs` 的 `agent_stop` 第四路（带 `cancelled=N` 的 info 日志 = C4 要的叙事读数出口）。
        子内核与父内核**共用同一张表**（"什么在途"是一件事实，不该有两本账）
      · 两处与设计稿原文不一样，都是取证后改的并写在稿里：① **不需要** `stop.rs` 那句
        `Arc::ptr_eq` 换代比较——键里带了 `call_id` + 登记序号，每次登记各占一位，
        "上一轮注销误删下一轮"在结构上不可能；② 登记的是 `CallCtx` **克隆**而不是内部
        `Arc<AtomicBool>` 句柄——取句柄要给 `plugin-api` 加新公开面，那是 [LOCK] 区，
        而 `CallCtx: Clone` 内部是 `Arc` ⇒ 克隆与工具手里那份是同一个标志位，等价且 ABI 零改动
      · 有意不收的两侧（不是漏）：headless 的普通 `step`（无会话身份）不登记——停止命令本来就
        带 `session_id`，没有一条通路会去取消它；`compensation.rs` 的补偿调用**故意**不登记——
        取消一条正在回滚的写操作得到的正是该稿风险① 的"半途状态"，代码注释里写明是有意
      · 验收物 = `crates/kernel-agent/tests/active_calls.rs` **4 条行为用例**（替身工具把"早停"
        与"跑完"做成 JSON 上可分辨的两种形状；另一侧任务先等工具真进循环再点停止，
        有界 2 秒且到点仍断言非零——否则取消打在空处、用例绿得什么都没测）
        + **四发变异各打中自己那条**：只登记不置位 ⇒ 红 3 条（含设计稿点名的那条）；
        `track` 后立刻 `drop` ⇒ 独红在"在途收到取消"；去掉登记序号 ⇒ 独红在"重叠登记不互抹"；
        无会话身份也登记 ⇒ 独红在"跳过不登记"。还原全部逐字节 `cmp` + `sha256sum`
        （active_calls e3d4320811ac / kernel.rs 9652dd2827d9）
      · **禁区边界守住的一件事**：C3 要往 `HostVtable` **末尾**追加 `call_cancelled` 槽，
        形式上属 append-only、`PLUGIN_ABI_VERSION` 可不动，但该稿明文"要过 ABI 的那半
        每一刀单独等你点头"，且 AGENTS §5 把 `crates/plugin-api` 列为 [LOCK] ⇒ **我没有自决做**，
        也没碰任何既有签名。C2（`deadline` 生产者）同样没做：它的前置是"先测出审批卡超时与
        全局 `callTimeoutSecs` 在现网的实际关系"，没测就接会把所有调用提前掐死
      · 读数（同一份代码两侧各一次）：Windows `cargo test --workspace` **542 通过 / 0 失败 / 1 ignored**、
        `desktop/src-tauri` **584 / 0 失败**、`make check` 末行 `▶ check 全绿`、
        `demo` / `demo4` rc=0 且 demo4 三条自判读数不变（danger 执行 1 次 / 被拒闭合 1 条 /
        echo 吃到改写后的 `mode=true`）。Linux（本机 WSL，热 target 的 clone 先补
        `build_plugins.sh` + `build_market.sh` 两条前置，产物落点 10 + 2 个 `.so` 已核）
        `cargo test --workspace --no-fail-fast` **rc=0 · 40 个 test target · 541 通过 / 0 失败 /
        1 ignored**，本刀那四条 `active_calls` 在 Linux 上逐条 ok。
        **两侧差的那一条按用例名字取证，不靠记忆也不靠上一批的结论**（现查：两侧各跑一次
        `cargo test --workspace --no-fail-fast`，抽 `^test .* \.\.\. ok$` 的名字排序后 `comm` 比）：
        只在 Windows 跑的是三条（`tests::keyside_norm::windows_spelling_variants_all_land_on_one_row` ·
        `wskey::tests::windows_collapses_case_and_slash_style_into_one_keyside` ·
        `wskey::tests::missing_and_existing_projects_agree_across_the_two_branches`），
        只在 Linux 跑的是两条（`tests::keyside_norm::posix_keeps_two_spellings_apart_because_they_are_two_directories` ·
        `wskey::tests::posix_never_folds_case_and_never_treats_backslash_as_a_separator`）
        ⇒ 净差 = 3 − 2 = **1**，正是 542 与 541 那个差；五条全是路径大小写折叠 / 反斜杠分隔符
        这一族的**方向性**断言（POSIX 上反斜杠是合法文件名字符，硬要"两侧同数"就得写恒真断言）。
        ★ 顺带订正一处我自己写错的读数：上一提交正文写"两侧仍差那一条 `#[cfg(windows)]` 档"，
        把 3−2 说成了 1 条单侧档——数字对、**形状说错了**，按名字比才知道 POSIX 侧也有两条自己的
        对照（那句是从 T11/T2 的正文里抄来的，抄的时候没复跑 = 本仓第 116 条那一族）
      · 剩下的 C2/C3/C4/C5 不在本台账里记，只记在 specs/04 的勾选框里（进度唯一出处）；
        写设计稿时顺带查出的那条文档失实（AGENTS §3 说"内核八条设计纪律"而 `DESIGN.md`
        附录 C 实测 **11 条**）也仍挂在 specs/04 风险③，**不在这条里顺手修**——
        `AGENTS.md` 自己不是 [LOCK]，但那条数字的对错该由读 `DESIGN.md` 的那一刀一起定，不是现在

- [x] **T14** 逐批验收与证据核对（2026-10-18 用户点头"剩下四条也收账"⇒ 清单出稿，本节就是交付物）
      · 交付物 = 本页上面那一节「按批验收清单」：A 组是人核表（打开哪 · 点哪 · 该看到什么 ·
        不对的话说明哪一种缺陷形状），B 组是复跑表（一条命令对一个末行读数），
        再加「没有可核对象」那一小节把 T9 / T12 这类**验收物本身还不存在或判定就是不动**的
        两条说清——不放"看看代码"这种没有判读的条目充数
      · 每条读数都是收尾那一遍**在同一份代码上现跑**的，不从各刀提交正文里抄：T3 的分发表 27 条
        在改后仍在 151 条总量里、SS9 现读 `6|6`、C1 那四条 `4 passed; 0 failed`、
        对账合计 10、`make check` 末行 `▶ check 全绿`。会漂的数字不钉进正文（第六道闸那条纪律），
        要现查就读那一列命令
      · 出稿时订正了自己两处失实（都写在对应条目里，不悄悄改）：① T11 那行原本说"改
        `plugins-src/` 立刻能看到"，实测展示面读的是**暂存目录** `plugins/<id>/<ver>/manifest.json`，
        不重跑 `build_plugins.sh` 不会变；② T18/T20 那行原本要求「**0 ignored**」，实测有 1 条
        是 `plugin-api/src/meta.rs:215` 的 doctest `ignore` 围栏（文档形状示例，不是平台跳过）
        ⇒ 改的是**口径**不是代码，把示例假装成能跑的测试才是造假
      · 本刀还补了 A 组缺的那三条（T10 四处 Esc / 链没被自己的补丁堵死 / T4 实验室夹具），
        以及 B 组缺的三行（T10 两闸、T4 产物、T13 C1）；T13 的 C2–C5 不在这张表里，
        它们没做完，验收物也不存在——进度只看 specs/04 的勾选框
      · **仍然欠的一件事，要说破**：A 组这几条我**没能在真实窗口里替他点过**。要跑通需要一条
        真实 pending 审批 + 子代理活动 + 通知记录，我用 mount 真实 `Composer` 的两条用例
        （`desktop/tests/esc-routing.spec.ts`）替了这半证 —— 那是"两个消费者抢同一键"这个缺陷
        的可执行反证，但它不是眼睛看到的画面。剩下的按 A 组那张表你来点，我按你说的改

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

- [x] **T22** 拆 `desktop/src/stores/chat.ts`（1148 行）⇒ 同目录 11 片（2026-10-18 用户一句
      「超限那么多，能拆肯定是拆啊。除非是必须不能拆的」⇒ 本条原文那句"正确终态 = 拆完删行"兑现）
      · 三问按 `check_length` 自己的口径重跑：①只管一件事？**否**（六族同住，与本条原登记一致）
        ③改它常牵动无关部分？**是** ⇒ 拆，不在名单里长留
      · 终态：`stores/chat/` = types 167 · state 87 · ids 9 · getters 64 · **projection 321**（最长片）·
        notices 101 · session 185 · config 89 · attachments 103 · send 191 · index 190（只剩装配 + 薄转发）；
        名单里 chat 那行**已删** ⇒ `check_length` 超限 **20 · 名单 20 · 零新增**
      · 形状选型（本条真正的取舍，记下来防下一个人重走）：片函数一律收 `st: ChatState`、action 里
        `return chatXxx.yyy(this, …)` 薄转发；**不**把 actions 拆到各片继续用 `this` —— 那要给每片写
        `this: ChatStore`，而 ChatStore 的类型又来自本文件的 defineStore ⇒ 类型自引用环（TS 7022/2502）。
        副带好处：片间断了"靠 `this` 的隐式全通道"，谁调谁变成显式方向（projection → send → session）
      · 公开面一条没减：39 个 action · 3 个 getter · 21 个 state 字段 · 7 个导出类型 ⇒ **41 处消费点
        一行没改**（`moduleResolution: bundler` 把 `../stores/chat` 解析到 `chat/index.ts`）
      · 顺序与本条原"建议线索"不同（先动的是类型面与状态面，不是会话生命周期）：因为先要验两件事
        ——目录化后消费面到底改不改（刀一）、`this` 那面墙撞不撞得开（刀二的形状选型）——
        这两件任一失败都要换方案，所以把它们排在前面，而不是按职责从外往里切
      · 等价与自证：`tmp/t22-equiv2.mjs`（用 typescript 自己的 parser/printer 规范化函数体，不用正则
        剥注释：这批文件里有正则字面量与模板字符串，正则版必咬错）跑四条判据 = 公开面名单相等 ·
        状态面三方同序 21=21=21 · 新旧单元双向有归处（改名只登记 `onEvent→applyEvent`）· 42 个单元体
        **逐字符相等**；`tmp/t22-mut2.mjs` 四条窄变异各红在指定一档（状态面键名漂一个字母 / 删重放守卫 /
        装配点改名 / 片里多个没人认领的导出）= `MUT_ALL_CAUGHT`
      · 判据自己空转过一次并被当场抓到：`= (): ChatState => ({…})` 的 initializer 要连剥
        ArrowFunction 与 ParenthesizedExpression 两层才到对象字面量，前两版各漏一层 ⇒ 读数 0=0 是
        **假绿**；现补"任一侧解析出 0 个键即判红"+ 实际比对数下限 42
      · 必须同批核的闸：`check-channels` 拿 `desktop/src/stores/` 里的 `case "<kind>"` 当**前端消费面**，
        它的 walk 实测是递归的 ⇒ 拆完承重读数 `HANDLED=21` 一字不变（只有扫描面 `STORES_SCANNED 12→22`）；
        不验这一条，拆分就等于把事件消费面静默拆掉一半
      · 门禁：`vue-tsc --noEmit` rc=0 · vitest 17 文件 / **151 条全绿**（一条测试没改，含 T3 那 27 条
        投影分发表与 Q12 的 8 条工具行相位）· `pnpm build` rc=0 · `make quick` 全绿 · 前端形态 96 文件干净
      · 两件**不在本刀做**（都属删除类，未点名不办）：① 对外 re-export 那 7 条类型里实测只有 4 条真被
        外部 import（ChatItem / SystemItem / ToolItem / AttachmentItem）；② 39 个 action 里若干条外部零引用
        （它们本来是为内部 `this` 调用而暴露的）。摘哪几条等你点头，我不在拆刀里顺手收窄导出面
      · 又踩一次 **prose 动闸读数**（本仓第三次，前两次的形态记在 T4 条目里）：我在 `projection.ts`
        的头注释里用反引号写了那个重放守卫的字段名，`check-i18n` 的"间接豁免"把反引号里的点分字面量
        也算进计数 ⇒ 读数 1129 → 1130。改措辞还原。**注释里不许把字段名写成"点分 + 引号"的形态**，
        否则闸的读数不再只反映代码；定位方法 = 把改前改后两侧的命中做多重集差，一条就点名
      · 跨平台：纯前端 TS 结构面，不涉及路径语义与插件生命周期 ⇒ 本刀不取 WSL 侧证据（口径见 ADR-0005）

### 三、超限名单 20 件的逐件三问判定（2026-10-18 用户口径「能拆肯定是拆啊，除非是必须不能拆的」）

判定做法：四路并行**通读文件**（不是我按行数猜），每件把三问各附证据 —— ①列职责簇带行号与体量、
③举一个真实耦合；每件结论写在下面那张表的「判」列里（**不在这里钉"拆几件/留几件"**：T23 落地后
已经有一件重判，这种数一次就漂一次），**留的每一件都给不许用行数的硬理由**。
判定顺带数出三笔新欠账 ⇒ 当场登记 T23/T24/T25（勾没勾看框，不看这句话）。

| 文件（现行数） | ①只管一件事 | ③改它常牵动无关部分 | 判 | 依据与切法（一句） |
| --- | --- | --- | --- | --- |
| `src-tauri/src/memory.rs`（916 → 48） | 否：5 簇 | 是：`KEY_MAX`/`VALUE_MAX` 三段校验**逐字重复**（304/365/475），upsert SQL 两份（374/482） | 拆 ✅T33 | 已沉 `memory/{shapes,store,crud,reconcile,recall,schema,source}.rs` 七片（判定时写的是四片 `{admin,reconcile,inject,source}`：一片要装四件事 = 违 ①，落地按面开七片，`admin`→`crud`、`inject`→`recall`+`source`）；建表串进 `shapes`、迁移进 `schema` ⇒ **T23 那道"整根恰 1 处"的闸第一次真起作用**，出处读数自己跟着搬、判据一行没改；14 处 `pub(super)` + 6 行 impl 外壳全部枚举在 spec 里；那两组重复本刀没并，新登 **T34** |
| `src/lib/api.ts`（814） | 否：10 个后端子域 + 一张通道表，域间零共享符号 | 是：加一个 `ChatUpdate` 字段要与两处探测/流式走同一 diff | 拆 | 域包装下到 `lib/api/*.ts` 再 `export *`；通道表在 `lib` 整根里**恰好 1 处**即可（T23 起跟定位，不必钉在 api.ts） |
| `components/chat/Composer.vue`（835） | 否：7 簇（模型 chip / 推理档 / 斜杠面板 / 附件 / 输入与发送键状态机 / 坞卡贴合 / 坞高上报） | 是：附件条高动一次 ⇒ `App.vue` 的 composerH 与转录回底钮位跟着算 | 拆 | `ComposerAttachments.vue` + `ModelChip.vue`；**Esc 让位链与发送键必须留本文件**（SS9b 判的就是"只许 1 个出处"） |
| `settings/model/ProviderSetupDialog.vue`（650） | 否：4 簇，其中自定义请求头编辑器 ≈117 行、自带三态四操作 | 是：`maybeAdoptSweep` 写 baseUrl → watch 再探测 → 喂 ModelPanes | 拆 | `HeaderEditor.vue`（≈140，v-model 整表）；落值那条"整表替换 + 空键跳过"只许一处实现 |
| `settings/model/ModelPanes.vue`（616） | 否：4 簇 | 是：`mergeCatalog` 的 watch 触发 `emit("update")` 回父置 touched = 跨文件回环 | 拆 | `ModelAdvancedRow.vue`（≈190）；`pinEdit` 那条"编辑即钉死 sources=user"只许一处 |
| `lib/tool-detail.ts`（612） | 否：两个渲染面 + 三套表 | 是：`shell` 系三个工具名在三处各写一遍（67/131/512）⇒ 加一个工具改三处 | 拆 | `kinds` / `labels` / `blocks` 三片 + 主分派留 index，导入路径不变 |
| `plugins-src/memory-sqlite/src/store.rs`（581） | 是：存储层四簇同族 | 是：同一条 UPSERT 两份抄写（276/522） | **拆**（T23 后重判） | 原判"留"混进了一个外部约束（闸只读这一个路径），而三问里 ③ 本来就答"是"⇒ 约束一消失就该回到三问：`store/{schema,open,crud,undo,mdsync}.rs`，顺手把那份 UPSERT 抄写并成一处 |
| `settings/LlmSection.vue`（577） | 是：七块行全走 SettingsGroup+SettingsRow 唯一形态 | 是，但方向相反 | **留** | 该文件实测 5 处组件直调 IPC + 4 处 `settings.refresh()`，已偏离「编排只在 stores」（新登 **T24**）⇒ 先收编排，拆是后面的事 |
| `llm/client.rs`（575 → 493 · 仍留名单） | 半：装配 + 重试环 + 单次尝试是一件事，逐模型 ping 是另一件 | 是：`repair_*` 修复旗标跨 6 处 | 拆（薄）✅T32 | 只切走 ping 那条旁路 = `llm/client/test_probe.rs`(105)，**主环一字未动也没再切**（再切就出现第二条线路形状，违「同类只许一种形态」⇒ 本件留在名单里，理由在此列与提交正文）；表写的落点是 `llm/test_probe.rs`，落地改成 `client/` 的**子模块**更好：私有超时常量经 `super::` 就够得着 ⇒ 0 处可见性抬升、`llm/mod.rs` 不必动 |
| `llm/wire.rs`（548 → 24） | 否：预检 / 媒体投影 / 序列化 / 累积器 / usage 五簇 | 是：预算参数在 80/128/307/372 四处 | 拆 ✅T31 | 已沉 `llm/wire/{payload,media,messages,stream}.rs` 四片（五簇落四片：usage 只有 `parse_usage` 一个函数、只被累积器调用，单立 = 为行数造结构 ⇒ 并进 `stream`）；跨片 2 处 `pub(super)` 抬升枚举成声明式改动；判据里那句"`super::wire::` 遮蔽有案底"复核结果 = **没发生**（tests 的 9 处引用靠 glob 再导出照旧解析，见 T31 条目的磁盘变异） |
| `chat/TranscriptFlow.vue`（540） | 否：约 180 行**零 DOM** 的投影算法住在组件里 | 是：inner 单块分支与 tail 两段**逐字重复** ⇒ 新增一类 kind 必改两处 | 拆 | `use-flow-projection.ts`（≈190）+ `FlowSingle.vue`（≈60）把两遍并一遍；composable 不触形态标准 |
| `src-tauri/src/events.rs`（531） | 是：通道常量 + 事件 enum + `channel()` 映射 = **同一张契约表** | 是，但那是这张表**固有的形状**（加一条事件本来就要同时动三处） | **留** | 三问皆是的一族：一份契约的三个侧面，拆开 = 把一条事实摊成三个出处。T23 之前它还多一条"拆了闸会安静少一条"的外力，现在那条已消（C4 实测数得到）⇒ 本件**只按形态判**，判留 |
| `settings/DeveloperSection.vue`（523） | 否：写值区 + 三个**只读观测面板**（⑫ 暴露调试 / 缓存诊断 / reconcile 触发） | 是：`refresh()` 拿写值字段 `cacheDiagnostics` 当取数门 | 拆 | `ExposureDebugPanel.vue` + `CacheObservationPanel.vue` → 主件 ≈360；先例就在同目录（`MemoryTreePanel.vue` 116 行是从它拆出去的） |
| `commands/llm.rs`（523 → 30） | 否：清单+推荐 / 连通测试 / 代理与档位 CRUD / 目录诊断 四簇 | 是：`enrich` 与 `apply_dialect_ladder` 被两簇共读（44/298/478） | 拆（只切面）✅T30 | 已沉 `commands/llm/{views,models,probe,test,providers,catalog}.rs` 六片：共读段单立 `views` 片（不塞进命令片当"顺手的公共文件"），`清单` 与 `推荐` 分家；`apply_dialect_ladder` 抬 `pub(super)` 是本刀**唯一声明式改动**；`llm_provider_save` 的业务下沉仍归 **T25**，本刀没碰 |
| `src-tauri/src/commands.rs`（519 → 56） | 否：9 簇（聊天与停止 / undo / 列表投影 / 审批与规则 / 设置 / 探测 / 暴露调试 / 原生对话框 / 通知） | 是：跨文件调 `sessions::ensure_associatable`，还有 `cfg(test)` 补丁式再导出 | 拆 ✅T26 | 已沉 `commands/{chat,approvals,settings,probe,dialog,notifications}.rs` 六片（判定时的 4 片切法里 `system` 一片装四件事 = 违 ①，落地时按簇分成四片）；`check-commands.mjs` 走目录扫全部 .rs ⇒ **不动任何闸**，实测该闸在本刀后读数不变 |
| `memory/TreeSection.vue`（505） | 否：会话导航与档位 / 章节展开与原文缓存 / M5 编辑面 三簇 | 是：`loadBundles` 一次重置六个本地态，新增任一编辑态都要动它 | 拆 | `use-tree-chapters.ts`（≈120，先例 = 同目录 `use-model-probe.ts`）；保存就地改引用与两步删除互斥必须同处 |
| `commands/sessions.rs`（474 → 35） | 否：元数据 / 删除与搜索 / 导出与标题 / 截断与悬空裁决 / 子转录与外壳 五簇 | 是：忙守卫文案两份（296/326）、`fsRoots` 校验两份（32/434）、shell 三分支两份（403/452） | 拆 ✅T27 | 已沉 `sessions/{meta,lifecycle,export,heal,transcript,shell}.rs` 六片（判定表把"子转录"与"外壳"合成一片 = 违三问 ①，落地拆开）；那三组重复本刀**没并**（纯搬行不许夹带），新登 **T28** 单独一刀 |
| `chat/MessageRow.vue`（468） | 否：附件部件簇与 `ToolRow.vue` 的 thumbCache/thumbOf **逐字同形** | 是：`.msg--brief` 的 padding 归零靠 TranscriptFlow 的兄弟规则补（注释自陈实抓过） | 拆（薄） | `lib/use-asset-thumb.ts`（≈35，ToolRow 同用）+ `MessageParts.vue`（≈95）→ 余 ≈330 |
| `tests/chat-projection.spec.ts`（456） | 是：5 个 describe / 24 条用例只驱动同一个 `onEvent` 入口 | 是：共用夹具 `rowOf`/`kinds`/`fresh` 是用例立得住的前提 | **留** | 拆开要么复制夹具、要么新造共享夹具 = **为了行数造结构**（前端侧也没有"测试目录不许有用例"的等价规矩可依据） |
| `chat/PermissionCard.vue`（451） | 是：倒计时 / 硬门 / 决策防重 / 分体钮与 scope 上弹 / 预览元信息，都只服务这一张卡 | 否：改细条不动按钮 | **留** | 卡面归 Composer 的 `dock-cards` 持有，本卡刻意不 `overflow:hidden` 让上弹冒出卡顶；再包一层直接打断贴合柱面，上弹皮属 §二③ 唯一浮层式 |

· ★ **本节最值钱的一条不是那张表，是判定过程中摸到的三道锁**（⚠ 本段写于 T23 落地之前 ——
  那段锁**已被 T23 解掉**，留着是因为它记的是判定当时摸到的事实；四件的重判结果在 T23 条目末尾）：
  四路结论里有三件的"留 / 切法"被同一个
  形状卡住 —— 闸把**文件路径钉死了**。实测（不是推断）：`check-channels.mjs:25-26` 写死
  `desktop/src-tauri/src/events.rs` 与 `desktop/src/lib/api.ts`；`check-schema-parity.mjs:25` 写死
  `memory.rs` 与 `store.rs`。所以：① 拆 `api.ts` 只能搬域包装，通道表与专用 `listen` 一动就是
  BLOCK 或 R4 假红；② 拆 `memory.rs` 时建表串得留在主文件才不动 B1；③ `store.rs` 的 25–151 行
  根本搬不动；④ `events.rs` 拆出 sink 是**安静少一条**那种坏法。⇒ 这四条锁本身可以解，
  `check-i18n.mjs` 已有现成先例（从装配点读它真正 import 了什么 ⇒ 语言包再怎么挪闸跟着走），
  登记成 **T23**；解掉之后表里"留"那 5 件至少 2 件要重判。
- [x] **T23** 两道对账闸的硬编码路径改成"跟随定位"（2026-10-18 用户「认了，从 T23 开始按你排的顺拆」⇒ 本节的锁由这一刀解掉）
      · 只动闸，不动被管代码（工作区实测就 `check-channels.mjs` 与 `check-schema-parity.mjs` 两个文件）：
        两条死路径 ⇒ 扫描根（后端 `desktop/src-tauri/src`、前端 `desktop/src/lib`、插件侧
        `plugins-src/memory-sqlite/src`），锚在整根里必须**恰好命中 1 个文件** —— 0 处 = 闸瞎、
        2 处 = 第二个真相，两种都响亮失败；定义处所在文件进读数（`DEF_IN=`、「出处 建表 … ⇄ …」）
      · ★ 解锁之外白赚两件**判据变强**（本刀真正的收益不是"方便拆"）：① `B1`/`B2` 的"建表串只许一个出处"
        从"在被钉的那个文件里数"变成"整侧数" —— 旧版对别的文件里再内联一份建表串完全看不见；
        ② `DIRECT_EMIT` / `DEDICATED_LISTEN` 改整根扫 —— 旧版把 sink 搬出 `events.rs` 会**安静少一条**
        （假绿），现在数得到（能力档 C4 实测）
      · 读数与改前**逐字相同**（改造只许不动语义）：`CONSTANTS=23 VARIANTS=22 MAPPED=22 SUBSCRIBED=19
        HANDLED=21 WHITELIST=2 STORES_SCANNED=22 FAILED=0`；新增 `RUST_SCANNED=276 FRONT_SCANNED=31
        DIRECT_EMIT=1 DEDICATED=8`；schema 十档 `SCHEMA_PARITY_OK`
      · 自证两批：内存 `--selftest` 十档各红在自己那档（`SELFTEST_ALL_CAUGHT`）；**定位档只能上磁盘**
        ⇒ `tmp/t23-mut.mjs` 七条：宿主/插件各一发"另一文件里第二处定义"、一发"根换到没锚的子树 = 0 处"、
        前端表两发、后端表一发，加一条**期望是绿**的能力档 C4（把 `emit(channels::…)` 的证据放进新文件
        ⇒ `DIRECT_EMIT` 1→2 且仍全绿）= `T23_MUT_ALL_CAUGHT`；临时文件由 `finally` 删除，复跑基线
        证明回到 `DIRECT_EMIT=1` 且三条临时路径都不存在
      · 当场抓到并修掉一条**扩面副作用**：`E1` 旧取材 = "整侧所有含 SET 的行拼起来再测"，扫单文件时
        恰好没事，扩到整侧后别的表的 upsert 被吸进来 ⇒ 判成 dirty（假红）。修 = 取材锚到语句本体
        （`INSERT INTO memories … DO UPDATE SET …"`），并要求每份 memories upsert 都干净。
        口径记在这儿：**扩面最容易把"判据变宽"当成"判据变强"**，宽错方向的扩面等于换了判据
      · 另两处口径变化说破：`order()` 由"在那一个文件里比先后"变成"整侧首次出现比先后"（实测仍 ok/ok；
        今后若别的文件先出现 learning 调用会假红，那时按 open 路径逐函数比，不许放宽期望）；
        `DEDICATED=8` 的扩面方向核过 —— 全仓 `listen<` 实测**只在 api.ts**（9 处 / 8 条唯一通道）⇒
        当前代码上零语义变化，将来组件直接 listen 本来就该算"有人接住"
      · 门禁：`make quick` 全绿 · `make selftest` 全中 · 两闸直调 rc=0 · 零被管代码改动
      · ★ 下面那段"三道锁"是判定当时的事实，**自本刀起作废**（原文留着标日期，不抹）：
        `store.rs` 重判为**可拆**、`events.rs` 仍判留但理由只剩"那张契约表本就是一件事"；
        `memory.rs` 与 `api.ts` 的切法不再有"某段必须留主文件"这种约束
- [ ] **T24** `LlmSection.vue` 里 5 处组件直调 IPC + 4 处 `settings.refresh()` 收进 `stores/`
      · 违 AGENTS §3「状态与 IPC 编排的唯一去处是 stores（组件不许自己 apply）」；实测出处 =
        该文件 103/123/249/297/314 行直调、298/315/323/566 行 refresh
      · 这条是 T22 判定顺带数出来的（不是"感觉违反"）；把它列成独立一条就是**不**在拆刀里顺手改
- [ ] **T25** `commands/llm.rs` 的 `llm_provider_save` 就地做档位装配 / keyring 写 / 清单去重，业务进了命令层
      · 违 §3「Tauri command 薄适配层：只做参数转换，不写业务」；实测 = 该闭包 327–407 行 ≈ 80 行
      · 下沉目标 = `src-tauri/src/settings/llm.rs`；与 commands/llm.rs 的切面分开两刀，别混
- [x] **T26** 拆 `src-tauri/src/commands.rs`（519 → 56 行只留声明）⇒ 同名目录加六片（2026-10-18 第一批第一刀）
      · 六片按**簇**分：`chat`(179) = 跑轮次 / 停止 / 回退压缩 / 会话列表 / 全量投影 ·
        `settings`(142) = 读写 + 保存后的热更新广播 · `approvals`(56) = 审批决策 + 粘性规则 ·
        `probe`(60) = 探测与诊断读数（全不落盘）· `dialog`(56) = OS 原生对话框 · `notifications`(31)。
        判定表里预写的四片切法有一片叫 `system`，装的是"探测 + 对话框 + 通知 + 调试"= 违三问 ①，
        落地时按簇拆成四片 —— **表的切法是草案，不是合约**
      · 公开面与消费点：`commands.rs` 靠 `pub use X::*` 转发，`lib.rs` 的 `generate_handler!` 与
        前端 invoke **一个字没改**；`check-commands.mjs` 读数与切割前逐字相同
        （`定义=88 · 注册=88 · 前端引用=88 · 预留名单=0`）
      · 等价判据（本刀不吃"我以为搬对了"）：切割脚本 `tmp/t26-cut.mjs` 落盘前判三条
        （区间无缝覆盖 48–519 / 每片正文是原文连续子串 / **非空行多重集逐种守恒 445 行**，任一红就不写盘）；
        事后对账 `tmp/t26-verify.mjs` 五档 T1–T5 独立吃磁盘（原文备份 + 六片 + 装配文件），
        **不复制区间** —— 区间只在切割脚本出现一次，两处手写必漂
      · 窄变异各红在指定档：`--selftest` 五档 = `T26_SELFTEST_ALL_CAUGHT`（T1 下限与 T4 行守恒**同因共红**，
        已登记为 `EXPECT_CO`，不伪装成独立档）；另加一条磁盘变异 `tmp/t26-mut.mjs`：把 `commands/chat.rs`
        里一条命令改名 ⇒ `check-commands` 定义 88→87 且 rc=1（证"闸在子目录布局上是活的"，
        这是 T23 那批"闸钉死路径"教训的正面验证），逐字节写回 + sha 复核后回到 88/88/88 = `T26_MUT_ALL_CAUGHT`
      · 编译与测试：`cargo clippy --all-targets -- -D warnings` rc=0 · `cargo test` 584 passed / 0 failed（与改前同数）·
        `cargo fmt --check` rc=0
      · 闸与名单：`check_length.sh` 的 OVERLIST 删掉 `commands.rs` 那一行（现 超限 19 · 名单 19 · 零新增）·
        `STRUCTURE.md` 那行地图改成"commands.rs 只留 mod + pub use 声明"
      · ★ 两条踩坑说破（都写进本条就是为了不靠记忆）：
        ① **变异动作看着改了、其实没改判据吃的东西** —— T4 的对照第一版写 `bl.pop()`，删掉的是
        `split("\n")` 产物末尾那个恒有的空元素，非空行一条没少 ⇒ 判据仍绿（对照打空）。改成"倒序找
        最后一条非空行再删"才红。与历批"负对照打空"同族，新形态：**尾部空元素**
        ② **删实现不等于删干净** —— 切割按 48–519 划，把原文件第 15–46 行那批顶层 `use` 留在装配文件里，
        clippy 当场红 `unused_imports: std::sync::Arc`。装配文件的 use 面只能由编译器裁，不能由我以为
      · 跨平台：纯 Rust 模块结构面，不涉及路径语义与插件生命周期 ⇒ 本刀不取 WSL 侧证据（口径见 ADR-0005）
- [x] **T27** 拆 `commands/sessions.rs`（474 → 35 行只留声明）⇒ 同名子目录加六片（2026-10-18 第一批第二刀）
      · 六片 = `meta`(140) 元数据写侧 · `lifecycle`(63) 删除与搜索 · `heal`(116) 截断回退与悬空裁决 ·
        `export`(89) 导出与标题三层 · `shell`(85) 打开目录与链接 · `transcript`(32) 子 Agent 转录。
        判定表那行把"子转录"与"外壳"写成一片 = 违三问 ①（两件事只共用"都碰文件"这一点），落地拆开 ——
        又一次证明**表的切法是草案不是合约**
      · 公开面与消费点：`sessions.rs` 用 `pub use X::*` 转发 ⇒ `lib.rs`、前端 invoke、
        三个 crate 内消费点（`chat.rs` 调 `ensure_associatable`、`assets.rs` 与 `memory.rs` 调
        `open_in_explorer`、`tests_session_log` 用两个标题纯函数）**一个字没改**；
        `check-commands.mjs` 读数与切割前逐字相同（`定义=88 · 注册=88 · 前端引用=88 · 预留名单=0`）
      · ★ 判据从这一刀起**通用化**（`tmp/split-cut.mjs` + `tmp/split-verify.mjs` + `tmp/split-mut.mjs`，
        各刀只交一份 spec）：每刀重打一遍档位 = 制造"某档被悄悄放宽"的机会，判据只许写一次。
        切割器落盘前 C1–C3（区间无缝 / 连续子串 / 行守恒）任一红就一个文件都不写；
        事后对账 T1–T6 独立吃磁盘；通用版先回头跑 T26 那六片复现全绿，才拿来判本刀
      · 新档 **T6 保序**：各片正文按给定顺序拼接必须逐字节等于原文区间（去掉片间空行）。
        T4 只守恒**多重集**，把两片内容对调它看不见 —— 上一刀没有这一档，这一档是补上的
        （自测里 T6 的变异 = 整片对调，T3/T4 全绿、只有 T6 红）。T1/T3/T4 与 T6 **同因共红**已登记：
        T6 在数学上强于 T4，T4 留着只为失配时直接点名是哪一行漂了
      · 磁盘变异 `SPLIT_MUT_ALL_CAUGHT`：把 `sessions/meta.rs` 里一条命令改名 ⇒
        `check-commands` 定义 88→87 且 rc=1；逐字节写回 + sha 复核后回 88/88/88（= 本刀之后
        常驻闸在新布局上仍然扫得到，不靠"它本来就是递归扫的"这种相信）
      · 编译与测试：`cargo clippy --all-targets -- -D warnings` rc=0（**一次就过** —— 上一刀的
        `unused_imports` 教训落地为"装配文件的 use 面交给编译器，不靠自己数"）· `cargo fmt --check` rc=0 ·
        `cargo test` 584 passed / 0 failed · `make check` 全绿 · `check_length` 名单再减一行
      · ★ 两条**实测**发现，都不在"搬行"职责里 ⇒ 只登记不动（AGENTS §5「发现问题就登记」）：
        ① glob 再导出会把 `pub(crate)` 项一起带出来，可见性取"再导出声明"与"项自身可见性"的较小者。
          做法 = 把点名行注释掉再编译，`rc=0` 且消费点照旧解析（两次实验：`sessions.rs` 内点名、
          `commands.rs` 内点名，各自注释掉都仍然绿）⇒ 本刀不在 `sessions.rs` 写重复点名
        ② `commands.rs` 里台账 T6 那条注释的机制断言（"`pub use sessions::*` 只带得出 `pub` 项"）
          与 ① 的实测**相反** ⇒ 登记 **T29**（纯注释更正，另一刀）
      · 判定表点的三组「同一事实两个出处」（忙守卫文案 / `fsRoots` 校验 / `#[cfg]` 三分法）本刀
        **一组都没并** —— 并 = 语义改动，不许夹带在搬行刀里 ⇒ 登记 **T28**
      · 跨平台：同 T26，纯结构面不取 WSL 证据
- [ ] **T28** `commands/sessions*` 里三组「同一事实两个出处」并成一处（T27 判定表点的，本刀刻意不夹带）
      · 实测出处：忙守卫拒删/拒编辑重发/拒裁决的文案三份（`lifecycle.rs` 内 1 份 + `heal.rs` 内 2 份）、
        `fsRoots` 白名单校验两份（`meta.rs::ensure_associatable` 与 `shell.rs::reveal_in_explorer`，
        错误文案的**后半句**不同是刻意的，前半句同一句话）、`#[cfg]` 三分法两份
        （`shell.rs::open_in_explorer` 与 `shell.rs::open_external_url`）
      · 这一刀要答的问题：并成一处之后**用户看到的文案不许漂** ⇒ 需要先把三份文案的差异登记成夹具用例
        （同 T22 的做法：先有用例再动形状），不许"顺手统一文案"
      · 与拆刀分家：T27 已经把形状摆好，本片改的是**内容**，风险方向不同，红了好回退
- [ ] **T29** `commands.rs` 里台账 T6 那条注释把 glob 机制说反了（T27 实测推翻）
      · 现文字说"`pub use sessions::*` 只带得出 `pub` 项，这两个是 `pub(crate)`，所以在这里点名再导出"；
        实测（T27 条目 ① 的两次注释实验）glob 会按收窄后的可见性把 `pub(crate)` 项一起带出来，
        所以那条点名是**冗余**而非必需
      · 该注释的其余部分仍然成立（`#[cfg(test)]` 是必需的：非测试编译没人用这两条 ⇒ `unused_imports`
        ⇒ `-D warnings` 红），本条只更正机制断言；要不要顺手删冗余点名行由那一刀判，
        判据 = 删了之后 `cargo clippy --all-targets` 仍 rc=0 且 `tests_session_log/export_title.rs` 仍绿

- [x] **T30** 拆 `commands/llm.rs`（523 → 30 行只留声明）⇒ 同名子目录加六片（2026-10-18 第一批第三刀）
      · 六片 = `providers`(228) 代理与档位 CRUD · `test`(100) 连通测试两路 · `probe`(92) 表单探测与推荐 ·
        `views`(55) IPC 视图与目录合成的共读段 · `models`(58) 选择器清单 · `catalog`(46) 目录三件与诊断。
        判定表写的是四片切法，本刀落六片：`清单`（全档并行）与 `推荐`（单档扫描 + 预选）合成一片违三问 ①；
        被两簇共读的 `enrich_with_catalog` + `apply_dialect_ladder` 单立 `views` 片 ——
        **不塞进任何一个命令片当"顺手的公共文件"**（那样公共段会寄生在某一簇里，下一个人无从找）
      · ★ 这一刀起，拆刀获得**声明式改动**机制（`tmp/split-edits.mjs`）：搬行刀允许
        "除了枚举出来的这几处，其余逐字节相同"。为什么必须有：私有函数拆到兄弟模块必须抬可见性，
        抬完那行超 100 列又被 rustfmt 折成四行 —— 这是拆分的**机械后果**不是语义改动；
        没有枚举机制时只有两条坏路：判据假红，或为了绿放宽整行守恒（= 判据作废）
        口径三条：每条 `find` 在该段最多命中 1 次；各片分到的改动合计 == 声明条数
        （少 = 有条款没进任何片，多 = 进重了）；`edits` 写的是**折好行的终形**（否则"改完再让 fmt 改一遍"
        = 第二处未声明改动）。本刀的 1 条 = `fn apply_dialect_ladder(…) {` → `pub(super) fn apply_dialect_ladder(` 换行参数换行 `) {`
      · 机制自身的证明 = 自测里两发**定向**变异：X1"改一个词却没声明" ⇒ 红在 T3/T4/T6；
        X2"片里多一行原文没有的语句" ⇒ 红在 T3/T4/T6（`SPLIT_SELFTEST_ALL_CAUGHT（含 X1/X2）`）。
        改完判据先回归：拿 T26 / T27 已提交的产物重跑通用对账，两把都仍 `SPLIT_VERIFY_OK` 才算通用化没削弱
      · 公开面与消费点：`lib.rs` 注册表、`commands/settings.rs` 的 `use super::llm::enrich_with_catalog`、
        前端 invoke 全部不改字；`check-commands.mjs` = `定义=88 · 注册=88 · 前端引用=88 · 预留名单=0`；
        磁盘变异 = 把 `llm/providers.rs` 里 `llm_provider_reorder` 改名 ⇒ 定义 88→87 且 rc=1，
        逐字节写回 + sha 复核回 88/88/88 = `SPLIT_MUT_ALL_CAUGHT`
      · 编译与测试：`cargo clippy --all-targets -- -D warnings` rc=0 · `cargo fmt --check` rc=0 ·
        `cargo test` 584 / 0 · `make check` 全绿 · `check_length` 名单再减一行
      · ★ 对账抓到我自己的一处误删：改 OVERLIST 时用 Edit 删 `commands/llm.rs` 那一行，
        连带把相邻的 `chat/TranscriptFlow.vue` 一起吞了（`old_string` 圈大了）。
        抓手 = **与 HEAD 做行集差**（`comm -23/-13`）：要求"只少一行、多出行集为空"，
        当场看到少了两个文件 ⇒ 补回。口径：删名单行不许靠肉眼，必须做集合差
      · 跨平台：纯 Rust 结构面，不取 WSL 证据（ADR-0005 口径）；`llm_provider_save` 的业务面属 T25，没动
- [x] **T31** 拆 `llm/wire.rs`（548 → 24 行只留声明）⇒ 同名子目录加四片（2026-10-18 第一批第四刀）
      · 四片按线路上的位置 = `media`(176) 发送前门控 + 工具结果图片投影 · `stream`(172) 流式累积器与
        用量映射 · `payload`(131) 请求装配 + 能力上限 + Q8 截断 · `messages`(105) 中立消息 → 线上消息。
        判定表列五簇、落四片：`usage` 只有 `parse_usage` 一个函数且只被累积器调用，
        单立一片 = **为行数造结构** ⇒ 并进 `stream`（表给的切法本来就是这四片）
      · ★ 声明式改动第一次真用上：2 处 = `clip_tool_output` 与 `message_value` 抬 `pub(super)`
        （都是"被兄弟片共读"的私有函数；抬完 `clip_tool_output` 那行 82 列仍在 100 内 ⇒ 单行终形）。
        预算参数那四处（80/128/307/372）本刀**没并**：它们是穿参不是重复读数，拆完仍是一条链
        （`payload` 取 caps → 形参传给 `messages`），没有新增第二出处
      · ★ 编译器一次抓到两件事，都不是"搬行"能预判的：
        ① `use super::dialect::Dialect` 在新层级指到 `wire` 而不是 `llm` ⇒ 头部 import 改 `crate::` 锚定
          （头部是我写的、不进守恒判据；动手前实查过**正文里没有一条 `super::`**，才敢只改头）
        ② `messages` 片没有任何 `pub` 顶项 ⇒ 给它写 `pub use messages::*;` 编译器直接判红
          （"glob 导不出任何 pub 项"）。判据跟着改强：T5 从"每片都要有 mod + glob"改成
          **有 `pub` 顶项的片必须有 glob、没有的不许有**，装配器 `split-assemble.mjs` 同规则先判一遍
          —— 把编译器的规则前移成机器判据，而不是等 clippy 红了回来猜
      · 公开面与消费点：`app_llm.rs` 的 `use super::wire::WireCaps` 与四处 `super::wire::{…}`、
        `client.rs` 的 `use super::wire::{build_payload, Delta, StreamAcc, WireCaps}`、
        `llm/tests/assets.rs` 的 9 处 `super::wire::project_tool_assets` 全部一字未改；
        判据里那句"`super::wire::` 命名空间遮蔽有案底"本刀复核 = **没发生**
      · 磁盘变异（这一刀的"闸活着"由 Rust 测试面担保，不是 `check-commands`）：把 `wire/media.rs` 里
        `project_tool_assets` 改名 ⇒ `cargo test --lib llm::tests::assets` 编译红 **9 处**
        （正是 tests 的 9 个引用点）；逐字节写回 + sha 复核回 rc=0 = `SPLIT_MUT_ALL_CAUGHT`
      · ★ 判据四件套成型：`split-cut`（落盘前 C1–C3）· `split-verify`（磁盘后 T1–T6 + X1/X2）·
        `split-assemble`（装配文件 A1–A4）· `split-mut`（磁盘变异 + sha 还原）。
        改过判据先回归：T26 / T27 / T30 三把已提交产物重跑通用对账全部 `SPLIT_VERIFY_OK`
      · 编译与测试：`cargo clippy --all-targets -- -D warnings` rc=0 · `cargo fmt --check` rc=0 ·
        `cargo test` 584 passed / 0 failed（含 `llm::tests::assets` 全跑）· `make check` 全绿 ·
        `check_length` 超限 17→16（名单行删除走**与 HEAD 的行集差**：只少一行、多出为空）
      · 跨平台：线路格式层不碰路径语义与插件生命周期 ⇒ 不取 WSL 侧证据（ADR-0005 口径）
- [x] **T32** 薄切 `llm/client.rs`（575 → 493 行，**仍在名单**）⇒ 只切出旁路 `client/test_probe.rs`（2026-10-18 第一批第五刀 · 第一批到此清完）
      · 判定是"拆（薄）"：搬走的只有逐模型连通测试那条旁路（`TestFailure` 五分类 + `snippet` +
        `test_model`，原 457–548），主环（装配 / 重试环 / 单次尝试）**一字未动也没再切** ——
        再切就出现第二条线路形状，违「同类只许一种形态」。本件因此仍留在 `check_length` 名单里，
        493 行是有理由的超限，不是漏网
      · ★ 这一刀是**第一种"部分提取"形状**：表写的落点是 `llm/test_probe.rs`（兄弟模块），
        落地改成 `llm/client/test_probe.rs`（子模块）更好 —— 子模块经 `super::` 就够得着主环的
        私有超时常量与两个 `pub(super)` 装配函数 ⇒ **0 处可见性抬升、`llm/mod.rs` 不必动**，
        "15 秒"这个读数仍只在 `client.rs` 出现一处
      · 判据为此长出新的一条 **T7 主文件可重算**：主文件不许"手工改成看起来对"，它必须等于
        **由备份 + spec 重算出来的字节**（算法只有一份 = `split-main.mjs`，切割/装配/对账三处共用）。
        full 刀与 partial 刀同一条判据；T31 那把回跑也过 ⇒ 上一刀的装配文件同样是可重算的
      · T5 随之称重：`partial` 刀的主文件**本来就该保留其余实现**，"不许含实现"只对 full 刀判。
        这不是放宽 —— 换它的是更强的 T7 逐字节重算，且自测里有一条专门证明：
        X3「往主文件手工塞一条实现」在 partial 下红在 T7、在 full 下红在 T5（`SPLIT_SELFTEST_ALL_CAUGHT`）
      · 主文件的声明式改动 1 处（枚举在 `assembly.mainEdits`）：`Instant` 随旁路搬走，
        那条 use 收成 `use std::time::Duration;` —— 不改就 `unused_imports` ⇒ `-D warnings` 红。
        被搬段取 **457 起**（含段前那个空行）：取 458 起主文件会留下两个连续空行，rustfmt 折叠后
        重算结果与磁盘必差一行 ⇒ T7 假红，这种坑只能靠"边界含空行"这条规矩躲
      · 公开面：`llm/mod.rs` 的 `pub use client::{test_model, CompatLlm}` 与 `commands/llm/test.rs`
        两处调用点全部一字未改（靠 `client.rs` 里新增的 `mod test_probe;` + `pub use test_probe::*;`）
      · 磁盘变异：把 `client/test_probe.rs` 里 `pub async fn test_model` 改名 ⇒ `cargo check --all-targets`
        编译红（调用点找不到），逐字节写回 + sha 复核回 rc=0 = `SPLIT_MUT_ALL_CAUGHT`
      · 编译与测试：`cargo clippy --all-targets -- -D warnings` rc=0 · `cargo fmt --check` rc=0 ·
        `cargo test` 584 / 0 · `make check` 全绿 · `check_length` 超限 16 · 名单 16（本件**不删行**）
      · 踩坑登记：片头 import 我又写成 `use super::models::apply_proxy`（T31 同一门课重犯一次）——
        新片是 `client` 的**子**模块，`super` = `llm::client` 而不是 `llm` ⇒ 外部件一律 `crate::` 锚定
      · 跨平台：纯结构面，不取 WSL 证据（ADR-0005 口径）

- [x] **T33** 拆 `src-tauri/src/memory.rs`（916 → 48 行只留声明）⇒ 同名子目录加七片（2026-10-18 第二批第一刀）
      · 七片 = `shapes`(58) 三个上限常量与行/表形状 · `store`(208) 结构与装配 + 开库、行装配 ·
        `crud`(186) 管理页五条读写 · `reconcile`(110) 首启导出与对账直写 · `recall`(115) 全局层召回与埋点 ·
        `schema`(112) 旧库迁移两条 · `source`(215) `MemorySource` 实现与组装点便利函数。
        最大一片 215 行 ⇒ 七片全在 400 以下，本件从名单上**整行删掉**（不是换个路径继续挂着）
      · ★ **`impl MemoryStore` 也分家**（分进 4 个文件）：留成 517 行一整块 = 加一条管理页功能要顺手
        读懂召回埋点，那是三问 ③ 说的牵动；Rust 允许同一类型多个 impl 块，分家不破任何调用点。
        机械后果 = **产物里多出 6 行语法外壳**：原文那对花括号只归两处（开在 `store` 第 105 行、合在
        `recall` 第 617 行），于是 `store` 补一条合、`crud`/`reconcile` 各补一对、`recall` 补一条开。
        外壳必须在 spec 的 `wrap` 里逐片声明，对账**先剥掉声明过的壳**再比连续（T3）与守恒（T4）⇒
        "多写的行只有这 6 行、且只补在声明的那几片"是被判出来的，不是被相信的。判据为此长出
        X4「外壳多补一行（没声明）」与 X5「声明了外壳却没写」两发定向变异
      · 14 处可见性抬升一律 `pub(super)`（`shapes` 两个常量 · `store` 四个字段 + 四个方法 ·
        `recall` 两个入口 · `schema` 两个迁移函数），逐条枚举在 spec 的 `edits` 里，判据吃"施加之后
        的基线"。`pub(super)` = 只在 `memory` 子树内可见 ⇒ **模块外的公开面一字未变**：
        `crate::memory::{MemoryStore, MemoryRow, memory_source, KEY_MAX, VALUE_MAX, FactLine}` 的
        消费点（`organize/` · `state/` · `instruction_files.rs` · `recall_log/tests/` · `tests/pressure_wiring.rs`）
        零改动，靠装配文件那三条 glob 带出；`LIST_MAX`/`CREATE_TABLE` 从私有抬到 `pub(super)` 也出不了模块
      · ★ **T23 那道闸第一次真起作用**：`check-schema-parity.mjs` 十档读数与拆前**逐字相同**
        （`same / all / 0 / 1 / 1 / same / 0 / 1/1 / ok/ok / clean/clean`），而两条"出处"读数自己跟着搬了：
        建表串 `memory.rs` → `memory/shapes.rs`、迁移函数 `memory.rs` → `memory/schema.rs`，
        判据一行没改、闸没瞎 —— 这正是 T23 把闸门从"钉死两个文件"改成"整根恰 1 处"要买的东西
      · 磁盘变异两发，都带"还原后仍定位到搬入的新文件"这条期望（`--expect` 吃闸的出处那行）：
        改 `shapes.rs` 的 `const CREATE` ⇒ `SCHEMA_PARITY_FAIL（锚取不到 = 闸瞎了）` rc=1；
        改 `schema.rs` 的 `pub(super) fn ensure_learning` ⇒ 同 rc=1；两发还原 sha 逐字节复核 + rc=0 ⇒
        `SPLIT_MUT_ALL_CAUGHT`
      · ✗ 变异打空登记（自造判据的第 12 种说谎形状）：第二发最初锚的是 `fn ensure_learning`，命中的是
        本片**头注里那句**"锚从 \`fn ensure_learning_schema(\` 起算" ⇒ 改的是注释，而闸吃 `code()` 剥注释
        ⇒ 定义纹丝不动、闸照绿 = 打空。锚必须锚到**定义行本身**（连 `pub(super) ` 前缀一起锚）；
        判据没错，是这发对照自己没使上劲 —— 打空时先怀疑锚，别急着说闸瞎
      · 判据本身被这刀改了三个地方，改完全部回归：`split-main.mjs` 的 `assembly.extra` 允许单行
        `#[cfg(...)]`（`memory.rs` 的 `#[cfg(test)] mod tests;` 必须留在装配文件，上一版把它当实现行判红）·
        `split-verify.mjs` 的 X1 从"钉死 `NAMES[0]`"改成"挑正文含 `fn`、没声明外壳、本来就有 `pub` 顶项的片"
        （T33 第一片是纯常量文件，旧锚打空）· T3 增加"外壳没声明/声明了没写"这条读数。
        回跑 T31 / T32 两把已提交产物 = `SPLIT_VERIFY_OK` + `SPLIT_SELFTEST_ALL_CAUGHT` ⇒ 上一刀的判据
        没被这一刀弄弱
      · 等价判据：`split-verify` T1–T7 全绿（848 非空行逐种守恒 · 逐字节同序 · 装配文件 48 行与重算
        逐字节相同）· `--selftest` 全中（T1–T7 七档 + X1/X2/X3/X4/X5 五发定向）
      · 编译与测试：`cargo clippy --all-targets -- -D warnings` rc=0（片头 import 一次写对，零 `unused_imports`）·
        `cargo fmt --check` rc=0 · `cargo test` 584 passed / 0 failed（`memory/tests/` 四簇 777 行
        **一字未改**仍然绿 = "只搬不改"的行为证据）· `make check` 全绿 ·
        `check_length` 超限 16→15（名单删一行走**与 HEAD 的行集差**：只少一行、多出为空）
      · 判定表点的两组重复（`KEY_MAX`/`VALUE_MAX` 校验三份 · memories upsert 两份）本刀**一组都没并**
        ⇒ 登记 **T34**
      · 跨平台：纯 Rust 结构面，不取 WSL 证据（ADR-0005 口径）
- [ ] **T34** `memory*` 里两组「同一事实多个出处」并成一处（T33 判定表点的，那把刀刻意不夹带）
      · 实测出处：`key`/`value` 上限校验这一句三份（`memory/crud.rs::update_value`、`memory/crud.rs::write`、
        `memory/reconcile.rs::reconcile_upsert`）；memories 的 upsert SQL 两份（`crud.rs::write` 与
        `reconcile.rs::reconcile_upsert`）—— SET 列表**不许含学习列**这条由 `check-schema-parity.mjs`
        E1 按"整侧每一份都干净"判，抄两份就是它盯的风险面
      · 插件侧 `plugins-src/memory-sqlite/src/store.rs` 另有同形抄写（判定表登记的 276/522 两份），
        两侧能不能并、并到哪一侧，要等第二批第二件（拆 `store.rs`）把形状摆好之后再判 ⇒ 顺序在后
      · 与拆刀分家：这一刀改的是**内容**。判据 = 既有记忆面用例一字未改仍然绿 + 三条错误文案逐字节相同
        （不许"顺手统一文案"，那是要单独登记的语义改动）
  第一批 Rust 无锁件 = **清完**（T26 / T27 / T30 / T31 / T32）
  第二批 Rust 侧 schema 两件 = `memory.rs` ✅T33 → `store.rs`（建表串不必再钉在主文件：定义处"整根恰 1 处"由闸自己盯 —— T33 实测：搬到 `memory/shapes.rs` 与 `memory/schema.rs` 之后十档读数一字未变）
  第三批 前端 lib = `tool-detail.ts` → `api.ts`（通道表同样不必钉文件名）
  第四批 组件 = `DeveloperSection.vue` → `TreeSection.vue` → `ProviderSetupDialog.vue` → `ModelPanes.vue` → `Composer.vue` → `TranscriptFlow.vue` → `MessageRow.vue`
  一件一刀，每刀按 T22 那刀的规格交：公开面不减 · 消费点不动 · 等价判据（规范化后逐单元比）· 窄变异各红在指定档
· 这一节不承诺做完，也**不在话里钉"几件几刀"**：判与理由都在上面那张表的「判」列里，数一次漂一次
  —— `store.rs` 从判留改成判拆就是现例子（判留的外力一消失，三问自己的答案浮出来了）。
  认哪几件就拆哪几件；判留那几件如果不服，说一声我重判

## 验收标准

- [x] 第一节七条全部做完或明确转入第二节（附理由），没有第三条状态
      · 第一节（`### 一、` 到 `### 二、` 之间）未勾 0 条 / 已勾 7 条 = 结清。
        第二节不写条数（上一版在这里钉了"10 条 / 4 条"，本刀收尾一勾账它当场变假）。现查命令（区间取到
        第三节标题为止，否则会把第三节新登记的欠账算进第二节的结账单）：
        `awk '/^### 二、/,/^### 三、/' specs/03-structural-debt/spec.md | grep -c '^- \[ \]'`
        （这条命令跑出来的就是当前真数，不在这里钉：T22 结账之后它已经是 0，
        把 `\[ \]` 换成 `\[x\]` 得已勾数 —— 两个数都不写进文档）
        （方括号必须转义：`grep -c '^- [ ]'` 在 POSIX 字符类里只匹配一个空格，
        对着 "- [ ] xxx" 恒给 0 —— 这条我在本节里写错过一次，当场跑出来才发现，
        正是"永不命中的判据和没有判据是同一件事"）
- [x] 每条做完后，路线图「结构性欠账清零」那行的"现在到哪了"由**实测**给剩余条数（不手写计数）
      · 路线图那一行已改成**不给数、只给现查命令**（`grep -c '^- \[ \]' specs/03-structural-debt/spec.md`）：
        上一版写着"T8 与 T18 已结"，那是**手写清单**，结到第六条时它就已经是假信息了 —— 与"会漂的数不进文档"同族
- [x] T1/T15/T16 做完后 `check_length` 的名单**只减不增**，且 i18n 键集读数不变（纯结构收敛不该动文案）
      · 实测：名单 22 → **21**（T1 那刀撤掉 `approval/mod.rs` 一行，此后 T15/T16 两批 `超限 21 · 名单 21 · 零新增`）；
        i18n 三批同数 `zh=970 en=970`、`引用字面量键=793 + 索引键=50`
        —— 那三批**内部**同数，不是全仓永久读数：此后 T4 删净实验室夹具连那 15 条文案键，
        现查 `node scripts/check-i18n.mjs` 得 `zh=955 en=955 · 引用字面量键=778`，T22 拆完 chat store
        后仍同数。会漂的数本来就不该钉进文档（本条留着只因为它是那三批的比对基线，别拿它当现状读）
- [x] T6/T7 做完各自有一条能红的行为用例（不是只改代码不看结果）
      · T6 = `tests_session_log/export_title.rs` 两条（旁车真读盘的三档优先 / 截断宽度吃共享常量），
        窄变异两发各打一条
      · T7 = `tests_approval/hang_guard.rs` 三条（没卡片必须在预算内失败 / `bounded` 喂永不完成的
        future 必须报出来 / 有卡片时照旧正常决议 = 阴性对照），变异三发各打不同读数
- [x] T8 收口后，仓库里"判据脚本"只有一个地方：`scripts/`；`tmp/` 里的历史脚本整体处置完毕（删或明确作废）。
      加对账仍然**不新建闸**，只分两支走：同一个数字/名字在几处必须相等 → 往 `check-single-source.mjs` 的
      CHECKS 加一项；两侧各有一份实现、要比整段代码块同源 → `check-schema-parity.mjs`（本次升格就是这类，
      它的顺序档与 SET 白名单档塞进 CHECKS 会变成把三种形状硬压成一个读数，故独立成支）。这条由 `STRUCTURE.md` 的口径承重
- [x] 第二节那一组**没有任何一条被我自作主张做掉**（尤其 T4/T12 属删除与不可逆类）
      · 2026-10-18 收尾核对：**两次点头在先**（"这10条可以收账，同意" → "需要 剩下4条也收账了吧"），
        所以第二节这批不是自作主张。但我确实**代他裁了两处原本写给他的选择**，两处都在条目顶部
        标明"可翻案"：T10 的 (a)/(b) 二选一选了 (a)（理由：(b) 是把标准自己欠的债洗成特例），
        specs/04 那三问答了 (a) 复用 error 通道 / "已取消"状态词要加但排 C5 / 显示"取消中…"
      · **不可逆动作只做过一件，且正是用户点名那件**：同日傍晚他验收完说「实验室 T4 那 2 个可以删除
        干净了。不需要那 2 个了」⇒ 三件夹具 + 两份语言包各 15 条 `bui.*` 键 + 为它加的那根 i18n
        引用面同批删除（实测与七处清单都记在 T4 条目末尾 ★，不在这里重述）。**未点名的删除依旧一条
        没做**：T12 那 67G 编译产物判定为"不清"（没删任何东西）；`plugin-api` 既有签名与
        `PLUGIN_ABI_VERSION` 未动（C1 只用现成的 `CallCtx: Clone`，C3 那种追加槽位明确停下来等点头）；
        `DESIGN.md` 的规范语义未动（T13 三答里那条"日志闭合形态 (b)"因此**没做**，只登记）；
        没有 push、没有 force、没有 hard reset、没有动用户数据目录
      · 条数不写在这里（原句写着"九条"，登记 T17 之后就漂成十条 —— 正是本页禁止的那种数）；
        现查 = `awk '/^### 二、/,/^### 三、/' specs/03-structural-debt/spec.md | grep -c '^- \[ \]'`
        （区间必须截到第三节：第三节是**另一张账**，它勾没勾都不该混进第二节这张结账单里读数）

## 按批验收清单（T14 的交付物）

分成两组是因为**能核的方式不同**，不是一组重要一组不重要。A 组要你自己在窗口里点；
B 组机器已经钉住了，你复跑那一条命令、对一下读数就行——我不在这里放"看看代码"这种没有判读的条目。

### A 组：打开哪 · 点哪 · 该看到什么

| 批 | 你做什么 | 该看到什么 | 不对的话说明 |
| --- | --- | --- | --- |
| T10 浮层 Esc（本刀补的那一半） | 四处各开一层再按一次 Esc：① 顶栏「子代理」面板（本会话跑过 subagent 才有那颗钮）② 侧栏底部通知浮层 ③ 审批卡右侧 `⌄` 上弹的「允许本次对话」下拉 ④ 会话行 `…` 菜单或右键菜单 | 每处**按一次就关那一层**，关掉后现场不变：审批卡还在、默认模型没被换、通知没被读掉 | 按 Esc 没反应 = 那层没吃到 document 级 Esc（④ 是这一刀的第 4 处：菜单元素那份 Esc 只在焦点进层后才接得到键，而开层时焦点不进层）；按 Esc 顺手拒了审批 = `anyLayerOpen()` 让位没生效（SS9b 会红） |
| T10 链没被自己的补丁堵死 | 有 pending 审批、**什么浮层都没开**时，在输入框里按 Esc | 照旧拒掉这一次审批：卡消失、过程行里落下一条 deny 记录 | 什么都不发生 = 让位判定写成恒真（变异①那个形状），链被堵死比不补更糟 |
| T4 实验室夹具（**已按你同日傍晚的点名删净**，所以下面这行的"该看到什么"是终态，不是当初搬目录那一刀） | 起桌面端 dev，侧栏版本号连点 7 次解锁「开发者」分区，滚到最底 | 那两块（审批卡·多题向导 + 推荐卡·置信度）**整块没有了**；分区其余部分照旧——危险开关、缓存诊断、最底下那块记忆树面板都还在，控制台没有"取不到的模块" | 还看得见 = 没删干净（`desktop/lab/` 或入口那行没删净）；记忆树面板也跟着没了 = 删多了（它与那两块同层，但它是生产件）；中英切换后残留半截英文 = 语言包的 `bui` 域只删了一边。生产侧不用你验：产物里那四串零命中、`.js` 总量又降 888 字节，读数记在台账 T4 末尾 ★ |
| T16 即时生效 | 设置页依次改：界面语言 → 主题 → 界面字号 | 每一项**当场**生效，不用重开窗口；关掉再开仍是这个值 | 某项要重开才生效 = 那份"即时"又退回各自实现；改完闪回原值 = 回滚读的是新投影（T16 抓到的原缺陷形状） |
| T15 设置分区 | 设置页每个分区各改一项并保存（共 11 个分区），重开应用 | 11 项全部保留；报错文案点名**是哪个分区**出界 | 某分区保存后丢失 = 那个分区没自持 `apply_update`；报错不点名 = 编排层又在替分区铺校验 |
| T11 插件卡 | 改**暂存面**的 `plugins/<id>/<ver>/manifest.json` 里的 `description`，重进插件页（源清单 `plugins-src/` 改完不重跑 `bash scripts/build_plugins.sh` 是不会变的——展示面读的是暂存目录，不是源） | 卡片文案随暂存清单变；徽章与该工具代码声明的副作用等级一致 | 改清单界面不动 = 展示面又去读了代码那份；徽章与实际副作用不符 = 重叠区没被对账钉住 |

### B 组：复跑一条命令对的读数

读数一律**跑一遍再写**（2026-10-18 收尾那一遍：下表每条都在同一份代码上实测过，命令原样可抄）。

| 批 | 命令（在仓库根） | 该看到的末行 |
| --- | --- | --- |
| T2 注入环 | `node scripts/check-single-source.mjs` | `SINGLE_SOURCE_OK`，其中 SS8 实测与期望同为「7 对 7」、SS8b 实测 0、SS8c 实测 7 |
| T2 内核 | `cargo test -p kernel-agent --test waterfall` | `6 passed; 0 failed`，六条含 `guard_writes_back_identical_bytes_when_nothing_changed` 与 `a_ring_that_forgets_to_give_back_is_caught_by_the_next_ring` |
| T3 分发表 + 全量前端 | `pnpm -C desktop run test:run` | `Test Files 17 passed (17)` / `Tests 151 passed (151)`（T3 的 27 条、T10 的 9+2 条都在里面） |
| T10 浮层行为 | `pnpm -C desktop exec vitest run tests/popover-esc.spec.ts tests/esc-routing.spec.ts` | `11 passed`（登记/归还那 9 条 + mount 真实 Composer 的 2 条） |
| T10 防回潮闸 | `node scripts/check-single-source.mjs` 然后 `make selftest` | SS9 实测 `6\|6`、SS9b `1\|1`、SS9c `1`；自测末行 `SELFTEST_ALL_CAUGHT`（**17 档全中**，一档打空就是那条判据零证明力） |
| T4 夹具出产物 | `pnpm -C desktop run build` 然后 `grep -rl "形态库实验室" desktop/dist \|\| echo 零命中` | `零命中`（改前能搜到，串在 `dist/assets/index-*.js`）；dev 面另见台账 T4 那条的 `T4_DEV_FACE_OK` |
| T13 取消·C1 | `cargo test -p kernel-agent --test active_calls` | `4 passed; 0 failed`，四条是 `stop_reaches_the_call_in_flight_and_the_table_empties_after` · `registry_is_scoped_to_one_session` · `registry_skips_calls_without_session_or_call_id` · `overlapping_registrations_of_one_call_id_do_not_erase_each_other` |
| T11 插件对账 | `cargo test --workspace manifest_json_and_describe_agree` | 十条通过合计 **10**（每个内置插件一条，加插件不结对账当场缺数） |
| T21 + T2 叙事 | `cargo run -p kernel-bin -- demo4` 然后 `echo $?` | 退出码 0，末行三条读数自洽：danger 实际执行 1 次、被拒闭合 1 条、echo 吃到改写后的 `mode=upper`（结果行里是 `HELLO LOCOWRK`） |
| T18 + T20 Linux | `wsl -d Ubuntu-24.04` 里按 `AGENTS §7` 那行做（先补两份产物再 `cargo test --workspace --no-fail-fast`） | 每个 `test result:` 都 `0 failed`；本刀实测 40 个 test target · **541 通过 / 0 失败 / 1 ignored**（Windows 542，差的不是漏跑：按用例名比 = Windows 独有 3 条、Linux 独有 2 条，净差 1，都是路径大小写折叠族的方向性断言）。★ 这条原先写的是「**0 ignored**」，实测不成立：那 1 条是 `crates/plugin-api/src/meta.rs:215` 的 doctest 形状示例（`ignore` 围栏），**不是被平台跳过的用例**——T20 要消灭的是 `#[cfg]` 单侧跳过，两者不许混谈；改口径不改代码，因为把示例块假装成能跑的测试才是造假 |
| T7 审批测试 | `cd desktop/src-tauri && cargo test tests_approval` | 全过；若某条挂了，它是**在预算内报失败**而不是把测试进程卡住 |
| T17 + T19 闸自己 | `make doc && make selftest` | `check-docs: OK` 与 `DOCS_SELFTEST_ALL_CAUGHT`——闸要能为插进去的断链报红，打空就是零证明力 |
| T5 + T6 热路径 | `cargo test -p kernel-agent -p session-store` | 全过；这两批的判据（少一次取锁 / 两处取数改源）在 `check-single-source.mjs` 的 SS3 与对账用例里 |
| T33 记忆库搬家后闸还跟得到 | `node scripts/check-schema-parity.mjs` | 出处那行两条宿主路径已是 `desktop/src-tauri/src/memory/shapes.rs`（建表串）与 `…/memory/schema.rs`（迁移函数），十档全绿、末行 `SCHEMA_PARITY_OK`；还写 `memory.rs` = 闸又退回钉死文件的老写法（那正是 T23 拆掉的锁） |
| 全量收尾 | `make check` | 末行 `▶ check 全绿` |

### 这两批没有可核对象

- **T9**（度量基线）：转入路线图的独立阶段，现在那一行写着 ⬜ 没开始——它的验收物本身还不存在，没有"该看到什么"可给。
- **T12**（两份 `target/` 约 67G）：已给判定"不清"，依据是实测 25G + 42G 而盘剩 1.3T。你哪天改主意要清，只需要删两处目录再重跑一次构建，不需要我这边动代码。

## 非目标

- 不在这里重开审计线：审计已收口，本阶段只做"当时没结的小事"，不新增审计条目
- 不做性能优化：想量化收益属于「度量基线」阶段（T9），先立夹具再谈快慢
- 不为了压行数而改注释、搬字段、拆无辜文件 —— 那是把建议值当硬闸，规范明令禁止
- 不重写历史：台账原文只读，结项状态只写在这一页

## 风险与边界

这些条目彼此独立，可以一条条做、每条单独提交；**不要**把三条不同域的欠账塞进一个提交里。
T1/T3/T4/T15/T16 都动前端与审批面，属用户可感知区域 —— 纯结构收敛一旦改动行为就要重新走界面自检，
所以每条做完要跑 `make quick`，改到 `desktop/src/**` 的还要跑前端测试面。
