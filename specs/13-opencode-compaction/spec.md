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

## 偏离上游清单（明确写出来，好让你一眼否掉）

| # | 偏离 | 理由 | 我的建议 |
| --- | --- | --- | --- |
| D1 | **保留总时长预算**（`specs/12` 刀① 的 90s 三档，ADR-0008） | 上游没有。但本仓主对话轮是「重试不设上限」，压缩请求一旦改走**主模型管线**就会继承这条 ⇒ 不加预算 = 把「卡在思考中」原样请回来。而你刚抱怨的正是「慢得吓人、好长时间后显示没压缩」 | **保留** |
| D2 | **保留熔断**（连败 3 次 ⇒ 自动压缩停手，界面说「已暂停」） | 上游没有（它失败就失败，靠会话重试策略）。本仓这枚是 `specs/12` 刀② 已落界面的一相；去掉它 = 「老是失败」又变成黑盒 | **保留**（可改成 2/5 次） |
| D3 | 摘要请求**照上游宣告真实工具定义** | 上游照宣告（靠提示词禁调用）。差别只是我们宣告的是宿主真实工具集 | 照上游 |
| D4 | 手动压缩失败 ⇒ 主轮继续 | 上游的 manual 失败是「记事件 + 继续」（见 A21 与那条测试） | 照上游 |
| D5 | 「压缩阈值」绝对档退役 | 见「唯一保留」节 | 照上面写 |

## 验收标准（必须可测：跑什么命令、看到什么结果）

- [ ] AC-1 **门槛口径**：占用 ≥ `窗口 × 用户百分比` 时才自动压；占比不到不压；窗口未知不自动压。
      用例名：`the_auto_trigger_follows_the_user_threshold_against_the_model_window`。
- [ ] AC-2 **摘要请求形态**：压缩请求**逐条等于**主对话轮的窗口（真实 system + 真实工具定义 + 真消息），
      且**最后一条是摘要提示词那条 user 消息**；hooks 看到的是**加提示词前**的那份。
      用例名：`the_compaction_request_is_the_real_conversation_plus_the_summary_prompt`。
- [ ] AC-3 **提示词逐字**：四个变体（首次 / 更新 / 提醒 / legacy）的正文与上游**逐字节相等**；
      模板里的 10 个标题行**逐字且逐序**钉死（上游 `session-compaction.test.ts:210-224` 同款断言）。
      用例名：`the_compaction_prompt_matches_opencode_byte_for_byte`。
- [ ] AC-4 **输出校验**：回复里出现**任一**模板标题行即算通过；一个都没有 ⇒ 追加一次 `NUDGE` 重发；
      再不过 ⇒ 失败且**错误文案逐字**为上游那两句之一。
      用例名：`a_summary_without_the_template_gets_one_nudge_then_fails`。
- [ ] AC-5 **checkpoint 逐字**：压缩写回后，下一轮请求里的第一条 user 消息**逐字节**等于
      `<conversation-checkpoint>` + 那句说明 + `<summary>…</summary>` + `<recent-context>…</recent-context>` + `</conversation-checkpoint>`；
      保留区为空时整段 `<recent-context>` 省略。
      用例名：`the_checkpoint_message_matches_opencode_byte_for_byte`。
- [ ] AC-6 **保留区**：只存 `keepID` 指针；读历史时重建出的文本与保留段逐字相等；
      工具输出截断到 **1250 字符**；切点落在 user 消息边界（不把一次往返劈开）。
      用例名：`the_kept_recent_text_is_rebuilt_from_the_pointer`。
- [ ] AC-7 **三种触发**：auto（到线才压）/ manual（`/压缩`，自动压缩关掉也照样能压）/ overflow（provider 拒收「太长」后压一次、重试一次）。
      用例名：`the_three_triggers_behave_like_opencode`。
- [ ] AC-8 **失败处置**：auto 失败 ⇒ 整步失败；overflow 失败 ⇒ 原始错误照常冒泡；manual 失败 ⇒ 主轮继续。
      用例名：`a_failed_compaction_does_not_break_the_queue`。
- [ ] AC-9 **收缩矩阵**：装不下时依次尝试 原样 → 去保留区 → 压文本丢最老 → 70%/50%/35% 三档；
      用尽 ⇒ 失败且错误文案逐字为上游那句。
      用例名：`the_compaction_shrinks_in_opencode_steps`。
- [ ] AC-10 **失败也记账**：失败/中断的压缩照样把已发生的 token 记进会话用量。
      用例名：`a_failed_compaction_is_still_billed`。
- [ ] AC-11 **界面**：转录里出现一条压缩消息（进行中转轮 / 完成后摘要全文 / 失败原因 / 用量读数）；
      摘要正文**流式**出现（delta 相）。
      用例名：`the_transcript_shows_the_compaction_summary_while_it_runs`。
- [ ] AC-12 **门禁**：`make check` 全绿；本刀每条新判据做**变异**必须报红；
      `check-channels` 认得新增的通道/事件；被判据迁移影响的同源档（SS19a/SS20b/SS20c/SS21b/SS22a-c/SS24/SS25）
      **要么改判据要么退役**，不许留一档读着旧形状的判据。

## 非目标

- **不实现 provider 原生压缩（native）**：需要供应商侧的 compact 端点/触发器，本仓没有对等能力（上游依赖见 `packages/ai/src/route/client.ts:196-248`）。
- **不改主对话轮的「重试不设上限」裁定**（用户 2026-08 原话；ADR-0008 明确不动）。
- **不删用户磁盘上的任何会话数据**；旧会话里已落盘的老格式摘要**不迁移**（它就是一段文本，仍能被下一次压缩吃进去）。
- **不做「压缩模式」的插件化**（上游的 `session.compaction` hook 留给插件系统那一层，不在本刀）。
- **不动记忆面**（S3 刀③b 已整族退役）。

