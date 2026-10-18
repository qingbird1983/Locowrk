# 04 · 工具级取消与 deadline

## 这个阶段是什么

审计 H5 留下的那件"对外 ABI 上永远无效果的承诺"：`CallCtx` 里有 `deadline` 与取消标志，
内核每次调用都去读，但**没人写过、也没人能收到**——停止键今天只落在 LLM 流式与步间边界，
已经在跑的工具调用跑到底。设计稿要回答的是"要真接线，得动哪几层、按什么顺序、每层怎么验收"。

形状其实早就登记在源码里了：`crates/plugin-api/src/tool.rs:191-195` 那段头注写着最小完整形态
（内核加活动调用登记表 → 停止时逐个 `cancel()` → 审批/预算剩余时间写进 `deadline` →
`HostVtable` **末尾**追加 `call_cancelled` 槽 → 长任务工具真去轮询）。
**本稿不是重新发明这套形状，而是把它拆成能单独验收、能单独回退的五刀，并补上它没写的两件事**：
取消这个事实怎么进会话日志（现在被明确判为"宁缺勿假"），以及它到界面上是什么状态词。

## 现在到哪了

**C1 已落地**（2026-10-18，台账 T13 收账时一并做掉的第一刀：内核侧登记表 + 停止按会话逐个置位 +
RAII 注销，四条行为用例 + 四发变异各打中一条）。下面「要你定的三件事」由 Qoder 代裁定，
**答案与理由都写在那里，你要翻案直接说哪一条**。C2–C5 仍未开工，进度只看下面的勾选框。

C1 之后信号能到达的是**宿主内建的 Rust 工具**；第三方插件与 wasm 仍看不见（那要 C3 的
追加槽位 = 动 ABI 面，本稿明文"每一刀单独等你点头"，我不会自决）。

## 现状的六条事实（逐条给出处，设计建立在这上面）

1. `CallCtx` 四个字段里 `deadline: Option<Instant>`（`crates/plugin-api/src/tool.rs:200`）与
   `cancelled: Arc<AtomicBool>`（:201，配 `is_cancelled()`:214 / `cancel()`:218，都是
   `Ordering::Relaxed`）。全仓 `deadline:` 的**赋值**只出现在构造函数 :209 —— 有消费者、无生产者。
   **【C1 后订正】** `cancelled` 那一半自 2026-10-18 起**两端都接上了内核侧**：登记表 =
   `crates/kernel-agent/src/active_calls.rs`，登记点在 `kernel.rs` 那次进链调用之前，
   置位点在 `desktop/src-tauri/src/commands.rs` 的 `agent_stop`（第四路）。
   `deadline` 仍是"有消费者、无生产者"（那是 C2）。
2. 消费者是活的：`crates/kernel-agent/src/middleware/timeout.rs:90-96` 每次调用都拿
   `deadline` 的剩余预算与全局 `callTimeoutSecs` 取小 ⇒ 现网恒走 `None` 分支，只吃全局那个数。
3. 两个构造点都不写：`crates/kernel-agent/src/kernel.rs:334-336`（填 session_id + call_id）、
   `crates/kernel-agent/src/compensation.rs:139-140`（只填 call_id）。
4. **过不去 ABI**：插件的 `call` 是同步 `extern "C"`（`ffi::CallFn`，`crates/plugin-api/src/ffi.rs:67-75`），
   七个参数不含 ctx；`HostVtable`（:100-117）五个槽（log / kv_get / kv_put / notify / session_context）里
   **没有**取消或轮询入口；适配器 `crates/plugin-host/src/adapter.rs:55-92` 只把 `ctx.session_id`
   转成 `ffi::CallContext` 塞进调用期 thread-local，`deadline` 与 `is_cancelled()` 一律丢弃。
   全仓未找到任何"已预留但没接线"的 cancel/poll 槽。
5. 调用跑在 `spawn_blocking` 里（内核纪律 4），且本项目**永不 dlclose**（纪律 3）⇒
   宿主不能强杀一个正在跑的插件调用，只能等它自愿早停。
6. 停止通路现在长这样：`desktop/src/stores/chat.ts:1084` → `lib/api.ts:165` →
   `desktop/src-tauri/src/commands.rs:137-142`（置位 + 审批 fail-closed 拒绝 + ask 跳过），
   注册表 `stop.rs:16-59`（按 session_id 一个 `Arc<AtomicBool>`，RAII 注销 :63-84），
   消费两处：LLM delta（`events.rs:474,526`）与步间边界（`runner/mod.rs:125-140` →
   `TurnOutcome::Interrupted`）。`stop.rs:6` 原文就写着"已在执行的工具调用不可中断，如实边界"。
   事件面 `events.rs:66-67`：`TOOL_STATUS` 的 `cancelled` **刻意不存在**，理由是
   "在途调用会跑完并落 ToolResult ⇒ 日志里没有『被取消』这件事（宁缺勿假）"。

