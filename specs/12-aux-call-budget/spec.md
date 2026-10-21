# 12-辅助调用预算与压缩可见性

- 状态：进行中（刀① ✅ 2026-10-21；刀②/刀③ 待做）
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
      且请求数不增长。**界面那半（系统条）属刀②**。
      ⚠ 这条在改之前**测不出来**：旧口径下失败不返回 ⇒ 计数永远到不了 3。
- [x] AC-4 **停止按钮能停掉摘要**：摘要请求进行中点「停止生成」⇒ 摘要在 1s 内返回，
      整轮以 `Interrupted` 收束（不是 `Completed`、不是失败），且**不计入熔断失败数**
      （用户主动停不是失败）。
      用例名：`stopping_during_a_summary_call_does_not_count_as_a_failure` +
      `a_stop_during_compaction_ends_the_turn_as_interrupted`。
      **✅ 刀① 已达成**（2026-10-21）：压缩层报 `Skipped("stopped")` 且熔断计数仍为 0；
      接线面整轮以 `TurnEnd{outcome: Interrupted}` 收束。
- [ ] AC-5 **界面看得见**：自动压缩到线 ⇒ 界面先出「正在压缩对话上下文…」系统条；
      摘要失败 ⇒ 该条被收走并换成「没压缩：…」系统条（文案随失败原因不同）；
      熔断 ⇒ 出「自动压缩已暂停…」系统条。三种文案中英成对（`check-i18n` 绿）。
      用例名：`the_auto_compaction_reports_its_start_and_its_failure_in_the_transcript`。
      ⏳ **刀②**（后端两枚判据名 `budget` / `stopped` 的文案已在刀① 补齐——
      SS24 强制的前后端同源义务；自动路径的事件投影属刀②）。
- [ ] AC-6 **主对话轮行为不变**：给主对话轮（不传预算）注入一个恒 5xx 的端点 ⇒ 仍然
      **无限重试**（现行裁定不变），且重试仍然推 `agent://llm-retry`、仍然可被「停止生成」中止。
      用例名：`the_main_turn_keeps_its_unbounded_retry_and_its_stop_hook`。
      **✅ 刀① 已达成**（2026-10-21，能力面 + 接线面两半）：
      `the_main_turn_keeps_its_unbounded_retry_and_its_stop_hook`（`CompatLlm` 传 `None`
      时仍无限重试 + 停止钩子有效）+ `the_main_turn_call_site_passes_no_budget`（主对话轮
      的实际调用点真的传 `None`——少这一半，谁在 `app_llm.rs` 里手滑填一个
      `Some(...)`，前一条照样绿）。
- [ ] AC-7 **设置面**：设置 → 上下文管理多一行「摘要调用预算」三档预置（30s / 90s / 300s，
      默认 90s），**不是数字输入框**（S4 刀④ 的普通分区约束不变）；
      改动即时生效（下一轮压缩读新值，不重启）。
      用例名：`the_summary_budget_is_a_three_preset_picker_and_takes_effect_next_round`。
      ⏳ **刀②**（后端读口与三档校验已在刀① 落 `ContextSettings.summary_budget_ms`；
      前端控件与文案属刀②）。
- [ ] AC-8 **门禁**：`make check` 全绿；本刀新增/改动的每条判据做**变异**必须报红
      （`make selftest` 读数逐条列出）；`check-channels` 认得新通道；
      `check-single-source` 的 SS24 两侧取值域仍相等。
      **⏳ 刀③**（SS24/SS25 的期望值已在刀① 同步改到 11/9，`make selftest` 已绿——
      变异仍报红；`check-channels` 的新通道属刀②）。

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
`agent://compact-status`，载荷 `{ session_id, phase: "running" | "failed", reason }`。
`reason` 取值域与手动 `/压缩` 那张 `COMPACT_SKIP_KEYS` **同源**（SS24 钉住），
新增 `budget` 与 `fused` 两键，两侧同步。

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
  ⇒ `Compactor` 拿停止标记 ⇒ runner 收束。产出 AC-1/2/3/4/6。
- **刀② 压缩可见性 + 设置面**（界面主线）：新通道 `agent://compact-status` ⇒ 前端投影与文案
  ⇒ 设置页三档预算。产出 AC-5/7 与 `check-channels` / `check-i18n` 对账。
- **刀③ 收口**：ADR-0008 落纸、挂账登记、`make check` + `make selftest` 全绿、
  roadmap 勾选、按 `CONTRIBUTING.md` 落提交。

## 任务

- [x] 刀①：`llm/client.rs` 重试环加 `budget`（总时长，含单次尝试超时包裹）
- [x] 刀①：`llm/retry.rs` 加预算耗尽判据（`[budget]` 前缀）
- [x] 刀①：`llm/aux_client.rs` 入参收成 `SummaryRequest`；摘要路径接停止标记
- [x] 刀①：`compact/deep.rs` 拿 `StopFlags`；`stopped` 不记失败；`budget` 单独一档 why
- [x] 刀①：`runner/mod.rs` 收 `stopped` ⇒ 轮以 `Interrupted` 收束
- [x] 刀①：SS24/SS25 的期望值随两枚新判据名同步（9→11 / 7→9）+ 前端两条文案补齐
- [ ] 刀②：`events.rs` 加 `CompactStatus` 事件与通道
- [ ] 刀②：前端 `projection.ts` 投影 + `notices.ts` 收拢 `COMPACT_SKIP_KEYS`（单一出处）
- [ ] 刀②：中英文案成对（`compactFused` 待补；`compactBudget` / `compactStopped` 已在刀① 补）
- [ ] 刀②：设置页三档预算 + 即时生效
- [ ] 刀③：ADR-0008 + 挂账登记 + roadmap 勾选
- [x] 补测试（AC-1/2/3/4/6 七条：`llm::tests::budget` 五条 + `compact::tests_deep` 一条
      + `tests::compact_wiring` 一条）
- [x] 变异自证五条（逐条报红，见提交正文）
- [ ] `make check` 全绿 + `make selftest` 变异逐条报红

## 影响范围

| 文件 / 目录 | 改动 |
| --- | --- |
| `desktop/src-tauri/src/llm/client.rs` | 重试环加预算参数 + 单次尝试超时包裹 |
| `desktop/src-tauri/src/llm/retry.rs` | 预算耗尽判据 |
| `desktop/src-tauri/src/llm/aux_client.rs` | `SummaryRequest` 重构；接停止标记 |
| `desktop/src-tauri/src/compact/deep.rs` | 预算透传；`stopped` / `budget` 两档 |
| `desktop/src-tauri/src/compact.rs` | `Failures` 语义注释（熔断现在真的能触发） |
| `desktop/src-tauri/src/runner/mod.rs` | `stopped` ⇒ `Interrupted` 收束；失败投影 |
| `desktop/src-tauri/src/events.rs` | `CompactStatus` 事件 + 通道 |
| `desktop/src-tauri/src/settings/context.rs` | 预算一枚（三档） |
| `desktop/src-tauri/src/state/background.rs` | 组装点注入停止标记 |
| `desktop/src/stores/chat/{projection,notices,send}.ts` | 投影分支 + `COMPACT_SKIP_KEYS` 收拢 |
| `desktop/src/components/settings/` | 三档预算控件 |
| `desktop/src/locales/{zh-CN,en}/` | 新文案成对 |
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
