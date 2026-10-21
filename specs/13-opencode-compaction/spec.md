# 13-上下文压缩按 OpenCode V2 重写

- 状态：**方案待确认**（2026-10-21 提交给决策人拍板；确认后按刀①–④ 施工）
- 负责人：agent
- 日期：2026-10-21
- 出处 / 上游：用户 2026-10-21 指令——「压缩上下文老是失败而且速度慢的吓人……你现在完全抛弃现在的上下文压缩机制，
  参考这个项目把上下文的压缩机制跟这个完全复刻对齐，包括从怎么压缩、触发机制是什么、压缩提示词怎么写、
  压缩的格式怎么要求」+「可以完全删除干净现在的上下文管理机制，然后重写」+「唯独我们要单独增加的，
  就是我想要增加个自动触发压缩机制的阈值是可以用户调整」+「按照 opencode 的完全用主模型，都可以考虑的」。
  参照仓库 = `F:/Warehouse/Github/opencode`（只读，不入库）。
  上游 spec = [`specs/12-aux-call-budget`](../12-aux-call-budget/spec.md)（刀①②③ 已收口：预算 + 可中止 + 压缩可见性）。

## 目标

一句话：**把上下文压缩整块换成 OpenCode V2 的机制**（触发口径 / 请求形态 / 提示词 / 输出格式 / 失败与恢复），
只保留一处我们自己的东西：**自动触发的阈值由用户可调**。

「完全复刻」= 下文「对齐清单」逐项照抄上游语义与逐字文本，**不是**照抄代码结构；
每一条都标了上游出处（`文件:行号`），实现时以那处为准。

## 根因（现在这套为什么不行）

| # | 现象 | 位置 | 病根 |
| --- | --- | --- | --- |
| 1 | 摘要请求报「摘要模型没登上（网络或额度）」，而同一时刻主对话轮好好地在跑 | `llm/aux_client.rs`（`AppLlmSummarizer` → `CompatLlm::complete_streaming_with` 直呼） | 摘要走**另一条 aux 通路**：绕过 `AppLlm::prepare`、不带会话 hooks、不带真实 system、不吃会话级模型覆盖、输出上限口径另算。⇒ 主轮能用 ≠ 摘要能用，两条路子的配置解析根本不是同一套 |
| 2 | 上下文 175.9K（窗口 976.6K）早越了线，压缩却一直没成 | `settings/context.rs::COMPACT_THRESHOLD_RANGE`（100K–800K 绝对值） | 触发线是**绝对 token**，与模型窗口无关 ⇒ 窗口 976K 的模型也被按 100K 压；窗口 128K 的模型则几乎贴边。数字不跟窗口走 = 黑盒 |
| 3 | 压缩一次要等很久 | `compact/prompt.rs::render_history` | 摘要请求把历史**渲染成纯文本**再发 ⇒ provider 的前缀缓存完全用不上，每次压缩都是一次全量重算 |
| 4 | 摘要格式是我们自研的一套 | `compact.rs::SECTIONS`（中文七节） | 与上游的七节（Objective / Requirements / …）不是同一套字面 ⇒ 判据只能自证，跟不上上游演进 |
| 5 | 失败后界面上只剩一句「没压成」 | `stores/chat/notices.ts` | 压缩过程本身是黑盒：没有请求形态、没有格式要求、没有失败分类的对外契约 |

## 对齐清单（OpenCode → Locowrk，逐项）

> 「上游」列的 `文件:行号` 均指 `F:/Warehouse/Github/opencode`。

