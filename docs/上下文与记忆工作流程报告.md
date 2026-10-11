# Locowrk 上下文与记忆完整工作流程报告（as-built）

> 生成于 2026-10-09，基于 v2.135（a5d8f96）代码实况盘点：四路并行调查（内核组装管线 / 记忆 KV 链路 / 记忆树管线 / 设计文档）汇总而成。
> **⚠ v2.136（同日）修订**：用户裁定记忆模型 v2——autoExtract（§3.2）、experiences（§3.2/§4.1 经历段）、handoff 后台自动改写（§3.3）**三机制退役**；注入面收敛「项目侧全提醒制」（§3.2 标注处已按新语义改写）。
> **⚠ v2.137（同日）修订**：上下文面重构——整理触发改**绝对 token**（§5.1 公式已改写，`context.thresholdPct/strategy/stabilityRounds` 与 ctx-compact 桌面链退役）；模型窗口自动识别（§3.1 窗口链改 `session_window` 四层链，不再只认手填档位）；**标准整理（legacy）整面退役**（§2/§3.4 的「legacy 会话」分支不复存在，一切会话皆树，旁车 `organizer` 键幻影化）；缓存命中/未命中单价两键与节省估算退役（§命中率观测保留）。
> **⚠ v2.138（同日）修订**：用量面统一真实锚点（runner.rs 新 `reported_input_tokens` 共享锚点）——`record_usage` 写的 `context_usage.used` 改**供应商真实 input 优先**（最后一条 `llm_usage.input`；无上报才回落树组装/字符估算），用量环/阈值进度条/浮层「已用」与 §5.1 触发锚点、自动收起从此同一把尺（「看到的 = 触发用的」）。背景：压力实测 UI 显 110K（估算 +18%）而真实 95,483 < 100K 阈值不触发、进度条 112% 全红照常不整理。
> **⚠ v2.139（同日）修订**：提示词分层拼装（《Prompt Assembly Layering Design v1》四批全落）——system 前缀新增**静态身份层**（kernel BaseSystemInject 六节：优先级金字塔「背景不是议程」+开场禁令）；waterfall 重排（思考语言上移、树目录殿后——§3.2 注入序全变）；`<workspace_context>` 升级 **`<environment>`**（+日期/平台，标签名即 §3.2 所述注入段名变更）；三注入块措辞按需化（§3.2/§3.3 的「开场先读」语义已改条件触发）；新增**指令文件层**（全局/项目 AGENTS.md → 合成首条 user 消息）与**尾部碎片**（整理完成/Plan/到顶 → 最后一条消息尾 system-reminder）。本报告 §3.2 注入清单以本标注为准，全文重写随下一版。
> **⚠ v2.149（批 E，2026-10-11）修订**：§1.2 管线图与 §1.3 监听器表按代码实况重写为**七监听器现役序**（`state/mod.rs:215-300` 逐行核过）——原表把已退役的 `CtxCompact` 列为现役第 ⑤ 步、且漏列 v2.139 新增的 BaseSystemInject / InstructionFilesInject / FragmentInject 三件；`MemoryInject` 行按 v2.136/v2.139 现役口径改写（`<environment>` 标签、只取全局层、经历段退役）；「规则切分降级」条的 ctx-compact 措辞改为「纪律源自它、现由树侧自持」（本体已摘）。本报告是 as-built 现状报告，凡「文档描述现役机制」类失真按批 E 逐条清零，不再只加修订注。
> 行号以当前工作区为准，仅供导航；与设计文档冲突时以文档为准（AGENTS.md 纪律）。

---

## 〇、一张图看两条链路

```
用户消息
  │  runner::run_turn：TurnStart / UserMessage 落盘（每 step append_slice 增量写 jsonl）
  ▼
内核 step_inner（crates/kernel-agent/src/kernel.rs:181）
  │  消息投影 derive_messages（事件流 → 消息列；树会话先经 TreeAssembler 切「目录+尾巴」）
  ▼
waterfall agent/request 决策环（监听器顺序 = 注册序，state/mod.rs:215-300）
  ① ToolExposure          工具目录打分截断 + 会话钉死（tools.pinSession）
  ② BaseSystemInject      静态身份层（v2.139：六节身份/工具策略/安全边界，system 字节最前缀）
  ③ MemoryInject          记忆/提醒段注入 system（会话级预取，M1）
  ④ ThinkingLanguageInject 思考语言提示（按会话首请求锁死）
  ⑤ TreeDirectoryInject   L0 目录追加 system 尾（树会话；殿后 = 只增前缀不断缓存）
  ⑥ InstructionFilesInject AGENTS.md 指令文件层（全局 + 项目根，快照锁字节）
  ⑦ FragmentInject        尾部碎片（整理完成 / Plan 模式 / 轮次到顶，轮开场取件一次即清）
  ▼
AppLlm::prepare（档位→方言 clamp→输出钳制→工具图片投影）→ CompatLlm（SSE + 无限次重试 + 180s 看门狗）
  ▼
响应落盘（Thinking → AssistantMessage → LlmUsage）→ 工具执行（先落 ToolCall 再执行 → 审批门 → 中间件链 → ToolResult）
  ▼
轮收束钩子（异步 fire-and-forget，不阻塞下一轮）
  ├─ 记忆树整理：after_turn → 触发判定 → 占位章节 → 记录员整理 → 章节落树 → 目录增长（下一轮生效）
  ├─ 事实提取：  autoExtract → auto.* 键落 sqlite + md
  └─ 交接更新：  handoff → 改写 <repo>/.Locowrk/handoff.md
```

