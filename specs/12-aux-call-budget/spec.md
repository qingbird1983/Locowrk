# 12-辅助调用预算与压缩可见性

- 状态：**已完成 · 刀①–③ 全收**（2026-10-21）——ADR-0008 落纸（刀①）、两笔挂账缺陷登记并另开一刀
  （刀③）；刀③ 收口时的真机证据与**证据边界**见文末「真机验收」节
- 负责人：agent
- 日期：2026-10-21
- 出处 / 上游：用户报告「**项目会话**在模型推理思考中经常卡死」（2026-10-21 对话）·
  根因分析见本 spec「根因」节；与 `specs/11` 刀⑤「自动压缩的界面反馈」同族（那一刀只补了
  **压成**的投影，**失败**那半至今没接）

## 目标

一句话：**摘要（辅助）调用不再能把整轮对话永久挂住，且挂住时用户看得见。**

三条落地：

1. **可中止**：摘要请求在重试等待与流式推进途中都查停止标记 ⇒ 用户点「停止生成」能停掉它。
2. **有预算**：摘要请求有**总时长预算**（默认 90s，三档可配）。超预算 = 这一次摘要失败 ⇒
   推进熔断计数 ⇒ 连续 3 次后自动压缩停手并如实告知，**主轮继续往下走**。
3. **看得见**：自动压缩的**开始**与**失败**在界面推系统条（复用手动 `/压缩` 那套文案），
   不再只有 `tracing::debug!`。

## 根因（这一节是本 spec 存在的理由）

卡死不在模型推理本身，而在「自动压缩的摘要调用」这条**辅助路径**上。它继承了主对话轮
「重试不设上限」那条裁定，却**没有继承停止能力**。

| # | 事实 | 位置 |
| --- | --- | --- |
| 1 | 压缩判定挂在 `kernel.step_as` **之前** ⇒ 界面此刻显示的就是「思考中」 | `desktop/src-tauri/src/runner/mod.rs:196-197` |
| 2 | 前端 `st.running` 全程挂着，直到 `agent_chat` 的 invoke 兑现才落 | `desktop/src/stores/chat/send.ts:32,41,53` |
| 3 | 摘要走 `aux` 通路 → `CompatLlm::complete_with` → **hooks 写死 `None`** | `llm/aux_client.rs:95` → `llm/client.rs:159` |
| 4 | 重试环**无次数上限**（2026-08 裁定「半次失败都不可接受」） | `llm/client.rs:182` + `llm/retry.rs:4-5` |
| 5 | 5xx / 网络错误 / 408 / 流空闲 180s 全归 `Transient` ⇒ 无限重试 | `llm/retry.rs:16,107` |
| 6 | hooks=None ⇒ `should_stop` 恒 false（**停止按钮无效**）；`on_retry` 无处可发 ⇒ 界面零反馈 | `llm/client.rs:428-442`、`llm/app_llm.rs:174-175` |
| 7 | 熔断（连续 3 次失败就停手）**永远触发不了**——失败压根不返回，`Failures::record` 也就永远不执行 | `compact.rs:45,150` + `compact/deep.rs:223` |
| 8 | 自动压缩失败在界面上**零提示**，只有 `tracing::debug!` | `runner/mod.rs:208` |

**为什么偏偏是项目会话**：项目会话多注入一层项目指令链（`InstructionFilesInject`）+ 历史更长
+ 工具输出更大 ⇒ 占用更容易越过压缩线 ⇒ 更容易进到第 1 行那条摘要路径。非项目会话同样会中招，
只是触发得晚。

**为什么这条裁定本身没错**：它的语境是「用户就在旁边看着的那一轮」。摘要路径用户根本看不见，
失败了就永远在那儿转。**错的是规矩漏了一条边界**，不是规矩错了。

## 验收标准（必须可测：跑什么命令、看到什么结果）