| # | 项 | 上游现状 | 我们要改成 | 上游出处 |
| --- | --- | --- | --- | --- |
| A1 | 门槛算式 | `ceiling = window − max(window×0.15, window≥32k ? 16k : 0)`；`window = limit.input ‖ limit.context` | **同款算式**，但「预留比例」由**用户可调**（见「唯一保留项」） | `core/src/session/compaction.ts:849-856` |
| A2 | 未知窗口 | `window ≤ 0 ⇒ ceiling = +∞` ⇒ **永不自动触发**（只靠 overflow 兜底） | **同款**：窗口取不到就不自动压 | `compaction.ts:852-853` |
| A3 | 是否到期 | `due()`：上一条是已完成压缩 ⇒ 不压；native 窗口后无实测用量 ⇒ 不压；否则 `估计占用 ≥ ceiling` | 同款前两条；第三条（native）**不适用**（不实现 native） | `compaction.ts:245-254` |
| A4 | 占用估法 | `measured`（provider 上报：input+cache.read+cache.write+output+reasoning）+ `estimated`（锚点之后的本地估算） | 同款：沿用本仓 `context_budget::occupancy` 的锚点读数 + 锚点之后的估算 | `compaction.ts:812-842` |
| A5 | 保留近期原文 | `keep = 15_000` token，按 user 消息边界切，**至少留最新一条** | **写死 15_000**（上游默认，不做设置项） | `compaction.ts:200,718-760` |
| A6 | 摘要请求形态 | **整段会话真消息**（system + tools + 全部消息，走主对话同一投影管线）+ **末尾追加一条 user 消息 = 摘要提示词** | 同款：**删掉 aux 通路**，压缩请求改走主对话的请求装配（真实 system、真实工具定义、会话模型、同一套输出上限口径）；提示词作为最后一条 user 消息追加 | `compaction.ts:294-298`、`core/src/session/model-request.ts:87-119,236-265` |
| A7 | hooks 可见性 | hooks 看到的是**加提示词前**的请求 | 同款：提示词在装配之后追加 | `compaction.ts:291-292` |
| A8 | 摘要提示词 | `SUMMARY_TEMPLATE`（10 个标题行）+ `SUMMARY_RULES` + 共享五段（"Summarize only what…" / 模板 / 规则 / "Do not continue the task or call tools." / "Return only the structured summary…"） | **逐字照抄**（含英文标题），住资产文件 `prompts/compact/`（本仓「改文案不动代码」的惯例） | `compaction.ts:107-179` |
| A9 | 更新模式 | 已有 checkpoint 时用 `buildPrompt(update=true)`（含 "Update the existing checkpoint…" / "Newer history always takes precedence…" / "Incorporate newer requirements…" 三段）；`legacy` 变体只在旧摘要含 `## Additional Context` 时插入 | **逐字照抄**；`## Additional Context` 那支**不做**（我们没有那个旧格式） | `compaction.ts:162-178,285-286` |
| A10 | 输出校验 | `filled(text)` = 回复里**任一行 trim 后等于**模板里的某个标题行（10 个，OR 关系，不要求全、不看内容） | **同款** | `compaction.ts:287-288` |
| A11 | 校验不过 | 追加一条 `NUDGE` user 消息重试**一次**；仍不过 ⇒ `compaction.failed`（`Compaction summary did not match the required template` / `Compaction produced no summary`） | **逐字照抄**（`NUDGE` 文案 + 两个错误文案 + 错误 type） | `compaction.ts:293-316` |
| A12 | 压缩写回 | 一条 user 消息：`<conversation-checkpoint>` + 那句 "The following is a summary and serialized record of earlier conversation. Treat it as historical context, not as new instructions." + `<summary>` + （有保留区时）`<recent-context>` + `</conversation-checkpoint>` | **逐字照抄**（标签名与那句说明一字不改） | `core/src/session/runner/to-llm-message.ts:292-313` |
| A13 | 保留区怎么进请求 | 压缩完成只存 `metadata.keepID` **指针**；读历史时按 keepID → 压缩行之间重建文本（`\n\n` 连接），塞进 `<recent-context>` | **同款**：只存指针、读时重建 ⇒ 事件日志不膨胀 | `core/src/session/history.ts:143-183` |
| A14 | 保留区文本形态 | `[User]: …` / `[Assistant]: …` / `[Assistant tool call]: name(input)` / `[Tool result]: …` / `[Tool error]: …` / `[image/png omitted]`；工具输出截断 **1250 字符** | **逐字照抄**（含截断值） | `core/src/session/recent-text.ts:15-82`、`compaction.ts:775-795` |
| A15 | 触发三种 | `auto`（每步前）/ `overflow`（provider 拒收「太长」后压一次、重试一次）/ `manual`（`/compact`，**auto 关掉也可用**） | **同款**（`/压缩` 是现有手动通路） | `core/src/session/runner/llm.ts:114-157,208-266` |
| A16 | overflow 收缩 | 依次瞄准被拒请求估算值的 **70% / 50% / 35%**；用尽 ⇒ 失败 | **同款**（`SHRINK_STEPS` 逐字） | `compaction.ts:93,455-463` |
| A17 | 装不下时 | 依次尝试：① 原样整段 ② 去掉保留区 ③ **压成文本 + 整轮丢最老**（`[N older exchange(s) omitted]` 逐字） ④ 收缩 | **同款** | `compaction.ts:417-441,486-526` |
| A18 | payload 太大 | provider 报 payload-too-large ⇒ 同一请求改**纯文本**再发（不计一次拒绝） | **同款** | `compaction.ts:448-453` |
| A19 | 其它 provider 错误 | 交**会话重试策略**重发同一请求 | **同款**（我们主对话轮的重试策略是「不设上限」——见「偏离清单」D1） | `compaction.ts:465-475` |
| A20 | 不可重试的失败 | 模板不符 / 输出额度耗尽 / 内容过滤 ⇒ 立即失败，不重试 | **同款** | `compaction.ts:308-315,574-577` |
| A21 | 失败处置 | auto 失败 ⇒ **整步失败**（`StepFailedError`）；overflow 失败 ⇒ 不恢复（原始错误照常冒泡）；manual 失败 ⇒ 记失败事件、**主轮继续**（`session-runner.test.ts:2370-2402`：失败不阻塞队列） | **同款**（manual 那条以该测试为准，实现时读一遍上游那处代码再定） | `core/src/session/runner/llm.ts:155-156,214-215` |
| A22 | 失败也照记用量 | 每次模型调用结束立刻记账，失败/中断也计 | **同款**（本仓现状已记） | `compaction.ts:628-639` |
| A23 | 事件四件套 | `started`（durable）/ `delta`（**ephemeral**，不落库，流式摘要正文）/ `ended`（durable，带 summary + tokens）/ `failed`（durable，带 error + tokens） | **同款**：在现有 `agent://compact-status` 四相上扩（`running/paused/done/failed` → 增 `delta` 相） | `packages/schema/src/session-event.ts:596-647` |
| A24 | 界面看得见 | TUI 一条压缩消息：running 转轮 / completed 摘要 markdown / failed 错误正文；用量 `· {in} in · {out} out` | **同款形态**：转录里一条压缩消息（进行中 / 摘要全文 / 失败原因 / 用量） | `packages/tui/src/routes/session/index.tsx:2040-2106` |
| A25 | native（供应商侧压缩） | 需要 provider 路由实现 `compact.trigger` / `compact.endpoint` | **不实现**（非目标，见下） | `packages/ai/src/route/client.ts:196-248` |

## 唯一保留 / 新增的我们自己的东西

**自动触发的阈值由用户可调**（上游只有配置文件里的 `compaction.buffer`，界面里没有）。

- 形态：设置 → 上下文管理的一格「**自动压缩阈值**」，**占模型窗口的百分比**，
  三档预置 **25% / 50% / 85%**（`SettingsSegmented`，现役形态，无数字输入框）。
- 公式：`ceiling = min(窗口 × 百分比, 窗口 − 输出预留)`。左半 = 用户意图，右半 = 物理约束（窗口封顶），
  与本仓既有 `context_budget::effective_threshold` 同构（上游是 `窗口 − 15%`，我们把「留多少」交给用户）。
- 缺省：**50%**。
  ⚠ 上游缺省是 85%（留 15%）。我们取 50%，理由：上游主力窗口 ~200K ⇒ 85% ≈ 170K；
  本仓主力模型窗口 **976K** ⇒ 85% 要 830K 才触发，等于长期不触发、白白养一个 976K 的上下文。
  **这一条是我们自己的取值**，回到上游口径只改一颗数（并在 spec 里记一笔）。
- 「压缩阈值（绝对 100K–800K）」那根滑条**退役**：绝对数不跟窗口走，正是「黑盒」的根因（根因表第 2 条）。
  若将来要绝对档，另开一刀做「模式切换 + 绝对值三档」，不在本 spec 内。
- 窗口未知 ⇒ 不自动触发（同上游 A2），靠 overflow 兜底。

## 契约（前后端对账面 · 两边各按这份实现）

### C1 设置项（`settings/context.rs` ↔ `types/settings.ts`）

| 键 | 类型 | 缺省 | 取值 | 说明 |
| --- | --- | --- | --- | --- |
| `autoCompactPct` | number | **50** | **三档之一**：25 / 50 / 85 | 自动压缩阈值 = **占模型窗口的百分比**（后端是校验权威，档外值拒收；同 `summaryBudgetMs` 那条纪律） |
| `compactEnabled` | boolean | true | — | 自动压缩开关（**手动 `/压缩` 不受它影响**，同上游） |
| `usageBar` | boolean | true | — | 用量条显隐 |
| `summaryBudgetMs` | number | 90000 | 三档 30000 / 90000 / 300000 | 摘要调用总时长预算（辅助路径硬上限） |

