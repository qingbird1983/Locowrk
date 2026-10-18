# 06-对话流重做：行壳统一 + 身份矩阵 + 接线做全

- 状态：在做（行壳/矩阵/展开体已交，状态位与接线未做）
- 负责人：青鸟（定取舍） / Qoder（实现）
- 日期：2026-10-18（同日晚上按用户裁定改写：本文不再只管「渲染矩阵」，**扩为对话流整块重做**）
- 出处 / 上游：设计稿 `docs/design/Conversation Flow Rendering Spec v1.md`（⊘ 不入库），**§5 渲染矩阵为准**，
  §6 组件契约、§7 展开体形态库、§8 折叠、§9 滚动分述。
  本稿取代 [`specs/02-tool-cards`](../02-tool-cards/spec.md)：那份指向已退役的 ZCode Alignment v1，
  范围只到「工具卡」，且把 7 件互不相关的事捆成一刀（设计稿 §10 后注已记这一条）。

## 目标

对话流现在不是「某一行不好看」，是**没有身份表、也没有统一的行壳**：

- `desktop/src/lib/tool-detail/kinds.ts` 只登记 23 个工具名，实际在跑的 38 个身份里，`memory_expand`（宿主内置）
  `ask_user` `browser_open` `echo` `shout` `memory_delete` `memory_undo` `subagent_verify` 等全部落到兜底
  「工具」+ 空摘要 —— 长会话里出现几行完全一样的空行。
- 行骨架是五列栅格（`20px 38px minmax(0,1fr) 80px 24px`）：80px 状态列逼得每一行都得有状态词，
  于是满屏对勾；1fr 描述列被夹窄，`clip(baseName(p), 32)` 又把扩展名切掉。
- 展开体给的是**元数据**（路径 / 大小 / 编辑处数 / 匹配方式 / 原始文件 / 备份），不是**内容**（改了什么、跑了什么）。

要让每一行一眼读出「是什么工具、在做什么、结果如何」，且新增工具只改登记表、不改组件。

**用户 2026-10-18 裁定的纲（比上面三条更上位）**：这是一次**重做**，不是补丁集。
判它做成的标准是三条：**有系统性设计**（一个属性一个落点）、**有规范性统一**（同一行为只许一处定义，
不因工具/状态不同而漂）、**有完整界面要求**（该有的交互先记账、再配套，不许「没有功能就空着」）。
对一行一行找案例来修的做法，用户已明确否决——几十种工具乘若干状态，改不完，最后必然全盘推翻。

## 统一模型（本次重做的纲）

> 表的每一行 = **一个属性**；每个属性只有**一个落点**；改这个属性只改那一处。
> 新工具/新状态进来时，只往 `render-matrix.ts` 加一行，组件与样式一行不动。
> 这张表**先落进上位标准**（`docs/design/authority/Front-end Design Guidelines.md`）再动代码——见 批 A。