- [x] AC-1 **预算生效**：把摘要预算设到最短档（30s），构造一个摘要端点恒 5xx 的会话跑到
      压缩线 ⇒ 这次摘要在**约 30s 内**返回失败（不是无限重试），压缩层记一次失败，
      主轮继续（后续 step 照常跑完，turn 正常收束）。
      用例名：`a_summary_call_over_budget_fails_instead_of_retrying_forever`。
      **✅ 刀① 已达成**（2026-10-21）：本地 TCP 假端点恒 500 ⇒ 600ms 预算内返回
      `[budget]` 前缀的 Err；用例外层套 15s 超时（去掉预算 = 回到改前口径时它会
      **永远等下去**，套上超时才变成一条红断言而不是挂死整个测试进程）。
- [x] AC-2 **单次尝试也受预算约束**：摘要端点**挂死不吐字**（连接建立但零事件）⇒ 在预算内
      放弃，而不是等满 180s 空闲看门狗。
      用例名：`an_idle_summary_stream_is_aborted_by_the_budget_not_by_the_idle_watchdog`。
      **✅ 刀① 已达成**（2026-10-21）：假端点接受连接后一个字节都不写 ⇒ 700ms 预算内
      被掐掉（实测 < 5s，远不到 180s 看门狗）。
- [x] AC-3 **熔断真的能推进**：连续 3 次摘要失败后，第 4 次到线时**不再发起摘要请求**
      （请求数不增长），且界面出现「自动压缩已暂停」系统条。
      用例名：`three_failures_fuse_the_auto_compaction_and_stop_further_requests`。
      **✅ 刀① 已达成**（2026-10-21，后端半）：三次失败各记一格、第 4 次 `Skipped("fused")`
      且请求数不增长。**界面那半（系统条）✅ 刀② 已达成**（见 AC-5）。
      ⚠ 这条在改之前**测不出来**：旧口径下失败不返回 ⇒ 计数永远到不了 3。
      ⚠ 刀② 起第 4 次那一趟的返回形状由 `Outcome::Skipped("fused")` 改成
      `Outcome::Fused("fused")`（熔断单独一档——它与"不到线/组数不够/无可压"那几种
      常态不同，是**该说给用户听**的一件事；混在一处时调用方只能靠猜判据名的含义
      来分辨该不该打扰用户）。既有三条判据同步改断言。
- [x] AC-4 **停止按钮能停掉摘要**：摘要请求进行中点「停止生成」⇒ 摘要在 1s 内返回，
      整轮以 `Interrupted` 收束（不是 `Completed`、不是失败），且**不计入熔断失败数**
      （用户主动停不是失败）。
      用例名：`stopping_during_a_summary_call_does_not_count_as_a_failure` +
      `a_stop_during_compaction_ends_the_turn_as_interrupted`。
      **✅ 刀① 已达成**（2026-10-21）：压缩层报 `Skipped("stopped")` 且熔断计数仍为 0；
      接线面整轮以 `TurnEnd{outcome: Interrupted}` 收束。
- [x] AC-5 **界面看得见**：自动压缩到线 ⇒ 界面先出「正在压缩对话上下文…」系统条；
      摘要失败 ⇒ 该条被收走并换成「没压缩：…」系统条（文案随失败原因不同）；
      熔断 ⇒ 出「自动压缩已暂停…」系统条。三种文案中英成对（`check-i18n` 绿）。
      用例名：`the_auto_compaction_reports_its_start_and_its_failure_in_the_transcript`。
      **✅ 刀② 已达成**（2026-10-21）：
      - 后端接线面（`tests/compact_status_wiring.rs`）：
        `the_auto_compaction_reports_its_start_and_its_failure_to_the_ui` 钉住相位事件序
        `[running, failed(summary-call)]`；`the_breaker_pauses_the_auto_compaction_in_the_ui`
        钉住四轮之后 `[running, failed] ×3 + paused` 且**摘要请求数 = 3**（第 4 次没发）。
      - 界面投影面（`desktop/tests/chat-projection.spec.ts`）：四相各一条用例，钉住
        **同一根线**（`tag=compact-auto`）被就地改写、压成了那根收走、认不出的判据名
        走兜底那条并把名字原样带出。
      - 文案：`notice.compactPaused`（熔断）新补；`compactBudget` / `compactStopped`
        沿用刀①。
      ⚠ 熔断那一相**走相位 `paused`、不走判据名机制**（订正见「设计要点 ⑤」）。