**退役**：`thresholdTokens`（绝对 100K–800K 滑杆）。老配置文件里那枚键**保留在文件里不报错**
（不删用户数据），但**不再被读**、设置面不显示。

### C2 事件（`agent://compact-status`）

`AgentEvent::CompactStatus { session_id: String, phase: String, reason: Option<String>, text: Option<String> }`

| phase | 含义 | reason | text |
| --- | --- | --- | --- |
| `running` | 这一趟开始压了 | None | None |
| `delta` | 摘要正文流式增量 | None | 增量文本（非空） |
| `failed` | 这一趟没成 | 失败判据名（见 C3） | None |
| `paused` | 熔断停手（连败 3 次） | None | None |
| `done` | 压成了 | None | None |

### C3 失败判据名（后端产出 ⇒ 前端查表，SS24 两侧对账 = **9 条**）

| 判据名 | 含义 | 前端文案 key |
| --- | --- | --- |
| `nothing-to-compact` | 没有可压的内容 | `notice.compactNothingToCompact` |
| `template-mismatch` | 摘要没按模板写（提醒一次仍不合格） | `notice.compactTemplateMismatch` |
| `no-summary` | 模型没吐出摘要 | `notice.compactNoSummary` |
| `output-limit` | 摘要撞输出上限 | `notice.compactOutputLimit` |
| `cannot-shrink` | 再缩就丢最新一轮了 | `notice.compactCannotShrink` |
| `provider-error` | 供应商报错（原文进 `text`，不进 reason） | `notice.compactProviderError` |
| `content-filter` | 被供应商内容过滤 | `notice.compactContentFilter` |
| `interrupted` | 被叫停（用户按停止 / 轮次收束） | `notice.compactInterrupted` |
| `budget` | 超出总时长预算 | `notice.compactBudget` |

**`fused` 不作判据名**：熔断走 `phase = "paused"`（同上游的相位分离；`fused` 在上游那张
「手动让开」族里，前端查表口径与上游不一致 ⇒ 两边都不留这个名字）。

**不产事件的三种跳过**（内部理由，不进 C3）：未到线 / 自动压缩关着 / 刚压完（`due()` 守卫）。

**`interrupted` 那一档的轮次收束形态**（实现时定的，spec 原稿没写）：
判据名仍是 `interrupted`（界面那句话照说「被叫停」），但**轮以 `Interrupted` 收束**、
不返回 Err、**不推进熔断计数**。依据：上游执行层被打断时收束是 `interrupted`
（`execution.ts:52`）而不是 failed，且 `specs/12` 刀① 既有行为如此。
⚠ 「不推进熔断计数」的理由：停过三次不该让压缩**永久**停手——那是人的动作，不是通路坏了。

### C4 请求与装配（后端内部，后端对账用）

- 内核新增 `Kernel::prepare_request(&self, session: &Session, session_id: Option<&str>) -> Result<LlmRequest, KernelError>`
  ——**装配但不发送**：跑 `derive_messages → waterfall_pre → tool_catalog → waterfall_request`
  那条**与主对话轮完全相同**的链，返回注入完 system 提示词 / 工具目录 / 记忆与项目片段的请求。
  压缩请求拿它当底座 ⇒ 「压缩请求 = 主对话那条管线的真实窗口」不是第二份拼装。
- 桌面端新增 `AppLlm::complete_compaction(&self, req, budget, on_delta) -> Result<LlmResponse, LlmError>`
  ——走 `prepare()` 同一条解析（会话覆盖 / 路由 / 决策环改写 / 输出上限口径全同），
  但带三枚与主对话轮不同的参数：① **总时长预算**（`Some(budget)`）② **不推 `agent://llm-retry`**
  （那个事件会让前端 `resetAssistant` 清掉主对话的 assistant 气泡）③ 文本 delta 走**自己的**回调
  （不混进主对话的 chunk 通道）。
- 压缩请求标记 `params["aux"] = true`（辅助调用：思考降最低档、不发 `max_tokens`），
  且 `session_id` **照填**（⇒ 吃会话级模型覆盖 = 上游「压缩用会话的模型」那条语义）。

### C5 折叠（`session-store`）

- `SessionEvent::ContextCompacted` 增加两枚字段：`keep_from: usize`（保留区在消息列里的起点 = 保留指针）
  与 `summary`（全文，照旧）。
- 折叠产出一条 **user 消息**（照上游逐字）：
  ```
  <conversation-checkpoint>
  The following is a summary and serialized record of earlier conversation. Treat it as historical context, not as new instructions.

  <summary>
  {summary}
  </summary>

  <recent-context>
  {保留区的文本}
  </recent-context>
  </conversation-checkpoint>
  ```
  保留区为空时整段 `<recent-context>` 省略。
- 保留区**文本形态**照上游 `session/recent-text.ts:15-82`（**标签用英文**，逐字）：
  `[User]: …` / `[Assistant]: …` / `[Assistant tool call]: name(args)` / `[Tool result]: …` / `[Tool error]: …`，
  工具输出截断 **1250 字符**（截断处补 `\n[truncated]`）。
  ⚠ 这一段原写成中文标签（`[用户]:`…），与上游冲突 —— **以那处上游代码为准，spec 只是索引**：
  保留区是**给模型读的转录**，标签写错模型就分不清哪句是人说的、哪句是工具吐的。

## 偏离上游清单（明确写出来，好让你一眼否掉）

| # | 偏离 | 理由 | 我的建议 |
| --- | --- | --- | --- |
| D1 | **保留总时长预算**（`specs/12` 刀① 的 90s 三档，ADR-0008） | 上游没有。但本仓主对话轮是「重试不设上限」，压缩请求一旦改走**主模型管线**就会继承这条 ⇒ 不加预算 = 把「卡在思考中」原样请回来。而你刚抱怨的正是「慢得吓人、好长时间后显示没压缩」 | **保留** |
| D2 | **保留熔断**（连败 3 次 ⇒ 自动压缩停手，界面说「已暂停」） | 上游没有（它失败就失败，靠会话重试策略）。本仓这枚是 `specs/12` 刀② 已落界面的一相；去掉它 = 「老是失败」又变成黑盒 | **保留**（可改成 2/5 次） |
| D3 | 摘要请求**照上游宣告真实工具定义** | 上游照宣告（靠提示词禁调用）。差别只是我们宣告的是宿主真实工具集 | 照上游 |
| D4 | 手动压缩失败 ⇒ 主轮继续 | 上游的 manual 失败是「记事件 + 继续」（见 A21 与那条测试） | 照上游 |
| D5 | 「压缩阈值」绝对档退役 | 见「唯一保留」节 | 照上面写 |

