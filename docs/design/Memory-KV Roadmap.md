# Locowrk 记忆树 × KV 前缀缓存 专项路线表

> 版本：v1.0（2026-10-03）

> **定位**：本表是"记忆树 + KV 前缀缓存"专项的**唯一进度表**，完成打钩（⬜/◐ → ✅ + 版本 + 提交哈希）。

> **与主路线表的关系**：`docs/design/ROADMAP.md` 主表**不列本专项**（用户裁定：不污染主路线表）；本专项各批完成时勾**本表**，`docs/design/CHANGELOG.md` 照常逐批落账，DESIGN / UI shell 规范正文随批修订。

> **设计依据**：《Memory Tree Design v2》《KV Prefix Cache Design v2》。实施与文档冲突时**先回到 v2 文档对齐**，不凭猜测实现、不改文档迁就实现。

---

## 一、批次纪律（每批通用，沿用 AGENTS.md）

- 动手前先列"将新建/修改哪些文件、各放什么"清单；单文件 400 行上限（`check_length`）
- 门禁：测试绿（workspace + desktop）、clippy 0 双侧、fmt、`pnpm build` 过；UI 批加 CDP 实机走查
- UI 变更批 **UI shell 文档先行**再动码；后端为前端服务，冲突时改后端
- 可配置原则：数值进设置页给合理缺省；开关按用户/开发者两层归位（两份 v2 文档的「可调项总表」是唯一清单）
- 走查数据清理：走查会话/设置改动/测试记忆全部还原
- 每批一个提交，完成即勾本表 + CHANGELOG 落账

---

## 二、阶段总览

| 阶段 | 名称 | 一句话 | 依赖 | 回退方式 |
|---|---|---|---|---|
| **M0** | 观测先行 ✅ v2.63 | 命中率仪表 + 省钱面板（无此不能判定实验） | — | 纯增量展示，可整体关闭 |
| **M1** | 前缀纪律兑现 | stable/dynamic 分段 + 工具集会话钉死 + 记忆预取确认 | 建议在 M0 后（立刻可见命中率变化） | `tools.pinSession` 等开关逐项关回 |
| **M2** | 记忆树地基 | 树 crate + 记录员 + `/整理` + 降级 + 崩恢复（**影子运行**：只写不接管） | M0 | 删投影重来，零损失 |
| **M3** | 只读呈现 | 记忆页树视图（只读）+ 指示器 + 整理通知 | M2 | UI 开关隐藏 |
| **M4** | 按会话切流 | 旁车 `organizer` mode + ContextAssembler + 尾巴模式 A/B + 护栏 | M0–M3 | 会话级 mode 拨回 legacy，老会话天然兼容 |
| **M5** | 数据裁决与剔除 | 缺省切 tree、ctx-compact 降级化、向量层关缺省 | M4 数据积累 | 全部保留为降级路径，可回拨 |
| **M6** | 差异化完善（可并行） | 断裂诊断 / Anthropic 断点 / 标题模糊匹配 / 项目 L0 联合视图 | 各自独立 | 各自独立开关 |

**总回退原则**：树是投影、组装是投影、事件流唯一真相源——任何阶段验证失败都只拆投影，不动原文。M5 之前不存在不可逆动作。

---

## 三、批次明细

### M0 · 观测先行 ✅（v2.63，429f726）

**目标**：让缓存命中率从"不可见"变"可见可算"——这是后续所有阶段用数据说话的前提（KV v2 §7）。

**落账**：命中率数据源零新增链路（消费 v2.58 `LlmUsage`）；`chat.sessionUsage` 会话累计（实时/重放同源）；浮层「会话累计」行 + 开发者「前缀缓存读数」面板 + 告警开关（缺省关，重放不判）；5 设置键 + 测试 2 例。走查：构造 jsonl 重放逐字段对上手算、真实轮（deepseek-flash）轮 93% vs 累计 77% 分离正确。**简化决策**：单价为全局一对（0.02/1.0），按 provider 覆盖延后——需要 LlmUsage 带 provider 归因，随 M6.2 一并评估。

**落点（模块级，动手前细化文件清单）**：
- desktop `runner.rs` / 用量聚合：消费 `LlmUsage.cache_read / input`，算轮命中率与会话累计（**分母口径 = 纪律 7**：分母不含 cache_write，总账对齐）
- 上下文用量浮层加一行"缓存命中率"（用户可关，缺省开）
- 开发者分区面板：本轮命中/未命中/总输入、会话累计、节省金额估算（内置单价表 + 按 provider 覆盖，用户层）
- 命中率告警开关（开发者，缺省关）
- zh/en 文案；测试：聚合口径回归（含"分母不加 cacheWrite"）

