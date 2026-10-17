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

设计稿。**一行代码没动**，等你对下面「要你定的三件事」点头后再开 C1。
进度只在下面的勾选框里，这一行不抄数。

## 现状的六条事实（逐条给出处，设计建立在这上面）

1. `CallCtx` 四个字段里 `deadline: Option<Instant>`（`crates/plugin-api/src/tool.rs:200`）与
   `cancelled: Arc<AtomicBool>`（:201，配 `is_cancelled()`:214 / `cancel()`:218，都是
   `Ordering::Relaxed`）。全仓 `deadline:` 的**赋值**只出现在构造函数 :209 —— 有消费者、无生产者。
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

- [ ] **C1** 内核加活动调用登记表：按 `(session_id, call_id)` 登记 `CallCtx` 的 `cancelled` 句柄，
      停止请求把该会话在途的逐个 `cancel()`，调用收束时注销（仿 `stop.rs` 现成的注册表 +
      RAII 形状，含"上一轮注销误删下一轮登记"那条按指针比较的先例）。
      · 验收 = 一条**行为**用例：一个会检查 `is_cancelled()` 的替身工具在途时被停止，
        它必须在下一轮询点返回、且会话日志里那次调用以**明确的一种收束**闭合，而不是跑完。
      · 变异：把登记表做成只登记不置位 ⇒ 该用例必须红。
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

## 要你定的三件事（点头之前 C1 也不动）

1. **被取消的调用在日志里怎么闭合**：
   (a) 复用 `ToolResult` 的 error 通道（`error: "cancelled by user"`），零新事件、重放天然同形，
       代价是"取消"和"失败"在一份字段里靠文案区分；
   (b) 新增一个事件变体或 `TOOL_STATUS` 的新取值，语义最干净，代价是动 `DESIGN.md` 的事件语义
       （`[LOCK]`）+ 重放投影要跟着改 + `scripts/check-channels.mjs` 那张对账表要加行。
   我的建议 = 先 (a)（不碰 [LOCK]、能立刻把 C1–C4 做完），(b) 留作它自己的刀。
2. **UI 要不要一档独立的"已取消"状态词**：不加就复用"失败"的视觉 + 文案区分；
   加就要同时定它在中英两侧的词、颜色语义档位（标准 §二 同类只许一种形态）。
3. **"尽力而为"这件事怎么对用户表达**：第三方插件不查询取消槽 ⇒ 它照样跑完。
   界面要不要在按停止后显示"取消中…"直到真结束（诚实但要多等），还是立刻按"已停止"显示
   （好看但撒谎）。我强烈建议前者——后者与 `events.rs:66-67` 那条"宁缺勿假"是同族的假。

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