## 验收标准（必须可测：跑什么命令、看到什么结果）

- [x] AC-1 **门槛口径**：占用 ≥ `窗口 × 用户百分比` 时才自动压；占比不到不压；窗口未知不自动压。
      用例名：`the_auto_trigger_follows_the_user_threshold_against_the_model_window`。
- [x] AC-2 **摘要请求形态**：压缩请求**逐条等于**主对话轮的窗口（真实 system + 真实工具定义 + 真消息），
      且**最后一条是摘要提示词那条 user 消息**；hooks 看到的是**加提示词前**的那份。
      用例名：`the_compaction_request_is_the_real_conversation_plus_the_summary_prompt`。
- [x] AC-3 **提示词逐字**：四个变体（首次 / 更新 / 提醒 / legacy）的正文与上游**逐字节相等**；
      模板里的 10 个标题行**逐字且逐序**钉死（上游 `session-compaction.test.ts:210-224` 同款断言）。
      用例名：`the_compaction_prompt_matches_opencode_byte_for_byte`。
- [x] AC-4 **输出校验**：回复里出现**任一**模板标题行即算通过；一个都没有 ⇒ 追加一次 `NUDGE` 重发；
      再不过 ⇒ 失败且**错误文案逐字**为上游那两句之一。
      用例名：`a_summary_without_the_template_gets_one_nudge_then_fails`。
- [x] AC-5 **checkpoint 逐字**：压缩写回后，下一轮请求里的第一条 user 消息**逐字节**等于
      `<conversation-checkpoint>` + 那句说明 + `<summary>…</summary>` + `<recent-context>…</recent-context>` + `</conversation-checkpoint>`；
      保留区为空时整段 `<recent-context>` 省略。
      用例名：`the_checkpoint_message_matches_opencode_byte_for_byte`。
- [x] AC-6 **保留区**：只存 `keepID` 指针；读历史时重建出的文本与保留段逐字相等；
      工具输出截断到 **1250 字符**；切点落在 user 消息边界（不把一次往返劈开）。
      用例名：`the_kept_recent_text_is_rebuilt_from_the_pointer`。
- [x] AC-7 **三种触发**：auto（到线才压）/ manual（`/压缩`，自动压缩关掉也照样能压）/ overflow（provider 拒收「太长」后压一次、重试一次）。
      用例名：`the_three_triggers_behave_like_opencode`。
- [x] AC-8 **失败处置**：auto 失败 ⇒ 整步失败；overflow 失败 ⇒ 原始错误照常冒泡；manual 失败 ⇒ 主轮继续。
      用例名：`a_failed_compaction_does_not_break_the_queue`。
- [x] AC-9 **收缩矩阵**：装不下时依次尝试 原样 → 去保留区 → 压文本丢最老 → 70%/50%/35% 三档；
      用尽 ⇒ 失败且错误文案逐字为上游那句。
      用例名：`the_compaction_shrinks_in_opencode_steps`。
- [x] AC-10 **失败也记账**：失败/中断的压缩照样把已发生的 token 记进会话用量。
      用例名：`a_failed_compaction_is_still_billed`。
- [x] AC-11 **界面**：转录里出现一条压缩消息（进行中转轮 / 完成后摘要全文 / 失败原因 / 用量读数）；
      摘要正文**流式**出现（delta 相）。
      用例名：`the_transcript_shows_the_compaction_summary_while_it_runs`。
- [x] AC-12 **门禁**：`make check` 全绿；本刀每条新判据做**变异**必须报红；
      `check-channels` 认得新增的通道/事件；被判据迁移影响的同源档（SS19a/SS20b/SS20c/SS21b/SS22a-c/SS24/SS25）
      **要么改判据要么退役**，不许留一档读着旧形状的判据。
- [x] AC-13 **手动那一趟落盘**（真机核对时抓到的缺陷，已修）：手动 `/压缩` 追加的
      `LlmUsage` + `ContextCompacted` 必须进会话日志 —— 漏了的话压完看着一切正常，
      **重开应用压缩连同摘要一起消失**（那轮 LLM 的钱花了、窗口又回到压前大小）。
      用例名：`the_manual_compaction_is_written_to_the_session_log`。
      落盘口 = `compact/deep.rs::run_manual_and_persist`（「跑一趟」与「把它追加的
      事件写进日志」合成一个调用，漏不了）；自动那条仍由 runner 在 step 边界落
      （那段有 runner 自己的落盘游标），溢出那条由步末的落盘口一起写。

## 刀⑤ 压缩详情的形态改造（用户 2026-10-22 裁定）

**用户原话**：「是真实的有bug，点击展开后下面又跟着一个小卡片，点击又展开一轮。所以这个机制好不好改成不要这几个卡片。直接在压缩的那个发丝线上触发展开，展开的不是以卡片的形式显示。就跟模型消息那样向md那样显示就可以了，同时还要配折叠回发丝线状态的按钮。让压缩的内容可展开可折叠。同时这个压缩内容需要在最顶层。不受那个L0折叠层管理。属于永不折叠状态。跟模型最后汇报总结消息在一个层级的。」

### 要改的到底是什么（改前 → 改后）

| # | 改前 | 改后 |
| --- | --- | --- |
| 1 | 压成的结果是一条**居中窄卡片**（`SystemNotice` 的非 hairline 形态） | 压缩这一族**只有一根发丝线**（hairline），结果线也是 |
| 2 | 「查看详情」往转录里 **push 一张新卡**（`pushRawSystem`）⇒ 点一次多一张、点几次堆几张 | 点同一枚钮**切换那根线自己的状态**（`expanded`）⇒ 转录里**不多出任何条目** |
| 3 | 详情是**纯文本预览**（200 字） | 详情是**模型正文形态**（md 渲染，`.md` 那一套：标题是 `h2`、列表是 `li`） |
| 4 | 压缩线落在**轮容器（第 0 层）内**时会被 L0 折叠管住 | 压缩线与展开体**摘到轮容器外**（顶层、永不折叠，与最终报告同级） |
| 5 | 压成时**收走在途线 + 另起一条** | **就地改写**同一根线（发起 → 收束是同一条线），展开态跟着留下来 |
| 6 | 重放时详情只有 **200 字预览**（截断的 md 是断掉的一半） | `context_compacted` 事件带**摘要全文**（会话日志里那一份，实时与重放同源） |

### 形态口径的落点

