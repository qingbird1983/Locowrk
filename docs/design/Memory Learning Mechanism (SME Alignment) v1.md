# 记忆学习机制设计（SME 对齐）v1

> **状态：部分兑现（v2.200 · 队列 Q9）** —— L0（§2）与 L1（§3.2）早已落，**§3.3（L2）与 §3.4（L3）随
> v2.200 落地**、**§3.1 原则 4 与 §5.1 的 L4（可解释 breakdown）随 v2.201 · 队列 Q10 落地**，就地逐条标注与原话的实测订正。**仍未做的只有 §5.4 的 E2「自动校准」**（权重靠人拧四个滑条，系统不自我调整）。本文立稿时是
> 问题实证 + 目标态设计 + 方案候选，**不是已实施设计**——立稿当批除本文与
> INDEX.md 登记行外零文件改动、零代码改动。与《Memory Redesign (ZCode Alignment) v1》**同属记忆
> 轨道但分工不同**（见 §0.4 分工表），两稿不互相覆盖。
> 立稿日期 2026-10-12；参照系 = Spatial Memory Engine（`F:\Warehouse\Github\spatial-memory-engine`，
> 下称 SME，Python；取证时 commit 为当日工作区）v1.3.0 状态。
> 行号证据以立稿日工作区为准，实现时须重新核对（本仓库纪律：验证器自己也会骗我）。

---

## §0 动机：为什么要在 ZCode 对齐之外再开一稿

### 0.1 本稿的出发点与 Memory Redesign v1 完全不同

《Memory Redesign v1》的诊断是**"召回失败"**：冷启动盲 × 工具不可见 × 写入侧怠工。那是
**可见性**问题，治法是补注入面、提曝光、改写侧触发。方向正确，立稿至今仍是记忆轨道的主线。

本稿处理的是**另一个维度**：Locowrk 的记忆**在被召回之后**如何变化。

### 0.2 一句话实证：Locowrk 的记忆是静态的

全仓 Rust 侧只读核查（2026-10-12）：

| 检索项 | 结果 |
|---|---|
| `reinforce` / `hit_count` / `ebbinghaus` | **零命中** |
| `access_count` / `last_accessed_at` | **零命中**（且两列在建表语句里根本不存在，见 §0.3） |
| 全局层召回排序 | `WHERE workspace='' ORDER BY updated_at DESC, key LIMIT ?`（`desktop/src-tauri/src/memory.rs:437-466`） |

**结论：Locowrk 的记忆是纯静态的**——写进去就永远不变。既不因被反复使用而变强，也不因
久未使用而变弱；召回只看"最近更新了哪几条"，与"这条记忆有多有用"无关。

**这与 SME 的核心假设正面对撞。** SME 的整个设计建立在"记忆价值随使用而漂移"这个前提上
（§1.1）。两者不是实现细节的差距，是**产品假设的差距**。

### 0.3 顺带订正：两条在既有文档里被当作"未实现配置"的键

`Context and Memory.md` §4.2.1 的 SQL 设计稿里画了 `access_count INTEGER` 与
`last_accessed_at INTEGER` 两列，但**实际建表语句只有 6 列**（`desktop/src-tauri/src/memory.rs:54-62`：

```
workspace / key / value / tags / updated_at / source
```

**这两列不是"没来得及实现的死列"，而是"从未存在于 schema"。** 实现本稿 §3 时应按**新增列**
处理（有存量库需走 `ALTER TABLE` 迁移，路径见 §5.2），不要按"把死列接上"处理。

另一处：SME 的会话层约定里那些键——`reinforce_on` / `graph_expand` / `consolidate_every`——
在 Locowrk **代码与文档中均零命中**。它们是外部引擎的概念，**不是本仓库"未实现的既定配置"**，
引用对照时必须标注，否则会误导实现者去找不存在的配置项。

### 0.4 两稿分工（防重复立项）

| | Memory Redesign v1 | **本稿** |
|---|---|---|
| 治的问题 | 召回不到（可见性） | **召回之后不演化**（学习性） |
| 主语 | 注入面 / 工具曝光 / 写入触发 | **权重 / 排序 / 生命周期** |
| 批次 | M1 读侧 / M2 写侧 / M3 清债 | **L1–L4（见 §6）** |
| 是否需用户拍板 | M2 需（v2.136 部分反转） | §3 强化回路需拍板（成本） |

**依赖方向**：L 批依赖 M1 的召回观测面（§2），M 批不依赖 L 批。建议 **M1 与 L1 并行**。

---

## §1 参照系：SME 的学习机制勘察结论

（源码位置统称 `sme/` = SME 仓库根下的相对路径；逐条 file:line 见附录 A。）