**验收**：真实轮（DeepSeek）浮层显示命中率且与日志字段手算一致；单价覆盖生效；关闭开关后浮层恢复原样。

---

### M1 · 前缀纪律兑现 ✅（v2.64，8b87de3）

**目标**：兑现 KV v2 §4/§5 的组装纪律（现网 ToolExposure 每轮 Top-N 是头号杀手，必须先钉死，否则 M4 的树模式 A/B 数据全部失真）。

**落账**：三中间件同批兑现——ToolExposure **会话钉死**（首请求确定暴露集缓存于会话侧，后续轮按名字序从当前目录回映射；围栏收紧 = 交集，`tools.pinSession` 开发者开关缺省开，关 = 恢复每轮 Top-N）；Memory-Inject **会话级预取**（带会话身份的请求首次注入时检索一次并缓存块文本，`memory_write` 中途写入 / 经历查询逐轮变化不再改写 system 前缀；top_k 变更重算；None 身份回落逐轮）；ctx-compact **稳定期**（结算后 N 个用户轮内不再触发，轮次锚点 = 用户消息数，`context.stabilityRounds` 1–10 缺省 3）。现状核查：tools 只走请求 `tools` 字段不进 system ✅、system 无日期等动态注入 ✅（纪律 1/3/5 现网本就合规，本批只加回归护栏）。两开关 UI 同挂开发者分区（JSON 分属 tools/context 区，UI shell v4.4）。**走查实抓存量缺陷修复（v2.58 起）**：runner 收敛判据读 `events().last()`——LlmUsage 紧贴 assistant 落日志后 last() 恒为 LlmUsage，报告用量的供应商**每轮多跑一步**（请求以 assistant 结尾，DeepSeek thinking 模式 400 拒收，每轮尾部假 error 项）；改从尾部找最近一条 AssistantMessage 判 tool_calls + 回归 2 例。测试：kernel-agent 68 绿（+7：钉死 3 / 预取 4，含**相邻轮请求前缀逐字节一致**集成断言）、桌面 148 绿（+2 收敛回归 + 2 设置键）；实机走查（CDP，真实轮 deepseek-flash）：memory_write 审批通过真实写入后，turn2 命中率仍 93.8%（预取失效 = 从第 0 字节全灭）、turn3 93.4%、累计 93.9%，全程 0 error 项；开发者分区两行渲染 + 开关/数值往返落盘；走查会话已删、memory.sqlite3/experiences.sqlite3/usage.sqlite3/notifications.json/settings.json 全部还原走查前备份、应用已重启干净实例。

**落点（模块级，动手前细化文件清单）**：
- kernel-agent：system stable/dynamic 分段梳理；动态信息（日期等）移消息尾部追加区；**工具 schema 不镜像进 system**（本批核查：现网本就合规，不虚改）
- kernel-agent ToolExposure：exposure 决策改为**会话首步计算一次 + 会话侧缓存**，后续轮复用；`tools.pinSession` 开关（开发者，缺省开）
- Memory-Inject 对照纪律 2 核查：确认/改造成会话级预取一次（会话内字节不变）
- 稳定期纪律：结算后 N 轮（缺省 3）不触发改前缀操作
- 测试：同一会话相邻轮请求前缀（system/tools/记忆块）hash 一致断言；pinSession 关闭恢复旧行为

**验收**：真实会话命中率对比（钉死前 vs 后，走查记录落 CHANGELOG）；`search_tools` 拉取新工具不改变已暴露集；权限切换仍接受失效。

---

### M2 · 记忆树地基（影子运行） ✅（v2.65，0ae85a3）

**目标**：树只写不接管——现网会话照常 ctx-compact + memory-sqlite，树并行记账，验证整理质量与工程件（树 v2 §11.2 阶段 1）。

