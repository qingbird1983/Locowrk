# docs/design 文档索引（INDEX）

> **为什么有这份文件**：`docs/design/` 里同时躺着**现役规范**、**已兑现的历史设计稿**和
> **从未采纳/已退役的旧方案**，文件名与目录位置看不出区别。审计《CODE-AUDIT-v1》H9 判的
> 「照 DESIGN.md 实现会写出一套与代码相反的策略层」就是这件事的代价——而 AGENTS.md 明写
> 「改核心模块前先读对应章节」。本表回答一个问题：**这篇现在还算数吗？**
>
> **维护纪律（v2.149 批 E 起）**：新建或退役任何 `docs/` 下的 md，**同一批里改本表**。
> 状态词汇只有六个：`权威` / `参考` / `已兑现` / `待研究` / `对照留档` / `存档·未采纳`。

---

## 一、权威规范（改代码前必读；与代码冲突时，先对齐文档再动码）

| 文档 | 地位 | 现役版本 |
|---|---|---|
| [DESIGN.md](DESIGN.md) | 唯一需求来源：内核架构、ABI 契约、决策表、目录树 | 批 E 已把 §7.1 洋葱链、§7.2 决策表、§8 目录树、§11.3 装载机制、§3.5 接线列订正到代码实况（v2.149） |
| [ROADMAP.md](ROADMAP.md) | **唯一进度看板**（⬜/◐/✅ + 版本 + 哈希） | 现役 |
| [CHANGELOG.md](CHANGELOG.md) | **逐批落点日志**（每版一整行，倒序；本文件是"写了什么"的真源，不是进度表） | 现役。检索：`grep -n "^\*文档版本：v2.14" CHANGELOG.md` |
| [UI Design System.md](UI%20Design%20System.md) | **UI 层最高权威**（色彩令牌映射 / 字号相对制 / 圆角嵌套降档 / 组件规则 / 冲突裁决表；ZCode DESIGN.md 微调移植，用户 2026-10-11 裁定） | v1.0；⚠ 代码面未对齐，逐批兑现；下位两文档冲突处一律以其附录 A 为准 |
| [UI shell and patterns.md](UI%20shell%20and%20patterns.md) | 导航模型 / 模式库 / 事件流向 / 弹层规则（**降为下位细则**，与 Design System 冲突处以其附录 A 为准，修订待办见其附录 C） | v4.30；⚠ 现状表里若干键/命令已退役，**逐条已加批 E 退役注**（§193/§202/§883/§999） |
| [Front-end Design Guidelines.md](Front-end%20Design%20Guidelines.md) | 前端形态清单 / 特例申请制 / 双闸门禁（**降为下位细则**，同上） | v2.1 |
| [Memory Tree Design v2.md](Memory%20Tree%20Design%20v2.md) | 记忆树规范正身（章节捆 L0/L1/L2 / 整理触发 / 记录员） | v2.0（§4.1/§15 已随 v2.137 修订） |
| [KV Prefix Cache Design v2.md](KV%20Prefix%20Cache%20Design%20v2.md) | 前缀缓存纪律（钉死集 / 预取 / 稳定期） | v2.0；⚠ 稳定期随 ctx-compact 退役，见其 §6.4 |
| [Context and Memory.md](Context%20and%20Memory.md) | 上下文 + 记忆总纲（注入顺序 / 双层隔离） | §3.2.2/§4 已随 v2.136–v2.138 加注 |
| [Post-Audit Roadmap v1.md](Post-Audit%20Roadmap%20v1.md) | **后审计施工队列 = 下面「三、待研究」六份稿的实施顺序真源**（Q0–Q20 逐批登记：出处·稿内代号 / 前置 / 体量 + §1 裁定记录 + §5 锚点漂移订正）。ROADMAP 仍是唯一进度看板（打钩与哈希），本表管顺序与依据，两边不重复正文 | v1.0（2026-10-12 立，用户当日裁定六份稿全部实施） |

**专题参考（各管一轨，现役）**