- [x] AC-6 **主对话轮行为不变**：给主对话轮（不传预算）注入一个恒 5xx 的端点 ⇒ 仍然
      **无限重试**（现行裁定不变），且重试仍然推 `agent://llm-retry`、仍然可被「停止生成」中止。
      用例名：`the_main_turn_keeps_its_unbounded_retry_and_its_stop_hook`。
      **✅ 刀① 已达成**（2026-10-21，能力面 + 接线面两半）：
      `the_main_turn_keeps_its_unbounded_retry_and_its_stop_hook`（`CompatLlm` 传 `None`
      时仍无限重试 + 停止钩子有效）+ `the_main_turn_call_site_passes_no_budget`（主对话轮
      的实际调用点真的传 `None`——少这一半，谁在 `app_llm.rs` 里手滑填一个
      `Some(...)`，前一条照样绿）。
- [x] AC-7 **设置面**：设置 → 上下文管理多一行「摘要调用预算」三档预置（30s / 90s / 300s，
      默认 90s），**不是数字输入框**（S4 刀④ 的普通分区约束不变）；
      改动即时生效（下一轮压缩读新值，不重启）。
      用例名：`the_summary_budget_is_a_three_preset_picker_and_takes_effect_next_round`。
      **✅ 刀② 已达成**（2026-10-21）：
      - 界面面（`desktop/tests/context-section.spec.ts`）：三档都在、缺省那档高亮；
        点档改值、载荷带上那一档；存量非档值（45s）**就近显示但不改磁盘值**；
        本区**零个数字输入框**（既有那条用例继续绿）。
      - 后端面（`tests/compact_status_wiring.rs`）：
        `the_summary_budget_takes_effect_on_the_next_round` 钉住摘要请求**真的带上**
        设置里那一档，且改完设置**下一轮**就换（新值 30s → 300s），不重启。
      - 形态：复用 `SettingsSegmented`（现役形态：主题/字号/列宽都在用）⇒ **不需要
        新形态申请**。档位清单在 `desktop/src/lib/summary-budget.ts`（真源 = 后端
        `settings/context.rs` 的 `SUMMARY_BUDGET_PRESETS_MS`，后端那份是校验权威）。
- [x] AC-8 **门禁**：`make check` 全绿；本刀新增/改动的每条判据做**变异**必须报红
      （`make selftest` 读数逐条列出）；`check-channels` 认得新通道；
      `check-single-source` 的 SS24 两侧取值域仍相等。
      **✅ 刀③ 已达成**（2026-10-21）：`make check` 退出码 **0**（Rust 438 passed ·
      vitest 146 passed / 21 files · clippy 两处零警 · `check-channels`
      `CONSTANTS=23 VARIANTS=22 SUBSCRIBED=19 FAILED=0` · SS24 `same(9)`、SS25 `9`）；
      `make selftest` 全绿（`SELFTEST_ALL_CAUGHT` + `DOCS_SELFTEST_ALL_CAUGHT`）。
      变异自证 = 刀① 五条 + 刀② 六条，逐条报红（刀② 那张表在下面）。
      ⚠ 门禁读数**不接管道**（`cmd | tail` 的退出码是 tail 的 = 假绿）：退出码单独取。

## 非目标