| 属性 | 唯一落点 | 规则 | 台账 |
| --- | --- | --- | --- |
| 行的段序、间距、三角位 | `desktop/src/components/chat/ProcessRow.vue` | 段序恒定：图标 → 类型词 → 描述 → 元信息 → 状态 → 三角；间距一律 `--lw-gap-6`；**所有段左靠，三角紧跟最后一段**（不许有的行右对齐、有的行紧贴）；空段不渲染 | AF-01 AF-03 AF-10 AF-11 AF-14 |
| 过程组头 | `desktop/src/components/chat/TurnProcess.vue` | 头行与过程行同吃同一段序规则与同一几何，不自成一套 | AF-06 |
| 身份数据（图标/动词/摘要来源/展开形态/徽章/diff/可折） | `desktop/src/lib/chat/render-matrix.ts` | 加工具 = 加一行；闸 `check-render-matrix.mjs` 双向对账 | ✅ |
| 摘要文本的截断 | `desktop/src/lib/format.ts` + `tool-detail/describe.ts` | 文件名中段省略保扩展名（`clipName`）；命令/查询按**段位宽度**截断，不按工具逐个写死 40/32/24 | AF-15 |
| 相位与状态位 | `render-matrix.ts` 的 `toolPhase()` + 行壳 status 段 | 5 相位 → 状态词唯一落点 `tool.state.*`（表见标准 §二⑰）；成功**什么都不出**；失败是整屏唯一 danger | AF-04 |
| 展开体形态 | `desktop/src/components/chat/ToolDetailView.vue` | 7 种形态（code/list/diff/props/term/text/media）；默认零裸 JSON | AF-07 |
| 展开体外壳（底 / 圆角 / 内边距 / 起止 x） | `ToolRow.vue` 与 `ThinkingRow.vue` **共用同一个外壳** | 展开体只有**一种**外壳：同一个底、同一个圆角、同一个内边距、同一个起点 x。内部形态（代码/终端/diff/属性行/文本）只负责自身内容，**不许再各带一层底** | AF-17 AF-02 |
| 展开体属性行 | `ToolDetailView.vue` | 标签与值两列；**标签列取宽口径一处定** = 行级 `max-content` + `column-gap: 8px`（已定，见 AF-02；值列不跨行对齐是明知的代价） | AF-02 |
| 折叠与状态记忆 | `ToolRow.vue` / `ThinkingRow.vue` | 三层折叠；层 3 按 `item.id` 记忆；`0fr↔1fr` 动效 | AF-12 AF-13 |
| 滚动与跟随 | `desktop/src/components/ChatTranscript.vue` | 跟随判定 48px / 出钮 120px；锚定策略一处定 | AF-09 |
| 行内可点元素 → 右栏 | `stores/work-panel.ts` + 各行的接线 | 凡行内出现文件名/代码块/diff，必须有对应右栏页签 | WF-01…WF-05 |

**红线（已由机器判据钉住，见 批 F）**：`.prow__*` 的布局只许出现在 `ProcessRow.vue`；
`.tdetail__*` 只许出现在 `ToolDetailView.vue`。判据 = `scripts/check-css-ownership.mjs`
（`.prow__*` / `.tdetail__*` / `.tool__*` 定义只许在各自归属组件，跨组件公共面 `style.css`
对这三族也算合法；`.lw-expand` 只许在 `style.css` 且必须有人在用；注释里的类名不算）。

## 问题台账

> **记账口径**（用户 2026-10-18 定，本文档即时生效）：
> ① 发现即入账，**当轮不改**——避免「找出一条改一条」；
> ② 一条 = 一个属性上的**一类**现象，不是某一次点击；
> ③ 每条必须带证据（文件:行 / 真实窗口读数）与**归属属性**；
> ④ 攒完按属性清账（批 B–E），清一条划一条，**不删行**（不然下次又忘）。
> 状态：⬜ 未修 · ◐ 部分 · ✅ 已修／口径已定