| 文档 | 管什么 |
|---|---|
| [LLM.md](LLM.md) | 模型接入：单线协议、韧性重试、方言档位、多模态、子代理模型档（v3.7） |
| [plugins.md](plugins.md) | 必备能力清单与落地顺序；⚠ §一 已加**现役状态列**（v2.149），别再照旧表重建 sandbox/LLM 插件 |
| [Plug-in packaging.md](Plug-in%20packaging.md) | 插件元数据与打包 v2（manifest / 版本策略 §9.4） |
| [Tool exposure strategy and usage statistics.md](Tool%20exposure%20strategy%20and%20usage%20statistics.md) | 工具暴露三层 / EWMA / 闲置分级 |
| [Setting and Constrained Optimization.md](Setting%20and%20Constrained%20Optimization.md) | 设置项与约束最小化清单（§2.3/§2.7/§3 已随 v2.145 加修订注） |
| [Memory-KV Roadmap.md](Memory-KV%20Roadmap.md) | 记忆树 × KV 专项路线表（M0–M6 + F1–F3 **全表收口**，主 ROADMAP 不列本专项） |

---

## 二、已兑现设计稿（内容 = 当时的方案与裁定；实现偏差以文内「落地标注」为准）

这些文档**不是待办**。它们的价值是"为什么长成这样"的决策记录。

| 文档 | 兑现批次 |
|---|---|
| [NEW/Permission Model Redesign (ZCode Alignment) v1.md](NEW/Permission%20Model%20Redesign%20%28ZCode%20Alignment%29%20v1.md) | v2.145（五条裁定全落 + 实机走查） |
| [NEW/Prompt Assembly Layering Design.md](NEW/Prompt%20Assembly%20Layering%20Design.md) | v2.139（四批全落，偏差三条已标注） |
| [Organize Summary Design v1.md](Organize%20Summary%20Design%20v1.md) | v2.141（四层多分辨率记忆） |
| [Cross-Session Recall Design v1.md](Cross-Session%20Recall%20Design%20v1.md) | v2.142（批 1 工具面；**批 2 常驻进展行注入挂账未做**） |
| [NEW/Model Capability and Multimodal Phase Plan.md](NEW/Model%20Capability%20and%20Multimodal%20Phase%20Plan.md) | v2.127–v2.132（六批收官；G4 行已加 v2.145 订正注） |
| [NEW/Model Vendor Configuration and Capability Probing Design.md](NEW/Model%20Vendor%20Configuration%20and%20Capability%20Probing%20Design.md) | 批 A/B（v2.127–v2.128） |
| [NEW/Multimodal Model Support Design.md](NEW/Multimodal%20Model%20Support%20Design.md) | 批 D/E（v2.130–v2.131） |
| [NEW/Reliability Reinforcement Design.md](NEW/Reliability%20Reinforcement%20Design.md) | G1 随批 B、G2 v2.134、G3 v2.133、G4 v2.135、G5 v2.132；**G6/G7 记档未立批** |
| [edit_replace.md](edit_replace.md) | v2.114（容错匹配 + always 暴露） |
| [three-column responsive layout V2.md](three-column%20responsive%20layout%20V2.md) | v2.111–v2.112 |
| [NEW/Tool Execution Process Display.md](NEW/Tool%20Execution%20Process%20Display.md) | v2.117 |
| [NEW/language of thought.md](NEW/language%20of%20thought.md) | v2.116（思考语言设置） |
| [Memory File-ization Design.md](Memory%20File-ization%20Design.md) | F1–F3（v2.71–v2.73） |

---

## 三、待研究 / 未实施（**不是规范，别照它写码**）

> **本节六份稿已于 2026-10-12 全部裁定采纳并排期**（用户裁定「六份文档都是我规划出来的，肯定都要实施，
> 你帮我梳理先后排序」）⇒ 实施顺序、前置、已结掉的拍板结论一律看
> [Post-Audit Roadmap v1.md](Post-Audit%20Roadmap%20v1.md)（下称**队列**），本表只把状态标到位、
> 不复制队列正文。队列里的 Q 号是唯一无歧义的批次指代；**各稿自起的代号（M/L/C/T/P 系）跨稿会撞名**
> （详见队列 §2），裸写 `M1`/`C1`/`T1` 一律不认。下面每行给的简称（CB/MR/CF/FP/ML/MB）= 队列 §2 的出处简称。