两条链路的关系：**上下文管线每轮重跑；记忆管线在轮收束后异步沉淀，下一轮注入时被上下文管线消费。** 一切记忆/树的注入都走 system 前缀（会话内冻结），消息历史只追加——这是 M1 前缀缓存纪律的两根支柱。

---

## 一、上下文组装管线（用户消息 → 请求字节）

### 1.1 轮次骨架

| 步 | 发生什么 | 代码位置 |
|---|---|---|
| T0 | `run_turn` 落 `TurnStart`/`UserMessage` 并落盘 | desktop/src-tauri/src/runner.rs:53-90 |
| T1 | 轮内循环，每步 `kernel.step_as`；步间查 StopFlags；步后投影增量事件 + 增量落盘 | runner.rs:92-169 |
| T2 | 消息投影：`derive_messages()`；树会话 `TreeContextSource.plan` → 目录+尾巴，`derive_messages_from(from)` 切片 | kernel.rs:186-206；organize/assemble.rs:164-184 |
| T3 | waterfall `agent/pre-step`（插件可拒绝/改写用户消息） | kernel/request.rs:34-54 |
| T4 | 工具目录快照（会话围栏：计划模式非 Pure 不出目录） | kernel.rs:220；request.rs:109-116 |
| T5 | 决策环监听器链逐环处理 `RequestPayload` → 组装 `LlmRequest` | request.rs:135-229 |
| T6 | AppLlm::prepare（见 1.4）→ CompatLlm 流式调用 | llm/app_llm.rs:210-332 |
| T7 | 响应落盘：`ContextCompacted`（若有）→ `Thinking` → `AssistantMessage` → `LlmUsage` | kernel.rs:229-270 |
| T8 | 工具执行：解析失败也落失败 ToolResult（防悬空）→ **先写 ToolCall 再执行** → 审批门 → 中间件链 → 落 ToolResult | kernel.rs:272-360 |
| T9 | 轮结束判据：从尾部找**最近一条 AssistantMessage** 判 tool_calls 是否为空（不能只看 last()，LlmUsage 紧贴其后会误判） | runner.rs:145-151 |
| T10 | 收束：`TurnEnd` + TurnDone；步数到顶正常收束 + TurnCapped；每步后落 `ContextUsage` 快照 | runner.rs:152-219 |

### 1.2 消息投影（session-store）

- `project_messages`（session-store/src/session.rs:164-258）：两遍投影。第一遍收集 ToolCall（call_id→tool/args）与裁决表；第二遍拼 Message（User 带 parts / Assistant 回填 tool_calls / ToolResult 回填），遇 `ContextCompacted` 就地折叠（留尾部 kept 条 + 摘要 User 消息）。
- **悬空调用诚实闭合**（G2，session.rs:221-233）：声明位无 ToolResult → 紧贴 assistant 合成一条 ToolResult（未裁决 = `status:"unknown"` 效果未知）——否则下一轮请求带悬空 tool_calls 会被供应商整单拒收。合成只在投影层，日志不回填。
- `derive_messages()` = 游标前全量投影；`derive_messages_from(from)` = 树会话尾巴切片（同一投影函数跑 `[from..cursor)`）。
- 跳过的事件：Thinking / LlmUsage / ContextUsage 等过程计量**不回传供应商、不进上下文**（DESIGN §4）。

### 1.3 决策环七监听器（注册序 = 执行序，state/mod.rs:215-300；⚠ v2.149 批 E 按代码实况重写：原「五监听器」表缺 v2.139 新增的三件、且把已退役的 CtxCompact 列为现役第 ⑤ 步）