| 编号 | 现象（一类） | 证据 | 属性 | 状态 |
| --- | --- | --- | --- | --- |
| AF-01 | 展开三角位置随行内容漂移：有描述段的行三角被推到最右，无描述段的行（思考）紧跟文字 | 两处根因：`ProcessRow.vue:108` 的 `.prow__desc { flex: 1 1 auto }` 吃掉剩余宽度；`style.css` 的 `.cv-head__chev { margin-left: auto }` 把组头三角甩到整行最右。用户 2026-10-18 目检点名「一会是紧跟着的 一会又变成右对齐的」 | 行壳几何 | ✅ 两半都修：行壳改 `flex: 0 1 auto`、组头去 `margin-left: auto`；真实窗口实测每行与组头的三角距最后一段恒 6px |
| AF-02 | 展开体属性行「标签与值间距太大」 | 两版都被驳：① 每行定宽 72px ⇒ 两字标签后空一大段；② 两列栅格挂 `<dl>` 上 ⇒ 标签列按**全块最长标签**取宽，记忆行里 `sessions` 被 `total_sessions` 撑出约 50px 空档（真实窗口实测 502→524）。用户两次点名（2026-10-18 上/下） | 展开体属性行 | ✅ 定为**行级** `max-content` + `column-gap: 8px`（取宽口径见「统一模型」行）；真实窗口实测每行标签↔值恒 8px。代价：值列不再跨行对齐 |
| AF-03 | 行壳空段占位时会出现「类型词 · · 状态」双分隔 | 已修：描述段改成有插槽才渲染，`·` 由行壳统一给（提交 `862db71`） | 行壳几何 | ✅ |
| AF-04 | 状态词曾散在各组件里各写一套（`tool.running` / `tool.failed` 等四个平铺键，词表无处可查） | 已修：收进唯一落点 `desktop/src/locales/*/tools.ts` 的 `tool.state.*`，词表进标准 §二⑰。**剩下的不是缺词是缺数据**：`已拒绝`（拒绝后那一步不落行——`TranscriptFlow.vue` 跳过 `kind === "decision"`，v2.98 裁定）与 `已停止`（只标轮级组头 `turn.stopped`，不逐行重复） | 相位与状态位 | ◐ 词表已收口；两态待数据/待裁 |
| AF-05 | 思考行两套形态：实时轮给时长（`思考 · 12 秒`），重放轮给首句摘要 | 落盘投影不带时间戳（`desktop/src-tauri/src/events.rs:221` 只有 `session_id`/`text`），口径写在 `stores/chat/types.ts:74-75`。**两套是刻意的**，要在标准里写明，别再当 bug 修 | 相位与状态位 | ✅ 口径已定 |
| AF-06 | 过程组头（`TurnProcess.vue`）自一套渲染：裸工具 id 已修，但段序/几何/状态位与工具行不同源 | `TurnProcess.vue` 的 `minimalText = clip(props.names.join(" + "), 32)`；`.cv-head` 原写死 `gap: 6px` / `font-size: 13px` / 三角 `margin-left: auto` | 行壳几何 | ✅ 几何与段序已同源：`gap` 走 `--lw-gap-6`、`font-size` 走 `--lw-fs-ui-caption`、三角紧跟末段；真实窗口实测组头左缘 456 = `.prow` 左缘 456。组头 meta 的构成（时长 + N 个工具）另见 AF-14 |
| AF-07 | `edit_replace` 的 diff 卡行号恒空 | 插件 output 只有 `additions`/`deletions`/`byte_range`（`plugins-src/tool-edit/src/replace.rs:110-121`），没有行号 ⇒ 宁缺勿假 | 展开体形态 | ◐ 已裁：**列在、号空**（不编造行号）。要真行号得先让插件把行号写进载荷——属后端改动，归批 E 与接线一起做 |
| AF-08 | 折叠行 ±N 只能在 v2.204 之后的会话里出数 | `additions`/`deletions` 2026-10-14（`02d44cf`）才进插件载荷；更早日志没有 ⇒ 不显示是对的 | 身份数据 | ✅ 口径已定 |
| AF-09 | 滚动抖动：禁掉原生锚定 + 每 chunk 自己 `scrollTo` 是主因 | `ChatTranscript.vue:191` 的 `overflow-anchor: none`（设计稿 §9.1 同判） | 滚动与跟随 | ✅ 已删该声明（回落浏览器缺省 `auto`）。真实窗口实测：在距底 200px 处往 `.scroller` 里注入一个 200px 高的块，`scrollTop` 前后都是 221（不再被拽回视口底部）；点「回到底部」后 `.back-bottom` 消失 |
| AF-10 | 状态段自带 `justify-content: flex-end`，与「所有段左靠」相冲 | `ProcessRow.vue:139-148` | 行壳几何 | ✅ 已删该声明（`ProcessRow.vue`），与 AF-01 同一条规则 |
| AF-11 | 回合产出物行（`N files changed · +A −D`）未做 | 设计稿 §6；`file_changes` 事件已有（挂用户行） | 行壳几何 | ⬜ |
| AF-12 | 三层折叠的层 3（单行展开体按 `item.id` 记忆）未做 | 设计稿 §8；现役只有 `everExpanded` 懒挂载 | 折叠与状态记忆 | ✅ 记忆搬到 `desktop/src/lib/chat/fold-state.ts`（模块级 `reactive(Map)`：键在表里 = 挂载过、值 = 展开着），`ToolRow.vue` 的 `expanded`/`everExpanded` 改成读它，`ChatTranscript.vue` 在 `watch(chat.sessionId)` 里 `resetRowFolds()` 清空。钉住用例 `desktop/tests/tool-row-fold.spec.ts`（换组件实例后仍展开）。思考行的展开体不做记忆：它没有懒挂载（`v-show` 常挂）且缺省收起就是设计缺省 |
| AF-13 | 展开体收起态安全三条（`overflow:hidden` / `pointer-events:none` / 高度 0）未按新形态核 | 设计稿 §8.3，是 v4.17「透明点击盾」的根因修复 | 折叠与状态记忆 | ✅ 核过：两条展开体都用 `v-show` ⇒ 收起即 `display:none`，三条天然成立。真实窗口实测收起态 `.tool__sub`：`display: none` · 高度 `0` · `checkVisibility() === false`（不参与命中测试），展开态 `display: flex` · 高度 `72` |
| AF-14 | 组头计时行 `已工作 N 分 N 秒` 与 `✦ 已完成` 分两行显示，且组头另有一枚 `34s` 与容器行同一属性两套写法 | 2026-10-18 真实窗口读数；用户 2026-10-18 裁定「同意你的意见，收掉吧」 | 行壳几何 | ✅ 读数收成一份：新件 `desktop/src/components/chat/TurnTimerReadout.vue`（图标 + 文案 + `useTurnClock` 走表），`TurnTimerLine.vue` 只剩壳、`TurnProcess.vue` 删掉 `durationText`/`useTurnClock`。落点二选一：容器行 **或** 首个动作块组头（`TranscriptFlow.vue` 的 `timerHostKey`：容器开着 + 首块是过程组 + 该组非 live 时才托管，`TurnTimerLine` 加 `v-if="!timerHostKey.has(turn.key)"`）。组头行根由 `<button>` 改 `role="button"` div（行内要嵌一颗「折叠整个轮容器」的按钮，故 `emit('toggleTurn')` + `@click.stop`），`.cv-head` 补 `cursor: pointer`。**真实窗口实测**：box0 `timerLine: true`（无组头，读数归容器行）；box1 组头 = `记忆 + 读取 · 已工作 0 分 12 秒`、`readouts=1`；box2 = `终端 · 已工作 0 分 5 秒`；三箱各只有一处时长；点组头读数 → container 收起、`TurnTimerLine` 接回（`已工作 0 分 12 秒`）；再点 → 回到组头托管。标准 `§二⑰` 同步（轮计时读数只有一份 + 不授权第二处落点） |
| AF-15 | 摘要截断口径按工具逐个写死（shell 40 / fetch 40 / subagent 24 / grep 24 / 路径 32） | `desktop/src/lib/tool-detail/describe.ts` | 摘要文本 | ✅ 收成一处：`describe.ts` 只留 `export const SUMMARY_CLIP = 40` 一道**病态长值兜底**，真正的截断交给可用宽度（`.tool__desc` 上 `overflow:hidden` + `white-space:nowrap` + `text-overflow:ellipsis`）。真实窗口实测：注入 250 字符文件名后 `scrollWidth 1711 > clientWidth 785`（出省略号），而行宽仍 `867`、三角右缘 `1320 ≤ 行右缘 1323`（行没被撑爆） |
| AF-16 | 语言徽章：按扩展名逐级回退到素材图标，彩色 + 专属字形 | `desktop/src/lib/file-icons.ts`；用户 2026-10-18 裁定，推翻设计稿 §2.2 的「不画各语言 logo」 | 身份数据 | ✅ |
| AF-17 | **同一个「展开体」并存多套外壳与底色**：思考行 = 一个 `--lw-bg-inset` 灰底圆角框包住全文；工具行的属性行 = 无底裸排；终端块 = 又一层 `--lw-bg-inset`；代码块 = 自带深底。两条路径的内容起点 x 也差约 6px（同窗口实测：思考框 x≈622、工具属性行 x≈628） | `ThinkingRow.vue:103-109`（`.thinking__body` 自己带底/圆角/内边距）vs `ToolRow.vue` 的 `.tool__sub`（只有 `border-left` 发丝线、无底）＋ `ToolDetailView.vue:331-334`（`.tdetail__term` 再一层 `background: var(--lw-bg-inset)`）与 `.tdetail__code`。用户 2026-10-18 目检：「思考 和 读取 同时展开…属于同一属性 同一状态 但你却做出了 2 种展示样式」 | 展开体外壳 | ✅ 已立唯一落点 `style.css` 的 `.lw-expand`（`margin-left: calc(14px/2)` + `padding-left: calc(14px/2 + gap-6 - 1px)` + `border-left: 1px var(--lw-line-subtle)`，**无底无圆角**），思考行与工具行都挂它；真实窗口实测两条路径的 `getBoundingClientRect().left` 同为 `557.5`、`padding-left` 同为 `12px`、`background` 同为透明。底层 `--lw-bg-inset` 只留给「井」（代码/终端/diff） |
| AF-18 | 身份矩阵里三个身份共用类型词「工具」：兜底 `fallback`、`ask_user`、`dev_echo`——「工具」说不清是哪个动作，且与 `.tool__*` 这类**代码标识**同名 | `desktop/src/lib/chat/render-matrix.ts:191/223/243` 都是 `verb: "tool"` → `tool.type.tool`；真实窗口 session 3 实测该行读作「工具」 | 身份数据 | ✅ 三个身份各自归词（用户 2026-10-18 裁定）：`ask_user` → `ask`「询问」、`dev_echo` → `echo`「回声」、兜底 `fallback` 保留 `tool` 但 `tool.type.tool` 的值由「工具」改「调用」（en `Tool` → `Call`），**「工具」这个类型词自此不存在**。为什么改兜底而不是给 `ask_user` 单开键：`tool.type.tool` 本来就只服务兜底，改值即可、不留死键。**真实窗口实测**：session 3/6 的类型词序列里出现「询问」（原先那行是「工具」），七个会话再无一处「工具」 |