### 1.1 强化回路：SME 唯一真正的"学习"

`sme/dynamics/reinforcement.py:38-58`，全部逻辑约 20 行：

```
retention = 1 / (1 + age/half_life_days)^power        # ebbinghaus_retention
weight     += hit_weight_delta     × retention        # 默认 Δw=0.10
importance += hit_importance_delta × retention        # 默认 Δi=0.05
hit_count   += 1
last_hit = now;  freshness = 1.0
```

参数默认（`sme/config.py:108-116`）：`half_life=7.0` 天、`power=1.2`、`max_weight=5.0`。
可读作**赌博机更新**：每次"用上它"是一次带折扣的奖励，越久没用折扣越狠。

### 1.2 带地板的衰减：变弱但永不消失

`sme/dynamics/decay.py:30-42`：

```
factor = floor + (1 - floor) × exp(-ln2 × age / half_life)
floor  = 1 - max_weight_decay = 0.15      # config.py:121-123，默认 half_life=30 天
```

`factor ≥ 0.15 > 0` ⇒ **记忆只会越来越难被召回，永不删除**。与 Locowrk 现役的
"永不删除"一致，但 SME 多了一层**可达性递减**。

### 1.3 落盘字段与计算字段分离（值得抄的纪律）

`sme/dynamics/decay.py:54-63`：`apply()` 把当前衰减因子**落盘**到 `Memory.decay_factor`
（为了导出/统计可见），但**刻意不动 base weight / importance**。注释写明理由：

> "The base weight and importance themselves are left untouched so reinforcement always
> builds on top of history."

即：**给人看的量落盘，参与计算的量不落盘**。Locowrk 的章节 `expanded_at` 恰好是反例——
它落盘了，但只用于自动收起时挑"最早展开"，**不参与任何排序**（`crates/memory-tree/src/chapter.rs:86-94`、`tree.rs:289-301`）。

### 1.4 排序：8 信号可解释加权

`sme/retrieval/ranking.py:105-114`，权重默认（`config.py:82-89`，和为 1）：

| 信号 | 权重 | 含义 |
|---|---|---|
| semantic | 0.40 | 混合相似度（向量+BM25+metadata） |
| importance | 0.12 | 学到的重要度 |
| freshness | 0.10 | 最近是否被碰过 |
| weight | 0.10 | 累积强化权重 |
| decay | 0.08 | 衰减因子 |
| hit_count | 0.08 | 历史命中次数 |
| recency | 0.06 | 距上次命中多久 |
| region | 0.06 | 所属区域分 |

每票带 `breakdown`（`ranking.py:121-131`），检索结果不是黑盒。

### 1.5 SME 自己证伪了它的招牌架构

**这是本稿最该记住的一节。** SME 的招牌是「Region 两阶段空间检索」，但作者自己的消融
（3 seed × 3 规模，`lab/scripts/ablation.py:60-75`，**全实验只消融了这一个组件**）结论是：

| 干扰规模 | 开 Region | 关 Region |
|---|---|---|
| 0 | 0.900 / 1.000 / 0.975 | 0.875 / 1.000 / 0.975 |
| 500 | 0.925 / 1.000 / 0.975 | 0.925 / 1.000 / 0.975 |
| 2000 | 0.925 / 1.000 / **0.950** | 0.875 / 1.000 / **0.975** |

作者在 `lab/战报.md:35` 明确推翻原归因：

> 「Region 在此场景的实测收益是**检索延迟平坦**，不是准确率」

对 Locowrk 的含义：单项目记忆量在**百条量级**（实测当前 226 条、5+ 项目混存），远未到需要
空间聚类的量级。**SME 的结构层不搬**（见 §4）。

### 1.6 其余可借鉴的工程手法（不单独立批，进 §3 设计时按需取用）

| # | 手法 | 出处 | 本稿用在哪 |
|---|---|---|---|
| E1 | **过滤前移到候选收集阶段**——只在最终结果上过滤会让候选池被"必然出局"的记忆填满，补全池永不触发，检索饿死为 0 命中 | `retriever.py:278-297` | §3.3 |
| E2 | **自动校准绝不覆盖显式配置**——只改"未被碰过的默认值"，用 `_explicit_keys` 识别用户是否设过 | `engine.py:244-261` | §5.4 |
| E3 | **死信号剔除 + 权重按比例折叠**——不能影响排序的通道直接剔除，权重按比例还给活信号，保持量纲 | `retriever.py:819-849` | §3.3 |
| E4 | **换算法必须对齐输出契约**——新旧路径的乘子活在同一刻度上，A/B 才可比 | `retriever.py:670-693` | §5.5 |
| E5 | **保留槽位非零保证**——图扩展固定占 `max(1, top_k//5)` 槽，且必留 ≥1 个直接命中位 | `retriever.py:711-716` | §3.3 |
| E6 | **增量精确 + 延迟几何 + 读前对齐**——写路径只维护精确质心，其余延迟算，消费者先 sync | `space.py:233-241` | §3.4 |
| E7 | **演化只扫候选集 + 合并滞后**——O(候选) 而非 O(全部)；刚合并过的不许立刻拆回去 | `region_manager.py:298-321`、`336-342` | 不搬（规模不够） |
| E8 | **旁路系统以事实状态为准**——只有真入库的才写 WAL，"被丢弃的话语不能在崩溃后复活" | `engine.py:315-330` | §3.5 |

