# Locowrk 记忆树 × KV 前缀缓存 专项路线表

> 版本：v1.0（2026-10-03）

> **定位**：本表是"记忆树 + KV 前缀缓存"专项的**唯一进度表**，完成打钩（⬜/◐ → ✅ + 版本 + 提交哈希）。

> **与总路线表的关系**：[`roadmap/README.md`](../roadmap/README.md) 是唯一的阶段进度表，本专项**待并档进它**（本表自称"唯一进度表"与那条冲突，两份进度表改一处必漏另一处）；本表的未完成项抄进总表后，本表整体归 `docs/history/`。本表各批完成时的细节写在**提交正文**，不再另建逐批流水。

> **设计依据**：《Memory Tree Design v2》《KV Prefix Cache Design v2》《Memory File-ization Design v1.1》（F 轨）。实施与文档冲突时**先回到 v2 文档对齐**，不凭猜测实现、不改文档迁就实现。

---

## 一、批次纪律（每批通用，沿用 [AGENTS.md](http://AGENTS.md)）

* 动手前先列"将新建/修改哪些文件、各放什么"清单；单文件 400 行上限（`check_length`）
* 门禁：测试绿（workspace + desktop）、clippy 0 双侧、fmt、`pnpm build` 过；UI 批加 CDP 实机走查
* UI 变更批 **UI shell 文档先行**再动码；后端为前端服务，冲突时改后端
* 可配置原则：数值进设置页给合理缺省；开关按用户/开发者两层归位（两份 v2 文档的「可调项总表」是唯一清单）
* 走查数据清理：走查会话/设置改动/测试记忆全部还原
* 每批一个提交，完成即勾本表 + CHANGELOG 落账

---

## 二、阶段总览

| 阶段 | 名称 | 一句话 | 依赖 | 回退方式 |
|---|---|---|---|---|
| M0 | 观测先行 ✅ v2.63 | 命中率仪表 + 省钱面板（无此不能判定实验） | — | 纯增量展示，可整体关闭 |
| M1 | 前缀纪律兑现 | stable/dynamic 分段 + 工具集会话钉死 + 记忆预取确认 | 建议在 M0 后（立刻可见命中率变化） | tools.pinSession 等开关逐项关回 |
| M2 | 记忆树地基 | 树 crate + 记录员 + /整理 + 降级 + 崩恢复（影子运行：只写不接管） | M0 | 删投影重来，零损失 |
| M3 | 只读呈现 ✅ v2.69 | 记忆页树视图（只读）+ 指示器 + 整理通知 | M2 | UI 开关隐藏 |
| M4 | 按会话切流 ✅ v2.70 | 旁车 organizer mode + ContextAssembler + 尾巴模式 A/B + 护栏 | M0–M3 | 会话级 mode 拨回 legacy，老会话天然兼容 |
| M5 | 数据裁决与剔除 ✅ v2.123 | 缺省切 tree、experiences 关缺省、树区块编辑/删除正式化、tree.md 人改回写 | M4 数据积累 | 全部保留为降级路径，可回拨 |
| M6 | 差异化完善（可并行） | 断裂诊断 / Anthropic 断点 / 标题模糊匹配 / 项目 L0 联合视图 | 各自独立 | 各自独立开关 |

### 记忆文件化轨（F 轨，2026-10-05 立项，设计 v1.1 定稿；与 M 轨并行互不等待）

| 阶段 | 名称 | 一句话 | 依赖 | 回退方式 |
|---|---|---|---|---|
| F1 | 双写与导出 ✅ v2.71 | memory_write / 整理收束双写 .md + 首启导出存量；组装零变化 | 设计 v1.1 定稿 | 删 \~/.Locowrk/memory/ 零损失 |
| F2 | 人的编辑面 ✅ v2.72 | reconcile（人编辑 md 生效）+ 记忆页「打开文件夹」+ <repo>/.Locowrk/ 交接（gitignore + handoff/requirements 注入组装） | F1 | 总开关关闭 |
| F3 | 主动记忆 ◐ v2.73（⚠ v2.136 部分退役：autoExtract 删、handoff 后台改写删，指引块改节点提醒钩子——见 ROADMAP v2.136） | ~~autoExtract 缺省开 + handoff 收束更新~~ + 提示词主动指引（改写） | F1（F2 的编辑面更好但非硬依赖） | 开关可退 |

**总回退原则**：树是投影、组装是投影、事件流唯一真相源——任何阶段验证失败都只拆投影，不动原文。M5 之前不存在不可逆动作。

---

## 三、批次明细

### M0 · 观测先行 ✅（v2.63，429f726）

**目标**：让缓存命中率从"不可见"变"可见可算"——这是后续所有阶段用数据说话的前提（KV v2 §7）。

**落账**：命中率数据源零新增链路（消费 v2.58 `LlmUsage`）；`chat.sessionUsage` 会话累计（实时/重放同源）；浮层「会话累计」行 + 开发者「前缀缓存读数」面板 + 告警开关（缺省关，重放不判）；5 设置键 + 测试 2 例。走查：构造 jsonl 重放逐字段对上手算、真实轮（deepseek-flash）轮 93% vs 累计 77% 分离正确。**简化决策**：单价为全局一对（0.02/1.0），按 provider 覆盖延后——需要 LlmUsage 带 provider 归因，随 M6.2 一并评估。

**落点（模块级，动手前细化文件清单）**：

* desktop `runner.rs` / 用量聚合：消费 `LlmUsage.cache_read / input`，算轮命中率与会话累计（**分母口径 = 纪律 7**：分母不含 cache_write，总账对齐）
* 上下文用量浮层加一行"缓存命中率"（用户可关，缺省开）
* 开发者分区面板：本轮命中/未命中/总输入、会话累计、节省金额估算（内置单价表 + 按 provider 覆盖，用户层）
* 命中率告警开关（开发者，缺省关）
* zh/en 文案；测试：聚合口径回归（含"分母不加 cacheWrite"）