## 接线欠账（不许留空）

> 用户 2026-10-18 点名：功能没做也要**先记账**——「未来还是会忘掉做这步的关联接线的。
> 而不是你计划里定的 没有功能就空着。空到最后 谁来管这块了」。
> 这些条目**依赖** roadmap 阶段「对话流：产出物与文件页签」（`docs/roadmap/README.md` 第 15 行）：
> 那个阶段开工时**以本节为需求**，不是「开工时再想」。

| 编号 | 交互（用户点哪） | 该发生什么 | 现状 | 依赖 |
| --- | --- | --- | --- | --- |
| WF-01 | 点工具行里的**文件名** | 右栏新开「文件」页签：`.md` 按渲染后正文打开（不是源码），其它按语法高亮打开 | **无接面**：文件名是纯文本 | 右栏新增 file 页签 + 读文件能力 |
| WF-02 | 点展开体里的**代码块** | 右栏同一「文件/代码查看器」页签打开，带行号、高亮、复制 | `ToolDetailView.vue:20` 只 emit `openTranscript` | 同 WF-01 |
| WF-03 | 点工具行的**变更统计（±N）** | 右栏「审阅」页签打开该 diff | 已通：`openDiff` → `rev:<callId>` | 待裁：与 WF-01 合并成同一页签族？ |
| WF-04 | 点子代理行 | 右栏子会话转录 | 已通：`openTranscript` | — |
| WF-05 | 点 diff 卡里的行号 / 文件头 | 右栏打开该文件对应行 | 无 | 同 WF-01 |