| 文档 | 状态 |
|---|---|
| [NEW/Context Budget Closure Design.md](NEW/Context%20Budget%20Closure%20Design.md) | 原「待研究」⇒ **已采纳**。**简称 CB**，排期 = §7.5 P2 → **队列 Q4**、§7.1+§7.2 P0-A/P0-B → **Q6**、§7.3+§7.6 P0-C/P3 → **Q7**、§7.4 P1 → **Q8**。⚠ §7.6 死代码清单**已部分随批 C 兑现**（`CtxCompact`/`suggested_threshold` 早已摘掉），动手前逐条重验（队列 §5）；§12 三条待拍板已按可配置原则结掉（队列 §1）
| [NEW/Memory Redesign (ZCode Alignment) v1.md](NEW/Memory%20Redesign%20(ZCode%20Alignment)%20v1.md) | 原「待研究」⇒ **已采纳**。**简称 MR**，排期 = §4+§6 M1 → **队列 Q2**、§5 M2 → **Q3**（⚠ 用户 2026-10-12 裁定 **走提案 A** = 整理副产物落库产事实卡，同批须登记 v2.136 裁定的**部分反转**）、§8 M3 → **Q10 搭车**（涉用户数据目录，动手前单独确认）。§4.2 即**解冻《Cross-Session Recall v1》批 2** 的那一刀；§6 的 workspace 键位归一化**趁 `memories` 表 0 行的红利窗口已实测仍在**（2026-10-12），过期就变成迁用户数据 |
| [NEW/Conversation Flow Redesign (ZCode Alignment) v1.md](NEW/Conversation%20Flow%20Redesign%20(ZCode%20Alignment)%20v1.md) | 文首自述「状态：待研究」；对话流完整复刻设计（§2 行模型注入现有投影 / §3 分层折叠 / §4 工具卡全集 / §5 状态三源 / §6 产出物 / §7 文件右栏链路 / §10 分期 C1-C4），上位依据 = 《UI Design System》。原「待研究」⇒ **已采纳**。**简称 CF**，排期 = §10 C1 数据链路 → **队列 Q12**（⚠ 前置 = **Q11 前端测试基建**，队列自立的插入件，它与审计余项 T3「chat.ts 投影纯化」共用同一个洞）、C2 工具卡全集 → **Q17**（体量建议再切 2–3 刀）、C3 产出物与文件页签 → **Q18**、C4 打磨 → ⏸ 搭车（其中 Rewind 的前置 = 磁盘快照能力至今 RESERVED 无生产读写点，属前置未立不是要不要做）。⚠ 本稿的 `C1` 与 FP 稿的 `C1′` 是两件事，FP 那一撇正是为躲开它 |
| [NEW/Feature Parity Roadmap (ZCode Alignment) v1.md](NEW/Feature%20Parity%20Roadmap%20(ZCode%20Alignment)%20v1.md) | 文首自述「状态：待研究」；**功能面对齐总路线**（§3 差距矩阵 37 项 / §4 归宿原则 / §5 逐域落法 / §6 明确不对齐含账号裁定 / §7 候选批次七群 A-G **不排序待用户安排**）；斜杠命令、skills、hooks、终端、远控、Bots、headless 等缺口全在这；与记忆 M 批、对话流 C 批互引不重复。**简称 FP**，原「待研究」⇒ **已采纳、§7 的"不排序待用户安排"已排定**：群 A 全部 → **队列 Q19**（零依赖，任何时刻可插当调节阀）、依赖对话流 C1 的各条（B4/B5/B6/B7/C1′/C6/E1/E2/B1/B2/B3/B8/E3 + 群 D1/G1/B9/A5）→ **Q20**。该稿 §7 依赖要点点名 **CF 的 C1 是最大汇聚点**（B5 checkpoint / B8 分屏 / C1′ steer / E3 远控四样全踩它），故队列把 C1 排在功能面群之前 |
| [NEW/Memory Learning Mechanism (SME Alignment) v1.md](NEW/Memory%20Learning%20Mechanism%20(SME%20Alignment)%20v1.md) | 文首自述「状态：待研究」；**记忆学习机制**（§0 实证 Locowrk 记忆纯静态：全仓零 reinforce/hit_count/access_count，召回纯 `updated_at DESC`；§0.3 订正 `access_count` 两列从未存在于 schema / §2 前置 L0 召回可观测性 / §3 加两列 + 4 信号可解释排序 + 强化回路 / §4 不搬 SME 的 Region 与 PPR，SME 已自证伪 / §5 L0–L4 分批 / §6 已知盲区含「全局层 0 行可能无东西可学」）；参照系 Spatial Memory Engine（附录 A 证据表）；**与 Memory Redesign v1 分工不同**（那份治召回不到，本稿治召回之后不演化）。**简称 ML**，原「待研究」⇒ **已采纳**，排期 = §2.3 L0 拆两刀 → **Q1a ✅ v2.183（注入侧埋点 + `store_empty_ratio`）** / **Q1b ✅ v2.184（三档检索回执 + `injection_yield`）⇒ §2.3 的 L0 整项已落**；切刀理由与兑现载体 = 回执不能挂中间件（实测 `Next::tool_name/spec/args` 全 `pub(crate)`，宿主自制中间件看不见工具名），也不能挤 `UsageRecorder`（统计写入器兼管记忆观测，三问②答否）或给 `run_turn` 加参数（10 处调用点陪绑）⇒ 落在 **`KernelToolSource::resolve` 出口的 `Tool` 装饰器**（`recall_receipt.rs`，内置与插件两条通路一次覆盖）。按实测订正稿内两处：`memory_project_history` 结果**无 memory key**（(c) 的分子不能含它）、检索结果**无键位串**（只有层标 `scope`，故 `scope_id` 由解析器现算）——详见队列 §5。§3.2 L1 → **Q5**、§3.3+§3.4 L2/L3 → **Q9**（稿内明写同函数同锁可合批）、§3.1 L4 → **Q10**。⚠ **两处须先处理**：①§2.3(e) 给的落点 `crates/kernel-agent/src/middleware/usage.rs` **该文件不存在**（Q1a 已按真身实施：`RecallLog` 自持连接、旁挂进 `desktop/src-tauri/src/usage.rs` 用的同一个 sqlite 文件，不复用 `UsageDb` 任何语句），`usage.sqlite3` 真身 = `desktop/src-tauri/src/usage.rs`（队列 §5）；②§7 五条待拍板已按可配置原则结掉（队列 §1），其中"权重初值照此、有读数后再调"与"§2.3(d) injected-vs-used 本批不做"两条照稿内建议采纳 |
| [NEW/Memory Tree Board Design v1.md](NEW/Memory%20Tree%20Board%20Design%20v1.md) | 文首自述「状态：待研究」；记忆面板重构（§0 项目移除级联与 v2.113 冲突调和待拍板 / §2 投影四层对位零新存储 / §3 无限画布只读看板交互规格 + 形态特例申请 / §5 一键记忆目录 / §6 落码清单 T1-T3）；配套原型 = [ui-prototype/memory-board.html](ui-prototype/memory-board.html)。**简称 MB**，原「待研究」⇒ **已采纳**，排期 = §7 特例标准修订 → **队列 Q13**、§2.3+§6 看板 T1（含唯一实质后端新增 `memory_tree_stream`）→ **Q14**、§4+§5 T2 → **Q15**、§6 T3 → **Q16**。⚠ 本稿的 `T1–T3` 与《AUDIT-FIX-PLAN-v1》§7 的 `T1–T14` **完全无关**，同名不同物（队列 §2 立了「Q 号 + 出处稿 + 稿内代号」三元写法专治它）。该稿两处待拍板已由用户 2026-10-12 结掉：无限画布属**特例设计、批准落地**（按规矩 Q13 先修订标准再写码）；§0 项目移除级联走稿内推荐 = **默认不勾**保 v2.113 非破坏语义、勾选才物理删两目录 |