- **不改主对话轮「重试不设上限」那条裁定**（用户 2026-08 原话，本 spec 明确不动）。
- **不删任何磁盘数据**，不动 `~/.Locowrk/` 下任何东西。
- **不动 `crates/plugin-api` 的冻结 ABI**。
- **不顺手修两处已定位但同族不相干的缺陷**（登记为挂账，见「风险与回滚」）：
  - `llm/wire/stream.rs:66-72` 推理优先 `return` ⇒ 同一 chunk 里同时带 `reasoning_content`
    与 `content` 时正文被丢。
  - `desktop/src/stores/chat/projection.ts:287` `resetAssistant` 只清 assistant 气泡、
    不清 thinking 行 ⇒ 只推过推理就重试时思考内容重复累加。
- **不把自动压缩改成后台异步**（压缩要同步、要在 step 前完成，这是设计口径不是 bug）。
- **不给摘要请求接 `agent://llm-retry`**（那会让摘要重试顺手清掉主对话的 assistant 气泡＝回归）。

## 设计要点

**① 预算是「辅助路径的例外」，不是全局改动。**
`CompatLlm::complete_streaming_with` 的重试环加一个 `budget: Option<Duration>` 参数：
主对话轮传 `None`（行为逐字不变），摘要路径传 `Some(设置值)`。
预算是**总时长**（含所有重试），不是单次尝试。

**② 预算必须包住单次尝试，否则形同虚设。**
只查「重试前累计耗时」不够：一次失败尝试自己就能等满 180s 空闲看门狗。
⇒ 每次尝试用 `tokio::time::timeout(剩余额度, attempt_stream(...))` 包住；
超时 = 预算耗尽 = 这一次摘要失败。

**③ 预算耗尽要与「别的失败」区分开，界面才说得出真话。**
`llm/retry.rs` 加一份判据（同款先例 = `is_overflow_message`）：错误文本以 `[budget]` 开头
= 预算耗尽 ⇒ 压缩层给 `why = "budget"` ⇒ 界面「没压缩：摘要调用超出 N 秒预算」。
不靠模糊关键词猜。

**④ 停止信号要一路通到摘要请求，且不污染熔断账。**
`SummaryClient` 的入参收成一个 `SummaryRequest`（system / user / model / session_id / budget_ms）；
`session_id` 只为**停止标记**与**日志归属**用，**不改** `LlmRequest.session_id`
（摘要不吃会话级模型覆盖是既有口径，动了就是回归）。
压缩层拿到 `StopFlags`（组装点已有一份）⇒ 摘要返回后查一次停止标记 ⇒ 命中则
`Outcome::Skipped("stopped")`：**不记失败**（用户主动停不是失败）。

**⑤ 可见性走新通道，不复用压成那条。**
`agent://context-compacted` 是「压成了」，语义已占；失败/开始是另一件事 ⇒ 新通道
`agent://compact-status`，载荷 `{ session_id, phase, reason }`。
`phase` 四相 = `running`（正在压）/ `failed`（这一趟没成，`reason` = 判据名）/
`paused`（熔断停手）/ `done`（压成了）。

**⑤-补 熔断走相位、不走判据名机制（2026-10-21 刀② 落地时订正）。**
原写「新增 `budget` 与 `fused` 两键」——`budget` 属实，`fused` **不成立**：
判据名 `fused` 在 SS24 的 **YIELDED** 集合里（手动 `/压缩` 让开熔断 ⇒ 手动通路永远
不会产生它），而那张「判据名 → 文案」表是**手动通路的取值域**。把 `fused` 塞进去
要么让 SS24 的两侧相等读数破（要么把名单写死在闸里——那正是该闸明令禁止的
「从代码里推、不写死」），要么让界面把"熔断"当成"没压成的一种原因"来渲染。

⇒ 裁法：**相位与判据名分两样东西**。相位说"这一趟处于什么状态"（running / failed /
paused / done），判据名说"为什么没成"（`summary-call` / `overflow` / `rejected` /
`budget` / `stopped`）。失败那几枚照旧走判据名（前端复用手动那张表，SS24 两侧
**一字未动**：11 / 9），熔断改走相位 `paused`，于是"被熔断停手"有了自己独立的一句话
（`notice.compactPaused`）。后端那侧对应地多一枚 `Outcome::Fused(&'static str)`
（它与 `Skipped` 那一族常态分开），而字面量 `"fused"` 仍**只有**
`compact.rs` 的 `Plan::stop("fused")` 一个出处。