---

## §2 前置：召回可观测性（**L0，建议先做**）

### 2.1 问题

`MemoryStore::recall()`（`desktop/src-tauri/src/memory.rs:437-466`）**零观测**：无计数、无命中率、
无 injected-vs-used 记录。`memory_inject.rs` 的 `GUIDANCE_BLOCK`（`:128-137`）文本里也无任何
埋点钩子。

现有 `audit.jsonl`（`crates/kernel-agent/src/middleware/audit.rs:30-44`）只覆盖**工具调用**，
覆盖不到"注入侧召回了什么、模型有没有用上"。

**后果有实测证据**：《Memory Redesign v1》§0 那份高质量诊断——"模型绕开有货的库去读
handoff.md"——是靠**人工翻 982 KB audit.jsonl** 推断出来的。下一轮记忆问题还得再翻一次。

### 2.2 为什么它必须先做

L1–L4 全部是对**排序**的改动。改了之后**无法证明是否变好**——因为没有 before/after 测量面。

**本仓库已吃过这个亏**：v2.73 落地 autoExtract，后以"实践确认无作用"退役
（`Context and Memory.md` §4.2.2 注）。那个结论方向大概率对，但**如果当时有召回观测面，
结论会硬得多**，也不用靠体感下判断。

### 2.3 L0 设计（纯增量、零行为变更）

**(a) 注入侧召回事件**：在 `recall()` 成功返回后记一条结构化事件。

| 字段 | 含义 |
|---|---|
| `ts` / `session_id` / `scope_id` | 基本三元组 |
| `n_candidates` | 库内满足注入条件的总条数 |
| `returned` | 本次实际注入条数 |
| `keys` | 注入的 key 列表（≤ top_k，体积可控） |
| `elapsed_us` | 召回耗时 |
| `mode` | `global`（现役唯一形态，为将来项目层预留） |

**(b) 命中回执**：模型真正用上哪条记忆，**当前无法自动判定**（需要判分，见下）。
先落**可判定的部分**——`memory_read` / `memory_search` 的调用已在 audit 里，补一条
`returned_n` 与 `hit_keys` 即可。

**(c) 命中率指标**（派生，不额外埋点）：
- `injection_yield` = 被 `memory_search` / `memory_project_history` 检索到的 key 数 ÷ 注入过的 key 数
- `store_empty_ratio` = 注入为空块（`entries.is_empty()`）的会话占比

**(d) 关于"injected-vs-used"**：真要判"模型有没有用上注入的记忆"，需要一个判据。可选
① 让模型在回答里标注引用的 memory key（改 prompt，侵入）；② 管理页让用户点"这条有用"
（人工）。**建议本批只做 (a)(b)(c)**，(d) 留到 §7 记档——它是增强项，不是本批前置。

**(e) 落点**：`usage.sqlite3`（`crates/kernel-agent/src/middleware/usage.rs:56-63`）旁挂一张
`memory_inject_log` 表，与工具热度同源同生命周期。**不新建库**（遵 §3 纪律：mtime/已有库优先）。

### 2.4 L0 验收

- 单测：注入非空 / 注入为空 / 库为空三种情形的埋点落库；`scope_id` 变更时重新计数
- 桌面端真跑：新会话首问「我之前在做什么」，`memory_inject_log` 应有一条记录且 `keys` 非空
  （当前应为空块——这本身就是 §0-2 的复现证据）

---

## §3 核心设计：Locowrk 的记忆学习机制

### 3.1 目标态原则（四条）

1. **永��删除**——与现役一致，与 SME 一致。衰减只降可达性，不删行。
2. **落盘与计算分离**——落盘字段只给人看（导出、管理页、诊断）；参与排序的量不落盘，
   每次召回现算。抄 SME §1.3。
