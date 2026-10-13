# Locowrk 上下文预算闭环设计

> 配套项目：Locowrk 版本：1.0 前置文档：《Context and Memory》《记忆树设计 v2》《LLM 客户端与调度层设计》
> 来源：v2.137 上下文面重构（压缩面退役）后的收口审计 —— 补齐「整理旁支接管压缩语义」之后留下的预算闭环缺口
> 状态：待研究。本文是问题陈述 + 方案候选，**不是已实施设计**。行号以 v2.137 工作区为准，仅供导航。

---

## 目录

1. [设计目标](#1-设计目标)
2. [v2.137 之后发生了什么](#2-v2137-之后发生了什么)
3. [核心缺口：在途期是预算盲区](#3-核心缺口在途期是预算盲区)
4. [缺口二：工具结果无统一预算](#4-缺口二工具结果无统一预算)
5. [缺口三：超限文案指向已退役的设置](#5-缺口三超限文案指向已退役的设置)
6. [缺口四：没有项目指令注入面](#6-缺口四没有项目指令注入面)
7. [方案：四组改造与落地顺序](#7-方案四组改造与落地顺序)
8. [在途期补触发的状态设计](#8-在途期补触发的状态设计)
9. [与 ZCode / PI-Desktop 的对照](#9-与-zcode--pi-desktop-的对照)
10. [不建议照搬的部分](#10-不建议照搬的部分)
11. [常见坑](#11-常见坑)
12. [待拍板项](#12-待拍板项)

---

## 1\. 设计目标

| 目标 | 含义 |
|---|---|
| 预算闭环 | 从「占用增长」到「触发整理」到「占用回落」，全程无盲区 |
| 旁支不拖累 | 后台整理失败或变慢时，主链路有确定的退路，不是无限期裸奔 |
| 错误文案不骗人 | 报错指向的设置必须真实存在 |
| 插件作者改不了纪律 | 预算上限由内核收口，不靠每个插件自觉 |

### 非目标

* **不恢复 ctx-compact**。v2.137 用户裁定「上下文根本不压缩了」，压缩语义由记忆树整理边界承担 —— 这个方向本文认同，不翻案。
* 不追求「上下文永不超限」。超限是现实，要给它确定的处理路径，不是消灭它。
* 不做上下文内容的语义压缩（摘要/重写）。那是 ZCode 路线，与记忆树章节化重叠。

### 前提：整理旁支能兜住多少

本文所有结论都建立在一个已核实的��实上：**整理是异步旁支，能有效压低占用。**

* `after_turn` → `self.spawn(task)` → `tauri::async_runtime::spawn`（`desktop/src-tauri/src/organize/mod.rs` 的 `spawn`），**不阻塞对话**
* 整理完成 → `chaptered_until` 推进 → 组装改为「目录 + 尾巴」，原文不再进 prompt（`organize/assemble.rs:157-173`）
* 触发锚点优先取供应商**真实 input tokens**（`organize/mod.rs:193`），无上报才回落估算（`:196-201`）

因此「上下文能放大很多倍也不崩」在稳态下成立。本文要处理的是**这个稳态被打破的窗口**。

---

## 2\. v2.137 之后发生了什么

v2.137 的上下文面重构砍掉了四件东西，方向是对的：

| 退役项 | 处置 | 现状核实 |
|---|---|---|
| `CtxCompact` 桌面链 | 整面退役 | 代码在 `crates/kernel-agent/src/middleware/compact.rs`，**零生产调用点** |
| 压缩阈值百分比 | 改绝对 token | `settings/context.rs:16` `ORGANIZE_THRESHOLD_DEFAULT = 100_000` |
| legacy 会话 | 一切会话皆树 | 旁车 `organizer` 键幻影化 |
| 缓存单价两键 | 退役 | 命中率观测保留 |

连带留下一批死代码（详见 §7.4）。

**压缩语义被完整地移交给了记忆树整理旁支。这是一次干净的责任转移 —— 前提是接收方能覆盖发送方原本覆盖的全部区间。**

本文审计的正是这个「覆盖是否完整」。

---

## 3\. 核心缺口：在途期是预算盲区

### 3.1 机制

`plan_auto` 的判定顺序（`organize/mod.rs:231-297`）：

```
① 模式 B 溢出整理（先于熔断、不记反弹）    mod.rs:240-257
② paused_auto || pending.is_some() → None   mod.rs:258-260   ← 缺口在这
③ should_auto_organize（阈值/增量判定）      mod.rs:264-272
④ 反弹熔断 → 暂停并告知                    mod.rs:275-288
```

第 ② 步：

```rust
if tree.paused_auto || tree.pending.is_some() {
    return None;
}
```

**整理在途期间（`pending.is_some()`），新的触发被直接丢弃** —— 不排队、不记账、不补跑。整理完成后，要等**下一轮** `after_turn` 才会重新判定。

### 3.2 数字

缺省值全部核实自源码：

| 量 | 值 | 来源 |
|---|---|---|
| 整理阈值 | 100K token | `settings/context.rs:16` |
| 记录员超时 | 60s | `settings/developer.rs:22` `MANAGER_TIMEOUT_SECS_DEFAULT` |
| 记录员重试 | 3 次 | `settings/developer.rs:25` `MANAGER_MAX_RETRIES_DEFAULT` |
| **在途最坏时长** | **60s × (1+3) = 240s** | `organize/mod.rs:477` `for _attempt in 0..=max_retries` |
| 模型窗口 | 200K / 1M（举例） | 四层链 `settings/llm.rs:365-398` |

**在 100K 触发整理后，有最长 4 分钟没有任何整理。** 这段时间里用户继续对话，上下文只涨不落。

### 3.3 为什么这条是 P0

三个放大因素叠加：

1. **在途期不丢触发** —— 上面的 ②
2. **在途期无退路** —— `memory_collapse` 缺省关（`Exposure::Hidden`，`organize/tree_tools.rs:197-200`），自动收起 `collapse_enabled` 缺省也是关
3. **超限即 Fatal** —— `llm/retry.rs:46-48` 判 `Failure::Fatal`，不压缩、不裁剪、不降级

也就是说，**在这 4 分钟窗口里，三层保护同时缺席**。

### 3.4 实证路径

```
t0   占用 100K → after_turn 触发 → spawn（pending 落盘）
t0   用户继续对话（对话不等整理，这是设计意图）
t1   记录员调用超时，重试 ×3
t1+240s 整理终于收束 —— 但这期间占用可能已 180K+
     若模型窗口 200K：下一轮请求直接 context overflow → Fatal
```

**注意**：这条路径不需要「用户疯狂发消息」才会触发。慢网络 + 模型限流 + 60s 超时是日常。

---

## 4\. 缺口二：工具结果无统一预算

### 4.1 现状：每个插件各定各的

核实结论：**全仓没有任何管线级字节预算**。裁剪全部在插件侧自行实现：

| 工具 | 缺省上限 | 位置 | 可否被模型放大 |
|---|---|---|---|
| `fs_read` | 32 KiB | `plugins-src/tool-fs/src/read.rs:29` `READ_MAX` | **可**（模型可传 `max_bytes`） |
| `shell` | 256 KiB | `plugins-src/tool-shell/src/exec.rs:20` `OUT_MAX` | 否 |
| `git` | 自定 | `plugins-src/tool-git/src/proc.rs:96` | — |
| `fetch` | 256 KiB | `plugins-src/tool-fetch/src/lib.rs:159` | 否 |

`max_output` **不是**结果体积裁剪 —— 它是模型行用户钉死的输出 token 值（`llm/app_llm.rs:246-248`），与工具结果无关。

### 4.2 为什么不是 P0

§3 的结论是整理旁支能兜住稳态。工具结果体积大，会**更快地**撞到 100K 触发线，从而**更快地**触发整理 —— 在这个意义上，它反而是整理旁支的燃料，不是敌人。

它的问题是三个具体的，不是「会崩」：

1. **单次就能吃掉半个预算** —— 一次 256 KiB `shell` 输出 ≈ 64K token，占 100K 阈值的 2/3。用户可能还没说几句话就触发整理，整理频率异常。
2. **口径不可预测** —— 同样的操作，`fs_read` 32K、`shell` 256K，用户和模型都无法预判。
3. **缺统一截断标注** —— 各插件自己拼文案，格式不一，模型不一定知道自己拿到的是截断结果。

### 4.3 对照

ZCode 在这一层收口得很好（`tool/executor/result-serialization.ts:33-40`）：

```ts
DEFAULT_RESULT_BUDGET = {
  maxInlineBytes: 100_000,
  maxModelBytes:   100_000,
  strategy: "truncate",
  preview.direction: "head",
}
```

**一处收口，插件作者改不了。** 这正是 Locowrk 缺的「结构保证」。

---

## 5\. 缺口三：超限文案指向已退役的设置

`desktop/src-tauri/src/llm/retry.rs:46-48`：

```rust
400 | 413 if is_context_overflow(body) => Failure::Fatal(
    "上下文超出模型窗口：请调低 设置 → 上下文 的压缩阈值，或新开会话后重试".into(),
),
```

而压缩阈值已经不存在了 —— v2.137 退役 `CtxCompact` 时一并删掉了这个设置项（`settings/context.rs` 只剩 `threshold_tokens` 作整理阈值，不是压缩阈值）。

**这条文案会把用户指向一个空设置项。** 这是纯文案 bug，但用户遇到超限时看到的就是它。

修复成本极低，见 §7.3。

---

## 6\. 缺口四：没有项目指令注入面

> ⚠ **本节的核实随施工进度过期（2026-10-13 Q4 动手前取证订正）**：项目指令注入面**早已随 v2.139 落地** —— 内核监听器 `crates/kernel-agent/src/middleware/instruction_files.rs`（`InstructionFilesInject`，挂 `agent/request`）+ 宿主 `desktop/src-tauri/src/instruction_files.rs`（G 全局 = `<数据根>/AGENTS.md`、H 项目 = repo root `AGENTS.md` 回落 `.Locowrk/AGENTS.md`）+ 设置开关 `memory.instructionFiles` + `(session, scope)` 键会话钉死缓存 + 单文件 100 KiB 截断，规范正身 = 《Prompt Assembly Layering Design》§4.3 / §7。因此 §7.5 的三条要求里：「与会话钉死同等对待」**已做到**、「注入进 `system`」与「逐层串联」**与已采纳规范正面冲突**（分层稿 §2 第 2 条「背景类内容一律不进 system」+ §7「子目录按需指令裁掉：Locowrk 是项目归属制，无 cwd 语义」）。⇒ 本节与 §7.5 的原文叙述**保留不动**（那是立稿日的核实记录），队列 **Q4 已按此重定义**，实测剩下的价值与冲突处置见《Post-Audit Roadmap v1》§5 的「Q4 的前提『全仓无运行时读取 AGENTS.md』= 过期核实」行——本行只留指针不重抄（同一读数两处手写必漂）。

核实：**全仓无任何运行时读取 `AGENTS.md` / `CLAUDE.md` 的路径。** grep 命中项全部是文档、`include_str!` 内置的记忆树提示模板（`crates/memory-tree/src/manager.rs:22,26`）与代码注释。

这是一个**功能性缺失**，不是优化点。Locowrk 自己就有 `AGENTS.md`（项目根），但运行时根本不读它。

当前唯一形态的项目指令面是 `repo_context` 静态提醒段（`memory.rs` 宿主实现 → `memory_inject.rs:302-307`），只说「`.Locowrk/` 目录里有 requirements.md / handoff.md，开场先读」。

### 6.1 两个参照实现

| 项目 | 候选名 | 向上查找策略 | 预算 |
|---|---|---|---|
| PI-Desktop | `AGENTS.override.md` > `AGENTS.md` > `CLAUDE.md` > `.claude/CLAUDE.md` | **逐层串联**，从 project root 到 target directory，嵌套文件排在最后、优先级最高 | 全局部 32 KiB + 项目链独立 32 KiB |
| ZCode | 仅 `AGENTS.md` | **找到第一个即停** | 100 KiB |

**建议抄 PI-Desktop，不抄 ZCode。** ZCode 的「找到第一个就停」在嵌套项目里会漏掉上层约束 —— 从 `D:/repo/subdir` 向上找到 `D:/repo/AGENTS.md` 就停了，`D:/AGENTS.md` 的全局约束丢失。

PI-Desktop 的逐层串联 + 四候选名更完整，代价是实现略复杂（需要判断文件在链上的层级排序）。

---

## 7\. 方案：四组改造与落地顺序

### 7.1 P0-A　在途期补触发（最该做）

**问题**：§3 的在途盲区。

**方案**：在途时不再丢触发，而是记一个「待整理」标记，在途任务收束后立即补跑。

具体三条：

1. **标记而非丢弃**：`pending.is_some()` 时，若当前 anchor 已超阈值，记 `organize_needed = true` 到投影（字段见 §8）。
2. **收束即补跑**：`run_task` 完成 `pipeline` 后，检查该标记；为真则立即重新 `plan_auto` 并 `spawn`，不等下一轮 `after_turn`。
3. **补跑去重**：进程内 `inflight` 已有去重（`run_task` 开头），补跑走同一条路径即可，不新增并发面。

**判据**：构造一个「整理超时 240s、期间占用翻倍」的场景，断言在途收束后 **1 秒内**发起第二次整理，而不是等下一轮。

> ✅ **已兑现（v2.190 · 队列 Q6，2026-10-13）**：三条全部落地，其中第 2 条**实现形状改口**——收束后不是 `spawn` 新任务，而是在 `inflight` 去重锁释放**之前**串行续跑（`organize/task.rs::rerun_while_marked`）。理由两条：① 补跑发生在后台任务里**手上没有 `&Session`**，材料改从磁盘会话日志重读（与崩溃恢复同一对 API，`Organizer::session_of`）；② `spawn` 出去的第二个任务在测试里无法 await ⇒ 判据退化成「轮询投影 + 赌时序」。串行 = 本节第 3 条「走去重路径、不新增并发面」的最强实现；**变异 M5 把这段挪到锁释放之后，功能测试 11 条全绿、只有跨文件形状判据抓到**（由此在队列 §6 新立第 10 条口径：跨文件的关系必须有跨文件判据）。

### 7.2 P0-B　缩短在途窗口

**问题**：60s × 4 = 240s 太长。

**方案**：整理是后台任务，用户体感是「整理半天没动静」。降到 **30s 超时 × 2 重试 = 最坏 90s**。

**为什么不是 60s×1**：慢网络 + 限流下 60s 超时太紧，会把本来能成功的整理打成降级规则切分，摘要质量下降（`organize/mod.rs:533-573` 的降级路径标题是「首条用户消息前 20 字」，质量明显不如模型产出）。

**判据**：`organize` 单测改用 30s/2，断言降级路径仍只在超时后触发。

> ✅ **已兑现（v2.190 · 队列 Q6）**：超时与重试**同调** = 60→30、3→2 ⇒ 最坏窗口 60×(1+3)=240s → **30×(1+2)=90s**（本稿原数）。⚠ 排期表《Post-Audit Roadmap v1》§4 的 Q6 行当时写成「30s×(1+1)=最坏 60s」= 把本节的 `1+2` 压错了，已按用户 2026-10-13 裁定**照本稿取 90s**，该表 §4/§5 同批订正。
> 交付物里加了一条**乘积不变量**判据 `settings/tests::defaults_keep_the_in_flight_window_at_90_secs`（打在缺省常量上而不是打在用户可改的内存态上）——§11 说的「在途时长只调一个数」这个坑只有乘积能证伪；**变异 M8 只回退超时一个数** ⇒ 该用例与形状判据同时见红。既有缺省断言 60/3→30/2 属**正当改写判据**，与批 Q4 立的「可调项缺省必须等于改前写死值」不冲突：那条防的是「顺手做成可调时把缺省改了」，这里是**明令改缺省**。

### 7.3 P0-C　修超限文案

**问题**：§5。

**方案**：`retry.rs:46-48` 改为指出现存手段 ——

```
上下文超出模型窗口。请新开会话继续，或减少单次工具输出（如 shell / fs_read 的返回量）。
```

**判据**：i18n 键同步更新（zh/en），设置页不再出现「压缩阈值」字样。

### 7.4 P1　统一工具结果预算层

**问题**：§4。

**方案**：在内核工具执行出口加**一处收口**，插件改不了：

- 缺省 **32 KiB**（待拍板，见 §12），head 截断 + 统一标注
- 大文件类工具可申请豁免，但**豁免必须留痕**（进审计日志）
- 放内核而非插件 —— 这正是 ZCode 的做法，也是「结构保证」与「自觉纪律」的区别

**判据**：构造 500 KB 工具结果，断言进上下文的 ≤ 预算，且截断标注格式统一。

### 7.5 P2　项目指令注入

**问题**：§6。

**方案**：新增 waterfall 监听器，与 `MemoryInject` 同级注册（`state/mod.rs:232` 附近）。抄 PI-Desktop 的四候选名 + 逐层串联。

**注意前缀缓存**：注入内容进 `system`，会改写前缀。必须与会话钉死同等对待 —— 首轮解析后按会话锁定，人改文件不重算（与 §7.1 的记忆预取缓存同纪律，`memory_inject.rs:275-328`）。

### 7.6 P3　清理死代码

**别误删 `compact_messages`** —— 它仍被 `session.rs:136` 快照折叠引用，只是不再走 `CtxCompact`。

| 符号 | 位置 | 状态 |
|---|---|---|
| `CtxCompact` + `CompactConfig`/`CompactStrategy` | `kernel-agent/src/middleware/compact.rs` | 生产无调用点 |
| `CompactionOutcome` 回填通道 | `kernel/request.rs:59-93` | 载荷在，恒为 `None` |
| `CONTEXT_COMPACTED` 事件 | `desktop/src-tauri/src/events.rs:25` | 永不 emit |
| `suggested_threshold()` | `compact.rs:55-62` | 无调用点 |
| `prompts/main/*.md`（4 个） | `crates/memory-tree/src/prompts/main/` | 全死（`main_prompt` 无生产调用） |
| `EventToChapterIndex` | `crates/memory-tree/src/index.rs:9-31` | 无生产调用 |
| `count_user_messages` / `tail_tokens` | `manager.rs:228` / `material.rs:133` | 仅测试用 |
| `MemoryStore::write` | `desktop/src-tauri/src/memory.rs:271` | 仅测试播种 |

---

## 8\. 在途期补触发的状态设计

### 8.1 字段

挂在 `MemoryTree`（`crates/memory-tree/src/tree.rs`），与现有字段同族：

```rust
pub struct MemoryTree {
    // ... 现有字段
    /// 在途期到达阈值但被丢弃的触发（P0-A）。
    /// 收束即补跑，消费后清空。不持久化语义 —— 投影丢了就丢了，
    /// 下一轮 after_turn 会重新判定，不是真源。
    pub organize_needed: bool,
}
```

**为什么不复用 `pending`**：`pending` 已经在位表示「有在途任务」，塞第二个标记会让崩溃重投逻辑（`organize/recovery.rs`）变复杂 —— 恢复时无法区分「占位是这次的还是上次的」。

**为什么不复用 `rebound`**：`rebound` 是熔断账本（`tree.rs:230-245`），语义是「惩罚反弹」，塞待办进去会让 `paused_auto` 的翻转判据失真。

### 8.2 落点

| 位置 | 动作 |
|---|---|
| `plan_auto` ②（`mod.rs:258-260`） | `pending.is_some()` 且 anchor 超阈值 → `tree.organize_needed = true; save(); return None` |
| `run_task` 收束（`pipeline` 返回后） | 读 `organize_needed`；为真 → 清零 + `save()` + 重新 `plan_auto` + `spawn` |

> ⚠ **本表两行的措辞按实测改口（v2.190）**：① 第一行「`pending.is_some()` 且 **anchor 超阈值** → 记标记」只覆盖 `Pacing::Batch` 档——`IncrementalChunk` 档的触发条件是**尾巴增量 token**，照抄这句等于在同一个判定上开第二把尺子（正是本稿 §11「阈值口径不一致」点名的坑）。现役写法 = 在途支与常规支**共用 `Organizer::trigger_input` + 同一个 `should_auto_organize`**（`TriggerInput` 字面量在 `trigger.rs` 里只剩一个出处，由形状判据 Q1/Q2 钉）；**变异 M1** 打「退成只看 anchor」、**M11** 打「常规支再拼一份」。② 第二行的 `spawn` 改成了同一条 await 链上的串行续跑（理由见 §7.1 的兑现注记）。另：`paused_auto` 那支**连标记都不记**——§8.4「补跑不是绕过熔断的后门」在这一刀的落点就是不进这支（**变异 M3**）。

### 8.3 为什么放投影而不是内存

放内存会在两个场景失效：桌面进程重启、调度引擎跨会话复用。放投影 = 崩溃恢复时 `scan_pending` 天然能捡到（`storage.rs:132-143`），与既有纪律同源。

**但它不是真源** —— 投影可丢，丢了就丢了，`after_turn` 下轮会重新判定。这个「可丢」性质必须写进注释，否则下一个人会误以为它是强一致状态。

### 8.4 反弹熔断的交互

补跑时若仍触发反弹判定（`mod.rs:275-288`），会被 `paused_auto` 拦住并清标记 —— 这是正确行为：**补跑不是绕过熔断的后门**。

需要确保补跑路径**不重置** `rebound` 账本。`plan_manual` 会 `mark_manual` 复位（`mod.rs:315`），补跑走 `plan_auto` 不碰它。

---

## 9\. 与 ZCode / PI-Desktop 的对照

| 能力 | ZCode | PI-Desktop | Locowrk |
|---|---|---|---|
| 章节化记忆 | ❌ | ❌ | ✅ 更强 |
| 崩溃重投 | ✅ 占位先行 | — | ✅ |
| 反弹熔断 | ❌ | — | ✅ |
| 前缀缓存纪律 | 3 段 cacheControl | 独立预算 | ✅ 更完整 |
| 压缩不删原文 | ✅ 新增 hidden 消息 | — | ✅ |
| **统一工具结果预算** | ✅ 100 KB 收口 | — | ❌ **§4** |
| **超限恢复** | ✅ Reactive 压缩 | ✅ | ❌ 只 Fatal **§3.3** |
| **在途期不丢触发** | — | — | ❌ **§3** |
| **项目指令注入** | ⚠️ 找到即停 | ✅ 四候选逐层串联 | ❌ **§6** |
| 记忆存储形态 | 文件 + 索引 | SQLite | SQLite + md 双写 |

**唯一实质性的「可借鉴单项」仍是记忆从数据库全文改为文件 + 索引。** 但 Locowrk 的 v2.136 已经在往那个方向走（项目记忆改成 `.Locowrk/` 文件 + 模型自维护），只是全局层 `memories` 表还在 SQLite —— 这是**渐进迁移的正确形态**，不必推倒。

---

## 10\. 不建议照搬的部分

1. **ZCode 把记忆索引包在 `# agentsMd` + "OVERRIDE any default behavior" 标题下**（`context/sections/request-user-context.ts:63-70`）—— 优先级错配。记忆是数据，不是高优先级指令。Locowrk 现在的降权措辞更严谨。
2. **ZCode 的 microcompact 默认关闭**（`runtime/methods/microcompact.ts:111` 要求显式 `enabled === true`）—— 他们自己都不敢默认开，不要学。
3. **ZCode 的 `legacy | preflight-v1` 双策略**、多个只有声明没有读取点的配置项（`modelContextBudgetStrategy: "legacy"`、`thresholdPercentOverride`）—— 重构债，不是设计。
4. **ZCode 的指令链「找到第一个即停」** —— 见 §6.1。

---

## 11\. 常见坑

| 坑 | 症状 | 正确做法 |
|---|---|---|
| 只看 Fatal 不看盲区 | 以为「超限只报错」就是全部问题 | 先问「这段时间有没有别的机制在兜底」—— 整理旁支确实兜，但有盲区 |
| 补跑绕过熔断 | `paused_auto` 被补跑重置，熔断形同虚设 | 补跑走 `plan_auto` 同路径，不碰 `rebound` |
| 清理时误删 `compact_messages` | 快照折叠崩溃 | 它仍被 `session.rs:136` 引用 |
| 把 `organize_needed` 当真源 | 投影丢了就以为触发丢了 | 写进注释：可丢，下轮 `after_turn` 重新判定 |
| 注入面改前缀 | 项目指令注入每轮重读文件，命中率归零 | 首轮锁定 + 按会话缓存 |
| 在途时长只调一个数 | 60s 超时改 30s 但重试仍 3 次，盲区没变小 | 超时与重试**同调** |
| 阈值口径不一致 | 整理用绝对 token，自动收起用百分比 | 同源：`organize/mod.rs:356` 的 `anchor*100 < window*pct` 与 `settings/context.rs` 的绝对阈值不同源，改一处要两处对齐 |

---

## 12\. 待拍板项

1. **P1 统一工具结果的默认预算是多少？** 32 KiB（对齐现有 `fs_read`）还是 100 KiB（对齐 ZCode）？这直接决定模型看到多少输出，也影响整理触发频率。
2. **P0-A 的补跑是否要限制频率？** 若整理持续失败（模型全挂），补跑会形成「收束→立刻再跑→再失败」的循环。需要一个连续失败计数器（当前**没有**连续失败熔断 —— 只有反弹熔断 `tree.rs:230-245`）。
   > ✅ **已裁定并兑现（v2.190 · 队列 Q6）**：那本账就是 `MemoryTree::rerun_streak`（投影里，`serde(default)` 零迁移、不抬 `SCHEMA_VERSION`），上限进**开发者分区**可调项 `organizeMaxRerun`（0–10，缺省 3；0 = 关掉补跑 = 可配置原则②「禁止项都要有开关」）。复位条件**不是「时间过去了」**而是任何一次**新的自然触发**（`after_turn` 真发起了一轮整理 = 整理追得上对话节奏）；写在 `plan_auto` 里面会等于续跑自己抹自己的账、上限永远打不到（**变异 M7** 专打）。到上限时**保留标记**——熔断只该挡「这一串续跑」，不该把已记账的触发再丢掉（判据 `rerun_stops_at_the_budget_and_keeps_the_mark`）。
3. **P2 项目指令注入现在做还是排后？** 这是功能性缺失而非优化 —— Locowrk 自己有 `AGENTS.md` 但运行时读不到。

---

## 附：本文的事实基础

全部结论来自 v2.137 工作区源码静态核实：

* 父代理亲自核实：监听器注册（`state/mod.rs:228,232,244,252` 恰 4 个 REQUEST 监听器）、超限分类（`retry.rs:58-70` 七条措辞匹配）、阈值口径（整理绝对 token / 自动收起百分比）、工具结果上限全仓普查（无管线级预算）、触发纯函数（`trigger.rs:88-98`）、窗口四层链（`settings/llm.rs:365-398`）、存储层原子写（`storage.rs:113-122`）
* 三路并行调查覆盖：记忆 KV 链路、记忆树管线、上下文组装与压缩

**未执行**：任何构建、测试、clippy。「死代码」判定基于全仓文本检索，非编译期 `dead_code` 验证 —— 可跑 `cargo clippy -p memory-tree -p kernel-agent --all-targets` 交叉确认。