| # | 监听器 | 行为 | 关键纪律 |
|---|---|---|---|
| 1 | ToolExposure | 打分 = 1.0×会话内用过 + 0.7×全局 EWMA + 0.5×语义相似；Always 直通；超出 max_tools（缺省 20）截断 | **会话钉死**：首次选择入 SessionPins（FIFO 128），后续按名字序回映射；目录放宽 = 新 always 工具尾部追加，不破旧前缀 |
| 2 | BaseSystemInject（v2.139） | `prompts/base/` 六节（引言/身份总纲/工具策略/安全边界/输出纪律/reminder 合法性）以 `include_str!` 编译期内嵌，追加在 system **最前** | **静态不可变层**：跨会话同前缀 ⇒ 供应商前缀缓存可跨会话命中；优先级金字塔「用户当前请求 > 背景 > 一切主动机制」+ 开场禁令（治「第一句话就查记忆/读项目」） |
| 3 | MemoryInject | system 依序追加 `<environment>`（v2.139 由 `<workspace_context>` 升级，+日期/平台两行）→ `<repo_context>` 静态提醒段 → `<persistent_memory>`（top_k=5，单条截 500 字符）→ `<memory_guidance>`；**现役口径**：只取全局层项目事实不自动注入（v2.136 项目侧全提醒制）、`<session_experiences>` 段随 experiences 退役 | **会话级预取**：首查后缓存整块文本，逐字节复用；键 = `{session_id}{scope_id}`；日期随记忆块预取会话锁死（v2.139） |
| 4 | ThinkingLanguageInject | 思考语言提示追加 system 真尾部 | 按 session_id 首请求解析锁死 |
| 5 | TreeDirectoryInject | 把 `tree_directory` 追加到 system 尾；Some("") 也算树会话标记 | 目录 = **稳定头行 + 追加清单行，无闭合标签**；占位章节不进目录；**殿后位**是 v2.139 定的——目录逐整理增长，放前面会断掉它后面所有静态段的缓存 |
| 6 | InstructionFilesInject（v2.139） | 全局 `<数据根>/AGENTS.md` + 项目根 `AGENTS.md`（回落 `.Locowrk/`）合成 `# agentsMd` + OVERRIDE 声明 | 走 **messages 面非 system**；(session, scope) 首查快照锁字节；100KB 截断；开关 `memory.instructionFiles` 缺省开 |
| 7 | FragmentInject（v2.139） | 整理完成 / Plan 模式 / 轮次到顶三碎片，轮开场（最后一条是 User）取件一次即清 | 合成消息与碎片**不落日志**（转录/重放/标题/整理材料零污染）；多碎片合并一个 system-reminder 包 |

> **❌ 原表第 ⑤ 步 `CtxCompact` 已退役（v2.149 批 E 登记，批 C 摘本体的账在此收）**：桌面压缩链
> v2.137 整面退役（压缩语义由记忆树**整理**承担 = 绝对阈值 `thresholdTokens` 缺省 100K +
> 章节目录 + 段落总结 + 尾巴切片），监听器本体 v2.147 批 C 摘除（`middleware/compact.rs`
> 332→56 行）。盘上只剩两件活物：**`CompactionOutcome`**（旧会话日志 `ContextCompacted`
> 的重放链在读：session-store 折叠 → runner 投影 → 前端通知 → 通知入账）与
> **`estimate_tokens`**（全仓唯一粗估尺：用量环/进度条/maxOutput 预检/子代理窗口/树尾巴）。
> 原表该行讲的四条纪律（目标压到阈值 50%、切点回退 User 边界、稳定期 stabilityRounds、
> 摘要直连 LlmService 防递归）里，**「切点落在 User 之前」这条纪律活着**——它被记忆树的规则
> 切分继承（本报告「规则切分降级」条即该处，措辞已随批 E 订正为「纪律源自它、现由树侧自持」）；其余三条随本体
> 一起退役，`context.stabilityRounds` 现为幻影键。

### 1.4 树会话组装（TreeAssembler，organize/assemble.rs）

- `plan(session_id)` → `TreePlan { directory, tail_from_event, collapsed_chapters }`；legacy 会话返回 None = 组装零变化。
- **目录格式**（assemble.rs:33-81）：头行 `[memory tree] Earlier turns … to read a chapter, call memory_expand(chapter_id, level=1 for its summary or 2 for its full transcript).` + 每章一行 `- {id}: {title}`（按创建序、id 数值序）。**只有标题，没有摘要**。
- **尾巴二档**（§7.2）：A 边界结算（缺省）= `chaptered_until` 排他游标之后全部原文；B 固定窗口 = `max(chaptered_until, 最近 N 轮起点)`（缺省 N=5），窗口滑出的孤儿轮由 organize 侧 `window_overflow` 溢出整理及时归章，绝不静默丢失。
- 收起章节的 expand 结果换桩（协议配对保全，不删事件）。
- 用量口径：树会话 `context_tokens` = 目录字节/4 + 尾巴估算（组装后口径）。

### 1.5 宿主 LLM 层（AppLlm::prepare，llm/app_llm.rs:210-270）

