# 阶段清单（唯一进度表）

> 这里只列**还没做完的阶段**：一行一个阶段简称 + 勾选标记 + 一句人话状态。
> 做完的从这里删掉：旧过程在 [`../history/`](../history/)（流水与旧表，只读 · ⊘ 不入库）或
> [`../design/archive/`](../design/archive/)（已兑现的设计稿 · ⊘ 不入库）里，该阶段的完成记录就是它那份
> [`../../specs/`](../../specs/) 计划里勾掉的复选框；版本号与提交哈希只在 git 里。
> 每个阶段"具体做什么、怎么算做完"在 [`../../specs/`](../../specs/) 里，一个阶段一个目录。
> 标记含义：⬜ 没开始 · ◐ 在做 · ✅ 做完 · ⏸ 停下等你定

## 阶段

| 阶段 | 状态 | 现在到哪了 | 详细计划 |
| --- | --- | --- | --- |
| 记忆与上下文重构 | ◐ 在做 | 方向 2026-10-17 定稿（决策记录 = [`ADR-0007`](../adr/0007-memory-flat-files-context-llm-compaction-primary.md)），**S1–S5 全部落地**：S1 数据面 = [`specs/05`](../../specs/05-memory-data-plane-s1/spec.md) 刀①–④d ✅；S2 压缩面 = [`specs/09`](../../specs/09-context-compaction-s2/spec.md) 刀①–⑧ 十二条验收全绿（挂账 ⓖ/ⓗ/ⓝ/ⓞ 已于 2026-10-24 注销）；S3 注入面 = [`specs/10`](../../specs/10-context-injection-s3/spec.md) 占用分子单出处 `occupancy` + 扣减链浮层 + 索引合并与 `<repo_context>` 真读交接层 ✅；S4 退役面 = [`specs/11`](../../specs/11-simple-compaction-memory-retirement/spec.md) 刀①–⑤ ✅（机械层与记忆整线删除、设置面数值归类、插件 9→8）；S5 压缩按 OpenCode V2 整块重写 = [`specs/13`](../../specs/13-opencode-compaction/spec.md) 刀①–⑨ ✅（决策 = [`ADR-0009`](../adr/0009-compaction-follows-opencode-v2.md)：主模型单链、阈值 = 窗口×百分比三档 25/50/85、压缩一发丝线就地展开 md、落盘口唯一 `persist_pending`、失败按档位归类）。**仍欠**：真机长会话一轮（主层 + spill、压缩三相真机触发，specs/12/13 的证据边界）与 spill 目录清理通路——两件都在「停下等你的事」 | [specs/01-memory-context-redesign](../../specs/01-memory-context-redesign/spec.md) |
| 对话流：渲染收口 | ✅ 做完 | [`specs/06`](../../specs/06-chat-flow-rendering/spec.md) 行壳统一 / 身份矩阵 / 展开体形态库与统一外壳 / 语言徽章 / 状态词收口全交；AC-3/9/10/11 四格 2026-10-24 真窗口收口（AC-9 首量抓出 **AF-19** 空 status 槽占位把三角距撑成两档，当场修 + 回归钉住）；[`specs/08`](../../specs/08-artifacts-and-file-tabs/spec.md) 产出物与文件页签批 1–5 全收、WF-01…05 全接；WF-03 小裁同日拍板——±N 徽章已补审阅入口、审阅签不并入 file 族 | [specs/06-chat-flow-rendering](../../specs/06-chat-flow-rendering/spec.md) |
| 工具级取消与 deadline | ◐ 在做 | C1+C2 已落地（C1 = 内核活动调用登记表 + 停止逐个置位；C2 = deadline 生产者落 Timeout 中间件，读数订正设计稿——审批钟不进 deadline，4 条用例 + 2 发变异；测量顺带坐实 ask_user 300s 守夜被 30s 全局掐死的同族缺陷，已登记等裁定）；三问 2026-10-18 由 Qoder 代裁定（可翻案）。**剩**：C3/C4/C5（要过 ABI，每一刀单独等你点头） | [specs/04-tool-cancel-deadline](../../specs/04-tool-cancel-deadline/spec.md) |
| 常用命令与技能包 | ⬜ 没开始 | 斜杠命令面板已随 v2.143 落地（`lib/slash-commands.ts` 注册表）；剩自定义命令、技能包、子代理预设、MCP 增强、钩子。开工时建 spec——先对齐现在「一切皆插件」与权限模型（v2.145）的形状再排 | 开工时建 spec |
| 子任务与后台任务 | ⬜ 没开始 | 后台子代理、运行队列、中途插话、终端与后台命令。开工时建 spec；子代理的超时与预算已由 v2.144 的 self-managed 预算先垫了一角 | 开工时建 spec |
| 度量基线 | ⬜ 没开始 | 性能优化要"快了多少"的基准数；原审计口径的"十二条优化"已随几轮重构漂移，开工时按现网重列要量的清单 | 开工时建 spec（= 台账 T9） |

