# 已退役的设计稿（只读）

> 这一层放**已经落地或已被取代**的设计稿：规矩三条 —— 不再追加、不再改写、要查"当时怎么设计的"就翻这里。
> 现在有效的规范看 [`../authority/`](../authority/)，还有用的待办稿看 [`../`](../)，进度看 [`../../roadmap/`](../../roadmap/)。
> 这里没有结项状态可言：一件做完就从路线图删一行，旧过程留在它自己的文件里。

## 这里有什么（15 件）

每件都给两条证据：**文档自己的落地标注**，以及**代码里抽查到的实现落点**（本仓规矩：只信自述不算证据）。

| 文件 | 当年是什么 | 为什么归在这儿（落地标注 · 代码落点） |
| --- | --- | --- |
| `Approval Keyside Alignment and Rules Store Migration v1.md` | 项目授权规则库的键位归一 + 存量迁移 | 已兑现（Q7d · v2.199）· `permission_rules.rs::keyed()` 内 `wskey::normalize` + `{:08x}.json`，`tests_approval/keyside.rs` 有键位面判据 |
| `Key Unification and Legacy Scope Merge v1.md` | 记忆面三套键位合一 + 旧目录迁移 | 已兑现（v2.197 · `cfadccd`）· `workspace.rs` 委托 `wskey::normalize`、`memory-tree/storage.rs::scope_dir` 收归一键 |
| `In-turn Pressure Release Design.md` | 轮内水位检查 + 有界机械泄压 | 已兑现（v2.193 · Q8b，Q8c 余注随 v2.194）· `organize/backfill.rs` + `memory-tree/summary.rs`，泄压判定挂在每 step 的 `organize/assemble.rs` |
| `Context Budget Closure Design.md` | 上下文预算面的缺口清点 | 四个缺口就地全部标 ✅（v2.190 Q6 / v2.191 Q7 / v2.192 Q8 / v2.139）· `rerun_while_marked`、`clip_tool_output`、`instruction_files.rs` 双侧在位。⚠ 首行还写"待研究"，那是**立稿时状态**，正文逐条已订正 |
| `Permission Model Redesign (ZCode Alignment) v1.md` | 权限模型重设计（四档 + 七序决策表） | 全落 + 实机走查通过（v2.145）· `approval/mod.rs` 的子会话改道、`permissions/<hex>.json` 直读；`unlockRisky` 与硬禁令已整批删除 |
| `Prompt Assembly Layering Design.md` | 提示词分层装配（全局 / 项目 / 尾部碎片） | 四批全落（v2.139）· 宿主与 `kernel-agent/middleware/` 双侧 `instruction_files.rs` + `fragments/` 通路在位 |
| `Model Capability and Multimodal Phase Plan.md` | 模型能力与多模态的阶段计划 | 六批全部完成（v2.127–v2.132）· `llm/catalog.rs`、`dialect.rs`、`keyring.rs`、`recommend.rs`、`subagent_llm.rs`、`import/` 全套在位 |
| `Model Vendor Configuration and Capability Probing Design.md` | 供应商配置与能力探测 | 未做项一律标"记档不做"（OAuth 订阅 / 借池 / FIM / 自编辑映射）· `dialect.rs::deepseek_thinking`、`keyring.rs::KEYRING_MARK`、`catalog_match.rs::normalize_id` |
| `Multimodal Model Support Design.md` | 多模态输入支持 | v2.0 修订摘要逐条"退役/降级记档"，非目标明确排除输出图与音视频 · `session-store/content.rs` 的 `ContentPart` 只有 Text/Image/File 三种 |
| `Reliability Reinforcement Design.md` | 可靠性补强（乐观锁 / 悬空调用 / 执行证明） | 七机制全表收口（G 系列，G6/G7 记档不做）· `expected_revision`、`scan_uncertain` 与 `ToolExecutionUncertain`、`tool-shell/src/proof/` |
| `Language of thought`（文件名 `language of thought.md`） | "思考语言"配置项的任务提示词 | 要求全部可核：`thinking_language.rs` 内三份模板 `include_str!`、`settings.rs::thinking_language()`、waterfall 注入、中英文案成对 |
| `Tool Execution Process Display.md` | 工具执行过程展示重构 | 被上位《Conversation Flow Redesign》列为"已兑现基线稿（v2.117）"· `ProcessRow.vue` 的五列网格定义与本稿逐值吻合 |
| `three-column responsive layout V2.md` | 三栏响应式（非对称级联折叠） | 核心结论已进代码原文：`style.css` 明写"断点只决定展开模式，永不决定按钮可用性"，`use-responsive-shell.ts` 的 1400/800 双沿在位 |
| `KV Prefix Cache Design.md` | 前缀缓存 v1 | 被 `authority/KV Prefix Cache Design v2.md` 取代；v2 原文写"两文档并存，以本文为准"，v1 留作对照 |
| `Memory Tree Design.md` | 记忆树 v1 | 被 `authority/Memory Tree Design v2.md` 取代；v1 的核心名词已退役（`MemoryBundle` → `MemoryChapter`，压缩链随 v2.137 退役） |

## 读这批文件时的两个坑

1. **首行状态会滞后**。有四件（`Context Budget Closure`、`Memory Tree Board`、`Reliability Reinforcement`、`Model Capability`）
   的第一行还写着"待研究"，而正文里逐条标了落地版本。**以正文的落地标注为准**，别照首行判断它做没做。
   按规矩归档件不改写，所以这条提醒写在索引里而不是去改那些文件。
2. **行号与代码位置全部过期**。这些稿子当年写的是"v2.1xx 工作区"的文件与行号，只能当导航线索，
   要动代码前必须重新查当前位置——本仓反复出过"照旧行号实现结果落点已漂"的事。