1. `effective_cfg`：设置活动档 → 会话覆盖 → **子代理路由 `params["route"]`** → 决策环 req.model（最高优先级）。
2. `WireCaps` 输出钳制：仅用户钉死 `max_output` 才发送；发送前 `min(钉死, 窗口 − estimate − 1000)`，下限 1024；辅助调用（aux）不发。
3. 推理档位：aux 降档 > route.thinking > 会话旁车档，经方言 `clamp`（deepseek/glm/qwen/anthropic/openai 五家方言）落 `params["thinking_level"]`，映射不了不发。
4. `image_allowed` 三态门（钉死 > 目录命中 > None 放行）→ `wire::project_tool_assets` 图片投影：工具产物图片挪出 tool 消息、随消息列尾部合成一条 user 消息发送；**历史轮图片不重发**；不支持时降级不报错；note 文案跨请求字节稳定。

### 1.6 传输与重试（llm/client.rs + retry.rs）

- 一条线协议：OpenAI Chat Completions + SSE；kind 只决定鉴权头与端点归一。
- 错误分类：401/403 鉴权 fatal；429 限流（Retry-After 优先，2s×2 指数+25% 抖动封顶 30s）；408/5xx/网络/断流瞬时（1/2/4/8s 封顶 8s）；400/413 命中上下文超限措辞 = fatal 带指引。
- **重试不设次数上限**（用户裁定「半次失败都不可接受」）；已推 delta 的重试先发 `agent://llm-retry` 让前端重置缓冲；等待期间可停止（200ms 轮询停止标记）；180s 零事件看门狗。
- `finish_reason=length` → Fatal 如实报错、截断消息不入史（G1 length 闸）。

### 1.7 M1 前缀缓存纪律：跨轮字节稳定面清单

| # | 面 | 保证机制 | 计划内失效（显式变更） |
|---|---|---|---|
| 1 | `<workspace_context>` 行 | 预取缓存 | 改归属 = scope_id 换键 |
| 2 | `<repo_context>` 段 | 内文 mtime 进 scope_id 指纹 | 人改 .Locowrk/ 文件 |
| 3 | `<persistent_memory>` / `<session_experiences>` / `<memory_guidance>` | 预取缓存（None 也缓存） | top_k 变更；FIFO 128 驱逐 |
| 4 | `[memory tree]` 目录 | 只增不改、追加在 system 块尾 | 整理边界结算；M5 用户编辑 |
| 5 | 思考语言 hint | 按会话锁死 | FIFO 驱逐 |
| 6 | tools 数组 | ToolExposure 会话钉死 | 权限模式切换；always 新增尾部追加 |
| 7 | 消息历史前缀 | 事件只追加 + 游标 | ContextCompacted 结算；rewind/fork |
| 8 | 工具图片 note | 位置无关纯函数 | — |

`scope_id` 指纹（memory.rs:540-556）= 归属 workspace ∪（repoContext 开时）requirements/handoff mtime + notes 清单指纹。命中率口径 = `cache_read / input`（分母不含 cache_write）；`developer.cacheDiagnostics` 开启后每 step 做分块 hash 链比对定位断裂块（M6.1）。

### 1.8 用量观测

`LlmUsage`（input/output/cache_read/cache_write/reasoning/total/duration_ms，供应商不报告就不落，宁缺勿假）；`ContextUsage`（轮前+每 step 后快照，树会话用组装后口径）；est_tokens = serde 字节/4，图片平铺 1024 / 文件 1000。

---

## 二、记忆系统总览：五层存储

| 层 | 位置 | 角色 | 真源? |
|---|---|---|---|
| 会话事件流 | `<数据根>/sessions/<id>.jsonl` | 唯一真相源（append-only），原文永不修改 | ✅ |
| 事实 KV | `<数据根>/memory.sqlite3`，主键 `(workspace, key)` | 机器召回真源（'' = 全局层） | 召回真源 |
| 事实 md | `<数据根>/memory/global|projects/<名-哈希>/facts/*.md` + MEMORY.md 索引 | 人类层与权威编辑面（F1 双写，失败不破契约） | 人读权威 |
| 树投影 | `<数据根>/memory-tree/<scope>/<session>.json` | 章节（标题+摘要+event_range 指针）+ 游标 + 台账；**原文不进树** | 投影可重建 |
| tree.md | `<数据根>/memory/projects/<名-哈希>/tree.md` | 树的人读层（人改保护 + 启动回写） | 投影 |
| 仓库交接 | `<repo>/.Locowrk/`（requirements.md / handoff.md / notes/） | 随项目仓库走的交接面，自动补 .gitignore | 文件即记忆 |

总回退原则：树是投影、组装是投影、事件流唯一真相源——任何验证失败只拆投影不动原文。

---

## 三、记忆写入路径（四条）

