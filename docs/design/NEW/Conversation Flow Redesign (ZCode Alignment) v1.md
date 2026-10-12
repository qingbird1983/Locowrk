# 对话流重设计（ZCode 对齐）v1 —— 完整对话流的复刻设计

> **状态：待研究。** 本文是问题陈述 + 复刻设计 + 分期路线，**不是已实施设计**；除本文与
> INDEX.md 登记外零代码改动。立稿 2026-10-11；参照系 = ZCode 3.15.1 桌面端对话流 UI
> 源码（`packages/ui` + `packages/shared`，file:line 证据见附录 A）；宿主 = 本仓库
> desktop（Tauri 2 + Vue 3）。**视觉与形态规则的上位依据 = 《UI Design System.md》
> v1.0**（本稿的卡片形态 / 动画白名单 / 圆角嵌套等冲突裁决全部以该文件附录 A 为准，
> 本稿不重复引用其法律效力）。
>
> 用户裁定背景（2026-10-11）：复刻 ZCode 的完整对话流——分层与折叠、每步执行过程的
> 包装显示（思考/读取/编辑/写入/待办/终端/子智能体/询问等全部类型）、执行中状态提示、
> 回合产出物、文件点击展开与右栏打开；既有硬规则与 ZCode 冲突的去除或调整（已由
> 《UI Design System》附录 A 落纸）。

---

## §0 现状盘点：已有 / 缺口

**已有（v2.117 已兑现稿 + 现役实现，保留为底座）**：五列栅格过程行 `ProcessRow`（列
对齐纪律继续有效）；按工具类型定制明细 `ToolDetailView`（零裸 JSON 红线按上位规范
降级为兜底专用）；四层折叠相位机（第 0 层计时行 `TurnTimerLine` → L1/L2/L3）；审批/
提问/排队卡在 composer 坞（`PermissionCard`/`AskCard`）；thinking 行；WorkPanel 三页签
（子代理转录 / diff 审阅 / 浏览器预览）；流式 chunk/thinking 直推；自动滚动跟随 + 回底；
重放与实时同形投影（`runner.rs::project_events` 单点）。

**缺口（本稿要补的 9 件）**：

| # | 缺口 | ZCode 对应物 |
|---|---|---|
| 1 | 回合无权威头状态（运行/成功/中断/失败 + 权威工时），计时靠前端自算 | `turnHeaderRow.state/activeMs` |
| 2 | 工具行状态机缺「等待审批」「已取消」，执行中只有脉动点 | `toolCallRow.status` 六态 + progress |
| 3 | 逐类型折叠规则粗放（读/搜索不可折、子代理单行+右栏等） | `resolveRenderer` 27+ 身份分流 |
| 4 | 轮尾无产出物卡 | `ConversationFileSummaryPanel` |
| 5 | 文件 chip 点击 → 右栏 code-viewer 不存在（WorkPanel 无文件页签） | PreviewPane + codeViewer tab |
| 6 | 思考行流式强制展开（与蓝本相反） | Reasoning 收起 + 单行滚动摘要 |
| 7 | markdown 流式容错弱、代码块无头栏（语言标签/复制） | Streamdown streaming 模式 + CodeBlock 头 |
| 8 | API 重试在流里不可见 | `ChatApiRetryStatus` |
| 9 | 停止按钮无独立状态机（stopState idle/stoppable/stopping） | `SessionControl.stopState` |

## §1 ZCode 对话流设计结论（浓缩，证据见附录 A）

**三层架构**：协议级行日志（`ConversationRow` 封闭 union，9 种 kind，每行带会话内单调
`rowId` + `turnId` 标签；**turn 不是容器，是行上的标签**）→ 前端纯投影 store（首帧
snapshot + 4 种行 op：`appended/upserted/removed/delta` + 1 种 `state.updated` 键级补丁，
断档不猜、携水位 resync）→ 按 `turnId` 分组的回合渲染单元（纯函数分组器）+ 虚拟滚动
（live turn 不进虚拟列表，作 live tail 直渲染）。

**行模型要点**：turnHeader（state/activeMs 权威工时/fileChanges ±N）｜userInput（origin/
attachments/epilogue 分界）｜assistantText（streaming 状态机 + feedback）｜reasoning
（durationMs）｜toolCall（六态 status + inputText/output + display 结构化 union +
progress{fraction,message}）｜artifact（产出物 ref）｜subagent（childSessionId 可下钻）｜
timelineMarker（compact/fork/modelChange…，按 lane 装配）。