## 判据迁移清单（本刀会让既有同源档失效，逐条处置）

| 判据 | 现状 | 处置 |
| --- | --- | --- |
| SS19a | 辅助补全直呼客户端（`llm/aux_client.rs`）唯一 | **退役**（aux 通路整条删掉） |
| SS19b | `AppLlmSummarizer::new(` 只在组装点 | **退役** |
| SS20a | 压缩层写进日志的 `strategy` 字面量唯一 | 保留（strategy 仍是 `"compact"`） |
| SS20b | `prompts/compact/` 四份稿子各被 `include_str!` 一次 | **改判据**（稿子换了内容与份数，形状仍是「各只 include 一次」） |
| SS20c | 七节节名唯一出处 | **改判据**（节名改成上游那 10 个英文标题行） |
| SS21a | `STRATEGY` 常量唯一 | 保留 |
| SS21b | 压缩层**不许自己拿尺**（`effective_threshold(` 在压缩面恒 0） | **退役/改**：上游的门槛算式**就在压缩层里**（`calculateCeiling`）⇒ 本仓也要在压缩层算 `窗口 × 百分比` |
| SS21c | `Compactor::new(` / `cp.run(` 各只一处 | 保留（结构不变） |
| SS22a/b/c | 熔断线唯一比较 / 熔断表唯一 new / 写侧读侧分工 | 保留（D2 保留熔断） |
| SS24 / SS25 | 判据名两侧对账（11 / 9） | **改判据**：失败分类换成上游那套 type（`compaction.unavailable` / `compaction.failed` / `compaction.interrupted` / `provider.*`），本地那张「判据名→文案」表要跟着换 |

## 计划（四刀）

- **刀① 核心算法与请求形态**（后端）：删 `llm/aux_client.rs` + `prompts/compact/*.md` + `compact/prompt.rs` 的自研部分；
  新 `compact/`（按上游切分：`prompt` / `plan` / `deliver` / `recent_text` / `outcome`）；
  摘要请求改走主对话装配；门槛 = `窗口 × 用户百分比`。产出 AC-1/2/3/4。
- **刀② 折叠与 checkpoint**（`session-store` + 事件）：`ContextCompacted` 带保留指针；
  折叠产出**一条** checkpoint user 消息（逐字标签）；保留区读时重建。产出 AC-5/6。
- **刀③ 三种触发与失败处置**（接线面）：auto / overflow / manual 三个入口 + 失败矩阵 + 收缩矩阵 + 记账。产出 AC-7/8/9/10。
- **刀④ 设置面与界面 + 收口**：阈值三档（替掉绝对滑条）、压缩消息形态（转录里一条消息 + delta 流式）、
  中英文案成对、判据迁移、`make check` + `make selftest`、ADR、roadmap、真机。产出 AC-11/12。

## 任务

- [ ] 刀①：读上游 `core/src/session/compaction.ts` 全文 + `model-request.ts` + `to-llm-message.ts` + `history.ts` + `recent-text.ts`（逐字来源）
- [ ] 刀①：删 `llm/aux_client.rs`（连带 `llm/tests/aux_client.rs`）+ `prompts/compact/*.md` 四份稿子
- [ ] 刀①：新 `compact/` 模块（prompt 逐字 / plan / deliver / recent_text / outcome）
- [ ] 刀①：摘要请求改走主对话装配（真实 system + 工具定义 + 会话模型 + 同一套输出上限）
- [ ] 刀①：门槛 = 窗口 × 用户百分比；窗口未知不自动压
- [ ] 刀②：`ContextCompacted` 带保留指针；折叠产出 checkpoint user 消息（逐字）
- [ ] 刀②：保留区读时重建（`\n\n` 连接，工具输出 1250 字符截断，切点在 user 边界）
- [ ] 刀③：auto / overflow / manual 三个入口 + 失败矩阵 + 收缩矩阵（70/50/35）+ 失败记账
- [ ] 刀④：设置面「自动压缩阈值」三档（替掉绝对滑条）+ 文案成对
- [ ] 刀④：转录里的压缩消息（进行中 / 摘要 / 失败 / 用量）+ delta 流式
- [ ] 判据迁移（SS19a/19b/20b/20c/21b/24/25）
- [ ] 补测试（AC-1…AC-12）+ 变异自证
- [ ] `make check` 全绿 + `make selftest` 变异逐条报红
- [ ] ADR（长期取舍：为什么整块换掉、我们保留的三处偏离）+ roadmap 勾选

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

## 风险与回滚

- **风险 ①**：上游是 Effect/TypeScript 架构，本仓是 Rust + 内核分层 ⇒ 逐字复刻的是**语义与文本**，不是结构；
  判据必须按「行为等价」写，不能按「代码同形」写。
- **风险 ②**：删 aux 通路 = 摘掉 `specs/12` 刀① 的一半（预算挂在 aux 上）⇒ 预算要迁到新路径上（D1）。
- **风险 ③**：`session-store` 的折叠形状变了 ⇒ 旧会话重放要仍能读（旧摘要就是一段文本）。
- **风险 ④**：阈值改成百分比后，用户原来那根绝对滑杆的语义没了（100K ⇒ 976K 窗口下 = 10%），
  默认档位若选得太高会「长期不触发」⇒ 缺省取 50%，并在设置页写清「占模型窗口的百分之多少」。
- **回滚**：四刀各自独立成提交 ⇒ 任一刀出问题可单独回滚；设置项可当场调回。