### 3.1 模型主动写（memory-sqlite 插件）

- 工具：`memory_write`（Compensable/Always，scope project|global）/ `memory_read` / `memory_list` / `memory_search` / `memory_delete` / `memory_undo`（plugins-src/memory-sqlite/src/lib.rs:64-198）。
- 键位裁决 `write_scope`（store.rs:157-184）：显式 global → '' 层；归属会话缺省 → 项目层；**未归属缺省写拒绝**；headless → ''。
- 执行序：读旧行快照 → UPSERT → **md 双写**（失败只进输出 `md_warning` 字段，sqlite 仍是召回真源）。
- undo 忠实还原：write 撤销 = 删新建键或整条还原旧值/旧键位；delete 撤销 = 整条重插；同样触发 md 双写。

### 3.2 自动提取（autoExtract）——⚠ v2.136 已整模块退役

> 用户裁定（2026-10-09）：项目记忆由主模型在工作中自己管理，软件不代劳；autoExtract 实践确认无作用（召回无来源加权会被 auto.* 霸榜、与树章节内容两遍整理）。extract.rs 整模块 + agent_chat 收束钩子 + app_state extract_deps/llm 字段全部删除；experiences 向量经历层（唯一写入方 = autoExtract 的 episode）一并退役。历史 auto.*/experiences 数据不迁移（管理页可见可删）。
>
> 原机制留档：轮收束后辅助调用提 ≤3 条事实（键强制 auto. 前缀）+ 1 段 episode → sqlite(source='extracted') + md 双写 / experiences 表（缺省关）。

### 3.3 handoff 收束更新——⚠ v2.136 已整模块退役

> 用户裁定：项目记忆是主模型的工作，记忆管理器只管上下文打包整理——后台 LLM 代写交接属越界机制。handoff.rs 整模块 + 两收束点钩子删除；交接文件改由主模型按 `<repo_context>` 提醒钩子自己维护（开场先读、状态实质变化时自行更新）。
>
> 原机制留档：轮收束后 fire-and-forget 辅助调用全量改写 handoff.md（UNCHANGED 净化 / 内容未变不落盘 = mtime 不动缓存不失效 / 首建 toast）。

### 3.4 管理面（记忆页）

`memory_list/update/delete/clear_all` 命令 → MemoryStore 直写 + md 同步；**全局行聊天工具禁删**（删除是人的操作）；`memory_reveal_folder` 打开 md 目录。

### 3.5 reconcile 判定矩阵（F2，memory_reconcile.rs:55-235）

| 情形 | 裁决 |
|---|---|
| 正文 == 索引值 | 已同步，仅补标记（零噪声） |
| 正文 ≠ 索引 且 标记 == mtime | 索引先行（历史同步失败）→ **从索引愈合文件** |
| 正文 ≠ 索引 且 mtime ≠ 标记（或无标记） | 文件被外部改过 → **文件为准** upsert |
| 行在而文件缺 且 标记在 | 人删了文件 → 删索引行 |
| scope facts 目录整体缺席 | 复位手势：不删行（首启导出按索引重建） |

配套：写路径冲突保护（文件带未合并人工编辑 → 先备份 `<key> · 人工编辑备份 <millis>` 再落新值）；reconcile 走专用直写 SQL，**绝不反向同步 md**。首启导出纪律：**先 reconcile 再导出**；导出对已存在文件一律跳过（人的编辑永不被覆盖）。

---

## 四、记忆注入路径

### 4.1 MemoryInject 块序（memory_inject.rs:322-421，system 追加序）

```
<workspace_context>   当前项目工作区一行（归属会话才带）
<repo_context>        静态提醒段（⚠ v2.136：正文不再注入——目录里有什么 +
                      两节点钩子：开场先读 requirements/handoff、状态实质
                      变化时主模型自己更新；跨轮逐字节稳定）
<persistent_memory>   top_k=5 条全局层记忆 key = value[tags]，value 截 500
                      （⚠ v2.136：只注入全局层，项目层事实按需 memory_read/search）
<memory_guidance>     主动记忆指引（无条件注入，编译期静态文本；v2.136 按新模型改写）
```

- 召回语义（v2.136）：`WHERE workspace = ''`（只取全局层），updated_at 降序。项目层事实不自动注入。
- **会话级预取（M1 核心）**：首次注入检索一次并缓存整块文本；`memory_write` 中途写入**不**改写本会话前缀；失效 = scope_id 变（改归属）、top_k 变更、FIFO 驱逐。**v2.136：人改 .Locowrk/ 文件不再触发重算**（提醒段静态、mtime 指纹退出缓存键）。
- GUIDANCE_BLOCK 要点：「永远/以后都」类持久指令当场 `memory_write scope="global"` 升格；重要决定当场记（project）；优先更新既有 key。