另外三条约束（决定方案边界）：

- `crates/plugin-api` 是冻结 ABI：字段只能追加到结构体**末尾**，`PLUGIN_ABI_VERSION = 1`
  且在 `plugin-api/src/lib.rs:92` 与 `tool/tests.rs:41` 各有一处硬断言 ⇒ 只要不动既有签名、
  不升版本号，追加槽位有两次先例（⑭ `notify`、㉘ `session_context`，见 `ffi.rs:89-98`）。
- 事件只追加、先写 `ToolCall` 再执行、副作用带逆操作、不可逆默认审批（DESIGN 附录 C，
  实测该附录是 **11 条**而非 AGENTS.md 里说的"八条"——这条文档失实另记，见风险③）。
- 插件侧超时现在各自自理：`tool-shell` 自己算 deadline 并 25ms 轮询 `try_wait` 后
  `kill_tree`（`plugins-src/tool-shell/src/exec.rs:24-41,188,209-224`），`tool-fetch` 用
  ureq 的 `.timeout()`（`plugins-src/tool-fetch/src/lib.rs:136-139`）。两者都不看宿主信号。

## 计划（五刀，按"能不能单独回退"切）

### 一、不碰 ABI、不碰界面的那半（可以连做）

- [x] **C1** 内核加活动调用登记表：按 `(session_id, call_id)` 登记 `CallCtx` 的 `cancelled` 句柄，
      停止请求把该会话在途的逐个 `cancel()`，调用收束时注销（仿 `stop.rs` 现成的注册表 +
      RAII 形状，含"上一轮注销误删下一轮登记"那条按指针比较的先例）。
      · 验收 = 一条**行为**用例：一个会检查 `is_cancelled()` 的替身工具在途时被停止，
        它必须在下一轮询点返回、且会话日志里那次调用以**明确的一种收束**闭合，而不是跑完。
      · 变异：把登记表做成只登记不置位 ⇒ 该用例必须红。
      · **实测（2026-10-18 落地）**：登记表 = `crates/kernel-agent/src/active_calls.rs`
        （新模块，113 行）。两处与设计稿原文不同，都是取证后改的：
        ① **不需要 `Arc::ptr_eq` 那条换代比较**——键里带了 `call_id` 与登记序号，
        每次登记各占一位，"上一轮注销误删下一轮"在结构上不可能发生（`stop.rs` 需要那条比较
        是因为它的键只有 session_id、会重复）；② **登记的是 `CallCtx` 克隆而不是内部句柄**——
        `cancelled` 是私有字段，要取出 `Arc<AtomicBool>` 就得给 `plugin-api` 加新公开面，
        而那是 [LOCK] 区、C1 一步都不该动；`CallCtx: Clone` 且内部是 `Arc` ⇒ 克隆与工具手里
        那份是同一个标志位，效果完全等价、ABI 零改动。
      · 只登记**带会话身份**的调用（`step_as` 通路）：停止命令本身带 `session_id`
        （`commands.rs` 的 `agent_stop`），headless 的普通 `step` 没有任何一条现成通路会去取消它，
        收进来只会让"置位了几条"这个读数掺水。补偿调用（`compensation.rs`）**故意不登记**
        ——取消一条正在回滚的写操作得到的正是风险① 说的"半途状态"，代码注释里写明了这是有意。
      · 验收物 = `crates/kernel-agent/tests/active_calls.rs` **4 条**（① 在途收到取消并早停 +
        收束后 `cancel_session` 归零证 RAII 真注销；② 只动本会话、不许动别人的在途调用；
        ③ 无会话/无 call_id 不登记；④ 同一 call_id 重叠登记不互相抹掉——这条就是序号的买主）。
        替身工具把"早停"与"跑完"做成 **JSON 上可分辨**的两种形状（`early_stop` + `round`），
        不是靠读日志肉眼比；另一侧任务先等工具真进循环（`started>0`，有界 2 秒 + 到点仍断言非零）
        才点停止，否则取消会打在空处、用例绿得什么都没测。
      · 变异四发各打中自己那条：M1 只登记不置位 ⇒ 红 3 条（含设计稿点名的①）；
        M2 `track` 后立刻 `drop` ⇒ 独红在①（置位条数掉到 0）；M3 键里去掉序号 ⇒ 独红在④；
        M4 无会话身份也登记 ⇒ 独红在③。还原全部逐字节 `cmp` + `sha256sum`
        （active_calls e3d4320811ac / kernel.rs 9652dd2827d9）
      · 两侧读数：Windows `cargo test --workspace` **542 通过 / 0 失败 / 1 ignored**（含本刀 4 条）、
        `desktop/src-tauri` **584 / 0 失败**、`make check` 末行 `▶ check 全绿`、
        `demo` 与 `demo4` rc=0 且 demo4 三条自判读数不变（danger 执行 1 次 / 被拒闭合 1 条 /
        echo 吃到改写后的 `mode=true`）；**Linux 侧复跑读数在紧随其后的取证提交里**
        （`AGENTS §7` 那两条前置：新 clone 先补 `build_plugins.sh` + `build_market.sh` 再 `cargo test --workspace`）