**⑥ 依赖方向不新增反向边。**
压缩层不引 Tauri：`Compactor::run` 收一个 `&(dyn Fn() + Send + Sync)` 的「开始了」回调，
由 runner 翻译成 `AgentEvent`（同款纪律：`llm` 模块零 UI 依赖）。

长期取舍（辅助路径与主对话轮的边界）另写一篇 `docs/adr/0008-aux-call-budget-vs-main-retry.md`，
本 spec 只给指针。

## 被否决的方案

| 方案 | 否决原因 |
| --- | --- |
| 全局给重试环加次数上限 | 推翻你 2026-08 的裁定，且主对话轮那一侧的韧性会下降——收益与代价不对等 |
| 只补界面反馈，不动重试语义 | 用户能看见「没压成」，但**停不下来**这件事还在——最恶劣的性质留着 |
| 把「关掉自动压缩」当长期方案 | 上下文会无限涨到撞窗；那是缓解不是修复（可作为本刀落地前的临时手段） |
| 让摘要改走 `AppLlm::prepare`（复用整条管线） | `prepare` 会应用**会话级模型覆盖** ⇒ 摘要会吃会话的模型/档位，违反既有口径 |
| 给摘要接 `agent://llm-retry` | 摘要重试会顺手清掉主对话的 assistant 气泡（`resetAssistant`）＝制造新回归 |
| 摘要失败就硬报错、终止整轮 | 压缩失败不该杀死主任务（上下文压不动 ≠ 任务做不完）；熔断 + 如实告知更合适 |

## 计划

- **刀① 摘要调用可中止 + 有预算**（后端主线）：`CompatLlm` 预算参数 ⇒ `SummaryRequest` 重构
  ⇒ `Compactor` 拿停止标记 ⇒ runner 收束。产出 AC-1/2/3/4/6。**✅ 2026-10-21**
- **刀② 压缩可见性 + 设置面**（界面主线）：新通道 `agent://compact-status` ⇒ 前端投影与文案
  ⇒ 设置页三档预算。产出 AC-5/7 与 `check-channels` / `check-i18n` 对账。**✅ 2026-10-21**
- **刀③ 收口**：ADR-0008 落纸、挂账登记、`make check` + `make selftest` 全绿、
  roadmap 勾选、按 `CONTRIBUTING.md` 落提交。**✅ 2026-10-21**
  （ADR-0008 实际随刀① 落纸；挂账两笔登记在本 spec「非目标」与「风险与回滚 → 挂账 / 遗留」两处，
  另开一刀，**不在本 spec 里顺手修**）

## 任务

- [x] 刀①：`llm/client.rs` 重试环加 `budget`（总时长，含单次尝试超时包裹）
- [x] 刀①：`llm/retry.rs` 加预算耗尽判据（`[budget]` 前缀）
- [x] 刀①：`llm/aux_client.rs` 入参收成 `SummaryRequest`；摘要路径接停止标记
- [x] 刀①：`compact/deep.rs` 拿 `StopFlags`；`stopped` 不记失败；`budget` 单独一档 why
- [x] 刀①：`runner/mod.rs` 收 `stopped` ⇒ 轮以 `Interrupted` 收束
- [x] 刀①：SS24/SS25 的期望值随两枚新判据名同步（9→11 / 7→9）+ 前端两条文案补齐
- [x] 刀②：`events.rs` 加 `CompactStatus` 事件与通道（四相常量同处一文件）
- [x] 刀②：前端 `projection.ts` 投影 + `notices.ts` 收拢 `COMPACT_SKIP_KEYS`（单一出处）
- [x] 刀②：中英文案成对（`compactPaused` 新补；`compactBudget` / `compactStopped` 刀① 已补）
- [x] 刀②：设置页三档预算 + 即时生效（`SettingsSegmented` 复用，无数字输入框）
- [x] 刀③：ADR-0008（随刀① 落纸）+ 挂账登记 + roadmap 勾选
- [x] 补测试（刀①七条：AC-1/2/3/4/6；刀②八条：AC-5 接线面二 + 界面投影五 + AC-7 界面面三
      / 后端面一 ⇒ 后端三 + 前端五）