**卡片解剖**（`ToolSummaryRow`）：`[图标(运行态不转)] [动词·运行态文字扫光] [目标
chip] [次要路径] [±N mono] [状态词(六态,失败虚线下划线+tooltip)] [chevron(hover 显)]`；
展开 = Collapsible + 延迟 300ms 卸载；入场单次淡入按 toolId 记忆防重播。

**三处状态源**（严格分源，不混用）：轮内折叠条「工作中 N 秒」（每秒 tick，权威
activeMs，完成后翻「已工作 N 秒」）｜轮尾 spinner（权限弹窗阻塞时不显示；API 重试第 3
次起显示重试计数）｜composer 旁 Stop 方块钮（stopState）。

**产出物**：轮尾 `N files changed · +A −D` 可折叠卡 + 逐文件行（Review/OpenSplit 按钮
→ 右栏 diff）+ Rewind（依赖快照能力）；之后是悬停操作栏（复制整轮/赞踩/fork/时间戳）、
轮尾 boundary marker。

**文件交互**：卡片内文件 chip 点击 → 右栏 code-viewer 页签（file=只读高亮 256KB 上限 /
patch=完整 diff），与内联展开并存；子代理 chip 点击 → 右栏子代理转录页签。

## §2 数据链路设计（Locowrk 落法：不推翻事件通道，行模型语义注入现有投影）

**原则**：Tauri 事件通道（`agent://*`）与 runner 投影单点（`runner.rs::project_events` +
`project_new_events`）保留；把 ZCode 的行语义**注入**现有 `AgentEvent` 枚举与前端
`ChatItem` union，而非新造 RPC 协议。理由：Tauri emit 本身有序可靠、已有三向闭合闸
（check-commands/CHANNELS 铁则），会话恢复已走重放投影单点——ZCode 行协议要解决的
「断线重连/多端订阅」问题在单进程桌面形态下不存在，照搬协议层是过度设计。

### 2.1 事件增补（`desktop/src-tauri/src/events.rs` + `runner.rs`）

| 新事件 | 载荷 | 投影来源 | 对应 ZCode |
|---|---|---|---|
| `TurnHeader` | `turn_id, state(running/completed/interrupted/failed), started_at, active_ms, steps` | TurnStart→TurnEnd 切片聚合；active_ms 扣除等待审批/提问时段（runner 已有各事件时间戳） | turnHeaderRow |
| `ToolStatus` | `call_id, status(pending_approval/cancelled)` | ApprovalRequest 发出时前置补投 pending_approval；中断/停止时对在途 call 补投 cancelled | toolCallRow.status 两态 |
| `ToolProgress` | `call_id, preview_line?, fraction?` | 宿主长任务（shell 流式输出、fetch 下载）可选发；缺省不发（插件工具无 progress 上报时 UI 退回脉动点） | toolCallRow.progress |
| `FileChanges` | `turn_id, files[{path, additions, deletions, kind(edit/create/delete)}]` | turn 收束时由 tool-edit/fs 写入审计聚合（同切片 ToolCall/ToolResult 统计） | turnHeader.fileChanges |
| `LlmRetry` 已有 | 补前端展示（§5） | — | apiRetry |

**不做**：`inputStreaming`（Locowrk 工具参数调用即知，无流式入参场景）；row.removed
（编辑重发走既有 `session_history` 重放刷新，不引入截断 op）。

### 2.2 前端投影（`stores/chat.ts`）

- `ChatItem` union 扩展：`ToolItem` 增 `status: "pending_approval"|"running"|"ok"|"error"|"cancelled"`（替换现 `pending:boolean`）与 `preview_line?`；新增 turn 头**不作为独立
  item**，作为 turn 容器元数据 `{turnId, state, activeMs, fileChanges?}` 挂在
  TranscriptFlow 分组上（对应 ZCode「turn 是标签不是容器」）。
- 投影纪律不变：store 只消费不生产；onEvent 开关表 = 唯一接线清单（UI shell §4.1 扩表
  不改行）；CHANNELS 同批补新事件名（三向闭合闸）。
- 会话切换/重放：全量重投影已单点，天然得到 snapshot 语义；不做 resync 水位（单进程
  不需要）。