**现状证据**：`desktop/src/stores/work-panel.ts:20` 的页签 kind 只有 `"subagent" | "review" | "browser"`
—— **没有 file/code 页签**。所以 WF-01/02/05 是「功能配套」欠账，不是渲染欠账；渲染侧不留空白等它，
该出的文件名先出，接线按本节补。

## 验收标准（必须可测：跑什么命令、看到什么结果）

界面类先起 `cd desktop && pnpm tauri dev`，下面每条在真实窗口核对。

- [x] AC-1 **身份完备**：`node scripts/check-render-matrix.mjs` 输出
      `IDENTITIES=38 PHASES=5 UNREGISTERED=0 MISSING_VERB=0`。往工具表里加一个不存在的新工具名再跑，必须报红。
- [x] AC-2 **形态唯一**：`node scripts/check-style.mjs` 0 报；`grep -rn "38px" desktop/src/components/chat` 无命中
      （五列栅格不留残迹）。
- [ ] AC-3 **五类真实调用逐类目视**：`fs_read` / `edit_replace` / `shell` / `memory_expand` / 一个未知工具兜底，
      每行显示 `图标 + 动词 + 对象 + 一条元信息`；`memory_expand` 不再是「工具」空行。
- [x] AC-4 **成功不喧哗**：连跑 ≥20 个工具，成功行**不出现**对勾或状态词；失败行是整屏唯一带 danger 色的行。
- [x] AC-5 **语言徽章**：`.ts` `.rs` `.md` `.json` `.vue` 五种文件显示五种可区分的 logo 字形；
      `pnpm run lint:style` 0 报；标准 §七 已登记彩色豁免。