- [x] 变异自证（刀①五条 + 刀②六条，逐条报红，见提交正文）
- [x] `make check` 全绿 + `make selftest` 变异逐条报红（刀③ 收口时复跑，退出码 0）

### 刀② 的变异自证（六条，逐条报红；读数见提交正文）

| # | 注入 | 报红的判据 |
| --- | --- | --- |
| ① | 压缩层不调「开始了」回调 | AC-5 接线面 + 熔断那一条（读数缺 `running` 相位） |
| ② | runner 不推 `failed` 相位 | 同上两条（读数缺 `failed` 相位） |
| ③ | runner 不推 `paused` 相位 | 只有熔断那一条（读数缺 `paused` 相位） |
| ④ | 摘要请求不带预算（`budget: None`） | 只有 AC-7 后端面那一条（读数 `None` vs `Some(30s)`） |
| ⑤ | 前端不就地改写（每相各落一根线） | 界面投影三条（读数多出 `notice.compactRunning`） |
| ⑥ | 设置面保存时不带预算那一键 | 设置面三条（载荷少一键 / 值 `undefined`） |

⚠ 六条都是**红断言**而不是挂死：变异①那一档原先会把测试进程挂住（正是"判据
对'永不返回'零证明力"那个形状），所以那几条的断言读数全是"事件序对不上"这种
一眼能读出差在哪的形态。

## 真机验收（2026-10-21，`pnpm tauri dev` 真实窗口）

| 面 | 怎么看 | 读数 |
| --- | --- | --- |
| 设置面「上下文管理」 | 新增第四格「摘要调用预算」 | 三档 `30 秒 \| 90 秒（选中） \| 300 秒`，无错位/裁切；首轮截图发现说明行第三行只剩「响。」的**孤儿行** ⇒ 已把中英两条说明缩短到一行并重截确认已消（该修正在刀③ 收尾的提交里） |
| 转录面（压缩状态线） | 空会话里敲 `/压缩`（英文别名） | 转录顶部出那根发丝线：「没压缩：对话还太短，没有可剪的一段（至少两轮往返）。」⇒ **压缩状态线在真实窗口渲染正确**（同一发丝线形态、同一文案通路） |

**证据边界（哪一条没在真机上亲眼见过）**：

- **自动压缩的三相（正在压 / 这一趟没成 / 熔断停手）在真机上未亲眼见过。** 触发条件是
  「占用 ≥ 有效阈值」，而有效阈值 = `min(滑杆 100K, 模型窗口 − 输出预留)` ⇒ 本机
  MiniMax-M3 取 **100K token**；要触发就得先攒够 10 万 token 的上下文（一次真实的大上下文
  调用）并往用户数据目录写一个巨型会话 ⇒ **未擅自做**（花用户额度 + 动用户数据，两样都要点头）。
- 这一段的证据在别处：后端接线由 AC-5 接线面三条钉住（**真 runner + 真事件 sink**，相位序
  `[running, failed]` / `[running, failed]×3 + paused`），界面投影由 AC-5 界面面四条 vitest 钉住
  （同一根线被就地改写、压成了那根收走、认不出的判据名兜底带名）。
- ⚠ 归类：这是**取证边界**，不是缺陷 —— 阈值 100K 是产品口径（`settings/context.rs` 的
  `COMPACT_THRESHOLD_RANGE` 硬校验 100K–800K），不是这一刀引入的。

**两处取证工具自身的坑（不是产品缺陷，但会让下一次取证白跑）**：