### 4.2 repo_context（F2 → v2.136 提醒制）

宿主 `MemoryStore::repo_context`：开关 `memory.repoContext` + 会话归属；首次顺手 ensure_layout（建目录 + requirements 模板 + .gitignore 追加）。注入内容 = **静态提醒段**（requirements.md / handoff.md / notes/ 是什么 + 开场先读 + 状态实质变化时主模型自己更新的节点钩子），正文不注入。

---

## 五、记忆树整理管线（触发 → 决策 → 执行 → 落树 → 通知 → 消费）

### 5.1 触发（轮收束钩子）

- 挂点：agent_chat（commands.rs:119）+ 调度引擎（scheduler/engine.rs:212），`after_turn` 先自动收起再 `plan_auto`，命中即 spawn 后台整理（**对话不等整理**）。
- 触发纯函数（memory-tree/src/trigger.rs:87-102）：
  - 批量档（缺省，v2.137 绝对阈值）：`anchor_tokens ≥ threshold_tokens`（整理阈值 `context.thresholdTokens`，100K–800K 缺省 100K；锚点**优先供应商真实 input tokens**（日志最后一条 LlmUsage），无上报回落估算；窗口未知照常触发——绝对阈值不依赖窗口；手动 `/整理` 不受限）。
  - 增量档：未整理尾巴估算 ≥ organizeChunkTokens（缺省 50K）。
  - 模式 B（固定窗口尾巴）孤儿轮：`window_overflow` 溢出整理先于熔断、不记反弹（例行维护）。

### 5.2 决策（organize/mod.rs:229-326）

- `plan_auto`：溢出整理 → 熔断/在途早退 → 触发判定 → **反弹熔断**（整理后 k=3 个用户轮内再触发 = 反弹 +1，连续 m=3 次 → paused_auto 熔断 + `lw://organize-paused` 如实告知）。
- `plan_manual`（`/整理`）：不受熔断限制且复位熔断账本。
- `plan_recovery`：启动时遍历在途任务重投；会话已删 = 终局放弃。

### 5.3 执行（异步管道，organize/mod.rs:443-574）

1. **占位章节先落盘**（标题「对话 #a-b」、pending=true）→ 投影保存——此后任意点崩溃都可重投。
2. **记忆管理器（记录员）调用**：模型 = memory.memoryManagerModel（空回落主模型）；提示词按模型家族路由（prompts/{manager}/{default,claude,deepseek,qwen}.md 编译期内嵌）；超时 60s × 重试 3 次；`params aux=true` 降档；session_id=None **不进会话日志**（影子纪律）。
3. **输出校验**：JSON 数组，每条 title（钳 40 字）+ summary（钳 2000 字，**用户明确指令与关键决策必须引号逐字保留**）+ event_range；**切分区间必须按材料项完整铺满 [0..n) 不重叠不留缺口**——垃圾整批转规则切分，绝不半信半疑落树。
4. **规则切分降级**（mode=rule, status=degraded）：按 ruleChunkTokens（缺省 20K）估算累计，在下一条 User 材料项处断（复用「切点落在 User 之前」纪律（**该纪律源自 ctx-compact，其本体已随 v2.147 批 C 摘除，现由记忆树规则切分自持**——同一实现思想，不是跨模块依赖），不拆 tool_calls↔ToolResult 配对）。
5. 落树前**重载最新投影**（防自动收起在管理器调用期间落盘被旧快照抹掉，M4 竞态修复）。

### 5.4 落树 + 人读层

- `tree.apply_organized`：撤占位 → 落真章节（id = b{max+1} 数值序）→ 游标 `chaptered_until = to+1`（排他）→ 台账进 history（FIFO 20）。
- 投影文件：`memory-tree/<scope>/<session>.json` 原子写；scope = 工作区编码 + fnv1a 短哈希，未归属 = `_root`。
- **tree.md 同步**：`# 记忆树 · 项目` + 会话 `## 旁车标题` 小节 + `### bN · 标题` + 摘要 + 隐性指针 `<!-- range: a-b · source · session -->`；**人改保护**：文件 mtime 新于全部投影则不静默覆盖；启动对账回写只吸纳标题/摘要、回打手动标记。

### 5.5 通知

`lw://organize-done`（三路同源：sessionId/users/chapters/mode/status/reason）+ `lw://organize-paused`（仅熔断翻转瞬间）。前端「📦 已整理 N 轮为 M 捆 [查看]」。

### 5.6 消费