- **标准先行**：《Front-end Design Guidelines》新增 §二⑲「压缩线与摘要展开体」（v2.5）—— 这一刀**先改标准再写码**（§零.5 特例申请制的规矩）。
- **tag 单一出处**：`desktop/src/lib/chat/compact-line.ts`（`manual` / `auto` / `result` 三个 tag + `isCompactItem`）。生产面（chat store）与渲染面（`TranscriptFlow`）都从那儿读。
- **渲染落点唯一**：`desktop/src/components/chat/SystemRow.vue`（线 + 展开体一起渲染）；`TranscriptFlow.vue` 只做两件事：把压缩线**摘到轮容器外** + 派发动作。
- **正文唯一出处**：`SystemItem.body`（旧的 `state.noticeDetails` 映射退役 —— 它与 `items` 平行存同一份正文，删条目必漏一处）。

### 验收标准

- [x] AC-14 **一根发丝线**：压缩这一族（发起 / 收束 / 没成 / 已暂停）在转录里都是 hairline 形态；
      **居中窄卡片那一档不再出现**。
      用例名：`压缩这一族是一根发丝线，不是居中窄卡片`（`tests/compact-summary-body.spec.ts`）。
- [x] AC-15 **就地展开 / 收起**：点线上那枚钮 ⇒ 展开体出现在**那根线下面**（md 渲染），
      再点 ⇒ 收起回发丝线状态；**全过程转录里不多出任何条目**。
      用例名：`点「查看详情」就地展开：md 正文 + 转录里不多出任何条目`、
      `再点一下收起回发丝线状态（同一枚钮切换两态）`。
- [x] AC-16 **顶层 · 永不折叠**：压缩线不是轮容器的后代；轮容器收起时它与它的展开体**仍在**。
      用例名：`压缩线在轮容器之外：轮容器收起时它仍在，展开体也还在`。
- [x] AC-17 **一根线不两根**：压成事件**就地改写**在途那根线（不另起一条），展开态跟着留下来。
      用例名：`压成事件 ⇒ 就地改写在途那根线（同一趟压缩界面上只有一根线，展开态跟着留）`
      （`tests/chat-projection.spec.ts`）+ `压成时那根线不收走也不多说一句：改成「已压缩」那句中间态`
      （`tests/compact-command.spec.ts`）。
- [x] AC-18 **全文**：重放（重开应用）后展开体是**摘要全文**（不是 200 字截断）。
      判据 = 事件带 `summary` 全文（`runner/projection.rs`）⇒ 前端 `body` 直接用那份。
- [x] AC-19 **门禁**：`make check` 全绿；本刀每条新判据做**变异**必须报红（逐条记在提交正文）。

### 真机核对（2026-10-22，`pnpm tauri dev`）

会话「询问项目协助」（`desktop-1792304492436.jsonl`）——**零设置改动、零新 LLM 调用**（靠上一刀
留在会话日志里的那次压缩事件做重放核对）：

| 核对项 | 读数 |
| --- | --- |
| 底态形态 | 压成那条是**发丝线**（居中窄卡片那一档不再出现），线上「查看详情 / 撤销」 |
| 点「查看详情」 | **就地展开**在那根线下面；正文按**模型消息那样**渲染成 md（`## Objective` / `## Requirements` / `## Decisions` / `## Work State` 等 **h2 标题** + 行内码 + 列表），是**全文**（不是 200 字预览） |
| 展开后有没有多出卡片 | **没有**——用户 2026-10-22 报的「点一次多一张小卡片、再点又展开一轮」不复现（转录条目数前后不变） |
| 那枚钮 | 展开态改说「**收起**」，收起态改说「**查看详情**」（同一枚钮切两态，不是两枚钮） |
| 点「收起」 | 回到发丝线态（正文撤掉，线还在原位） |
| 顶层 · 永不折叠 | 收起 L0 轮容器（点计时行）⇒ 压缩线**仍在**（与最终报告同级，不被折叠层带走） |

> 真机核对的坐标坑（记下来省下一轮）：窗口在 (78,78)，**屏幕坐标 = 图像坐标 + (78,78)**；
> 计时行会随转录滚动**上下漂 30px 左右** ⇒ 点之前先抓一张图按图找，别拿上一张的坐标直接点。

### 这一刀**没**做的（挂账见下）

- **落盘口收成一处 + 失败轮次步内事件落盘**（specs/13 挂账 ⑦）：要重构 runner 的落盘游标，
  改错就是日志重影或缺口 ⇒ **独立一刀**（刀⑥），不与形态刀混做。
- **权威层文档对齐**（挂账 ③）：`docs/design/authority/DESIGN.md` §⑨ 与 `Context and Memory.md`
  §3.2.2 的过期描述 —— 改它们要先向人说明并等确认（[LOCK]）。

## 刀⑥ 落盘口收成一处 + 失败轮次的步内事件落盘（挂账 ⑦）

**为什么单独一刀**：改错就是**日志重影或缺口**——两种都不报错、不变红、界面照常，
只有重开应用才看得见（那一轮 LLM 的钱花了、内容没了）。这类改动不能与形态刀混做。

### 要改的到底是什么（改前 → 改后）

| # | 改前 | 改后 |
| --- | --- | --- |
| 1 | 落盘游标**每个调用点自己算**（runner 的 `log_start` / `before` / `compact_start` / `len-1`，压缩层的 `from`，heal 的 `seq`） | 游标**住进会话**：`Session::persisted` = 已落盘到第几条事件，**单调前进**、调用方不传 `from` |
| 2 | 写盘有**多个实现**，各自算各自的起点 | **唯一落盘口** `session_log::persist_pending(dir, id, &mut session)`：写 `[persisted..]`，**写成功才推进游标** |
| 3 | 压缩这一族**两处各落各的**（自动那条在 runner 的 step 边界、手动那条在压缩层的 `run_manual_and_persist`） | 压缩三条通路（自动 / 手动 / 溢出）**都在压缩层自己落盘**，走同一个口（压缩器**认识日志目录**） |
| 4 | **失败的轮次**里那一步追加的事件不进日志（`finish_turn` 只落 `TurnEnd`） | 轮以失败收束时 **pending 段整体落盘**（含压缩那两条：`LlmUsage` + `ContextCompacted`） |

**游标为什么住进会话**：两个游标各写各的 ⇒ 谁也不知道磁盘上到哪儿了 ⇒ 重影或缺口。
住进会话之后，"已落盘到第几条"是**会话自己的一个事实**，谁落盘都从那儿接着写。

**失败语义**：写失败**不推进游标** ⇒ 下一趟重写这一段 ⇒ **至多一次、绝不缺口**
（改前是「各算各的 from」⇒ 失败那一段直接被跳过 = 静默缺口）。