1. 点击坐标：输入框在窗口坐标 y≈395–477，第一版脚本点的是 y=736（输入框**下方**的空白）
   ⇒ 文字压根没进输入框，界面当然什么也没有。**坐标逐字反算，别目测。**
2. 中文输入法把 `compact` 当拼音 ⇒ 候选条会抢走回车（敲下去选中的是「聪明」）。要么先切英文
   输入态再回车，要么走斜杠面板的「点击触发」。

## 影响范围

| 文件 / 目录 | 改动 |
| --- | --- |
| `desktop/src-tauri/src/llm/client.rs` | 重试环加预算参数 + 单次尝试超时包裹 |
| `desktop/src-tauri/src/llm/retry.rs` | 预算耗尽判据 |
| `desktop/src-tauri/src/llm/aux_client.rs` | `SummaryRequest` 重构；接停止标记 |
| `desktop/src-tauri/src/compact/deep.rs` | 预算透传；`stopped` / `budget` 两档；`Outcome::Fused` 变体；`run` 收「开始了」回调 |
| `desktop/src-tauri/src/compact.rs` | `Plan` 带 `fused` 标记（熔断单独一档）；两张共用表拆到 `compact/tables.rs`（400 行纪律） |
| `desktop/src-tauri/src/compact/tables.rs` | **新**：两张共用状态表（`Failures` / `Overflow`），路径不变（`compact.rs` 重新导出） |
| `desktop/src-tauri/src/runner/mod.rs` | `stopped` ⇒ `Interrupted` 收束；四相事件投影 |
| `desktop/src-tauri/src/events.rs` | `CompactStatus` 事件 + 通道 + 四相常量 |
| `desktop/src-tauri/src/tests/compact_status_wiring.rs` | **新**：压缩的界面反馈与预算接线面判据（AC-5 / AC-3 界面半 / AC-7 后端面） |
| `desktop/src-tauri/src/settings/context.rs` | 预算一枚（三档） |
| `desktop/src-tauri/src/state/background.rs` | 组装点注入停止标记 |
| `desktop/src/stores/chat/{projection,notices,send}.ts` | 投影分支 + `COMPACT_SKIP_KEYS` 收拢 + 自动压缩那根线（tag `compact-auto`） |
| `desktop/src/types/events-agent.ts` | `CompactStatusEvent` 镜像 + `CompactPhase` 取值域 |
| `desktop/src/lib/api/channels.ts` | 订阅 `agent://compact-status` |
| `desktop/src/lib/summary-budget.ts` | **新**：预算三档清单 + 就近显示 |
| `desktop/src/components/settings/` | 三档预算控件 |
| `desktop/src/locales/{zh-CN,en}/` | 新文案成对（`notice.compactPaused` + 设置面三键） |
| `docs/adr/0008-*.md` | 长期取舍落纸 |

## 风险与回滚

- **风险 ①**：预算设得太短 ⇒ 正常的大上下文摘要被误判失败（连续 3 次后自动压缩停手）。
  缓解：三档默认 90s；失败时界面说清是「预算」不是「网络」；手动 `/压缩` 不受此限。
- **风险 ②**：给 `SummaryClient` 换入参会波及它的测试替身（`compact/tests*.rs` 里的脚本化假实现）
  ——那是好事（替身能造出「预算耗尽」这个形状），但要同步改。
- **风险 ③**：新通道漏登记 ⇒ `check-channels` 报红（这正是它存在的意义）。
- **挂账 / 遗留**：
  - `llm/wire/stream.rs:66-72` 推理优先丢正文 —— 单独一刀。
  - `desktop/src/stores/chat/projection.ts:287` `resetAssistant` 不清 thinking 行 —— 单独一刀。
  - `specs/11` 刀⑤ 说的「失败不静默」只兑现了日志那一半，本 spec 补上界面那一半，
    该条从「已达成」改注「界面半由 specs/12 刀②兑现」。
- **回滚**：三个刀各自独立成提交 ⇒ 任一刀出问题可单独回滚；设置项的预算档位可调回长档
  当场缓解，不必回滚代码。