- [x] AC-6 **展开体给内容**：`edit_replace` 展开 = 带行号的 diff 卡（绿/红底走 `--lw-diff-add`/`-del`）；
      `shell` 展开 = 深底终端块；`fs_read` 展开 = 代码块带语言标记。元数据收进「原始数据」。
- [x] AC-7 **思考行**：默认收起为单行 + 时长；展开是灰底块；流式进行中不给折叠权。
- [x] AC-8 **滚动**：流式追加时视口不被拽回；用户主动上滚后不再自动跟随，距底 >120px 出现「回到底部」。
- [ ] AC-9 **一行一属性**（**新增，本次重做的纲的判据**）：`AF-01`/`AF-02`/`AF-10` 清账后，
      真实窗口里任取 5 种状态的工具行 + 思考行 + 组头，**三角位置逐个量**都在最后一段之后；
      记忆行/读取行/写入行的属性行「标签与值」间距逐个量相同（±1px）。
- [ ] AC-10 **接线在册**：WF-01…WF-05 每条在 roadmap 那个阶段的 spec 里有对应勾选项，或在本稿里划掉。
- [ ] AC-11 `make check` 全绿（含中英成对与形态两道闸）。

## 非目标

- **不**在本稿实现 WF-01…WF-05 的功能（右栏页签）；本稿只**记账**并把需求钉死，
  实现落在 roadmap「对话流：产出物与文件页签」阶段——**不许留白到没人认领**（用户 2026-10-18 裁定）
- 不改后端事件、不改工具行为与权限判定 —— 只改「它做了什么怎么让你看见」
- 不照搬参考实现的配色，只借结构与动效；色值一律走 `--lw-*`
- 不动 `MessageRow` 的助手正文排版与 markdown 渲染（另案）

## 设计要点

一句话：**身份表驱动渲染**——38 个身份 × 5 个相位注册在 `desktop/src/lib/chat/render-matrix.ts`，
组件只按登记结果渲染，不再在模板里 if 工具名。行壳与展开体的几何同理，只认「统一模型」那张表。

已裁定的两处（原「待裁项」，2026-10-18 用户定）：

1. 语言徽章：**彩色 + 各语言专属 logo 字形**（材料图标主题素材，逐级回退），已按标准 §零.5 登记豁免。
2. 用户气泡圆角：按现役 `--lw-r-lg-plus`(18) 不动，下位「语境特例保留 18」有效。

仍待用户裁定（不阻塞其它批）：

1. ~~AF-02：标签列取宽口径~~ —— **已定**：行级 `max-content` + `column-gap: 8px`（2026-10-18 落盘，
   真实窗口实测每行标签↔值恒 8px；代价是值列不跨行对齐）。