3. **每个信号要么影响排序、要么不出现**——不摆不能改变顺序的通道（抄 E3）。
4. **每个信号都要可解释**——管理页能回答"这条为什么排在这"。
   > ✅ **兑现注（v2.201 · 队列 Q10）**：落点 = 观测表新列 `memory_inject_log.breakdown` + 管理页第三区块
   > `InjectSection.vue`。与稿内说法有一条**实现上的分歧**：稿的直觉是"读的时候把分数再算一遍"，实现走的是
   > "**发生那一次存回执**"——再算一遍要同一个 `now` 与同一份词表（词表由最近三条用户消息切出），跨会话重放
   > 根本拿不到，编出来的解释会与真发生过的那次不一致；一旦有一次是编的，面板整体就不可信。四档占比是全稿
   > 唯一一处除法，`total<=0` 时如实给全 0（均分 250 = 把"没有信号说话"演成"四档均衡"）。界面按前端标准**用
   > mono 数字表达占比**，四段占比条属清单外形态（特例申请制），本批没申请就没有造。

### 3.2 数据模型（L1）

**只加两列，且都是"学出来的"，不是"手工填的"**：

```sql
ALTER TABLE memories ADD COLUMN hit_count     INTEGER NOT NULL DEFAULT 0;
ALTER TABLE memories ADD COLUMN last_hit_at   INTEGER;   -- NULL = 从未被召回过
```

> ✅ **已兑现（v2.189 · 队列 Q5，2026-10-13）**：两列按本节原样落地，但实现有两条稿内没写的事实——①`memories` 表**有两个实现**（宿主 `desktop/src-tauri/src/memory.rs` 与插件 `plugins-src/memory-sqlite/src/store.rs`，开同一个 `memory.sqlite3`），所以两列、两条 ALTER、迁移函数都各写了一份，一致性由 `scripts/check-schema-parity.mjs` 盯着（10 档，含「两侧各调用一次、顺序 workspace 在前」；它原本是 `tmp/q5-parity.mjs`，台账 T8 收口时升进常驻闸）；②迁移**必须逐列补缺**而不是「两列齐就跳过、缺一就两条都跑」——半迁状态（两条 ALTER 中途被杀）下后者会撞 `duplicate column name` ⇒ ROLLBACK ⇒ 那个库永远打不开。**本批不写入也不读这两列**（写入点 = §3.4 / L3，排序 = §3.3 / L2），所以用户可见差异为 0。取证经过见队列 §5 与 ROADMAP v2.189 节。

**不加** `access_count`：语义与 `hit_count` 重合，且名字更容易和现役 `usage.sqlite3` 的工具
热度混淆。命中次数与工具热度**必须分库**——一个是"这条记忆有用"，一个是"这个工具常用"，
混在一起会互相污染（这正是 SME 战报里"召回无来源加权会被 auto.* 霸榜"的教训）。

### 3.3 排序信号（L2）

> ✅ **已兑现（v2.200 · 队列 Q9）——落点与本节原话有 8 条实测订正，原文一律不擦**：
> ①四信号与权重都是**千分比整数**，排序取 `Σ 权重×信号` 而**不做归一除法**（同批候选分母相同）⇒
> 下面那张「权重初值（和为 1）」表从硬约束降为**缺省值恰好和为 1000**；把它钉成约束会逼出"动一个滑条
> 就摊平其余三个"的联动机制，而界面是四个独立输入。全 0 档合法 = 退回现役序。
> ②`strength = 1000 − ⌈1000/(h+1)⌉`、两个衰减都是 `1000·H/(H+age_days)`（H = 14 / 30 天），
> `relevance` = 命中词数（钳在 1000 内）——本节那四条 float 公式的形状不变，量纲换整数，理由是配置面靠
> `PartialEq` 判「变更即失效」而 float 既不进 `Eq` 也不闭合。
> ③`freshness` 的兜底字段是 **`updated_at` 而不是 `created_at`**——**`created_at` 从未存在**（Q5 只加
> `hit_count`/`last_hit_at` 两列，§0.3 那句"按新增列处理"兑现时就是这两列）；于是未命中期它与 `recency`
> 共线，属良性，已写成用例钉成"写下来的形状"而不是被下一刀当 bug 顺手改。
> ④"不建新索引、不分词"订正为 **ASCII 整词 + CJK 二字滑窗**：纯 `LIKE '%中文词%'` 在双字以上词组上
> 实测切不出命中 ⇒ 相关性档恒 0，整条学习回路退化成"最近更新优先"。仍不建索引。
> ⑤相关性匹配面**截到注入上限**并复用内核 `VALUE_INJECT_MAX_CHARS`——不截就是给模型根本看不见的第 8000 字
> 让路；宿主侧再写一遍 500 由判据拒。
> ⑥候选集**有界 200**，不是 §5.3 那句"全量取"（`value` 单条可达 256 KiB，无界读取没有上界保护）。
> ⑦下面的 **E5 保留槽在 `top_k = 1` 时如实不成立**：保留槽花的是一个相关性名额，单条档花费 100% ⇒
> 实现取 `limit ≥ 2` 才**顶替末位**（是顶替不是追加，名额总数不变）。
> ⑧本节末与 §3.1 原则 4 的"可解释 breakdown"属 **L4 = 队列 Q10**，本批 `MemoryEntry` 刻意不带学习字段
> ——把 `hit_count` 写进 system 既花 token 又把机器状态摆给模型看。
> 落点 = `desktop/src-tauri/src/memory_learn.rs`（四信号纯函数打分 + 候选 SQL）+
> `crates/kernel-agent/src/middleware/memory_inject/{mod,contract,terms}.rs`（契约与切词）。