## §3 分层与折叠（逐层规则，取代/对齐现役相位机）

| 层 | 规则 | 对现役的修订 |
|---|---|---|
| 第 0 层·回合容器 | 一条用户消息开一回合；容器头 = 折叠条：运行中「工作中 N 秒」每秒 tick（**权威 = TurnHeader.active_ms，只有运行中才吃本地时钟**），完成翻「已工作 N 秒」，中断「已停止」，失败「已失败」（danger 弱标）。完成后整回合工作区收进折叠条（点击重开）；**异常终态（中断/失败）强制展开** | TurnTimerLine 保留形态，工时权威改后端；新增失败终态 |
| live 尾 | 运行中回合不参与折叠（工作区全展开直渲染）——即现役 L3 live 相位，保留 | 无 |
| 过程组（L1/L2 相位机） | 完成后收起：L2 一行摘要（✓ 已完成 · 耗时 · N 个工具）保留；L1 动作名串联保留；折叠是本地 UI 状态不入 store（现役纪律保留） | 无 |
| 思考行 | **流式与完成态都默认收起**；收起态 = 「思考」+ 右侧单行滚动摘要（取思考文本最后一个非空行，随流推进）+ 完成后「思考 · N 秒」；手动展开状态会话内记忆（Map by id）；空文本流式行不渲染；展开内容限高滚动 + 吸底 | **反转现役**「流式强制展开」（上位规范附录 A #6） |
| 工具行 | 默认态/可折性/展开内容**逐类型**定义（§4 总表）；明细懒挂载 + 展开不换布局（缩进子行）继续有效 | 折叠规则从「一刀切」变逐类型 |
| 系统条/整理总结块 | 保留现役发丝线形态；compact marker 归 timelineMarker 语义（lane 概念不引入，单泳道足够） | 无 |

## §4 工具步骤卡全集（27 类身份 → Locowrk 工具映射）

通用解剖（对齐蓝本 + 现役五列栅格融合）：`[图标 16] [中文动词·运行态扫光] [目标 chip
(mono, 文件名/命令/URL 截断)] [次要文本(路径/参数, <360px 隐藏)] [+N −N (diff 类)]
[状态词] [chevron(hover 显, 展开转 90°)]`。运行中**图标不转**（蓝本刻意决策）；状态词
六态 i18n（`chat.tool.*` 族，check-i18n 双语闭合）；失败态虚线下划线 + tooltip（含复制）。

| 工具 | 动词 | 默认 | 展开内容 | 右栏入口 |
|---|---|---|---|---|
| fs_read | 读取 | **单行不可折** | —（内容看右栏） | 文件 chip → code-viewer（只读） |
| fs_list | 列出 | 可折 | 目录树形列表（现役定制保留） | 目录名 chip → 文件页签（C3 后） |
| tool-edit | 编辑 / 写入 | 可折（折叠态带 ±N） | 内联 diff（max-h 滚动；>120k 字符降级纯文本）；多文件 = 子行列表各自 ±N | 文件 chip → code-viewer（patch 模式） |
| tool-shell | 终端 | 可折 | 命令面板（收 3 行/展 10 行，`$` 前缀）+ 输出（5 行视口 + **运行中吸底跟随、上滚冻结**）；退出码不单列，失败走状态词+tooltip | — |
| tool-fetch | 抓取 | 可折 | URL + 状态 + 正文预览（mono 井） | URL chip → 浏览器页签 |
| memory_write / delete / undo | 记忆 | 可折 | key + scope 徽章（全局/项目）+ 值预览 | — |
| memory_read / list / search | 检索记忆 | 可折 | 命中 key 列表 + 值预览（空结果显示「无命中」而非空白） | — |
| memory_expand / search_titles / project_history | 展开章节 / 搜标题 / 项目时间线 | 可折 | 结果标题列表 / 时间线摘要 | — |
| subagent_run | 子智能体 | **单行不可折**（整行可点） | 运行中摘要行滚动显示子代理最新活动；点击 → **右栏子代理转录页签**（现役已有，入口从「查看完整对话」按钮升为整行点击） | ✓ |
| subagent_verify | 校验 | 可折 | 结论 + 差异摘要 | — |
| ask_user | 提问 | 可折（进行中锁死） | Q/A 列表；未答自动继续显示「未回答，已自动继续」 | — |
| browser_open / browser_console | 浏览器 / 控制台 | 单行 | — | URL chip → 浏览器页签（现役） |
| screenshot / read_image | 截图 / 看图 | 可折 | 缩略图行（现役 56px + 查看大图保留） | 图片 → ImageViewer（现役） |
| send_email | 邮件 | 可折 | 收件人 + 主题 + 正文预览 | — |
| pdf | 解析 PDF | 可折 | 文件名 + 页数 + 摘要 | 文件 chip → code-viewer（只读降级） |
| schedule 三件 | 定时 | 可折 | 标题 + cron + 下次触发 | — |
| search_tools | 搜索工具 | 可折 | 工具名 + 描述列表 | — |
| 未知工具 / 新插件 | 工具调用（兜底） | 可折 | 定制渲染缺位时：参数/结果 JSON 折叠块（mono 井，上位规范附录 A #2 的唯一豁免处） | — |