- [ ] **C2** `deadline` 的生产者接线：调用前把「本次剩余预算」与「审批等待剩余时间」取小写进去
      （今天恒 `None`，`timeout.rs:90-96` 那个"取小"分支从没被走过）。
      · 前置读数：先测出"审批卡超时"与"全局 `callTimeoutSecs`"在现网的实际关系，
        否则接上去可能把所有调用都提前掐死（数值一律做成设置页可调项，按 AGENTS §4 可配置原则）。
      · 验收 = `timeout.rs` 第一次有走 `Some` 分支的用例，并钉住"取小"两个方向各一条
        （预算更紧 ⇒ 用预算；全局更紧 ⇒ 用全局）。

### 二、要过 ABI 的那半（每一刀单独等你点头）

- [ ] **C3** `HostVtable` **末尾**追加 `call_cancelled: extern "C" fn() -> u32`
      （返回 0/1，非调用期返回 `code::ERR_UNSUPPORTED`，语义与 `session_context` 的
      "调用期 thread-local"完全对齐：插件自起的后台线程查询得 `ERR_UNSUPPORTED`，不得失败）。
      插件侧配一个便捷 `fn`（与 `log_str` / `notify` 同栏，源码级、不进 C 签名）。
      · `PLUGIN_ABI_VERSION` 不动；旧插件只读前缀字段（`ffi.rs:86-91` 的 append-only 规则）。
      · 验收 = 一条 native 后端的调用期用例（宿主翻标志 → 插件读到 1）+ 一条"非调用期读到
        `ERR_UNSUPPORTED`"的对照；wasm 侧另有一条（引擎 epoch 能掐死调用但不表达"被取消"，
        见 `crates/plugin-host/src/wasm/backend.rs:120`，得单独定它怎么闭合）。
- [ ] **C4** 长任务工具真去轮询：`tool-shell` 的 25ms 轮询点里加一次 `call_cancelled` 查询
      （命中即 `kill_tree` + 以取消语义返回），`tool-fetch` 在 `.timeout()` 之外给连接分段点加检查；
      `subagent_run` 与 `ask` 这两个宿主内置长任务把取消**传播给子代理**（子会话也要停止）。
      · 这一刀第一次产生"用户能看见的差别"，交付前要在真实窗口走查并截图。
- [ ] **C5** 取消这个事实进日志与界面（动语义，射程最大）：
      现在 `TOOL_STATUS` 明写 `cancelled` 刻意不存在 ⇒ 真接线后必须回答"被取消的那次调用
      在会话日志里长什么样、重放同一次会话能不能重建出同一个形状"。两种形态见下面「要你定」第 1 条。
      界面侧要新增/复用一个状态词（`specs/02-tool-cards` 的六态词表里没有"已取消"），
      中英成对进 i18n，并按标准 §二 的既有形态渲染，不新造视觉形状。

## 要你定的三件事（2026-10-18 由 Qoder 代裁定，答案与理由如下；要翻案直接说哪一条）

原文这三问是我写给他的，他这一轮的指令是"剩下四条也收账"，所以我按"能不能翻案代价小"
各自选了一条，并把**代价**写在每条后面。三答都只决定 C2–C5 往哪走，
**C1 已做完的那半与这三答无关**（它只送信号，不定义收束形态）。