## 遗留债收账清单（已实施、真留了债——你挑哪笔先收）

| # | 债 | 出处 | 规模与收法 |
| --- | --- | --- | --- |
| ~~A1~~ | ~~结构债 5 条：T25（`llm_provider_save` 业务下沉出命令层）/ T28（sessions 三组双出处并一处）/ T29（T6 注释机制更正）/ T37（`check-i18n` 间接键口径收紧）/ T39（api 两条错域条目归位）~~ | [specs/03](../../specs/03-structural-debt/spec.md) | **✅ 2026-10-24 收账结清**：T29 = 注释按实测改写 + 冗余点名行删除；T37 = 间接豁免按真键收紧（883→691/883，孤儿 7 逐字不变）；T39 = 新开 `api/shell.ts` 片归位两条；T28 = 三组双出处并一处 + 夹具逐字钉文案；T25 = save 业务下沉 `llm/profiles.rs`（+ 行为钉子五条）；全量门禁绿。T24/T34/T40/T41 已于同日注销（锚点消失） |
| A2 | 工具级取消 C2–C5 | [specs/04](../../specs/04-tool-cancel-deadline/spec.md) | **C2 ✅ 2026-10-24**（生产者落 Timeout 中间件；读数订正 = 审批钟不进 deadline；ask_user 同族缺陷登记）。**剩 C3–C5：要过 ABI 面，每刀单独点头** |
| ~~A3~~ | ~~压缩线剩三笔：摘要预算预置清单对账闸（②）/ `AppLlm` 第二实例收敛（⑤）/ 旧会话重放真机回归（⑥）~~ | [specs/13](../../specs/13-opencode-compaction/spec.md) 挂账表 | **✅ 2026-10-24 收账结清**：② = 对账闸 SS34（与 SS32 同形，变异打中）；⑤ = `AppLlm::compaction_view` 派生（同柄一个出处，`with_stop` 随之退役，逐柄判据）；⑥ = raw-JSON serde 判据 + 本机无旧会话的证据链 |
| ~~A4~~ | ~~S3 真窗口四格：AC-S3-1…4（索引指针行上界面 / 正文不注入 / mtime 即生效 / `<repo_context>` 读真实交接层）~~ | [specs/10](../../specs/10-context-injection-s3/spec.md) 验收区 S4 取代说明 | **✴ 2026-10-24 划销**：四格的标的物（索引注入块 / `<repo_context>` 交接层）已随 specs/11 刀③ 整线删除，specs/10 头部早已注明「不再执行」——本行是漏改的旧账 |
| ~~A5~~ | ~~交接层三件：`MEMORY.md` 写侧钳（ⓢ）/ `journal/` 按天封卷 / 轮收束模型主动更新提示词（ⓡ）~~ | [specs/10](../../specs/10-context-injection-s3/spec.md) 刀② 施工记录 | **✴ 2026-10-24 划销**：交接层本体已随 specs/11 刀③ 删除（`repo_context.rs` 不在、journal 全仓零命中），ⓢ/ⓡ 无标的物 |
| ~~A6~~ | ~~退役键 `compact_model` / `compact_instructions` 从结构体物理删除（现按退役键保留只为老配置可读；serde 忽略未知键 ⇒ 删了安全）~~ | `settings/context.rs` 头注 | **✅ 2026-10-24 收账结清**：两字段 + 缺省已删，头注照存原文；全仓引用面删前核过 = 仅该文件 |
| ~~A7~~ | ~~对话流收口四格（AC-3/9/10/11）+ WF-03 小裁~~ | [specs/06](../../specs/06-chat-flow-rendering/spec.md) | **✅ 2026-10-24 收账结清**：四格真窗口收口 + 顺手抓修 AF-19（空 status 槽占位撑出双档三角距）；WF-03 小裁同日拍板——① ±N 徽章已补成审阅入口（小刀）、② 不并入 file 签族 |

## 停下等你的事（不是技术选择，是要你点头或给方向）