---

## 四、对照留档（有意保留的旧版，以 v2 为准）

| 文档 | 说明 |
|---|---|
| [KV Prefix Cache Design.md](KV%20Prefix%20Cache%20Design.md) | v1.0，v2 文档明写「原文保留作对照，两文档并存，以本文为准」 |
| [Memory Tree Design.md](Memory%20Tree%20Design.md) | v1.0，同上 |

---

## 五、存档·未采纳 / 已退役（`archive/`，只作方向史）

| 文档 | 为什么在这里 |
|---|---|
| [archive/Sandbox Execution Design.md](archive/Sandbox%20Execution%20Design.md) | **CubeSandbox 集成路线从未采纳、从未实施**（全仓零引用、无相关代码与依赖）；且沙箱主题整体已随 v2.145 退役。v2.149 批 E 移入 `archive/` 并在首行加两行定性 |
| [icon replacement plan.md](icon%20replacement%20plan.md) | Tabler Icons 替换方案**未采纳**：现役图标 = 自绘 `desktop/src/lib/icons.ts`（lucide 风格 16 格，`grep -ri tabler desktop/src` = 0 命中）。留在原位，改图标前先读它了解曾考虑过的方案，别当待办 |

---

## 六、工单与台账（按目录归位，状态行须与自身处理记录一致）