2. AF-07：`edit_replace` 的 diff 行号——接受空列，还是让插件补行号（要动插件载荷）。
3. ~~AF-14：组头计时行与状态词是否并一行~~ —— **已定**（用户 2026-10-18「同意你的意见，收掉吧」）：
   轮计时读数收成一份（新件 `desktop/src/components/chat/TurnTimerReadout.vue`），落点二选一 =
   容器计时行 **或** 首个动作块的组头（见下表 AF-14 行）；**组头不自带组内耗时**。
4. AF-01 组头半边：过程组头（`TurnProcess.vue`）的三角是否也改成紧跟标签（现在是整行右对齐）。
5. WF-03 与 WF-01/02 是否合并成同一右栏页签族。

有长期影响的取舍（身份表位置、折叠状态存哪、台账口径）写进 `docs/adr/`，本文只给指针。

## 被否决的方案

- **逐文件打补丁**（`ProcessRow` 改 flex、`describe` 加 `clipName`、`kinds` 补几个名）：改了行改不了状态，
  改了状态漏身份表，漏一处就退回原形。这套做法已实测两轮，每次收口都留新的空行。
  2026-10-18 用户对「找出一条改一条」的否决记录：**这是这次改写 spec 的直接原因**。
- **换通用对话 UI 库**（assistant-ui 一类）：消息模型是 9 种 kind 的事件流投影，库契约是 role+content，
  接不上；且会撞「同类只许一种形态」。

## 计划（**按属性分批，不按案例**）

**批 A 先改标准（做任何一行代码之前）**：把「统一模型」那张表落进
`docs/design/authority/Front-end Design Guidelines.md`——行壳段序规则、三角位置规则、属性行两列规则、
状态位规则、组头同规则；并登记 §七 豁免（语言徽章已有）。**标准没落定，下面的批不许开工**（AGENTS.md §4）。

**批 B 行壳几何清账**（一处改，全部行/所有状态同时受益）：AF-01 三角位置 → AF-10 状态段左靠 →
AF-03 空段（已修，回归确认）→ AF-14 组头两行 → AF-06 组头并入同一几何。
**状态**：AF-01 / AF-06 / AF-10 / AF-14 已清（提交 `98d000b` + 批 A/B + 本刀）；AF-11 产出物行
**移入批 E**（它要 `file_changes` 事件的接线，属接线不属几何）。

**批 C 展开体清账**：**AF-17 外壳统一优先**——两条路径共用同一个底/圆角/内边距/起点 x，内部形态不再各带一层底；
这一条改完，「思考」与「读取」展开后必然同形（现在是一个灰框 vs 一串裸行）→ AF-02 属性行取宽口径 →
AF-15 截断口径收成一处 → AF-07 行号裁定 → AF-12 层 3 折叠 → AF-13 收起态安全三条。
**状态**：AF-17 / AF-02 / AF-15 / AF-12 / AF-13 已清（`98d000b` + 本刀）；AF-07 裁定为
「列在、号空」——插件载荷里没有行号（`plugins-src/tool-edit/src/replace.rs:110-121`
只回 `path/bytes/edits/additions/deletions/match_type/byte_range/existed/backup/revision`），
要补行号得先改插件载荷（另刀，归批 E 一并接线）；宁缺勿假，先留空列。

**批 D 状态与相位**：AF-04 状态词收进 `tool.state.*` 唯一落点 + 标准 §二⑰ 给表（✅ 本批）→ 失败唯一高亮复核 → AF-05 两套思考形态写进标准（✅ 批 A 已写）。剩余两态（已拒绝 / 已停止）**卡在数据与轮级裁定**，不属本批能清的范围——要动先翻 v2.98 的「decision 行全隐藏」与 v2.205 的「只标最新组」。

**批 E 接线做全**：WF-01…WF-05（**依赖 roadmap「对话流：产出物与文件页签」阶段**；
该阶段开工前先把它建成本稿的子规格或独立 spec，不许只在口头）。