- **memory_expand**（Pure + Always，树会话目录常驻）：level=1 摘要（~200 token）/ level=2 原文切片（磁盘日志按 event_range 切，与「看原文」/记录员材料同形同源）；结果**追加**进历史（append-only 缓存安全）；展开记账 expanded_at。
- **memory_collapse**（Compensable + Hidden，开关缺省关）：收起 = 已展开内容换桩（改写未来前缀 = 计划外失效，省空间不省钱）；自动收起：占用 ≥ 80% 时收起最早展开章节逐步泄压。
- **memory_search_titles**（Pure + OnDemand，M6.3）：**跨会话**扫当前项目全部树的章节标题 top-5——embedding 可用 = 语义 cosine（阈值 0.2），缺席/失败 = 关键词 contains 且 match_mode 如实改；返回 `{session_id, chapter_id, title, summary(截 160 字), score}`。
- 会话门控双保险：目录层（树会话才列工具）+ 调用层（legacy 调到即如实失败）。
- UI 面：记忆页树区块（只读展开原文）、M5 编辑面（编辑打「手动」标记自动层永不覆盖；删除仅摘投影游标不动）、M6.4 项目联合视图、开发者观测面板。

---

## 六、与「原始大流水线」设想的对照

| 设想环节 | 实现现状 | 判定 |
|---|---|---|
| ① 上下文每轮打包进组装线 | 每轮 waterfall 决策环组装（§一） | ✅ 一致 |
| ② 达到阈值时旁支整理器按章节整理成段落总结和标题 | 批量档阈值 30%（锚点=真实 input tokens）→ 异步整理器产出标题+摘要，全量铺满校验 | ✅ 一致（且比设想多：占位先行/熔断/降级/恢复） |
| ③ 下一轮组装 = 最新对话 + 章节段落总结 | 下一轮 = 最新对话（尾巴原文）+ **标题目录**（每章一行 `- bN: 标题`）；**摘要不自动进上下文**，模型按需 `memory_expand(level=1)` 点播 | ⚠️ 刻意改掉：进目录的是标题不是摘要（见下） |
| ④ 整轮回话过程 + 章节 + 标题沉淀成完整记忆树 | 树投影 = 章节（标题+摘要+event_range 指针）+ 游标 + 台账；**原文不进树**，永远留在 jsonl；另有 tree.md 人读层 | ✅ 一致（「过程」以指针形态在树里，树是投影可随时重建） |
| ⑤ 新会话提到某天某事 → LLM 调标题及章节反查旧上下文 | `memory_search_titles` 跨会话语义/关键词找标题 ✅ 返回 session+chapter_id+标题+160 字摘要；**但 `memory_expand` 只认当前会话的章节 id——跨会话章节读不到全摘要/原文** | ⚠️ 半闭环（v2.136 记档未立批） |

**偏差 1（刻意设计）：目录只进标题，摘要点播制。** 理由（Memory Tree Design v2 §7/§8/§14）：若每章 ~200 token 摘要自动进上下文，目录成本随章节数线性甚至永久膨胀；标题 ~10 token 让目录近乎免费。目录头行内嵌使用说明教模型点播：「level=1 摘要 / level=2 原文」。总纲 = **越远越压缩**：最远只有标题、中间摘要点播、最近原文保真。

**偏差 2（实现缝隙）：跨会话反查只走了一半。** `memory_search_titles` 能跨会话找到「某天某事」对应的章节（返回 session_id + chapter_id + 标题 + 160 字摘要截断），但 `memory_expand` 的入参只有 `chapter_id + level`，内部按**当前会话**的树加载（tree_tools.rs:133-146）——跨会话章节既读不到完整摘要，也读不到原文；更糟的是章节 id 各会话独立编号（b1、b2…），模型拿别会话的 `b3` 调 expand 会**静默命中本会话同 id 章节**（内容驴唇不对马嘴且无报错）。最小修法（未实施，待裁定）：给 memory_expand 增加可选 `session_id` 入参（限同 workspace），或 search_titles 命中跨会话时直接带回全文摘要。

---

## 七、项目级隔离（Context and Memory.md §4.6）

- 会话归属单一真源 = 旁车 `<id>.meta.json` 的 `workspace` 字段；**归属先行**：首聊挂起归属随 agent_chat 带入，建会话即写旁车，先于首轮任何注入/围栏/提取。
- 双层记忆：全局层（''，偏好，聊天工具显式 scope="global" 才可写）/ 项目层 `(workspace, key)`；读取 = 本项目 ∪ 全局层，行带层标；经历记忆不设全局层。
- 插件侧经 HostVtable 末尾 append-only 的 `session_context` 槽反向查询本次调用归属（plugin-host 调用期线程 RAII 挂上下文）；headless 不注入解析器 = 全局语义旧行为。
- 树按 `(workspace, session)` 键位存储隔离；未归属落 `_root`/`_unaffiliated`。

---

## 八、设置键总表（settings.json，camelCase）