**验收**：真实轮（DeepSeek）浮层显示命中率且与日志字段手算一致；单价覆盖生效；关闭开关后浮层恢复原样。

---

### M1 · 前缀纪律兑现 ✅（v2.64，8b87de3）

**目标**：兑现 KV v2 §4/§5 的组装纪律（现网 ToolExposure 每轮 Top-N 是头号杀手，必须先钉死，否则 M4 的树模式 A/B 数据全部失真）。

**落账**：三中间件同批兑现——ToolExposure **会话钉死**（首请求确定暴露集缓存于会话侧，后续轮按名字序从当前目录回映射；围栏收紧 = 交集，`tools.pinSession` 开发者开关缺省开，关 = 恢复每轮 Top-N）；Memory-Inject **会话级预取**（带会话身份的请求首次注入时检索一次并缓存块文本，`memory_write` 中途写入 / 经历查询逐轮变化不再改写 system 前缀；top_k 变更重算；None 身份回落逐轮）；ctx-compact **稳定期**（结算后 N 个用户轮内不再触发，轮次锚点 = 用户消息数，`context.stabilityRounds` 1–10 缺省 3）。现状核查：tools 只走请求 `tools` 字段不进 system ✅、system 无日期等动态注入 ✅（纪律 1/3/5 现网本就合规，本批只加回归护栏）。两开关 UI 同挂开发者分区（JSON 分属 tools/context 区，UI shell v4.4）。**走查实抓存量缺陷修复（v2.58 起）**：runner 收敛判据读 `events().last()`——LlmUsage 紧贴 assistant 落日志后 last() 恒为 LlmUsage，报告用量的供应商**每轮多跑一步**（请求以 assistant 结尾，DeepSeek thinking 模式 400 拒收，每轮尾部假 error 项）；改从尾部找最近一条 AssistantMessage 判 tool_calls + 回归 2 例。测试：kernel-agent 68 绿（+7：钉死 3 / 预取 4，含**相邻轮请求前缀逐字节一致**集成断言）、桌面 148 绿（+2 收敛回归 + 2 设置键）；实机走查（CDP，真实轮 deepseek-flash）：memory_write 审批通过真实写入后，turn2 命中率仍 93.8%（预取失效 = 从第 0 字节全灭）、turn3 93.4%、累计 93.9%，全程 0 error 项；开发者分区两行渲染 + 开关/数值往返落盘；走查会话已删、memory.sqlite3/experiences.sqlite3/usage.sqlite3/notifications.json/settings.json 全部还原走查前备份、应用已重启干净实例。

**落点（模块级，动手前细化文件清单）**：

* kernel-agent：system stable/dynamic 分段梳理；动态信息（日期等）移消息尾部追加区；**工具 schema 不镜像进 system**（本批核查：现网本就合规，不虚改）
* kernel-agent ToolExposure：exposure 决策改为**会话首步计算一次 + 会话侧缓存**，后续轮复用；`tools.pinSession` 开关（开发者，缺省开）
* Memory-Inject 对照纪律 2 核查：确认/改造成会话级预取一次（会话内字节不变）
* 稳定期纪律：结算后 N 轮（缺省 3）不触发改前缀操作
* 测试：同一会话相邻轮请求前缀（system/tools/记忆块）hash 一致断言；pinSession 关闭恢复旧行为

**验收**：真实会话命中率对比（钉死前 vs 后，走查记录落 CHANGELOG）；`search_tools` 拉取新工具不改变已暴露集；权限切换仍接受失效。

---

### M2 · 记忆树地基（影子运行） ✅（v2.65，0ae85a3）

**目标**：树只写不接管——现网会话照常 ctx-compact + memory-sqlite，树并行记账，验证整理质量与工程件（树 v2 §11.2 阶段 1）。