**不引入向量、不引入 BM25**（遵《Memory Redesign v1》§7-2：向量不作检索主路径）。
现役 `memory_search` 的 `LIKE '%q%'`（`plugins-src/memory-sqlite/src/store.rs:302-309`）
保持不变——它是**显式检索**路径，不是注入路径。

**注入路径改写为可解释加权**：

```
score = w_relevance × relevance      # 关键词命中（对本次用户消息）
      + w_recency   × recency        # 时间新鲜度（现役 updated_at 的平滑版）
      + w_strength  × strength       # 强化权重（hit_count 派生）
      + w_freshness × freshness      # 最近是否被命中过
```

**四个信号的精确定义**（全部现算，不落盘）：

```
strength  = hit_count / (1 + hit_count)                       # 饱和曲线，无上界陷阱
freshness = 1 / (1 + age_days(last_hit_at ?? created_at) / 14)  # 半衰期 14 天
recency   = 1 / (1 + age_days(updated_at) / 30)               # 半衰期 30 天
relevance = 命中数 / (命中数 + 1)，无命中 = 0                   # 见下
```

**`relevance` 的定义要克制**：对最近 N 条用户消息（建议 N=3）做 `LIKE` 计数，命中 key/value/tags
任一即计 1。**不建新索引、不分词**——与 `memory_search` 同纪律。

**采纳 E3（死信号剔除）**：全局层目前 `workspace` 恒为 `''`（`memory.rs:532-544`），"层标"不
参与区分 ⇒ **`scope` 信号在项目层落地前是死信号，不进权重表**。等到项目层事实真的开始注入
（Memory Redesign v1 §4.1 的 M1），再把它加进来，权重按比例从其余三项里让出。
**这条纪律正是防"权重表越加越花哨但没用"。**

**权重初值**（和为 1，需 M1 跑出基线后再调）：

| 信号 | 初值 | 理由 |
|---|---|---|
| relevance | 0.40 | 与 SME 的 semantic 权重同量级；相关性应主导 |
| strength | 0.25 | 学习信号，本稿的主角 |
| freshness | 0.20 | 被用过之后一段时间内保持热度 |
| recency | 0.15 | 保底信号，防止老但重要的记忆被永久沉底 |

**保留槽位（E5）**：召回结果恒保 **≥1 条最近更新的记忆**，不因相关性排序被挤掉。
理由：现役行为是"最近 5 条"，若纯按相关性排，用户刚写的东西可能被埋——那是**行为回归**，
不是优化。

### 3.4 强化回路（L3）

> ✅ **已兑现（v2.200 · 队列 Q9）——三处与本节原话不同，原文一律不擦**：
> ①触发点仍是"放进注入块"，但**计数单位是会话而不是每次 `recall()` 调用**：内核的会话预取缓存
> （M1 纪律 = 块字节在会话内钉死）决定同一会话第二轮根本不查库 ⇒ `hit_count += 1` 每会话至多一次。
> §5.5 的 A/B 读数与本稿 §6.1 那句"检索即命中污染"都按这个口径解释——污染仍在，只是从"每轮"降为"每会话"。
> ②写回 SQL 的 SET 列表 = `hit_count` / `last_hit_at` **两列，不许多出 `updated_at`**：那颗同时是
> `recency` 的唯一来源与管理页"最近编辑"的含义，被召回刷新等于把"被读到"冒充成"刚编辑过"，还会把 §6.1
> 担心的污染放大成永远置顶。形状判据（reinforce 的 SET 面）+ 端到端用例（写回后 `updated_at` 不变）各钉一头。
> ③§5.4 那句"`learnEnabled=false` 须逐字节回到现役"的等价性**不是由测试近似给的，是由调用图给的**：
> `MemorySource::recall_relevant` 带缺省实现（缺省支忽略 query 回落 `recall()`，故五个测试 fake 与
> headless 装配零改动），而宿主关档支路直接 `return self.recall(...)` 走**现役本体**——没有第二份"等价的老 SQL"。
> 另：本节"刻意不做"三行（不调 LLM 判分 / 不加权增量 / 不设独立 `weight` 列）本批一律照做，未扩契约。