| 文档 | 状态 |
|---|---|
| [BUG/环境已知限制.md](BUG/环境已知限制.md) | **长期维护**：所有"外网工具失败 / shell 命令奇怪行为"先对照本文 |
| [BUG/Tool Layer Issues Todo List.md](BUG/Tool%20Layer%20Issues%20Todo%20List.md) | 已处理（v2.116），唯余「shell 跨平台策略」⏸；原「待处理」状态行与 §六 自相矛盾，批 E 订正 |
| [BUG/bug-report.md](BUG/bug-report.md) | Bug 1/2 已修复转存档；**Bug 3 部分处理**（开放两支登记批 F·F12），原「待上报」标签已订正 |
| [../reports/CODE-AUDIT-v1.md](../reports/CODE-AUDIT-v1.md) | 2026-10-10 全仓审计（证据台账在 `_parts/`） |
| [../reports/AUDIT-FIX-PLAN-v1.md](../reports/AUDIT-FIX-PLAN-v1.md) | **审计修复执行计划 = 逐批证据台账**（批 A–E 已落，F/G/H 排队） |
| [../上下文与记忆工作流程报告.md](../上下文与记忆工作流程报告.md) | as-built 现状报告（v2.135 生成 + 逐批修订注；§1.2/§1.3 已按代码重写为七监听器） |
| [ui-prototype/](ui-prototype/) | UI 静态原型（方向参考，不是像素合同）：index.html = 桌面壳全态；memory-board.html = 记忆树看板交互稿（配套《NEW/Memory Tree Board Design v1.md》，复用 css/tokens.css） |

---

## 七、目录结构约定

```
docs/design/
  DESIGN.md / ROADMAP.md / CHANGELOG.md / INDEX.md   四个顶层件：规范 / 进度 / 落点 / 本表
  Post-Audit Roadmap v1.md   第五个顶层件（v2.182 立）：后审计施工队列 = 六份稿的实施顺序真源（Q 号）
  NEW/                    近几轮的设计稿（兑现后不搬走，靠本表标状态）
  BUG/                    工单与限制清单
  archive/                已退役 / 未采纳（含 AGENTS-progress-archive.md = 历史进度流水）
  ui-prototype/           静态原型
```

`archive/` 不是垃圾桶：进这里的文件都带**首行定性注**（未采纳还是退役、现役替代品指在哪）。
历史进度流水不再写进 `AGENTS.md`（会话注入源，65,536 字节预算），改写进
[archive/AGENTS-progress-archive.md](archive/AGENTS-progress-archive.md)。