**批 F 滚动与收口**（✅ 本刀）：AF-09 锚定策略已改（删 `overflow-anchor: none`，真实窗口实测注入内容不再拽视口）→
「布局落点唯一」机器判据已建（`scripts/check-css-ownership.mjs`，接进 `sh scripts/tasks.sh gates` 与 `selftest`）→
AF-15 残留复核（截断收成 `SUMMARY_CLIP` + 宽度交给 `text-overflow: ellipsis`）→ 标准 §六 第 5 条同步。
留待批 E 完后跑最终 `make check` 与 roadmap 指针同步（第 13 行仍指向已取代的 `specs/02-tool-cards`）。

**每批完**：先把当批的 AF 逐条在真实窗口量一遍（AC-9 的口径），给人看完再进下一批。

## 任务

- [x] 批 0 = C1 身份矩阵 + 行壳骨架（提交 `abe6ca5`）
- [x] 批 0 = C3 展开体形态库（提交 `7f9c221`）
- [x] 批 0 = C4 滚动两档 + 思考行收起 + `bulb` 字形（提交 `dfcfb49`）
- [x] 批 0 附带：语言徽章改听矩阵 `badge` 字段（提交 `575a0e7`）、属性并块与行壳空段（`862db71`）
- [x] 批 A 统一模型落标准（提交 `463bc0d`）
- [x] 批 B 行壳几何清账（提交 `463bc0d`）
- [x] 批 C 展开体清账（`98d000b` + 本刀：AF-17/02/15/12/13 已清，AF-07 裁定为列在号空）
- [x] 批 D 状态与相位（提交 `b1c76a1`：`tool.state.*` 唯一落点 + 标准 §二⑰ 词表；已拒绝/已停止 两态记明卡在数据面）
- [ ] 批 E 接线做全（依赖 roadmap 第 15 行那个阶段）
- [x] 批 F 滚动与收口 + 机器判据（本刀：`scripts/check-css-ownership.mjs` + AF-09 锚定修复）
- [ ] `make check` 全绿（批 E 做完后跑最终一次；本刀已跑 `sh scripts/tasks.sh gates` + 前端四闸全绿）

## 影响范围

| 文件 / 目录 | 改动 |
| --- | --- |
| `docs/design/authority/Front-end Design Guidelines.md` | 批 A：统一模型落标准（行壳段序 / 三角位 / 属性行 / 状态位 / 组头） |
| `desktop/src/lib/chat/render-matrix.ts` | 已建：38 身份注册表，唯一登记面 |
| `desktop/src/lib/tool-detail/kinds.ts` | 已删，并入 `render-matrix.ts` |
| `desktop/src/components/chat/ProcessRow.vue` | 已改 flex 行壳；批 B 收口三角位与状态段 |
| `desktop/src/components/chat/ToolRow.vue` `ThinkingRow.vue` | 状态弱化、语言徽章、收起默认；批 C 层 3 折叠 |
| `desktop/src/components/chat/ToolDetailView.vue` | 已建形态库；批 C 属性行取宽 |
| `desktop/src/components/chat/TurnProcess.vue` | 批 B 组头并入同一几何 |
| `desktop/src/lib/tool-detail.ts` `tool-detail/blocks.ts` `lib/chat/diff.ts` | 形态库与 diff 解析 |
| `desktop/src/style.css` | `--lw-lang-*`（素材自带色，不走 token）/ `--lw-diff-add/-del` |
| `desktop/src/locales/{zh-CN,en}/tools.ts` | 批 D：六态状态词与动词成对 |
| `scripts/check-render-matrix.mjs` | 已建：身份完备性闸 |
| `scripts/check-style.mjs` 或同族 | 批 F：「布局落点唯一」判据 |

## 风险与回滚

- 风险：38 身份 × 5 相位的矩阵一旦和真实事件流对不上，会出现比现在更多空行 —— AC-1 的闸是硬兜底
- 风险：**批 A 跳过就等于这次重做又变成补丁集**（用户已为此返工过一轮）；标准不落定不开工
- 风险：批 E 依赖另一个 roadmap 阶段，若那阶段不开工，WF-01/02/05 会一直挂着 —— 本节即为它存在的理由
- 挂账 / 遗留：`sandbox_run` 已随 v2.145 退役，但历史会话重放仍需渲染，保留在表里
- 回滚：每批一个提交，单批可回退；不动数据、不动后端