**触发点只有一个**：`recall()` 把某条记忆放进注入块时，`hit_count += 1`、
`last_hit_at = now`。

**刻意不做**（与 SME 的关键差异，逐条说明理由）：

| 不做 | 理由 |
|---|---|
| 调 LLM 判断"模型有没有真的用上" | 每轮多一次模型调用，成本不可接受；且判分本身有噪声（见 §6.5） |
| 按 retention 加权增量（`weight += Δ × retention`） | Locowrk 的命中是**纯读路径**，没有"这次回答质量"信号；加权增量会让权重语义变得不可解释，违反 §3.1 原则 4 |
| 独立 `weight` 列 | 饱和曲线 `hit_count/(1+hit_count)` 已够，省一列 |

**饱和曲线而非线性累加**：`hit_count` 无上界，线性会让"被检索 100 次的冷门记忆"永久霸榜。
`x/(1+x)` 在 hit_count=1 时给 0.5、=9 时给 0.9、=99 时给 0.99——**收敛快且永不超过 1**。

**衰减交给现算（E6 的精神）**：`freshness` / `recency` 每次召回现算，**不落盘**。
落盘的只有 `hit_count` 与 `last_hit_at`——它们是**不可从时间戳完全重建**的学习量
（`last_hit_at` 可以丢，但丢了历史命中序列就没了）。这与 SME §1.3 的分离标准一致。

### 3.5 与旁路系统的一致性（E8）

`memory_reconcile.rs:56-80` 的 md 回写路径会覆盖 sqlite 行。**若重建时漏掉两列新值，
强化计数会被清零**。实现时必须核对：`reconcile` 的字段白名单要包含 `hit_count` /
`last_hit_at`，或明确判定"以 md 为准时这两列可重置"（**建议不重置**，理由见 §6.4）。

---

## §4 明确不做（边界记档）

1. **向量/embedding 作注入排序主路径**——《Memory Redesign v1》§7-2 已裁定；本稿不翻案。
2. **搬 SME 的 Region 空间聚类 / PPR 图扩展**——SME 自己证伪（§1.5），且 Locowrk 量级不需要。
3. **衰减到某个阈值就归档/删除**——违 §3.1 原则 1。冷数据靠召回难度自然沉底。
4. **跨项目记忆权重共享**——作用域隔离问题不在本稿，见《Memory Redesign v1》§7-1。
5. **管理页手工编辑权重**——权重必须"学出来"，用户只能通过 `memory_delete` 表达"这条没用"。

---

## §5 落地方案与风险

### 5.1 分批

| 批 | 内容 | 规模 | 前置 |
|---|---|---|---|
| **L0** | §2 召回可观测性 | 小 | 无（**建议先做**） |
| **L1** | §3.2 加两列 + `ALTER TABLE` 迁移 + reconcile 字段白名单 | 小 | L0 |
| **L2** | §3.3 排序改写（注入路径） | 中 | L1 + M1（相关性需要会话消息流） |
| **L3** | §3.4 强化回路（`recall()` 内 +2 行写回） | 小 | L1 |
| **L4** | ✅ **v2.201 · 提交 `5da9a24` 落地**（兑现注见 §3.1 原则 4；本条哈希由同批回补提交落下） | 中 | L2/L3 |

L2 与 L3 都只改 `MemoryStore::recall()` 一处，**可合并做**（同一函数、同一锁）。

### 5.2 迁移注意

- 存量库需 `ALTER TABLE`；Locowrk 侧的迁移真身 = `desktop/src-tauri/src/memory.rs` 的 `ensure_workspace_schema`（**603 行起**；本稿原写的 `memory.rs:159-173` 已漂，那段现在是 `open_conn` 的建表语句）——形状是`pragma_table_info` 探针 + `BEGIN IMMEDIATE` + 等锁后**二次探针**，Q5 的 `ensure_learning_schema` 照此同形，并且**逐列**决定补哪一条（半迁状态可自愈）。插件侧 `store.rs` 是第二份实现，两侧都改、都由 `q5-parity.mjs` 钉。
- **风险**：若用户已有多个 workspace 的 sqlite，迁移必须**逐库执行**且幂等。
- 迁移不可逆（drop 列会丢学习数据）。⚠ 原稿要求「注明在 `DESIGN.md` 的回滚说明里」**没有落点**——DESIGN.md §6 的三层回退讲的是插件版本 / 会话游标 / 补偿事务，不含磁盘表形状；Q5 裁定不为一条注记新开一章，改记在这里与 ROADMAP v2.189 节：`ADD COLUMN` 只增列不动主键，旧库升级安全；要回退只能整库重建。