**重影为什么从结构上消失**：压缩层落完盘，runner 步末再落一次 ⇒ 第二次是**空段**
（游标已到末尾）⇒ 同一段永远不会被写两遍。（改前两处各带各的 `from`，才会重影。）

### 验收标准

- [x] AC-20 **唯一落盘口 + 游标单调**：写盘只有一个实现（`persist_pending`），调用方**不传起点**；
      同一段连落两次 ⇒ 磁盘上**不出现重复行**（幂等）。
      用例名：`the_persist_path_is_the_only_one_and_twice_is_not_a_double_write`。
- [x] AC-22 **失败轮次的步内事件进日志**：溢出压完仍失败的那一轮，磁盘上要同时看得到
      **压缩那两条**（`LlmUsage` + `ContextCompacted`）与**失败收束**（`TurnEnd{Failed}`），
      且与内存逐条相等。
      用例名：`a_failed_turn_keeps_the_step_events_in_the_log`。
- [x] AC-23 **重开应用不重写老历史**：恢复路径（`load_session`）把游标落定在事件数上 ⇒
      重开一次再落新的一条，盘上是「原来那几条 + 新的一条」，老历史**不被重写一遍**。
      用例名：`a_restored_session_does_not_rewrite_its_history`。
- [x] AC-24 **门禁**：`make check` 全绿；本刀每条新判据做**变异**必须报红（逐条记在提交正文）。

### 这一刀**没**做的

- **不动 `Session` 的投影游标**（`cursor` = 可见前缀）：那是**回放位置**，与「已落盘到第几条」是两件事，
  两者同名会混（本次新增的那格叫 `persisted`，注释里写死区别）。
- **不把落盘做成事务**：写失败仍是「告警不阻断聊天」，只是**不推进游标**（下一趟补齐）。

## 非目标

- **不实现 provider 原生压缩（native）**：需要供应商侧的 compact 端点/触发器，本仓没有对等能力（上游依赖见 `packages/ai/src/route/client.ts:196-248`）。
- **不改主对话轮的「重试不设上限」裁定**（用户 2026-08 原话；ADR-0008 明确不动）。
- **不删用户磁盘上的任何会话数据**；旧会话里已落盘的老格式摘要**不迁移**（它就是一段文本，仍能被下一次压缩吃进去）。
- **不做「压缩模式」的插件化**（上游的 `session.compaction` hook 留给插件系统那一层，不在本刀）。
- **不动记忆面**（S3 刀③b 已整族退役）。

## 判据迁移清单（本刀会让既有同源档失效，逐条处置）

| 判据 | 现状 | 处置 |
| --- | --- | --- |
| SS19a | 辅助补全直呼客户端（`llm/aux_client.rs`）唯一 | **退役成墓碑**（aux 通路整条删掉 ⇒ 那三枚名字在生产面恒 0） |
| SS19b2 | `AppLlmSummarizer::new(` 只在组装点 | **退役成墓碑**（模块路径 `aux_client` 恒 0；与 SS19a 分两档是防「换了文件名换个类型名」的改法） |
| SS20a | 折叠消息开头那串标记 `[earlier conversation summarized]` 唯一 | **改判据**：真源换成 `session-store` 的 `CHECKPOINT_TAG`（`<conversation-checkpoint>`）+ `CHECKPOINT_NOTICE`，旧标记进墓碑（恒 0） |
| SS20b | `prompts/compact/` 四份稿子各被 `include_str!` 一次 | **改判据**：稿子换成上游那**九份**（template / rules / preamble / epilogue / first / update-head / update-legacy / update-tail / nudge），各只 include 一次；扫描面从压缩子树换成**宿主生产面**（第二份 include 落在哪个文件都红） |
| SS21a | `STRATEGY` 常量唯一 | 保留（strategy 仍是 `"compact"`） |
| SS21b | 压缩层**不许自己拿尺**（`effective_threshold(` 在压缩面恒 0） | **反过来改判据**：上游的门槛算式**就在压缩层里**（`calculateCeiling`）⇒ 危险从"压缩面自己算"变成"两个地方各算一遍"。新判据盯「门槛算式只有一个出处」= `fn ceiling(` 恰好一处 + 「窗口×百分比」只乘一次 + `effective_threshold(` 的落点集合只有 `context_budget.rs`（定义）与 `compact/plan.rs`（唯一调用方）+ 压缩面不许自己查窗口 |
| SS21c | `Compactor::new(` / `cp.run(` 各只一处 | 保留（结构不变） |
| SS22a/b | 熔断线唯一比较 / 熔断表唯一 new | 保留（D2 保留熔断） |
| SS22c | 写侧读侧分工（`failures.of\|record\|clear` 都在 `compact/deep.rs`） | **改判据**（跟着 400 行纪律的拆分走）：`of\|record` 在 `compact/deep.rs`、`clear` 在 `compact/deep/settle.rs`、`fused` 仍零处 |
| SS24 / SS25 | 判据名两侧对账（11 / 9） | **改判据**：后端取值域改成扫 `compact/outcome.rs` 的 `REASONS: [&str; 9]` 那一行（字面量只许出现在表里），前端仍是 `COMPACT_SKIP_KEYS` 9 条 ⇒ 两侧逐条相等 |
| **SS31（新增）** | —— | 压缩请求的装配**只有一个出处**：`prepare_request(` 与 `complete_compaction(` 的落点集合 = 压缩面是唯一调用方（第二份拼装的机械面） |
| **SS32（新增）** | —— | 自动压缩阈值的**缺省与三档预置两侧对账**（后端校验权威 ↔ 界面候选清单） |

## 计划（五刀）

- **刀① 核心算法与请求形态**（后端）：删 `llm/aux_client.rs` + `prompts/compact/*.md` + `compact/prompt.rs` 的自研部分；
  新 `compact/`（按上游切分：`prompt` / `plan` / `deliver` / `recent_text` / `outcome`）；
  摘要请求改走主对话装配；门槛 = `窗口 × 用户百分比`。产出 AC-1/2/3/4。
- **刀② 折叠与 checkpoint**（`session-store` + 事件）：`ContextCompacted` 带保留指针；
  折叠产出**一条** checkpoint user 消息（逐字标签）；保留区读时重建。产出 AC-5/6。
- **刀③ 三种触发与失败处置**（接线面）：auto / overflow / manual 三个入口 + 失败矩阵 + 收缩矩阵 + 记账。产出 AC-7/8/9/10。
- **刀④ 设置面与界面 + 收口**：阈值三档（替掉绝对滑条）、压缩消息形态（转录里一条消息 + delta 流式）、
  中英文案成对、判据迁移、`make check` + `make selftest`、ADR、roadmap、真机。产出 AC-11/12。