**权限预览复用**：审批卡的参数预览块复用对应工具卡渲染器（`等待确认` 动词覆盖、
forceOpen、隐藏输出区——edit 例外保持折叠），与蓝本 PermissionDialog 同构；现役
PermissionCard 形态保留。

## §5 执行中状态提示（三源严格分源）

1. **轮内折叠条**：「工作中 N 秒」每秒 tick（`TURN_TICK=1000ms`），完成/中断翻终态——
   事实源 = TurnHeader。
2. **轮尾加载态**：live 尾末尾 spinner；可见性判定 = 无权限/提问阻塞时才显示；
   `agent://llm-retry` 第 3 次起改显「重试中 (n/max) · 下一次 Ns」——事实源 = control 相位
   + retry 事件（补前端渲染，事件已有）。
3. **Composer 控制簇**：运行中显示 Stop 方块钮（Esc 快捷提示），stopState 三态
   （idle/stoppable/stopping）由后端 turn 状态投影——事实源 = turn 运行态；与 Send 互斥。
4. （backlog，不在 C1-C3）顶部状态条：git 分区/后台任务/运行中子代理聚合行。

## §6 回合产出物

- 触发：回合终态且 `FileChanges.files` 非空。
- 形态：可折叠卡——标题行「**N 个文件改动 · +A −D**」（±mono、diff 色、数字短翻页动画
  可选）；展开逐文件行：`文件类型图标 + 文件名 + ±N + [查看] 按钮`。
- 交互：行点击/查看按钮 → 右栏 code-viewer 页签（edit/delete → patch 模式，create →
  只读全文）；悬停操作栏（复制整轮正文 / 赞踩 / 时间戳）挂最终正文行尾（hover 显现）。
- **Rewind 不做**：依赖磁盘快照能力（现役 RESERVED 状态，快照从未接线）；列入 C4+
  可选相，待快照能力立项后复评。
- 高速/工作流等 ZCode 特有收尾卡（Highspeed 页脚/workflow 卡）不适用，不做。

## §7 文件交互链路（WorkPanel 第 4 页签）

- 新页签 `code-viewer`（规范 §2.5 启动器清单挂账的「文件」条目落地）：
  - `source = file`：只读代码视图——复用现役 markdown 代码块高亮管线，256KB 上限，
    超限提示「文件过大，仅显示前 N KB」；头部 = 文件名 + 语言标签 + 复制 + 「在系统
    编辑器打开」（Tauri opener）。
  - `source = patch`：完整 diff 视图（±行配色用 `--lw-diff-add/del`，hunk 头 sticky 可选）。
  - 页签并存规则：与子代理转录/审阅/浏览器页签同栏互斥激活，页签标题 = 文件名；
    同文件重复打开 = 激活已有页签。
- 入口：§4 各工具卡文件 chip、§6 产出物行、（C3 后）fs_list 目录行。
- 安全边界：只读（不可编辑）；路径必须落在该会话归属 workspace 的 fsRoots 内（复用
  浏览器预览的 fsRoots 校验管线 `lwpreview` 同款纪律）。

## §8 审批卡 / 提问卡细节对齐