### 5.3 性能

`recall()` 现在是单条 `ORDER BY updated_at DESC LIMIT ?`（走索引）。改为加权排序后，
需要在内存里对候选集排序——**但候选集受 `top_k` 钳制前的大小有限**（全局层条目本就少）。
若全局层增长到千条以上，改 `ORDER BY` 为全量取 + 内存排序即可，**不需要建索引**。

### 5.4 配置面

新增设置项（遵守 E2：**默认值可被自动校准，用户显式设过的一律尊重**）：

```
memory.learnEnabled   bool  缺省 true
memory.relevanceWeight float 缺省 0.40
memory.strengthWeight float 缺省 0.25
memory.freshnessWeight float 缺省 0.20
memory.recencyWeight  float  缺省 0.15
```

**`learnEnabled=false` 时行为逐字节回到现役**（纯 `updated_at DESC`）——这是 E4 精神：
关掉新机制必须能拿到**可对照的旧行为**，否则 A/B 无从谈起。

> ✅ **已兑现（v2.200 · 队列 Q9）——三处量纲与强度订正**：键名与本节一致（`memory.learnEnabled` +
> 四档权重），但四档是**千分比整数 0–1000**（缺省 400/250/200/150 = 上面那四个 float 的整数形），
> 不设"和必须为 1"的校验（理由见 §3.3 兑现注①）。上面那句"逐字节回到现役"**兑现得比原话更强**：
> 关档支路直接 `return self.recall(...)` 走现役本体，而不是另写一份等价的老 SQL ⇒ 等价性由调用图给、
> 不由测试近似给。⚠ 本节开头那句 E2「默认值可被自动校准，用户显式设过的一律尊重」**本批未实现**——
> 当前没有自动校准器，四档只能由用户显式改；等 Q10 有 A/B 读数之后再谈校准，不在本刀伪造。

### 5.5 A/B 与判分（E4）

改动前后各跑一批会话，比对：
1. `memory_inject_log` 的注入 key 集合差异（客观，可机械判定）
2. LLM 判分"回答是否用上了历史"（主观，**注意噪声**，见 §6.5）

**判分只作参考不作结论**——这是 SME 作者自己反复标注的纪律（「判分 LLM 存在 ±1 题抖动，
单 seed 差异不作结论」，`lab/战报.md:63`）。

---

## §6 风险与已知盲区

### 6.1 学习信号可能被"检索即命中"污染

`recall()` 被调用 ≠ 记忆对回答有用。全局层每次会话都注入 top-5，**高频的旧记忆会被反复
计入 `hit_count`**，形成自我强化。

**缓解**：初版 `hit_count` 只由注入路径累加，且 §3.3 的 `strength` 权重刻意压在 0.25
（低于 relevance 的 0.40）。**M1 跑出基线后再评估是否收紧。**

### 6.2 注入路径 vs 显式检索路径的语义不同

`memory_search` 是**用户主动问**，命中就该排前面；`recall()` 是**系统主动注入**，命中只是
"可能相关"。本稿 L1–L3 **只改注入路径**，不改 `memory_search` 的排序。两条路径的信号不可
互换。

### 6.3 与 Memory Redesign v1 M1 的接口

M1 落地后，项目层索引会开始注入（`<project_memory_index>`）。届时 §3.3 的
`relevance` 需同时对全局层与项目层计算，`scope` 死信号问题自动解除，权重表要重新分档。
**本稿的权重初值只在"纯全局层"形态下有效。**

### 6.4 reconcile 覆盖风险

见 §3.5。**判定**：以 md 投影回写 sqlite 时，学习字段**以 sqlite 为准不覆盖**——
md 是人读投影，学习量是机器状态，二者不同源。若将来要让 md 也显示学习信息，另立议题。

### 6.5 判分噪声（继承自 SME 的教训）

任何"记忆改进了"的结论都需要**可复现的判据**。建议：
- 客观指标（注入 key 集合差异、命中率）作主判据
- LLM 判分只作辅证，且**固定题集 + 多轮**，报告时必须给出波动范围而非单点值

### 6.6 本稿的已知盲区（如实登记）

- **未做端到端实机验证**——立稿零代码改动，所有行号以当日工作区为准。
- **`hit_count` 的实际分布未知**——全局层当前 0 行（`Memory Redesign v1` §0-1 实测），
  学习机制在有数据之前无法验证效果。这是 L1 的**真实风险**：可能做完发现没东西可学。
  **L0 的观测面正是为了尽早暴露这一点。**