**落账**：新 crate `crates/memory-tree`（bundle / tree / index / trigger / material / recorder / storage 七模块 + prompts 两层目录 8 文件编译期内嵌，33 测试）——MemoryBundle（L0/L1/L2 + pending/source 追加字段）、EventToBundleIndex、触发纯函数（批量阈值 / 增量块，锚点优先 LlmUsage 估算兜底）、记录员输出校验（**切分区间必须按材料项完整铺满**，垃圾整批拒绝转规则切分）、规则切分（复用 ctx-compact「切点落在 User 之前」纪律不拆工具对）、(workspace, session) 键位原子存储（scope 编码 + 坏投影当不存在，整树重建兜底）。desktop `organize/` 编排（mod / client / debug / recovery 四文件）：决策（plan_auto / plan_manual，含反弹熔断账本）与执行（超时 × 重试 → 校验 → 降级 → 落树）分离，记录员客户端按档位路由（recorder_provider 整档换 / 模型名覆盖 / 同主模型，未配置报错走降级不落演示替身）；轮次收束钩子挂 agent_chat + 调度引擎双入口（fire-and-forget）；启动 `recover_pending` 重投（会话已删 = 终局放弃记 aborted，尾巴留原文流）。**影子纪律**：请求组装零改动，记录员调用直呼 GenAiLlm 不进会话日志（盘证 llm_usage 每轮恰好 1 条）。`/整理`（composer 前端拦截 = SystemNotice 反馈不建气泡）+ `organize_now` / `memory_tree_view` 命令；设置键：memory 区三键（organizePacing / organizeChunkTokens / recorderModel+recorderProvider，**空串 = 清除**——走查实抓双层 Option 的 null 在 serde_json 里与字段缺失不可区分、清除语义失效，弃用改约定）+ developer 区五键（熔断 k/m / 记录员超时 × 重试 / 规则切分块）+ settings.rs `project()` 拆 view.rs 守门禁。UI shell v4.5 先行（§3.36 新节）。测试：memory-tree 33 + 桌面 156（+9：organize 全链路 7 + 设置往返 2）绿、workspace 361 绿、clippy 0 双侧、pnpm build 过。实机走查（CDP，真实轮 deepseek-flash）：/整理 拦截反馈 → 真实记录员 2.1–11.5s 整理（**捆 b3 summary 把用户指令「永远用 pnpm 不要用 npm」引号原文逐字保留 = §5.6 实证**，真实 memory_write 工具往返如实记录）→ 增量自动触发（reason=incremental_chunk，7 轮长回答堆过 10K，6 捆 / 游标 34 / recorder ok）→ 批量阈值触发（窗口临时 16000，anchor 3596 ≥ 3200 → reason=threshold）→ 反弹熔断（k=1 内再触发 rebound=1=m → paused=true 跳过整理）→ 手动 /整理 不受限复位 → 崩溃重投（杀进程 → 构造 pending 投影 → 重启日志「重投在途整理任务」→ recorder ok、占位 b9 撤除换真捆）→ 降级（bogus 记录员模型 API 400 → mode=rule/status=degraded、捆照建游标照推）→ 开发者面板 / 模型区记录员行 / 记忆区节奏行全渲染 → 命中率无回退（92.6% / 81.1% / 92.5%，记录员独立调用不进会话流）。**预置隐患记录**：`LlmUpdate.window_tokens` 的「null = 清除」（v2.40）同款双层 Option 缺陷，待单独批修。走查会话已删（走查前旧会话还原）、memory-tree/ 投影已清、settings / memory / experiences / usage / notifications 五文件字节还原、应用重启干净实例。

**落点**：
- 新 crate `crates/memory-tree`：MemoryBundle / MemoryTree / EventToBundleIndex / 树存储（**(workspace, session)** 键位，对齐 28·A）✅
- 阈值触发（复用现网 10–90% 滑条 + 占用锚点优先 LlmUsage）+ **`/整理` 手动命令** ✅
- 异步记录员：`prompts/{main,recorder}/{default,claude,deepseek,qwen}.md` 两层目录 + 模型匹配 + 会话级预取纪律；**摘要逐字保留用户指令**（§5.6）✅（提示词内嵌逐字保留纪律，走查实证）
- 占位捆持久化 + 崩溃恢复（重启重投 / 整树重建兜底）✅
- 降级规则切分（复用 ctx-compact 切点逻辑）+ 反弹熔断 k/m ✅
- 设置：记录员模型行（同主模型 ▾）、整理节奏批量/增量、增量块 X ✅
- 整理节奏/锚点/记录员成本进开发者可观测 ✅（memory_tree_view 只读面板）

**验收**：真实长会话阈值自动整理 ✅ + `/整理` 即时整理 ✅；捆标题/摘要人工抽查（含用户指令逐字保留）✅；杀进程后在途任务重投 ✅；记录员不可用降级规则切分 ✅；影子运行期间现网命中率无回退 ✅。

---

### M3 · 只读呈现 ⬜

**目标**：树可见但不干扰——记忆页只读展示 + 对话内提示（树 v2 §12）。**UI shell 文档先行批。**

**落点**：
- 记忆页：新增"记忆树（情景）"区块（只读：展开捆/看摘要/看原文/搜索），现有 memory-sqlite 列表归入"记忆库（事实）"区块
- 上下文指示器加"未整理尾巴 X%"行
- 整理系统消息（`📦 已整理 N 轮为 M 捆 [查看]`）+ 熔断如实告知
- UI shell 文档 §3.x 新节先行

**验收**：CDP 走查树展开/收起/原文查看/搜索/双主题；影子运行下 UI 与现网组装零干扰。

---

### M4 · 按会话切流 ⬜