保留现役坞卡形态（与蓝本内联停靠卡同构），补齐四点：①选项前置序号 `1.` `2.` `3.` +
键盘数字直选 + 方向键循环 + Enter 确认（AskCard 选项列表先落）；②「等待确认」统一
徽章处理（上位规范 §3.2）；③预览 forceOpen（edit 除外）；④拒绝时可选反馈一行
（占位「告诉模型接下来应该怎么做…」，作为拒绝理由回传——`deny` 已带 reason 字段）。

## §9 markdown 与代码块

- 正文渲染管线保留 `lib/markdown.ts`；对齐三点：①**流式容错**——未闭合围栏/粗体不抖动
  （流式期解析容错，完成态静态重渲一次）；②**流式期关闭语法高亮**（async highlighter
  与流更新叠加的已知坑，蓝本同款规避），收口后整段高亮；③代码块头栏 = 语言标签 +
  复制钮（复制后 2s 对勾反馈）+ 换行切换，样式按上位规范卡片档。
- CJK 细节（删除线修正等）随实现按需补；`zcode-file-citation` 式文件引用不做（Locowrk
  正文引用走既有 chip 语法）。

## §10 分期路线（审计主线之后立项；每批先修订标准再写码）

| 批 | 内容 | 涉及 |
|---|---|---|
| **C1 数据链路** | TurnHeader/ToolStatus/FileChanges 事件 + runner 投影单点扩展 + chat store 行语义 + 三向闭合闸扩表 + 重放回归 | src-tauri(events/runner/commands) + stores/chat |
| **C2 工具卡全集** | §4 总表逐类型落地 + §3 折叠规则 + ThinkingRow 反转 + 状态词六态 i18n + 审批卡对齐（§8）+ Stop 状态机 + 重试可见性 | components/chat/* + locales |
| **C3 产出物与文件页签** | §6 产出物卡 + §7 code-viewer 页签 + fsRoots 校验 + git 色族令牌落地 | WorkPanel + 新组件 + style.css |
| **C4 打磨（可选）** | 同工具组卡（附录 A #9 挂账）+ 顶部状态条 + diff 短翻页动画 + Rewind 复评（依赖快照立项） | — |

验收基线（每批）：四道闸 + fmt/clippy 双侧 + check-i18n 双语闭合 + 双主题走查 + 
「长会话不卡」抽查（50+ 工具行的会话滚动/折叠流畅）；C2 起加端到端走查对照项：
思考收起摘要 / 编辑卡 diff / 终端吸底 / 子代理右栏 / 产出物卡五点逐一对拍蓝本。

---

## 附录 A：ZCode 证据表（仓库根 = `F:\Warehouse\Github\ZCode`）

| 结论 | 证据 |
|---|---|
| 行 union（9 kind）+ rowId/turnId 基座 | `packages/shared/src/zcode-protocol-v4/rows.ts:18-38,66-162,168-232,248-279,345-446` |
| turnHeader（activeMs 权威/fileChanges） | `rows.ts:66-111`（:102-109 fileChanges） |
| toolCallRow 六态 + progress + display union | `rows.ts:203-232`；`toolDisplay.ts:279-292` |
| 前端回合分组纯函数 / 渲染单元字段 | `packages/ui/src/v4/conversationTurnRenderUnits.ts:67-77,106-122,159-190,399-443`；`conversationTurnRenderUnitTypes.ts:20-60` |
| 视觉工作段 | `conversationTurnWorkSegments.ts:20-64` |
| delta 四 op + state patch / 断档 resync | `delta.ts:40-62,104-188`；`conversationProjectionStore.ts:2-7,664-770`；`apply.ts:64-232` |
| 合帧批处理 / delivery profile（30ms/150ms） | `coalesce.ts`；`core.ts:34-59` |
| 虚拟滚动 + live tail + 滚动锚定 | `ConversationTimeline.tsx:783-794,882-919,2013-2041`；`timelineScrollAnchor.ts:13,32-66`；`conversationTimelineLiveTail.ts:15-28` |
| Reasoning 收起 + 单行摘要 + 耗时 | `ConversationRowView.tsx:1774-1807`；`components/ai-elements/reasoning.tsx:255-359` |
| 卡片解剖/扫光/状态词/失败 tooltip | `ToolCallBlocks/ToolSummaryRow.tsx:53-234`；`ToolLayout.tsx:16-21,139-179,195-210,240-302`；`ToolCallBlocks.tsx:27-68,372-384`；`tool-call-summary.ts:38-44` |
| 工具身份/渲染器分流/兜底 JSON | `tool-identity.ts:1-96`；`resolveRenderer.ts:58-175`；`renderers/fallback.tsx:41-110` |
| 逐类型卡（read/edit/execute/todo/agent/ask） | `renderers/read.tsx:287`；`edit.tsx:43-62,163-250`；`execute.tsx:290-374`；`ExecuteCommand.tsx/ExecuteOutput.tsx`；`todo.tsx:36-154`；`agent.tsx:129-143,213-413`；`ask-question.tsx:17-92`；`ElicitationDialog.tsx:754-800` |
| 折叠状态内存 Map / 延迟卸载 | `ToolLayout.tsx:16,19-21,154-179` |
| diff 预览/降级/配色 | `renderers/EditInlineDiffContent.tsx:38-85`；`patchDiffPreview.ts:3-6,347-445` |
| 三状态源（折叠条/轮尾 loading/Stop） | `ConversationTurnGroup.tsx:217-243,640-728`；`chatLoadingVisibility.ts:38-62`；`ConversationComposer.tsx:2141-2230`；`snapshot.ts:151-161` |
| 顶部状态条 | `ConversationStatusPanel.tsx` + `conversationStatusPanelModel.ts:120-129` |
| 产出物面板 + Rewind + 悬停操作栏 | `ConversationFileSummaryPanel.tsx:238-411`；`ConversationTurnGroup.tsx:1639-1741`；`ConversationRowView.tsx:1478-1651` |
| 文件 chip → 右栏链路 | `renderers/read.tsx:260-270`；`useAppPanels.ts:352-382`；`PreviewPane.tsx:584-586,1482-1726`；`codeViewer.ts:25` |
| 子代理右栏下钻 | `ConversationAgentToolCallRow.tsx:39-97`；`SubagentDrilldown.tsx:21,37-127` |
| 权限停靠卡（序号/键盘/forceOpen） | `PermissionDialog.tsx:104-171,238-257,325-347,685-757,778-781,1008-1029` |
| markdown 流式容错/代码块头/流式关高亮 | `components/ai-elements/message.tsx:12-16,700-748,1314-1640`；`code-block.tsx:182-199,454-495` |
| 状态词 i18n 样例 | `packages/ui/src/i18n/locales/zh-CN.ts:4940-4988,5995-6013` |

## 附录 B：Locowrk 锚点（仓库根 = 本仓库）

| 事项 | 锚点 |
|---|---|
| 事件常量/CHANNELS 三向闭合 | `desktop/src-tauri/src/events.rs:15-61`；`desktop/src/lib/api.ts:191-243`；UI shell §4.2:1003 |
| 投影单点/枚举原文 | `desktop/src-tauri/src/runner.rs:236-494`；`events.rs:100-271` |
| ChatItem/ToolItem union | `desktop/src/stores/chat.ts:35-132`；onEvent 开关 `chat.ts:483-689` |
| 过程行/明细/思考行 | `components/chat/ProcessRow.vue`、`ToolRow.vue`、`ToolDetailView.vue`、`ThinkingRow.vue`；视图模型 `lib/tool-detail.ts:50-77` |
| 四层折叠相位机/计时行 | `components/chat/TranscriptFlow.vue:55-133,175-256,300-443`；`TurnProcess.vue:1-54` |
| 审批/提问/排队坞卡 | `components/chat/Composer.vue:364-377`、`PermissionCard.vue`、`AskCard.vue`；`stores/approvals.ts:10-48`、`stores/asks.ts` |
| WorkPanel 三页签/浏览器 fsRoots 管线 | `components/shell/WorkPanel.vue:27-94,184-208`；`stores/work-panel.ts:12-28`；`src-tauri/src/browser/preview.rs` |
| 滚动跟随/回底 | `components/chat/ChatTranscript.vue:68-112,135-204` |
| 上位规范/下位细则 | 《UI Design System.md》v1.0（权威）；《UI shell and patterns.md》§2.5 页签清单、§3.7-3.10、§4.1 流向表、§6.6 动效；《Front-end Design Guidelines.md》§二①⑥⑫ |
| 已兑现基线稿 | 《NEW/Tool Execution Process Display.md》（v2.117；卡片/JSON 红线由上位规范附录 A 接管，列对齐/明细定制/懒挂载继续有效） |