### memory 区
| 键 | 缺省 | 作用 |
|---|---|---|
| enabled / topK | true / 5 | 记忆注入总开关 / 条数（0=等效关） |
| ~~autoExtract~~ | ~~true~~ | ⚠ v2.136 退役（幻影键，存量文件解析不破） |
| files | true | 文件化总开关（md 双写 + tree.md） |
| repoContext | true | `<repo>/.Locowrk/` 静态提醒段 + 引导（v2.136 提醒制） |
| ~~handoffUpdate~~ | ~~true~~ | ⚠ v2.136 退役（幻影键） |
| ~~experiences~~ | ~~false~~ | ⚠ v2.136 退役（幻影键） |
| defaultOrganizer | "tree" | 会话缺省整理模式 |
| organizePacing / organizeChunkTokens | batch / 50000 | 整理节奏二档 / 增量块 X |
| recorderModel / recorderProvider | None | 记忆管理器档（空回落主模型） |
| tailStrategy / tailWindowTurns | boundary / 5 | 尾巴 A 边界结算 / B 固定窗口 N 轮 |
| collapseEnabled / collapseThresholdPct | false / 80 | memory_collapse 暴露 + 自动收起 |

### context / developer / tools 区（与两条链路相关）
| 键 | 缺省 | 作用 |
|---|---|---|
| context.thresholdTokens | 100_000 | 整理阈值（绝对 token，100K–800K；批量整理触发；v2.137——thresholdPct/strategy/stabilityRounds 三键退役为幻影） |
| tools.enabled / maxPerRequest / pinSession | true / 20 / true | 工具目录截断 / 会话钉死 |
| tools.maxTurnSteps | runner 缺省 | 单轮步数上限 |
| developer.organizeReboundK / M | 3 / 3 | 反弹判定窗口 / 熔断阈值 |
| developer.recorderTimeoutSecs / recorderMaxRetries / ruleChunkTokens | 60 / 3 / 20000 | 管理器超时×重试 / 规则切分块 |
| developer.cacheDiagnostics / cacheHitAlert(Percent) | false / false·80 | 前缀断裂诊断 / 命中率告警 |

---

## 九、设计文档 ↔ 主题索引

| 文档 | 主题 | 核心贡献 |
|---|---|---|
| docs/design/DESIGN.md | 内核总设计 | §3.1 Context 容器；§4 会话日志唯一真相源 + 过程数据不回传；§4.3 先写日志再执行；§7 中间件链序 |
| docs/design/Context and Memory.md | 上下文+记忆总纲 | §3.6 注入顺序定稿；§4.6 双层隔离/归属先行；§5.3 压缩不压记忆 |
| docs/design/KV Prefix Cache Design v2.md | 前缀字节稳定 | §4.1 四段前缀；§4.2 工具钉死；§5 七纪律；§6 结算+稳定期；纪律 7 命中率口径 |
| docs/design/Memory Tree Design v2.md | 记忆树 | §3 L0/L1/L2；§4 触发/锚点/切分；§5.6 摘要逐字保留；§7 五段组装+尾巴 A/B；§8 惰性加载；§11 三阶段迁移；§14 七纪律 |
| docs/design/Memory File-ization Design.md | 记忆文件化 | §2 双目录布局；§3 双向同步判定；§4 主动记忆三机制；§5 组装不变量 |
| docs/design/NEW/Reliability Reinforcement Design.md | 可靠性 | §2 五组判定矩阵；§8 In-Band Summary（P2 记档）；§13 七机制对照 |
| docs/design/Memory-KV Roadmap.md | 专项进度表 | M0–M6 / F1–F3 全勾状态与每批走查实证 |

---

## 十、已知边界与缝隙（记档）

1. **跨会话反查半闭环**（§六偏差 2）：search_titles 找得到、expand 读不了；b-id 撞车有静默错配风险。待裁定修法。
2. 树目录只进标题——模型若不点播 expand，摘要信息不进上下文（设计意图，但依赖目录头使用说明的提示强度）。
3. 批量整理触发依赖窗口已知（未知不自动触发，只可手动 `/整理`）。
4. 桌面落盘粒度 = 每 step（工具执行中杀进程无痕；悬空只在 mid-flush 崩溃毫秒级窗口出现，G2 已记档）。
5. 记录员调用走父会话模型辅助降档（aux），不走子代理钉档（批 F 记档）。
6. 子代理无旁车 → 记忆落未归属键位、注入只见全局层（跨项目零泄漏，设计意图）。
7. B 尾巴窗口档为实验档（窗口滑动段每轮重付未命中价，§7.2 原文）。
8. ~~autoExtract 召回无来源加权~~——⚠ v2.136 该机制整体退役，本条关闭。
9. **跨会话反查半闭环**（§六偏差 2）：search_titles 找得到标题、memory_expand 只认当前会话章节 id——待裁定修法（v2.136 未动）。