- **刀⑤ 压缩详情的形态改造**（用户 2026-10-22 裁定）：取消卡片 ⇒ 发丝线上就地展开/折叠、
  展开体按模型正文那样渲染成 md、压缩线与展开体**摘到轮容器外**（顶层 · 永不折叠）、
  压成时**就地改写**同一根线、事件带摘要**全文**。产出 AC-14…AC-19。
- **刀⑥ 落盘口收成一处 + 失败轮次步内事件落盘**（挂账 ⑦，✅ 已做）：落盘游标住进会话
  （`Session::persisted`）⇒ **唯一落盘口** `session_log::persist_pending`；压缩三条通路
  **自己落盘**（压缩器认日志目录）；轮收束落**整段 pending**（失败轮次也落）。
  产出 AC-20…AC-23。
- **刀⑦（待做）三枚判据的启发式归类收敛**（挂账 ①）与 **specs/12 两笔欠账**（挂账 ④）。
- **刀⑧（待做）权威层文档专项对齐**（挂账 ③）：`docs/design/authority/` 的规范语义在 [LOCK] 里，
  改前要向人说明并等确认。

## 任务

- [x] 刀①：读上游 `core/src/session/compaction.ts` 全文 + `model-request.ts` + `to-llm-message.ts` + `history.ts` + `recent-text.ts`（逐字来源）
- [x] 刀①：删 `llm/aux_client.rs`（连带 `llm/tests/aux_client.rs`）+ `prompts/compact/*.md` 四份稿子
- [x] 刀①：新 `compact/` 模块（prompt 逐字 / plan / deliver / recent_text / outcome）
- [x] 刀①：摘要请求改走主对话装配（真实 system + 工具定义 + 会话模型 + 同一套输出上限）
- [x] 刀①：门槛 = 窗口 × 用户百分比；窗口未知不自动压
- [x] 刀②：`ContextCompacted` 带保留指针；折叠产出 checkpoint user 消息（逐字）
- [x] 刀②：保留区读时重建（`\n\n` 连接，工具输出 1250 字符截断，切点在 user 边界）
- [x] 刀③：auto / overflow / manual 三个入口 + 失败矩阵 + 收缩矩阵（70/50/35）+ 失败记账
- [x] 刀④：设置面「自动压缩阈值」三档（替掉绝对滑条）+ 文案成对
- [x] 刀④：转录里的压缩消息（进行中 / 摘要 / 失败 / 用量）+ delta 流式
- [x] 判据迁移（SS19a / SS19b2 退役成墓碑 · SS20a / SS20b / SS21b / SS22c / SS24 / SS25 改判据 · **新增 SS31 / SS32**）
- [x] 补测试（AC-1…AC-12）+ 变异自证（后端四条 + 前端两条 + 闸侧逐档）
- [x] `make check` 全绿 + `make selftest` 变异逐条报红
- [x] ADR（长期取舍：为什么整块换掉、我们保留的两处偏离）+ roadmap 勾选
- [x] 刀⑤：标准先行（Front-end Design Guidelines §二⑲ + v2.5）
- [x] 刀⑤：压缩线族 tag 单一出处（`lib/chat/compact-line.ts`）+ 正文住进 `SystemItem.body`（退役 `noticeDetails`）
- [x] 刀⑤：压成事件**就地改写**在途那根线（不另起一条）+ 「已压缩」中间态
- [x] 刀⑤：新 `SystemRow.vue`（线 + md 展开体）+ 压缩线**摘到轮容器外** + 「查看详情 ⇄ 收起」同一枚钮
- [x] 刀⑤：`context_compacted` 事件带摘要**全文**（重放不再是 200 字截断）
- [x] 刀⑤：判据（`tests/compact-summary-body.spec.ts` 四条 + 投影/命令两条改写）+ 变异自证报红
- [x] 刀⑥：落盘游标住进会话（`Session::persisted` + `mark_persisted`）+ 唯一落盘口
      `session_log::persist_pending`（调用方不再传 `from`）
- [x] 刀⑥：压缩三条通路**自己落盘**（压缩器认日志目录）+ `run_manual_and_persist` 退役
- [x] 刀⑥：轮收束落**整段 pending**（失败轮次的步内事件不再丢）
- [x] 刀⑥：判据（`tests/compaction_opencode/persist_uniqueness.rs` 四条）+ 变异五条逐条报红

## 影响范围

| 文件 / 目录 | 改动 |
| --- | --- |
| `desktop/src-tauri/src/compact/**` | **重写**（prompt / plan / deliver / recent_text / outcome） |
| `desktop/src-tauri/src/llm/aux_client.rs` | **删**（含 `llm/tests/aux_client.rs`） |
| `desktop/src-tauri/src/prompts/compact/*.md` | **删四份、换新资产**（上游逐字文本） |
| `desktop/src-tauri/src/context_budget.rs` | 门槛算式迁到压缩层（SS21b 退役）；占用读数继续供它 |
| `desktop/src-tauri/src/settings/context.rs` | 阈值改「百分比三档」；删绝对滑杆那枚 |
| `desktop/src-tauri/src/runner/mod.rs` | 三个触发入口 + 失败处置 + 收缩接线 |
| `desktop/src-tauri/src/commands/compact.rs` | 手动通路按上游语义改（auto 关掉也可用） |
| `desktop/src-tauri/src/events.rs` | 压缩事件加 `delta` 相 / 换失败分类 |
| `crates/session-store/src/session/{project,event}.rs` | 折叠改成 checkpoint 消息 + 保留指针 |
| `desktop/src/stores/chat/{projection,notices}.ts` | 压缩消息形态（一条消息 + 流式） |
| `desktop/src/components/settings/ContextSection.vue` | 阈值那格换成三档百分比 |
| `desktop/src/locales/{zh-CN,en}/` | 文案成对 |
| `scripts/check-single-source.mjs` | 判据迁移（SS19a/19b/20b/20c/21b/24/25） |
| `docs/adr/0009-*.md` | 长期取舍落纸 |
| **刀⑤**：`docs/design/authority/Front-end Design Guidelines.md` | **§二⑲ 新增「压缩线与摘要展开体」+ 版本行 v2.5**（先改标准再写码） |
| **刀⑤**：`desktop/src/lib/chat/compact-line.ts` | 压缩线族三个 tag + `isCompactItem`（**唯一出处**） |
| **刀⑤**：`desktop/src/components/chat/SystemRow.vue` | 线 + md 展开体的**唯一渲染落点**（新文件） |
| **刀⑤**：`desktop/src/components/chat/TranscriptFlow.vue` | 压缩线**摘到轮容器外** + 「查看详情 ⇄ 收起」同一枚钮 |
| **刀⑤**：`desktop/src/components/ChatTranscript.vue` | 详情动作改为**切换那根线自己的状态**（不再 push 新卡） |
| **刀⑤**：`desktop/src/stores/chat/{types,state,session,notices,projection,send,index}.ts` | `body`/`expanded` 两格 + 退役 `noticeDetails` + 就地改写收束线 |
| **刀⑤**：`desktop/src-tauri/src/{events.rs,runner/projection.rs,export.rs}` | `context_compacted` 事件带摘要**全文**（`summary_preview` → `summary`） |
| **刀⑤**：`desktop/tests/compact-summary-body.spec.ts` | 形态判据四条（新文件） |