**目标**：树模式成为会话级可选项，A/B 数据开始积累（树 v2 §11.2 阶段 2）。

**落点**：
- 会话旁车 meta +`organizer: legacy | tree`（缺省 legacy，老会话零迁移）；会话缺省档进开发者设置
- `ContextAssembler`（新 crate 或 kernel-agent 模块）：目录（L0 只增不改）+ 尾巴策略 **A 边界结算（缺省）/ B 固定窗口** + 本轮消息；树会话**不注册 ctx-compact**
- 护栏：记录员连续失败 → 规则切分接管；反弹熔断树会话生效
- `memory_expand` 暴露（树会话，exposure=always）；`memory_collapse` 开关（缺省关）
- 设置行落地（§15 可调项总表的用户层项：尾巴策略/窗口 N/增量块/collapse）
- 召回质量埋点（memory_expand 调用统计进用量面板）

**验收**：树会话与 legacy 会话并行互不干扰；老会话重放/切走切回一致；模式 A 命中率显著高于模式 B（走查数据记录）；树会话命中率不低于 legacy 基线；实验判据五项（命中率/成本/记录员开销/召回抽查/反弹次数）开始出数。

---

### M5 · 数据裁决与逐步剔除 ⬜

**目标**：用 M4 攒的数据做切换决定（树 v2 §11.2 阶段 3）。**本批开工前置条件：M4 数据复盘报告完成并经用户确认。**

**落点**：
- 会话缺省 organizer 切 tree（转用户层设置）；ctx-compact 退役为降级路径（规则切分继续复用其实现，不删代码）
- experiences/向量层关缺省（保留开关观察一个版本；`search_tools` 嵌入基建不动、另行决策）
- 记忆页双区块正式化（编辑标题/摘要/删除捆开放）
- A/B 复盘数据落 CHANGELOG

**验收**：缺省切换后全量回归绿；老会话按旁车 mode 永久兼容；可整体回拨 legacy。

---

### M6 · 差异化完善（可并行，各自独立） ⬜

| 项 | 内容 | 依赖 |
|---|---|---|
| M6.1 断裂诊断 ⬜ | 分块 hash 链（system/tools/记忆块/逐消息）+ 断点定位面板 + 告警（KV v2 §7.4，ZCode 未做的差异化） | M0、M1 |
| M6.2 Anthropic 断点 ⬜ | stable 段末 ephemeral 断点 + 末消息增量锚点 + skipCacheWrite（KV v2 §8.3/纪律 6） | 有 Anthropic provider 需求时 |
| M6.3 标题模糊匹配 ⬜ | L0 标题向量 top-5（树 v2 §10.2，阶段末可选） | M4、向量基建保留 |
| M6.4 项目 L0 联合视图 ⬜ | 记忆页项目维度跨会话捆视图 + 模糊匹配候选池（树 v2 §11.4） | M4 |

---

## 四、实验判据（M4 数据积累 → M5 裁决输入）

| 判据 | 采集方式 | 合格线（建议，M4 期可修订） |
|---|---|---|
| 前缀缓存命中率（树·模式 A vs B vs legacy） | M0 面板 | 模式 A ≥ legacy 基线 −2pp |
| 每轮输入成本 / 节省金额 | M0 面板 | 模式 A 不高于 legacy |
| 记录员开销占比（token 与时长） | M2 可观测 | 会话总成本 +5% 以内 |
| 召回质量抽查 | memory_expand 埋点 + 人工出题 | 该找到的信息可找到 |
| 整理反弹次数 | 熔断计数 | 无连续熔断 |

> 建议观察窗口：M4 合入后真实使用 1–2 周再进 M5；数据不留情面，模式 B 若实测更优就翻缺省。

---

## 五、风险登记

| 风险 | 缓解 |
|---|---|
| 钉死工具集后模型需要被截掉的工具 | `search_tools` 元工具按需拉取（现成）；pinSession 开关可关回 |
| 记录员质量不稳定 | 重试 + 规则切分降级 + 影子期人工抽查后才切流 |
| 树会话上下文失控（记录员连续失败） | 护栏：规则切分接管 + 熔断暂停自动整理 |
| 整理—反弹循环烧钱 | 熔断 k/m + 记录员成本进面板 |
| 模式 B 缺省误用导致命中率塌方 | 缺省 A；B 是显式实验档 |
| 剔除 ctx-compact 后想回头 | 它不删代码，降级路径常驻，随时回拨 |

---

*本表由 2026-10-03 ZCode 机制调查 + 两份 v2 设计文档 + 用户探讨裁定派生；批次完成顺序可按实际情况在 M6 内并行调整，M0→M1→M2→M3→M4→M5 主线不建议跳批。*