| 事 | 为什么卡在这 |
| --- | --- |
| ~~ⓠ 注入埋点三选一~~ | **✴ 2026-10-24 划销**：观测面（`recall_log` / `inject_view`）已随 specs/11 刀③ 整线删除 = ⓠb 的彻底版，三选一出局（specs/10 头部补记） |
| 工具级取消 C2 / C3 先后 | C2 要先测现网两个超时的实际关系；C3 动 ABI——先后与开不开都要你点头（三问已代裁定，可翻案，见 specs/04） |
| 真机长会话一轮（烧额度） | 主层 + spill 的真机长会话、自动压缩三相真机触发（specs/12 证据边界）、specs/13 流式摘要真机全欠这一趟；触发线 = 占用 ≥ 有效阈值，要攒 token，什么时候跑由你定 |
| spill 目录清理通路 | 按"删会话 ⇒ 清 spill 子目录"建通路 = 动删除，要点头（specs/09 挂账） |
| 清磁盘上的旧记忆数据 | 旧记忆库残留 / `usage.sqlite3` 计数器 / `plugin-api` memlayout 无消费者——删除动作按规矩要点头（specs/11 挂账） |
| worktree 并发的三条自守规矩要不要写进 CONTRIBUTING | 「公共文件谁改谁当场提 / 只 `git add <路径>` 不用 `-A` / authority 件按文件认领」——一直在照做，落成规矩要你点头 |
| 界面英文档的一处显示选择 | 需要你实际看一眼再定 |
| 市场页的副作用徽章要不要改成运行期来源 | 能力看代码、展示看清单已定死（[`ADR-0006`](../adr/0006-plugin-metadata-authority.md)）；剩"未装载的卡片要不要显示工具清单与徽章"属界面信息多寡，要你定 |

## 2026-10-24 治理刀：失效账注销记录（防回潮，别照旧账开工）

- **ⓖ/ⓗ**（摘要模型 / 自定义压缩指令控件形态）→ specs/13 刀① 主模型单链后**无对应物**，`compactModel` / `compactInstructions` 已按退役键处理；specs/09 划销，roadmap 旧行摘除。
- **ⓝ/ⓞ**（微压缩稳态读数 / polaris 轮数口径）→ 机械层与 polaris 装置随 specs/11 刀① 退役；北极星「成本平坦」的现行证据通路 = specs/13 的真机长会话；specs/09 划销。
- **specs/11 刀⑤ 两笔观测挂账、specs/12 两笔缺陷挂账** → 已由 specs/12 刀②（`agent://compact-status` 四相）与 specs/13 刀⑦a/刀⑤ 兑现，两处划销。
- **specs/03 T24 / T34 / T40 / T41** → 锚点消失（LlmSection 直调 IPC 已为 0、memory 后端整线已删、DeveloperSection 已 S4 重建、第四批组件刀不开），全部划销；剩 T25/T28/T29/T37/T39 五条（= 收账清单 A1）。
- **roadmap 行摘除**：看板收尾（TreeBoard 已随 S4 刀② 整删，待走查的东西不存在了）、攒提交要不要 push（与远端已同步）、旧文档并档（本轮做完：Memory-KV 表按自身条款归 `docs/history/`，未完成项已处置）、「产出物与文件页签」重复行与已收口行（specs/08 全勾）、S3 走查行（刀①–④ 已落地且真窗口复核过）、S2 两条账行（ⓝ/ⓞ 注销，真机长会话并入上表）、章节编号差一行（树投影与看板已退役，取证面不存在；若另有所指你再点名）。
- **状态头对齐**：specs/02 → 已退役（被 specs/06 取代）；specs/13 → 已完成 · 刀①–⑨ 全收；specs/01 → S1–S5 现状 + 剩账。
- **文档归档**：authority 三份正身（Memory Tree v2 / Organize Summary v1 / Cross-Session Recall v1）随记忆树 / 整理线退役 + design 根七份已兑现或被取代的方案稿 → `docs/design/archive/`；Memory-KV Roadmap 与 Feature Parity Roadmap 两张旧表 → `docs/history/`；两处历史索引与 authority 清单（15→12）同步，`check-docs` 长行名单同步除名。
- **撞号修复**：specs/10 刀② 登记的「交接层写侧钳」由 ⓠ 改号 **ⓢ**（原号与刀③b 的注入埋点 ⓠ 相撞）。
- **收账清单对账补查（2026-10-24 第二批，A7 开工前查实）**：A4（真窗口四格）/ A5（交接层 ⓢ/ⓡ）/
  ⓠ（注入埋点三选一）三笔的标的物均已随 specs/11 刀③ 记忆整线退役而消失——specs/10 头部的
  S4 取代说明早已写明四格「不再执行」，roadmap 侧漏改；同日划销，并补 AGENTS.md 内置能力清单
  （去「记忆树工具」）。stage 行的「仍欠」随之收成真机长会话 + spill 清理两件（都在停下等你）。

文档三处分工：方案稿住 `docs/design/archive/`（⊘ 不入库）、计划与验收住 spec、决策与理由住 ADR。

## 三条使用约定

1. 想看某个阶段的完整来龙去脉：先翻这份清单那一行给的 spec，再翻 [`../history/README.md`](../history/README.md)（流水 · ⊘ 不入库）
   或 [`../design/archive/README.md`](../design/archive/README.md)（设计稿 · ⊘ 不入库）——这两层只在作者的机器上。
2. 想知道当初为什么这么定、否掉了什么：看 [`../adr/`](../adr)，一篇一个决策，只增不改。
3. 这里**不写**版本号、提交哈希、测试计数、门禁读数 —— 那些只写在提交记录里，需要时我给你查。