## 风险与回滚

- **风险 ①**：上游是 Effect/TypeScript 架构，本仓是 Rust + 内核分层 ⇒ 逐字复刻的是**语义与文本**，不是结构；
  判据必须按「行为等价」写，不能按「代码同形」写。
- **风险 ②**：删 aux 通路 = 摘掉 `specs/12` 刀① 的一半（预算挂在 aux 上）⇒ 预算要迁到新路径上（D1）。
- **风险 ③**：`session-store` 的折叠形状变了 ⇒ 旧会话重放要仍能读（旧摘要就是一段文本）。
- **风险 ④**：阈值改成百分比后，用户原来那根绝对滑杆的语义没了（100K ⇒ 976K 窗口下 = 10%），
  默认档位若选得太高会「长期不触发」⇒ 缺省取 50%，并在设置页写清「占模型窗口的百分之多少」。
- **回滚**：四刀各自独立成提交 ⇒ 任一刀出问题可单独回滚；设置项可当场调回。

## 遗留 / 挂账（这一刀**没**解决、也不在这一刀顺手修的）

| # | 事项 | 为什么不在这一刀修 |
| --- | --- | --- |
| ① | `output-limit` / `content-filter` / `payload-too-large` 三枚判据**靠错误文本匹配**（本仓传输层不暴露 `finish_reason`，内容过滤只能按供应商原文关键词认，payload 太大只能按 HTTP 413 那一族认） | 判据名表本身是上游那 9 条（对齐没打折），但**归类依据是启发式** ⇒ 会随供应商措辞漂。要收敛得动 `llm/retry.rs` 的接入面，那不是这一刀的文件 |
| ② | 摘要预算那对预置清单（`SUMMARY_BUDGET_PRESETS_MS` ↔ `lib/summary-budget.ts`）**没有对账闸** | 这一刀新加的 SS32 只盯自动压缩阈值那一对。给旧的那对补闸 = 修无关代码（AGENTS §5「发现问题就登记，别在同一刀里顺手修」） |
| ③ | `docs/design/authority/` 里**现役机制描述已过期**（`DESIGN.md` §⑨ 与 `Context and Memory.md` §3.2.2 仍写 v2.137 的「整理阈值 = 绝对 token」与「桌面压缩链整面退役」；§4.2 的 `Session` 结构示意里也没有刀⑥ 新增的那格落盘游标） | `DESIGN.md` 的规范语义在 AGENTS §5 的 [LOCK] 里，改它要**先向人说明并等确认**；且 specs/09/11/12 三批压缩改造同样没动它 ⇒ 权威层的这一段欠一次专项对齐。**刀⑤ 只动了 `Front-end Design Guidelines`（新增 §二⑲）** —— 那是用户 2026-10-22 明确裁定的新形态，按该标准 §零.5「先改标准再写码」的规矩必须同刀落地；**刀⑥ 也没动它**（§4.2 那段是**示意**不是字段清单，代码多一格游标不与它的规范语义冲突）；`DESIGN.md` 那两处仍在 [LOCK] 下待专项对齐（刀⑧） |
| ④ | `specs/12` 那两笔挂账缺陷（`llm/wire/stream.rs` 推理优先 return 丢正文 / `stores/chat/projection.ts` `resetAssistant` 不清 thinking 行） | 已在 specs/12 登记，**另开一刀** |
| ⑤ | 压缩用的 `AppLlm` 是**第二个实例**（同一批 settings / overrides / catalog / assets / spill / stop 句柄；差别：不推重试通知、不记拒收读数、独立 diagnoser） | `state/mod.rs` 的 `Init` 不在本刀的文件面内；这一刀先保行为对齐，实例化那一层以后再收 |
| ⑥ | 旧会话重放（旧日志的 `ContextCompacted` 没有 `keep_from` ⇒ 按 `kept_messages` 从尾部回退） | 只在 `session-store` 的单测里证过，**未做真机旧会话回归** |
| ⑦ | ~~**落盘口仍是两处**（自动那条在 `runner/mod.rs` 的 step 边界、手动那条在 `compact/deep.rs::run_manual_and_persist`）~~ + ~~**失败轮次的步内事件不进日志**（`finish_turn` 只落 `TurnEnd`）~~ | **刀⑥ 已结清（2026-10-22）**：落盘游标住进会话（`Session::persisted`）⇒ **唯一落盘口** `session_log::persist_pending`（调用方不再传 `from`）；压缩三条通路（自动 / 手动 / 溢出）**自己落盘**（压缩器认日志目录，`run_manual_and_persist` 退役）；轮收束落**整段 pending** ⇒ 失败轮次的步内事件（溢出压完仍失败那一轮）也在盘上。判据 = `tests/compaction_opencode/persist_uniqueness.rs` 三条（AC-20…AC-22），变异四条逐条报红。**余下的**：写盘失败仍是「告警不阻断聊天」（只是不推进游标、下一趟补齐）—— 不是缺陷，是刻意的取舍 |
| ⑧ | **判据环境差异**：本仓前端测试环境是 `happy-dom`，它的 `document.createNodeIterator` 首个 `nextNode()` 返回**根节点**（违反 DOM 规范）⇒ DOMPurify 把 body 当待消毒节点 ⇒ **输入里的第一个元素连标签一起被吃掉**（浏览器 / WebView2 按规范走，没这回事） | 不是本仓代码的缺陷，是**写判据时的坑**：断言押在"第一个元素"上，红的是环境不是代码。已写进 `desktop/tests/compact-summary-body.spec.ts` 的文件头注记（摘要以一段话开头，标题与列表项都不是第一个元素 ⇒ 断言照常成立）。**不另开一刀**；将来别的 md 渲染判据撞上同一条，先读那条注记 |