1. **被取消的调用在日志里怎么闭合** ⇒ 选 **(a)**：复用 `ToolResult` 的 error 通道，零新事件。
   · 理由：(b) 要动 `DESIGN.md` 的事件语义——那是 AGENTS §5 的 [LOCK] 区，我不自决；
   而 (a) 不挡住后面任何一刀（真要升 (b) 时，历史日志那条 error 通道还能被重放投影认出来）。
   · **代价（写明白）**：`events.rs:66-67` 那句"`cancelled` 刻意不存在"的理由是"在途调用会跑完
   ⇒ 日志里没有『被取消』这件事（宁缺勿假）"。C1/C4 之后在途调用**真的可能不跑完**，那句理由
   就失效了，而"取消"和"失败"挤在同一字段里靠文案区分是**语义债**。所以 (a) 是**分期**不是终态：
   升 (b) 那一步要单独走"动 [LOCK] 前先向人说明并等确认"的流程，这条登记在 C5 里。
2. **UI 要不要一档独立的"已取消"状态词** ⇒ 答 **要加**，但**实施排在 C5**，不在 C1–C4 里顺手做。
   · 理由：C1 只送信号，现网还没有任何一条通路能产出"被取消"的收束（能产出的那条是 C4 的
   内置长任务轮询）。先落一个没有生产者的状态词 = 死码（本仓判过同族："带不动可观测差就砍"）。
   · 加的时候一并定：中英成对进 i18n（`check-i18n.mjs` 拦漏一边）、颜色语义档复用
   `specs/02-tool-cards` 六态词表里已有的一档、**不新造视觉形状**（标准 §二 同类只许一种形态）。
3. **"尽力而为"怎么对用户表达** ⇒ 选 **前者**：按停止后显示"取消中…"直到真结束。
   · 理由：与 `events.rs` 那条"宁缺勿假"同源——立刻显示"已停止"而实际有条调用还在跑，
   是界面在替用户做一件没发生过的事。这条也正是 C3/C4 的价值所在：不响应取消的第三方插件
   本来就只能等它跑完，界面骗不了这个物理事实。
   · 代价：多一个瞬态（"取消中"）要定义它在中英两侧的词、以及超时后要不要转成警告色 —— 落在 C5。

## 验收标准

- [ ] C1/C2 各自有**一条能红的行为用例**（不是"字段终于不是 None 了"这种结构断言）。
- [ ] C3 的 ABI 追加后：`PLUGIN_ABI_VERSION` 与两处硬断言不变、旧插件前缀读取仍成立，
      且新增两侧（Windows / WSL）各一条 native 调用期用例。
- [ ] C4 完成后，改内核必跑的两条验收叙事（`demo` / `demo4`）仍 rc=0，且新增一条
      "在途调用被停止"的叙事读数进提交正文。
- [ ] C5 完成后：同一次会话的实时与重放投影**产出同一形状**（这条是本仓的硬纪律，
      做不到就还不许叫"接完"）。
- [ ] 全程不为了过闸删测试、不放宽规则；每刀按改动面选档跑门禁，全绿才算完成。

## 非目标

- 不做"强杀线程/杀插件进程"：与纪律 3（永不 dlclose）和纪律 4（FFI 进 blocking）冲突，
  取消只能是**自愿早停**；这条不是妥协，是这个架构下唯一诚实的形态。
- 不做网络字节级中断（`tool-fetch` 一次 HTTP 请求内部不可分割，只能在分段点检查）。
- 不给多用户服务端做会话抢占（多用户服务端本身不在范围内，AGENTS §1）。
- 不重开"停止生成"的既有语义：LLM 流式与步间边界那两处消费保持现状。

## 风险与边界

① 取消与补偿要同时成立：被取消的写操作**已经落盘的那一半**必须走补偿链（纪律 1），
   否则"取消"变成"半途状态"。C1 之前要先确认 `compensation.rs` 对"未返回结果的调用"
   的处理是哪一种（现在它只填 call_id，见事实 3）。
② 取消与幂等：同一 `call_id` 被取消后重试，补偿层要认得出是同一次（纪律 7 用 `call_id` 去重）。
③ 顺带查出的文档失实：AGENTS.md §3 写"内核八条设计纪律"，而 `DESIGN.md` 附录 C 实测有
   **11 条**（:2816-2828）。这条不属于本阶段射程，登记在台账，改它要动 `[LOCK]`。
④ wasm 侧的 deadline 是引擎 epoch（`wasm/backend.rs:120`）：它能掐死调用但**不表达原因**，
   接 C3 时要单独决定"epoch 到期"与"被取消"怎么区分，否则日志会写一件没发生过的事。
⑤ 本稿的六条现状事实全部带文件行号，开工前若对不上就是**代码漂了**，以现查为准订正本稿。