- **未考虑项目层树章节（memory-tree）是否也该纳入学习**——本稿只覆盖事实层（sqlite）。
  树章节的学习信号在 `crates/memory-tree/` 另一套存储里，机制相同但落点不同，待 §7 排期。

---

## §7 待拍板与后续

| # | 事项 | 状态 |
|---|---|---|
| 1 | **§5.1 分批顺序**：L0 先于 L1/L2，还是 M1 与 L 批并行 | 待拍板 |
| 2 | **§3.3 权重初值**是否照此，或先跑 M1 基线再定 | 待拍板 |
| 3 | §2.3(d) "injected-vs-used" 判据选哪条路（改 prompt / 人工点选 / 不做） | 记档待议 |
| 4 | §6.6 树章节是否纳入学习机制 | 记档待议 |
| 5 | L 批与《Memory Redesign v1》M 批的合批可能性 | 待排期 |

> **本稿推荐**：先立 L0（观测面），L1/L2/L3 合并为一批做，L4 单独排。
> **未拍板前 L1 及之后不立项**（遵本仓库"先聊后动"纪律）。

---

## 附录 A：SME 证据表（仓库根 = `F:\Warehouse\Github\spatial-memory-engine`）

| 结论 | 证据 |
|---|---|
| 强化回路全逻辑 | `sme/dynamics/reinforcement.py:38-58` |
| 强化参数默认 | `sme/config.py:108-116` |
| 衰减带地板 | `sme/dynamics/decay.py:30-42`；参数 `sme/config.py:121-123` |
| 落盘/计算分离纪律 | `sme/dynamics/decay.py:54-63` |
| 8 信号排序与权重 | `sme/retrieval/ranking.py:105-114`；`sme/config.py:82-89` |
| 可解释 breakdown | `sme/retrieval/ranking.py:121-131` |
| Region 消融设计 | `lab/scripts/ablation.py:60-75`（唯一被消融的组件） |
| Region 归因被推翻 | `lab/战报.md:35` |
| 判分噪声自述 | `lab/战报.md:63` |
| 过滤前移 | `sme/retrieval/retriever.py:278-297` |
| 显式配置护栏 | `sme/engine.py:244-261` |
| 死信号剔除 + 权重折叠 | `sme/retrieval/retriever.py:819-849` |
| 输出契约对齐 | `sme/retrieval/retriever.py:670-693` |
| 保留槽位非零保证 | `sme/retrieval/retriever.py:711-716` |
| 增量精确 + 延迟几何 | `sme/space/space.py:233-241` |
| 演化候选集 + 合并滞后 | `sme/space/region_manager.py:298-321`、`:336-342` |
| 旁路以事实状态为准 | `sme/engine.py:315-330` |

## 附录 B：Locowrk 现状锚点（仓库根 = 本仓库）

| 事项 | 锚点 |
|---|---|
| 事实库建表（**仅 6 列**） | `desktop/src-tauri/src/memory.rs:54-62`、`:159-173` |
| 全局层 recall（纯时间序） | `desktop/src-tauri/src/memory.rs:437-466` |
| 注入侧 scope 恒 global | `desktop/src-tauri/src/memory.rs:532-544` |
| 注入块组装与钉死 | `crates/kernel-agent/src/middleware/memory_inject.rs:272-327`；预取常量 `:120` |
| 单条注入上限 500 字符 | `crates/kernel-agent/src/middleware/memory_inject.rs:117` |
| guidance 静态文本（无埋点） | `crates/kernel-agent/src/middleware/memory_inject.rs:128-137` |
| `memory_search` 的 LIKE 实现 | `plugins-src/memory-sqlite/src/store.rs:302-309` |
| 工具调用审计（仅覆盖工具） | `crates/kernel-agent/src/middleware/audit.rs:30-44` |
| 工具热度库（可挂载点） | `crates/kernel-agent/src/middleware/usage.rs:56-63` |
| md 回写索引（覆盖风险） | `desktop/src-tauri/src/memory_reconcile.rs:56-80` |
| 章节 expanded_at（落盘但不参与排序） | `crates/memory-tree/src/chapter.rs:86-94`、`tree.rs:289-301` |
| 章节 keywords/token_count 死字段 | `crates/memory-tree/src/chapter.rs:77-78` |
| 树目录只含当前会话 | `desktop/src-tauri/src/organize/assemble.rs:65-80`、`:159-173` |
| 既有设计稿（主轨道） | `docs/design/Memory Redesign (ZCode Alignment) v1.md` |

---

> **登记纪律**：本稿兑现后不搬走，靠 `INDEX.md` 状态行从「待研究」改标「已兑现」并登记批次
> （v2.149 批 E 起的规矩）。立稿时除本文与 INDEX.md 登记行外，**零文件改动、零代码改动**。