**落账**：新 crate `crates/memory-tree`（bundle / tree / index / trigger / material / recorder / storage 七模块 + prompts 两层目录 8 文件编译期内嵌，33 测试）——MemoryBundle（L0/L1/L2 + pending/source 追加字段）、EventToBundleIndex、触发纯函数（批量阈值 / 增量块，锚点优先 LlmUsage 估算兜底）、记录员输出校验（**切分区间必须按材料项完整铺满**，垃圾整批拒绝转规则切分）、规则切分（复用 ctx-compact「切点落在 User 之前」纪律不拆工具对）、(workspace, session) 键位原子存储（scope 编码 + 坏投影当不存在，整树重建兜底）。desktop `organize/` 编排（mod / client / debug / recovery 四文件）：决策（plan_auto / plan_manual，含反弹熔断账本）与执行（超时 × 重试 → 校验 → 降级 → 落树）分离，记录员客户端按档位路由（recorder_provider 整档换 / 模型名覆盖 / 同主模型，未配置报错走降级不落演示替身）；轮次收束钩子挂 agent_chat + 调度引擎双入口（fire-and-forget）；启动 `recover_pending` 重投（会话已删 = 终局放弃记 aborted，尾巴留原文流）。**影子纪律**：请求组装零改动，记录员调用直呼 GenAiLlm 不进会话日志（盘证 llm_usage 每轮恰好 1 条）。`/整理`（composer 前端拦截 = SystemNotice 反馈不建气泡）+ `organize_now` / `memory_tree_view` 命令；设置键：memory 区三键（organizePacing / organizeChunkTokens / recorderModel+recorderProvider，**空串 = 清除**——走查实抓双层 Option 的 null 在 serde_json 里与字段缺失不可区分、清除语义失效，弃用改约定）+ developer 区五键（熔断 k/m / 记录员超时 × 重试 / 规则切分块）+ [settings.rs](http://settings.rs) `project()` 拆 [view.rs](http://view.rs) 守门禁。UI shell v4.5 先行（§3.36 新节）。测试：memory-tree 33 + 桌面 156（+9：organize 全链路 7 + 设置往返 2）绿、workspace 361 绿、clippy 0 双侧、pnpm build 过。实机走查（CDP，真实轮 deepseek-flash）：/整理 拦截反馈 → 真实记录员 2.1–11.5s 整理（**捆 b3 summary 把用户指令「永远用 pnpm 不要用 npm」引号原文逐字保留 = §5.6 实证**，真实 memory_write 工具往返如实记录）→ 增量自动触发（reason=incremental_chunk，7 轮长回答堆过 10K，6 捆 / 游标 34 / recorder ok）→ 批量阈值触发（窗口临时 16000，anchor 3596 ≥ 3200 → reason=threshold）→ 反弹熔断（k=1 内再触发 rebound=1=m → paused=true 跳过整理）→ 手动 /整理 不受限复位 → 崩溃重投（杀进程 → 构造 pending 投影 → 重启日志「重投在途整理任务」→ recorder ok、占位 b9 撤除换真捆）→ 降级（bogus 记录员模型 API 400 → mode=rule/status=degraded、捆照建游标照推）→ 开发者面板 / 模型区记录员行 / 记忆区节奏行全渲染 → 命中率无回退（92.6% / 81.1% / 92.5%，记录员独立调用不进会话流）。**预置隐患记录**：`LlmUpdate.window_tokens` 的「null = 清除」（v2.40）同款双层 Option 缺陷，待单独批修。走查会话已删（走查前旧会话还原）、memory-tree/ 投影已清、settings / memory / experiences / usage / notifications 五文件字节还原、应用重启干净实例。

**落点**：

* 新 crate `crates/memory-tree`：MemoryBundle / MemoryTree / EventToBundleIndex / 树存储（**(workspace, session)** 键位，对齐 28·A）✅
* 阈值触发（复用现网 10–90% 滑条 + 占用锚点优先 LlmUsage）+ **`/整理` 手动命令** ✅
* 异步记录员：`prompts/{main,recorder}/{default,claude,deepseek,qwen}.md` 两层目录 + 模型匹配 + 会话级预取纪律；**摘要逐字保留用户指令**（§5.6）✅（提示词内嵌逐字保留纪律，走查实证）
* 占位捆持久化 + 崩溃恢复（重启重投 / 整树重建兜底）✅
* 降级规则切分（复用 ctx-compact 切点逻辑）+ 反弹熔断 k/m ✅
* 设置：记录员模型行（同主模型 ▾）、整理节奏批量/增量、增量块 X ✅
* 整理节奏/锚点/记录员成本进开发者可观测 ✅（memory_tree_view 只读面板）

**验收**：真实长会话阈值自动整理 ✅ + `/整理` 即时整理 ✅；捆标题/摘要人工抽查（含用户指令逐字保留）✅；杀进程后在途任务重投 ✅；记录员不可用降级规则切分 ✅；影子运行期间现网命中率无回退 ✅。

---

### M3 · 只读呈现 ✅（2026-10-05，v2.69，90dba21）

**目标**：树可见但不干扰——记忆页只读展示 + 对话内提示（树 v2 §12）。**UI shell 文档先行批。**

**落账**：UI shell v4.6 先行（§3.37 新节 + §4.1/§4.2 扩表）。**记忆页双区块**：「记忆库（事实）」= 现有 memory-sqlite 列表只加区块头（行为零变化，头在 ListPage 外空列表也显示）；新区块「记忆树（情景）」（`components/memory/TreeSection.vue` 自持状态）= 会话切换器（`memory_tree_sessions` 扫投影目录按 mtime 降序，缺省选中当前会话否则首项，行 = 自动标题·项目·捆数，未归属跳过项目段不留空段）+ 捆列表（▸/▾ 折叠 + L0 标题 + 相对时间；展开 = L1 摘要全文 + 来源徽章 recorder/rule 如实 + **看原文** = `memory_tree_original` 与记录员材料同形同源 build_items，按 event_range 磁盘切片行内展开）+ 本地搜索（标题+摘要 contains，不搜原文）+ 空态「还没有整理记录」不发明数据 + 零写操作（编辑/删除属 M5）。**上下文指示器**：用量浮层「记忆树：N 捆 · 未整理尾巴 ≈X tokens (Y%)」行——打开拉取 `memory_tree_view` 一次不轮询，有树才渲染，窗口未知省略 %。**整理系统消息 + 熔断如实告知**：后端两事件（`lw://organize-done` 手动/自动/恢复三路收束同源 emit、`lw://organize-paused` 仅 paused_auto 翻转瞬间发），Organizer 持 `OrganizeNotifier` 窄接口闭包（**走查实抓环境缺陷修复**：`Emitter::emit` 符号会把 tao 窗口栈拉进桌面单测二进制，无 Common-Controls v6 清单的进程从 System32 拿 comctl32 5.82 缺 SetWindowSubclass/TaskDialogIndirect，加载即 STATUS_ENTRYPOINT_NOT_FOUND——emit 调用收敛到 state/mod.rs 组装闭包，单测不引用 state 即不拉 tao；cargo 1.98 无测试作用域 link-arg 指令，清单方案弃用）；前端 chat store 订阅，非当前会话忽略；done → 「📦 已整理 N 轮为 M 捆 \[查看\]」（degraded 走如实降级文案），\[查看\] = 跳记忆页（SystemNotice 既有 action 机制）；paused → 「自动整理已暂停（反弹熔断）+ /整理 指引」。**测试**：memory-tree 34 绿（+1 扫描清单）、桌面 168 绿（+1：视图/清单/原文切片盘读全链）、workspace 39 套件绿、clippy 0 双侧、fmt、pnpm build 过；check_length 报告 7 文件超建议值（5 个 v2.66–68 已裁定 + 本批 organize/mod.rs 483 编排单职责、state/mod.rs 412 组装点——三问皆是不拆）。**实机走查（CDP 真实轮，deepseek 主模型兼任记录员）**：/整理 拦截（无用户气泡）→ 📦 已整理 3 轮为 1 捆 \[查看\] → 点击跳记忆页 → 双区块/选择器/捆行 → 展开捆（摘要逐字引用 + 记录员徽章 + 原文行 \[0\]用户…\[13\]助手 带事件位）→ 搜索命中/不命中空态/收起 → 阈值自动触发（临时窗口 8000+阈值 10%）→ done 第二条（users=5 bundles=2）→ 反弹熔断 paused 通知（如实 + 指引）→ 暂停态手动 /整理 照常运行且复位 → 用量浮层「记忆树 1 捆 · 未整理尾巴 ≈141 tokens 0%」→ 双主题截图全对 → **零干扰**（六轮 = 6 条 llm_usage，记录员 4 次真实调用零入日志；浮层命中率 93% 照常）。**搭车修复 v2.68 缺陷**：MemoryPage.vue 行 key 拼接写入了字面 NUL 字节（源文件变二进制，工具链拒读），改 `\\\\u0000` 转义（运行时语义不变）。走查清理：走查会话已删、memory-tree/ 投影已清、settings 逐字节还原（窗口/阈值/k/m/主题五处临时改动全部回退）、应用重启干净实例。**已知缝隙（记档不属本批）**：未归属会话不进侧栏树（31 批严格口径）→ 行菜单物理不可达，删除只能走后端命令——随侧栏/⌘K 批次补。

**落点**：

* 记忆页：新增"记忆树（情景）"区块（只读：展开捆/看摘要/看原文/搜索），现有 memory-sqlite 列表归入"记忆库（事实）"区块 ✅
* 上下文指示器加"未整理尾巴 X%"行 ✅
* 整理系统消息（`📦 已整理 N 轮为 M 捆 \\\\\\\[查看\\\\\\\]`）+ 熔断如实告知 ✅
* UI shell 文档 §3.x 新节先行 ✅（v4.6 §3.37）

**验收**：CDP 走查树展开/收起/原文查看/搜索/双主题 ✅；影子运行下 UI 与现网组装零干扰 ✅。

---

### M4 · 按会话切流 ✅（2026-10-05，v2.70，15dc938）

**目标**：树模式成为会话级可选项，A/B 数据开始积累（树 v2 §11.2 阶段 2）。

**落账**：UI shell v4.7 先行（§3.38 新节 + §3.2/§3.6/§3.12/§4.2 扩表）。**会话整理模式**：旁车 `<id>.meta.json` `organizer: legacy|tree`（session-store `write_organizer`，`session_list` 透出；`session_set_organizer` 命令写穿 + `OrganizerModeStore` 缓存即时生效）——**未显式设置会话跟随开发者缺省档**（`developer.defaultOrganizer` 缺省 legacy，保存即热换，permission_mode ㉘·B 同款「只存显式选择」）；任务行 `…` 菜单「切换为记忆树整理 / 切换为标准整理」按**生效档**渲染标签，切换即时生效、事件流零改动。**树模式组装**（树 v2 §7.1）：kernel-agent 新 `TreeContextSource` 契约（内核定契约、组装点实现，headless 不注册 = legacy 语义不变）——`step_inner` 按计划把消息切片到未整理尾巴（`Session::derive_messages_from` 带偏移投影；切点半轮时兜底前进到第一条 User 防协议非法），`tree_directory` 随 `RequestPayload` 进决策环、新中间件 **TreeDirectoryInject**（注册序 ToolExposure → Memory-Inject → TreeDirectoryInject → ctx-compact）把 L0 目录追加到 system 尾部——**目录格式 = 稳定头行 + 追加清单行、无闭合标签**（kernel 测试实抓：闭合标签随追加移动破坏字节前缀，违反纪律 3）；占位捆不进目录；**树会话 ctx-compact 直接跳过**（`tree_directory` Some 即标记，压缩语义由整理边界承担）。**尾巴二档**（§7.2）：A 边界结算缺省（`bundled_until` 排他游标）；B 固定窗口实验档 = `max(bundled_until, 最近 N 轮起点)`，窗口滑出的孤儿轮由**溢出整理**（reason=window_overflow）及时归捆、先于熔断判定且**不记反弹**（例行维护非烧钱信号）——防「孤儿轮既不在目录也不在尾巴」的静默丢失。**memory_expand / memory_collapse**（宿主内置工具，§8）：expand Pure + exposure=always（level 1 摘要 / 2 原文材料同形，展开 = 工具结果追加缓存安全；`expanded_at` 记账投影）；collapse Compensable + Hidden、**设置开关缺省关**（§8.3 宁缺勿假），收起 = expand 结果换桩文本（协议配对保全）；目录可见性会话门控双保险（KernelToolSource 列表过滤 + resolve 拒，legacy 不可见不可调）；**自动收起**（阈值缺省 80%）挂在 after_turn：占用 ≥ 阈值收起最早展开捆、一次一个。**用量口径**：树会话 `agent://context-usage` 的 used = 组装后估算（目录 + 尾巴），不再报全量历史。**召回埋点**：用量浮层「记忆展开 N 次」行（前端从转录工具行统计，>0 才渲染）。**走查实抓缺陷修复 ×2**：①目录闭合标签破坏前缀（见上，设计格式修正 + kernel 回归）；②**M1 钉死集不吸纳新增 always 工具**——会话中途切树模式后 memory_expand 被「按建立时名单回映射」丢弃（模型如实报告「没有这个工具」并用 search_tools 三次验证），修 = 回映射时目录内 always 工具无条件保留、追加尾部（旧集逐字节保留 = 前缀不破；钉死钉的是 on_demand 截断决策，always 直通是 ⑩ 语义）+ 回归测试。**测试**：memory-tree 36 绿（+2 展开收起生命周期/serde 兼容）、session-store 26 绿（+2）、kernel-agent 82 绿（+8：切片/目录次序/半轮兜底/收起桩/树会话跳压缩/legacy 零变化/目录前缀/钉死吸纳）、desktop 187 绿（+19：模式存储 2/组装计划 5/工具 3/溢出与自动收起 3/设置 3/门控组装 3）、workspace 39 套件全绿、clippy 0 双侧、fmt、pnpm build 过；check_length 报告 9 文件超建议值（三问自检皆「是」保留：5 个 v2.66–69 已裁定 + kernel/tests.rs 单主题测试、organize/mod.rs 整理编排单职责、organize/tests.rs 同、state/mod.rs 组装点）。**实机走查（CDP 真实轮 deepseek，主模型兼任记录员）**：新会话 → 行菜单切树（toast + 旁车 `organizer:"tree"` 盘证）→ 真实轮 jsonl 全量事件照常（组装不改日志）→ `/整理` recorder ok 1 捆 → 📦 通知条在场 → **树目录进请求实证**（模型原话「对话开头给我的目录只有一行」）→ 诱导 memory_expand 真实调用（补口修复后模型原话「我错了，memory_expand 确实存在」+ b2 摘要如实转述）→ 用量浮层「记忆树 1 捆 · 未整理尾巴 ≈3.4K tokens 0%（单 ≈）」+「**记忆展开：本会话 1 次**」→ 用量环按组装后口径（used ≈3.2K ≪ 全量）→ **命中率数据**：树模式稳态 94–96%（切模式/切工具集瞬间一次计划内失效 0% 后回落），legacy 并行会话 95%/93% 无回退 → 切走切回重放一致（33→33 items）→ 尾巴策略 window 落盘往返 → 开发者缺省档 tree 落盘 → 新会话缺省生效（菜单标签「切换为标准整理」）→ 缺省切回 legacy **热换翻转**（未选择会话标签即时翻转）→ legacy 会话工具目录无 expand/collapse（门控 ✓）。走查清理：走查会话 5 个已删（行菜单误触把用户旧会话 organizer 置 tree 已当场还原字段）、memory-tree 走查投影已删、usage/notifications/schedule 还原备份、settings 差异仅本批 5 个新键（值=缺省，字段级缺省零迁移）、memory/experiences 逐字节未动、developer.unlocked 已锁回、应用重启干净实例。**已知边界（记档）**：模式 B 溢出整理单测覆盖未实机走查（触发链路同 M2 已验）；钉死集与切模式的目录放宽交互依赖补口（回归已锁）。

**落点**：

* 会话旁车 meta +`organizer: legacy | tree`（缺省 legacy，老会话零迁移）；会话缺省档进开发者设置 ✅
* `ContextAssembler`（kernel-agent `TreeContextSource` + TreeDirectoryInject）：目录（L0 只增不改）+ 尾巴策略 **A 边界结算（缺省）/ B 固定窗口** + 本轮消息；树会话**不注册 ctx-compact**（运行时跳过）✅
* 护栏：记录员连续失败 → 规则切分接管（M2 既有）；反弹熔断树会话生效；模式 B 溢出整理不记反弹 ✅
* `memory_expand` 暴露（树会话，exposure=always）；`memory_collapse` 开关（缺省关，含自动收起阈值）✅
* 设置行落地（§15 可调项总表的用户层项：尾巴策略/窗口 N/增量块/collapse）✅
* 召回质量埋点（memory_expand 调用统计进用量面板）✅

**验收**：树会话与 legacy 会话并行互不干扰 ✅；老会话重放/切走切回一致 ✅；模式 A 命中率显著高于模式 B（走查数据记录）——**模式 B 未实机对比（记档：A 档稳态 94–96% 达标，B 留实验期真实使用数据）**；树会话命中率不低于 legacy 基线 ✅（94–96% vs 93–95%）；实验判据五项（命中率/成本/记录员开销/召回抽查/反弹次数）开始出数 ✅（命中率与节省已入浮层与开发者面板；记录员开销在 M2 台账；反弹计数在投影台账）。

---

### M5 · 数据裁决与剔除 ✅（2026-10-07，v2.123）

**目标**：用 M4 攒的数据做切换决定（树 v2 §11.2 阶段 3）。原明文前置条件「M4 数据复盘报告完成并经用户确认」由用户 2026-10-07 指令「继续吧 把这块完善了 清账」兑现——A/B 判据数据（命中率 A 稳态 94–96% vs legacy 93–95% 达标、记录员开销 M2 台账、反弹计数投影台账、召回走查实证）随批落 CHANGELOG v2.123。**ctx-compact 退役为降级路径一条改记档**：树会话跳过压缩已是现状，legacy 会话压缩路径原样保留 = 天然降级路径，无需代码变动（规则切分继续复用其实现）。

**落账**：见 CHANGELOG v2.123 全文。四件：**① 缺省切 tree**（`memory.defaultOrganizer` 缺省 tree，developer 键退役，老会话旁车档永久兼容）；**② experiences 关缺省**（`memory.experiences` 缺省关，注入 + 提取双门控，观察一个版本再定去留）；**③ 树区块编辑/删除正式化**（§12.1 + 纪律 5：编辑标题/摘要打「手动」标记自动层永不覆盖、删除捆两步确认仅摘投影游标不动；`memory_tree_edit_chapter`/`memory_tree_delete_chapter` 两命令 + TreeSection 编辑面）；**④ [tree.md](http://tree.md) 人改回写**（新 `memory_tree_reconcile.rs`：文件新于投影才解析、range 指针回配、只吸纳标题/摘要、回写打手动标记；reconcile 命令/启动/记忆页挂载三触发）。搭车：组装点四处 TreeStore 收敛 `AppState.tree_store` 单例。

**落点**：

* 会话缺省 organizer 切 tree（转用户层设置）✅（ctx-compact 退役 = 记档不改码，见上）
* experiences/向量层关缺省（保留开关观察一个版本；`search_tools` 嵌入基建不动、另行决策）✅
* 记忆页双区块正式化（编辑标题/摘要/删除捆开放）✅（+ [tree.md](http://tree.md) 人改回写 = F2 记档挂账同批兑现）
* A/B 复盘数据落 CHANGELOG ✅（v2.123 判据五项段）

**验收**：缺省切换后全量回归绿（workspace 39 套件 + desktop 231）✅；老会话按旁车 mode 永久兼容 ✅（显式档优先，回归锁在 organizer_mode 测试）；可整体回拨 legacy ✅（设置行热换）。实机走查 CDP 16/16 PASS（独立数据根，编辑/删除/回写全链盘证）；走查数据已清。

---

### M6 · 差异化完善（可并行，各自独立）——1/4 落地，1/4 记档闭账，2/4 挂账

| 项 | 内容 | 依赖 | 状态 |
|---|---|---|---|
| M6.1 断裂诊断 | 分块 hash 链（system 子块/tools/逐消息）+ 断点定位面板 + 告警（KV v2 §7.4，ZCode 未做的差异化） | M0、M1 | ✅ v2.124 |
| M6.2 Anthropic 断点 | stable 段末 ephemeral 断点 + 末消息增量锚点 + skipCacheWrite（KV v2 §8.3/纪律 6） | 有 Anthropic provider 需求时 | ⛔ 记档闭账（见下） |
| M6.3 标题模糊匹配 | L0 标题向量 top-5（树 v2 §10.2，阶段末可选） | M4、向量基建保留 | ✅ v2.124 |
| M6.4 项目 L0 联合视图 | 记忆页项目维度跨会话捆视图 + 模糊匹配候选池（树 v2 §11.4） | M4 | ✅ v2.124 |

**M6.2 记档闭账（v2.124）**：KV v2 §8.3 的断点打法建立在「适配层内部策略、不外露 trait」（§8.2）上，而 v2.75 LLM 兼容层重构已裁定**单线协议**——Anthropic 走官方 OpenAI 兼容端点（自动缓存，兼容层不暴露的扩展字段宁缺勿假是 v2.75 明文边界）。断点的前置 = 直连原生 `/v1/messages`，属「有 Anthropic provider 需求时」的 LLM 层批（届时 skipCacheWrite 一并做，纪律 6 不变）；在现行单线协议上硬做 = 破坏宁缺勿假裁定，**记档闭账不开工**。

**挂账（记档待用户）**：①模式 B 溢出整理实机走查——单测已覆盖（window_overflow_organizes_orphans_without_rebound / overflow_respects_pending_and_legacy_mode），实机触发需真实长会话 + B 档切换，随实验期真实使用自然验证；②未归属会话删除入口——M3 记档的「不进侧栏树 → 行菜单不可达」缝隙，⌘K 会话行现只切换，删除入口的 UI 形态待用户裁定后随侧栏/⌘K 批补。

---

### F 轨 · 记忆文件化（《Memory File-ization Design v1.1》，与 M 轨并行）

**定位**：记忆从「sqlite 机器索引」升级为「双层存储」——.md 是人类层与权威编辑面（人可查可读可写），sqlite/投影保留为机器索引（召回快路径）；布局 = `\\\~/.Locowrk/memory/`（全局 + 项目缓存记忆，上下文拼装源）+ `<repo>/.Locowrk/`（交接层：handoff / requirements / notes，自动引导 gitignore）。**主动记忆原则**（用户方向②）：模型主动记录不等提醒。

#### F1 · 双写与导出 ✅（2026-10-05，v2.71，faa5266）

**落账**：设计 v1.1 定稿 + F 轨立项同批先行（e9db32b，用户裁定四点全按推荐）。**共享布局 `plugin_api::memlayout`**（新模块，插件与宿主同一份实现）：项目文件夹 = `<可读名>-<fnv 短哈希>`（已裁定②）、key 文件名安全（Windows 保留名防护）、fact frontmatter（name / description / type / source / **workspace 原值** / updated——身份落盘供 F2 reconcile 读回）、[MEMORY.md](http://MEMORY.md) 索引一文件一行、总开关 env（缺省开）。**插件三落点双写**（memory-sqlite 0.1.1→0.1.2 小迭代）：write / delete / undo（还原→写、撤销删→删）sqlite 成功后 best-effort 同步 md，**失败不破契约**——输出 `md_warning` 字段如实。**宿主四点同步**：提取 write / 管理页 update·delete·clear。**[tree.md](http://tree.md) 整理收束双写**：会话 `##` 小节（旁车标题）+ 捆 `###` 数值序 + range 隐性指针 + **人改保护**（文件新于全部投影不覆盖）+ 未归属落 `\\\_unaffiliated`；Organizer 三个落树点后同步。**首启导出**：后台幂等（已有文件跳过，人编辑永不被覆盖；[tree.md](http://tree.md) 人改保护同款）；零数据零目录。**总开关** `memory.files`（缺省开）+ 记忆设置行（UI shell v4.8）+ 插件 env 热换。**搭车修复 M4 竞态（走查实抓）**：整理管线落树前重载最新投影（两路）——收起可在记录员调用期间落盘，旧快照回写抹掉 collapsed_at。测试：plugin-api 27（+5）、memory-sqlite 20（+1）、memory-tree 37（+1）、desktop 193（+6）全绿、clippy 0 双侧、pnpm build 过。实机走查（CDP 真实轮）：memory_write 过审批卡 → facts md + 索引落盘 → /整理 → [tree.md](http://tree.md)（三捆 recorder 产出）→ 设置行往返 → probe 行重启首启导出 facts=1 trees=0（幂等证明）。走查清理全还原（会话/行/memory 目录/投影/settings 字节）。

* F1 双写与导出：布局落地（memory_write/整理收束双写 + [MEMORY.md](http://MEMORY.md) 索引）；首启导出（幂等，md 已存在不覆盖）；总开关 + 组装/插件契约零变化 ✅
* 验收：真实轮 memory_write 后 md 文件在场且内容正确 ✅；存量导出幂等重跑不重复 ✅（trees=0 证明）；关开关回纯 sqlite ✅（单测锁 + 实机往返落盘）

#### F2 · 人的编辑面 ✅（2026-10-05，v2.72，1544345）

**落账**：**memlayout 拆目录模块**（[mod.rs](http://mod.rs) 布局 + [sync.rs](http://sync.rs) 同步机械件）——per-file mtime 标记（`.sync/<rel>.ts`，写删随行）、**原子写**（临时文件 + 同目录 rename，md/索引/标记并发不再撕裂）、`parse_fact`（frontmatter 宽松读回，name 权威）、**写路径冲突保护**（stash_human_edit：标记在而 mtime 已变且内容 ≠ 新渲染 = 人改未合并 → 先另存 `<key> · 人工编辑备份 <millis>` 再落新值——保留双方、时间戳新者占主键，设计 §3 同步纪律）。**reconcile 引擎**（desktop `memory_reconcile.rs` 新）：判定矩阵 = 内容等价（尾空白不敏感）补标记零噪声（人格式化不改写）/ 索引先行（标记 == mtime 而内容漂移 = 历史 md 同步失败）从索引愈合文件 / 文件为准更新·新增（tags·source 沿行，frontmatter updated 优先）/ 行在文件缺且标记在 = 人删单文件 → 删行刷索引 / scope 目录整体缺席 = 复位手势不删行（首启导出按索引重建）；人建的无 frontmatter 文件按项目目录名反查（known = fsRoots ∪ sqlite 键位）；MemoryStore 专用 `reconcile\\\_\\\*` 直写 SQL **绝不反向同步 md**。触发 = 启动（**先 reconcile 再导出**，顺序反了会把人删文件按旧索引写回）+ 记忆页挂载 + 开发者手动。**`<repo>/.Locowrk/` 引导与注入**（desktop `repo_context.rs` 新）：ensure_layout 幂等（建目录 + [requirements.md](http://requirements.md) 模板 + `.git` 存在时 `.gitignore` 追加一行——用户方向①「主动加入 git 排除」）；注入段 = requirements 全文（截 2000 字符）+ handoff 全文 + notes/ 文件名清单（按需用文件工具读），全空 = 一行提示段；kernel `MemorySource::repo_context` 缺省 None（headless 组装零变化），MemoryInject 在 `<workspace_context>` 之后、记忆块之前包 `<repo_context>`；**M1 预取兼容 = 指纹进 scope_id**（requirements/handoff/notes 的 mtime 签名）——人改文件 = 显式变更自动重算，键不变逐字节复用。**总开关** `memory.repoContext`（缺省开，记忆设置行）。**UI（shell v4.9）**：记忆页区块头「打开文件夹」（`memory_reveal_folder`，自有数据根不经 fsRoots 白名单）+ 挂载先 reconcile 再拉列表；开发者分区「记忆文件化」组「立即合并」按钮 + 上次合并计数行 + toast。**搭车**：`open_in_explorer` 自 reveal_in_explorer 抽出共用。测试：plugin-api 30（+3 标记/解析/冲突备份）、desktop 200（+7：reconcile 引擎 5 / repo_context 2）、kernel-agent 84（+2 注入次序/缺省零变化）绿、workspace 39 套件全绿、clippy 0 双侧、fmt、pnpm build 过、插件 memory-sqlite 0.1.2→0.1.3 重建（memlayout 副本带标记+stash+原子写）。**实机走查（CDP 真实轮 deepseek）**：挂起归属首轮 → `.Locowrk/` + requirements 模板 + `.gitignore` 行落盘 + 模型首轮即如实转述注入段（「requirements 由用户主编辑、模型只读、目前只有一行注释」）→ 人改 requirements 加金线 BAZ-42 → **模型禁用工具纯凭注入段逐字引用金线**（指纹失效重算实证）→ memory_write 过审批卡落 md → 人改 md 正文 → 记忆页挂载 reconcile 列表即显人改值（3 次重挂 3/3 确定性）→ **冲突 stash 实机**：人改未合并窗口内模型同键覆盖 → 主键 = 模型新值 + `f2.walkthrough…1791166959826.md` 备份文件保全人的版本 → 记忆页两行并存（备份按 frontmatter name 吸呐为独立行、正文保全）→ 「打开文件夹」开资源管理器无错 → 开发者「立即合并」toast 计数 + 上次合并行渲染。走查清理：会话删、记忆行删归零、`\\\~/.Locowrk/memory/` 移除、tmp 仓库删、settings 字节还原；**记档**：usage.sqlite3 工具级聚合计数器含走查增量（无会话归因不可外科删除，EWMA 衰减无害）；首轮记忆页挂载断言一次 NOT-FOUND 为环境干扰（实时窗口被切到设置页），三复验干净。**已知边界（记档）**：[tree.md](http://tree.md) reconcile（人改标题/摘要回写投影 + title_edited_by_user）随 M5 编辑面同一批落地——避免两处编辑回写实现；[MEMORY.md](http://MEMORY.md) 并发刷新以原子写缓解撕裂，语义收敛靠 reconcile。

* reconcile：启动 + 记忆页打开 + 手动命令——diff 文件 ↔ 索引以文件为准回写（冲突保留双方 + 时间戳）✅（写路径 stash + reconcile 吸呐备份）
* 记忆页「打开文件夹」；管理页编辑/删除同步写文件 ✅（F1 已落，本批加入口）
* `<repo>/.Locowrk/` 引导：首次创建自动补 .gitignore + handoff/requirements/notes 注入组装（M1 预取缓存键含 mtime）✅（notes 只注清单，全文按需工具读）

#### F3 · 主动记忆 ✅（2026-10-05，v2.73，d99624b）

**落账**：设计 §4 三机制全落 + 裁定③④兑现，UI shell v4.10 先行（§3.2 交接更新行 + autoExtract 缺省翻转说明 + §4.1 `lw://handoff-created` 事件行）。**① autoExtract 缺省改开（裁定④）**：Default 翻转，显式保存过 false 的存量不受影响（走查实抓：用户存量 `"autoExtract": false` 系旧版整区自动写盘幻影键，本机外科移除使缺省生效——其余存量安装同样受影响，显式 false 优先，随批披露）。**② handoff 收束自动更新（裁定③）**：desktop 新 `handoff.rs`——归属会话轮次成功收束（agent_chat + 调度引擎两收束点）fire-and-forget 一次 AppLlm 调用，读当前 [handoff.md](http://handoff.md) + 本轮转录（extract 转录口径复用），提示词要求全量改写（Current state/Next steps/Notes ≤40 行、保留用户仍有效内容、无变化回 UNCHANGED）；净化 = UNCHANGED/空不写 + 剥代码围栏 + 8000 字符硬顶；**内容未变不落盘**（mtime 不动 → scope_id 指纹不换 → M1 缓存不失效）；`ensure_layout` 幂等 + `memlayout::atomic_write`（仅写 [handoff.md](http://handoff.md) 一个文件 = 写边界宁严勿松）；**首次创建 toast**（`lw://handoff-created` → §3.22 toast 一次，更新不发生）；未归属/模型未配置跳过、失败只 warn；开关 `memory.handoffUpdate` 缺省开（记忆设置行 zh/en）。**③ 注入块尾主动记忆指引**：memory_inject `GUIDANCE_BLOCK` 随注入块尾（设计「随注入块」无独立开关）、**无条件注入**（零记忆新会话也要知道该自己记 = 主动记忆本意；memory.enabled=false 整块缺席）；文案 =「永远/以后都」升格 `scope="global"` + 重要决定当场记 + 更新既有 key 优先；静态文本与块同吃预取缓存。**搭车**：`memlayout` re-export `atomic_write` + [workspace.rs](http://workspace.rs) `repo_root_of` + repo_context `HANDOFF` pub + **F2 缺陷补：en.ts 缺 memoryRepoContext/Hint 两键补齐**。测试：kernel-agent 85（+1）、desktop 205（+5 handoff 纯函数）绿、workspace 36 套件全绿、clippy 0 双侧、fmt、pnpm build 过；check_length 14 超（12 前批已裁定 + memory_inject.rs 418/tests 579 三问不拆）。**实机走查（CDP 真实轮 deepseek）**：首轮模型自发 memory_write 记录重要决定（指引实证）→ autoExtract 缺省开实证（3 条 auto.\* 行零人工干预）→ .Locowrk/ 引导 + [handoff.md](http://handoff.md) 首建（三段如实）+ toast 实发 → 轮 2「以后都 pnpm」→ 模型自发 **scope="global"** 写入（升格实证，sqlite 落全局行）→ handoff 随轮更新且 toast 计数不增 → 设置行渲染 + 开关落盘往返。走查清理全还原（会话/四件备份/memory 目录/tmp 仓库/settings/fsRoots）。**已知边界（记档）**：机器重写 handoff 的注入可见性 = 下一请求（指纹重算），实质进展轮一次计划内缓存失效属设计 §3 内；[handoff.md](http://handoff.md) 人的编辑靠提示词保留语义承载（无事实层 stash）；演示替身不生成。

* F3 主动记忆：autoExtract 缺省开（裁定④）+ handoff 收束更新（裁定③，仅 `.Locowrk/` 内 + 首次创建 toast）+ 注入块尾主动记忆指引 ✅
* **⚠ v2.136 部分退役（用户裁定）**：autoExtract 整模块删除、handoff 后台收束更新整模块删除——项目记忆改由**主模型**在工作中自己读/写/改（记忆管理器只管当前上下文打包整理），软件只在 `<repo_context>` 静态提醒段给节点提醒钩子（开场先读 requirements/handoff；状态实质变化时主模型自行更新）；提示词指引块按新模型改写。经历记忆层（experiences，M5 关缺省观察的那层）一并退役——其唯一写入方就是 autoExtract 的 episode。
* 验收：真实轮模型自发 memory_write（决定当场记 + 「以后都」升格 global）✅；handoff 首建 toast + 随轮更新 + 内容未变不落盘 ✅；开关可退（handoffUpdate/autoExtract 两行设置）✅

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
| 钉死工具集后模型需要被截掉的工具 | search_tools 元工具按需拉取（现成）；pinSession 开关可关回 |
| 记录员质量不稳定 | 重试 + 规则切分降级 + 影子期人工抽查后才切流 |
| 树会话上下文失控（记录员连续失败） | 护栏：规则切分接管 + 熔断暂停自动整理 |
| 整理—反弹循环烧钱 | 熔断 k/m + 记录员成本进面板 |
| 模式 B 缺省误用导致命中率塌方 | 缺省 A；B 是显式实验档 |
| 剔除 ctx-compact 后想回头 | 它不删代码，降级路径常驻，随时回拨 |

---

---

*v2.137 追记（2026-10-09，上下文面重构批）：整理触发从「窗口百分比」改**绝对 token**（`context.thresholdTokens`，100K–800K 缺省 100K，用户裁定「达到多少 K 就触发」）——批量档 = `anchor ≥ threshold_tokens`，**窗口未知不再放弃自动触发**；ctx-compact **桌面链退役**（内核中间件保留、headless 不变；风障表「剔除 ctx-compact 后想回头」的回拨路径更新为：桌面组装走 git 回退即可，内核代码原样）；M4 旁车 `organizer` 键 / M5 `defaultOrganizer` 键随**标准整理整面退役**——一切会话皆树，旧数据里的存量键为幻影（serde 忽略），legacy 会话下次触发即整理整段历史（首个捆可能较大）。*

*本表由 2026-10-03 ZCode 机制调查 + 两份 v2 设计文档 + 用户探讨裁定派生；批次完成顺序可按实际情况在 M6 内并行调整，M0→M1→M2→M3→M4→M5 主线不建议跳批。*