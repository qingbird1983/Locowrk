# Locowrk 工具暴露策略与使用统计设计

> 配套项目：Locowrk 版本：1.0 前置文档：《Agent 内核设计》《插件元数据与打包设计 v2》《Agent 必备插件清单》

---

## 目录

 1. [设计目标](#1-%E8%AE%BE%E8%AE%A1%E7%9B%AE%E6%A0%87)
 2. [背景问题](#2-%E8%83%8C%E6%99%AF%E9%97%AE%E9%A2%98)
 3. [数据基础：使用统计](#3-%E6%95%B0%E6%8D%AE%E5%9F%BA%E7%A1%80%E4%BD%BF%E7%94%A8%E7%BB%9F%E8%AE%A1)
 4. [渐进式披露：三层机制](#4-%E6%B8%90%E8%BF%9B%E5%BC%8F%E6%8A%AB%E9%9C%B2%E4%B8%89%E5%B1%82%E6%9C%BA%E5%88%B6)
 5. [高频优先：动态热度](#5-%E9%AB%98%E9%A2%91%E4%BC%98%E5%85%88%E5%8A%A8%E6%80%81%E7%83%AD%E5%BA%A6)
 6. [完整暴露流水线](#6-%E5%AE%8C%E6%95%B4%E6%9A%B4%E9%9C%B2%E6%B5%81%E6%B0%B4%E7%BA%BF)
 7. [与 ctx-compact 协同](#7-%E4%B8%8E-ctx-compact-%E5%8D%8F%E5%90%8C)
 8. [插件闲置统计与停用](#8-%E6%8F%92%E4%BB%B6%E9%97%B2%E7%BD%AE%E7%BB%9F%E8%AE%A1%E4%B8%8E%E5%81%9C%E7%94%A8)
 9. [UI 呈现](#9-ui-%E5%91%88%E7%8E%B0)
10. [后端 API](#10-%E5%90%8E%E7%AB%AF-api)
11. [i18n 处理](#11-i18n-%E5%A4%84%E7%90%86)
12. [落地路径](#12-%E8%90%BD%E5%9C%B0%E8%B7%AF%E5%BE%84)
13. [常见坑](#13-%E5%B8%B8%E8%A7%81%E5%9D%91)
14. [设计哲学](#14-%E8%AE%BE%E8%AE%A1%E5%93%B2%E5%AD%A6)

---

## 1\. 设计目标

| 目标 | 含义 |
|---|---|
| 控制上下文开销 | 不让全部工具 schema 塞满上下文窗口 |
| 提升选择准确率 | 工具越少，模型挑对的概率越高 |
| 按需发现工具 | LLM 能主动检索未暴露的工具 |
| 高频优先 | 用户常用的工具自然浮上来 |
| 会话感知 | 本会话用过什么，下一轮优先保留 |
| 数据驱动决策 | 用户看到"多久没用"，不看"N 次调用" |
| 提示优先于自动 | 只提示闲置，不自动停用/卸载 |

### 非目标

* 不做调用次数趋势图
* 不做按小时/按天的使用分布
* 不做自动停用（可选设置项，默认关闭）
* 不做工具推荐的机器学习模型（EWMA + 向量检索够用）

---

## 2\. 背景问题

### 2.1 工具数量增长曲线

| 场景 | 工具数 | 上下文开销 | 模型表现 |
|---|---|---|---|
| 现在（echo/shout） | 2 | 忽略不计 | 好 |
| 第一批工具齐全 | \~10 | 2-5k token | 好 |
| 接一个 GitHub MCP | +30 | +6-15k token | 开始抖 |
| 接三个 MCP | +80 | +20-40k token | 本地模型直接崩 |
| 完整生态 | 150+ | 超上下文窗口 | 不可用 |

### 2.2 两个具体问题

**问题 1：上下文爆炸。** 每个工具的 JSON Schema 平均 200-500 token。100 个工具就是 20k-50k token，Ollama 上的 7B 模型窗口才 8k。

**问题 2：选择准确率下降。** 即使上下文够，模型在 100 个工具里挑对的准确率会显著下降。这不是模型问题，是信噪比问题。

### 2.3 反直觉的观察

**工具越多，模型表现越差。** GitHub Copilot 团队公开分享：把工具从 50 个砍到 15 个，任务成功率反而上升。

所以核心目标不是"让 LLM 看到所有工具"，而是"**让 LLM 只看它需要的那几个**"。

---

## 3\. 数据基础：使用统计

工具暴露、插件闲置统计、停用建议，都依赖同一份使用统计数据。

### 3.1 工具级统计（用于暴露打分）

```sql
CREATE TABLE tool_usage (
    tool_name     TEXT PRIMARY KEY,
    plugin_id     TEXT NOT NULL,
    call_count    INTEGER NOT NULL DEFAULT 0,
    success_count INTEGER NOT NULL DEFAULT 0,
    last_used_at  INTEGER NOT NULL,
    ewma_score    REAL NOT NULL DEFAULT 0.0
);

CREATE INDEX idx_tool_usage_plugin ON tool_usage(plugin_id);
```

`ewma_score` 是高频优先的核心，用于暴露打分（见 §5）。

### 3.2 插件级统计（用于闲置展示）

```sql
CREATE TABLE plugin_usage (
    plugin_id     TEXT PRIMARY KEY,
    first_used_at INTEGER NOT NULL,
    last_used_at  INTEGER NOT NULL
);
```

**只有两个时间戳。** 用于闲置等级分类（见 §8）。

**为什么插件级不存 `call_count`：** 用户对"调用 N 次"没有概念，只对"多久没用"有概念（详见 §14）。

### 3.3 写入时机

`ToolCall` 事件产生后**异步写**，不阻塞主循环。

```rust
async fn usage_writer(
    mut rx: mpsc::Receiver<ToolCallEvent>,
    db: SqlitePool,
) {
    let mut tool_pending: HashMap<String, i64> = HashMap::new();
    let mut plugin_pending: HashMap<String, i64> = HashMap::new();
    let mut ticker = tokio::time::interval(Duration::from_secs(5));

    loop {
        tokio::select! {
            Some(evt) = rx.recv() => {
                let now = unix_now();
                tool_pending.insert(evt.tool_name, now);
                plugin_pending.insert(evt.plugin_id, now);
            }
            _ = ticker.tick() => {
                flush_tool_usage(&db, &mut tool_pending).await;
                flush_plugin_usage(&db, &mut plugin_pending).await;
            }
        }
    }
}
```

**5 秒批量刷盘。** 崩溃时最多丢 5 秒统计数据。

### 3.4 ToolCall 事件需要 plugin_id

现有 `SessionEvent::ToolCall` 只有 `tool` 名字，需要补 `plugin_id`：

```rust
SessionEvent::ToolCall {
    call_id: CallId,
    tool: String,
    plugin_id: String,          // ← 新增
    args: Value,
    plugin_version: Version,
    side_effect: SideEffect,
}
```

serde 向后兼容，旧日志解析时缺失字段用 `default` 补。

### 3.5 会话内统计（不落盘）

从 SessionEvent 实时算，不存 DB：

```rust
impl Session {
    pub fn tool_usage_in_session(&self) -> HashMap<String, u32> {
        self.log.iter()
            .filter_map(|e| match e {
                SessionEvent::ToolCall { tool, .. } => Some(tool.clone()),
                _ => None,
            })
            .fold(HashMap::new(), |mut m, t| {
                *m.entry(t).or_insert(0) += 1;
                m
            })
    }
}
```

会话结束就没了。会话内的热度是临时的。

### 3.6 隐私

**只记工具名、插件 ID、时间戳。** 不记参数、不记结果、不记用户消息。

这个表可以放心同步到云端做推荐（如果将来做插件市场推荐）。

---

## 4\. 渐进式披露：三层机制

### 4.1 层次 1：静态优先级（manifest 声明）

在 `ToolSpec` 里加 `exposure` 字段：

```json
{
  "name": "fs_read",
  "description": "...",
  "schema": { ... },
  "side_effect": "Pure",
  "exposure": "always"
}
```

| 值 | 含义 | 典型工具 |
|---|---|---|
| always | 每次请求都暴露 | fs_read、fs_grep、edit_replace、fs_restore、search_tools |
| on_demand | 进候选池，按热度/检索激活 | fs_write、edit_lines、git_commit |
| hidden | 只有显式检索才暴露 | 危险工具、低频工具（现网唯一实例：memory_collapse） |

**默认值**：`on_demand`。作者不写，就当候选池处理。

**约束**：`always` 总数不超过 16 个（2026-10-06 由 10 上调——语义分层裁定后 读侧/编辑/撤销族整体入 `always`，见 §4.4）。再超仍在加载时一次性告警—— 那是作者的设计失误信号。

### 4.2 层次 2：语义检索（embedding）

对每个工具的 `description` 做 embedding，建索引。

```rust
pub struct ToolVectorIndex {
    embeddings: Vec<(ToolId, Vec<f32>)>,
    model: Arc<dyn EmbeddingModel>,
}

impl ToolVectorIndex {
    pub async fn search(&self, query: &str, top_k: usize) -> Vec<(ToolId, f32)> {
        let q_emb = self.model.encode(query).await;
        let mut scored: Vec<_> = self.embeddings.iter()
            .map(|(id, emb)| (id.clone(), cosine_similarity(&q_emb, emb)))
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        scored.truncate(top_k);
        scored
    }
}
```

每次发 LLM 请求前：

```rust
let query = last_n_user_messages(3);       // 最近 3 条用户消息
let candidates = index.search(&query, 15).await;
```

**降级策略**：embedding 模型未加载时，退回到关键词匹配（工具名 + description 的 BM25）。

### 4.3 层次 3：元工具（LLM 主动拉取）

暴露一个 `always` 的元工具：

```json
{
  "name": "search_tools",
  "description": "搜索完整工具目录。你上下文里的工具清单只是高频子集；动手绕行（shell 管道、反复读文件）之前先在这里查",
  "schema": {
    "type": "object",
    "properties": {
      "query": { "type": "string", "description": "描述你要做的事" }
    }
  },
  "exposure": "always",
  "side_effect": "Pure"
}
```

返回匹配的工具列表（名字 + 描述 + schema），另附 `catalog` 字段：结果之外的 全目录工具**名**（只给名字不给 schema——名字是廉价 token，schema 才是大头）。 LLM 拿到后，下一轮就能调用这些工具；全目录名字留在转录里，全会话有效。

**这个机制是兜底。** 前两层没覆盖的工具，LLM 可以主动发现。

**Claude Tool Search / Cursor skill 机制，本质都是这个思路。**

### 4.4 发现性补丁（2026-10-06，真实模型实测反馈）

**实测教训**（Timo 打地鼠会话，用户转述测试反馈）：模型想检索文件内容时 手写 shell grep 被拒，退回多次单文件读取——`fs_grep` 全程不在可见清单里， 且模型从没想过 search_tools。失败是**静默**的：不表现为「找不到工具」， 表现为「多花轮次 + 更差的替代品」，这类损耗不会自己浮出水面。

诊断：这是**发现性**问题，不是暴露量问题。失败模式有二，召回率再高也救 不了——①**不知道该搜**（没有触发信号）；②**想搜的时机不对**（等意识到 缺工具时已经绕完路）。另有一个结构缺陷放大二者：

**三道发现之门与断点**：

1. **进 Top-N 暴露集**——冷启动零信号时打分全同分，stable sort 按目录 注册序补位（= 抽签）；`pin_session` 使截断结果全会话冻结，会话中期的 绕行信号无法把被截工具浮上来（跨会话 EWMA 可以，当次会话不行）。
2. **被可见工具的 description 交叉引用**——引用图原本有断点：`fs_grep` / `fs_list` / `fs_delete` 全库零引用（最常用的内容检索恰是最孤儿； `fs_restore` 被 fs_write/edit_replace/edit_lines/edit_diff 四处引用， `edit_lines` 被 edit_replace 引用，线索本不缺）。
3. **模型自发调用 search_tools**——没有任何信号告诉模型「可见清单是 子集」，触发时机全靠运气。

**四条补丁（按 ROI；①②③落地，④离线审计不新建遥测）**：

1. **初始清单自述不完整**（成本一行字）：search_tools description 明说 「你看到的是高频子集」，触发时机从「不确定工具是否存在」提前到 「想绕行之前」。直接杀死失败模式①。
2. **语义分层，不按频率分层**：兜底/撤销类工具的存在价值就在意外时刻， 而意外时刻最不可能停下来探索——
   * `fs_restore` 恒暴露：它是**全部四个**改文件工具（fs_write / edit_replace / edit_lines / edit_diff，皆 Compensable）的唯一撤销 路径，整个编辑族的安全叙事压在它的可见性上；描述同步改写——删去 「Usually invoked by the compensation layer, not by the model」的 劝退措辞，明确模型可拿原调用的 {call_id, args, output} 手动自救。
   * `fs_grep` 恒暴露：读侧三件（read/list/grep）是模型的眼睛，内容 检索是编码循环的一等操作；Pure 零风险、schema ≈120 token，消的是 实测中代价最高的绕行模式。描述补「别用 shell grep 管道」指引。
   * `schedule_list` 恒暴露：能 `schedule_create` 就能查看自己建了什么 （创建/查看不对称是同类毛病）。
   * `edit_lines` / `fs_delete` / `fs_list` 维持 on_demand（真低频，符合 分层本意）。随批 always 预算 10 → 16（§4.1）。
3. **search_tools 结果附全目录名索引**：`catalog` 字段只给名字不给 schema——约 30 个名字 ≈150 token，一次检索后留在转录里全会话有效； resolve 本就不过滤（⑩②），看到名字即可调用。命中/降级/落空三条 返回路径都带。
4. **埋点不新建**：search_tools 调用（含 query 与 no_match）与 shell 调用（含完整命令）本就落会话 jsonl，离线审计即可量化「检索调用率 / 空命中率 / grep 型绕行」，数据够再谈下一步。

**明确不做**：全量铺开工具（上下文成本 + 选择噪声，且失败模式不在暴露 量上）；为会话中期发现去动 `pin_session` 做动态重排——tools 块逐轮变化 = 前缀缓存从 tools 位置起全灭（M1 纪律），用缓存代价换发现性是亏本 买卖，按需发现交给 search_tools 正是现管设计。

---

## 5\. 高频优先：动态热度

### 5.1 EWMA 打分

```rust
fn update_ewma(old: f64, now: i64, last: i64) -> f64 {
    let days = ((now - last) as f64 / 86400.0).max(0.0);
    let decay = (-days / 7.0).exp();   // 半衰期约 5 天
    old * decay + 1.0
}
```

**为什么用 EWMA 而不是纯计数：**

纯计数会让"三个月前用过 100 次"的工具永远压在榜首，但用户最近根本不用它。EWMA 有自然衰减，最近用的权重高。

### 5.2 两个维度

| 维度 | 用途 | 存储 | 权重 |
|---|---|---|---|
| 会话内热度 | 当前会话用过的工具优先保留 | 从 SessionEvent 实时算 | 1.0 |
| 全局热度 | 跨会话的长期偏好 | tool_usage 表 | 0.7 |

**会话内权重更高。** 用户在这个会话里用过 `git_diff`，下一轮就该继续暴露它。

### 5.3 组合公式

```
score(tool) = 
    1.0 * session_used(tool)           // 会话内用过，最高权重
  + 0.7 * global_ewma(tool)            // 全局热度
  + 0.5 * retrieval_score(tool)        // 语义相关度
  + 1.0 * is_always(tool)              // 静态 always
```

按 `score` 排序，取 top-N（N = 20，可配置）。

**权重的直觉：**

* `always` 无条件入选（满分）
* 会话内用过 → 至少 +1.0，基本稳进
* 全局热 → 最高 +0.7，需要配合其他信号
* 语义相关 → 最高 +0.5，作为补充信号

### 5.4 EWMA 更新时机

每次 `ToolResult` 事件产生后更新：

```rust
fn on_tool_result(event: &ToolResultEvent, db: &SqlitePool) {
    let now = unix_now();
    let tool_name = &event.tool_name;
    let success = event.error.is_none();

    tokio::spawn(async move {
        // 读旧值
        let old: f64 = sqlx::query_scalar(
            "SELECT ewma_score FROM tool_usage WHERE tool_name = ?1"
        )
        .bind(tool_name)
        .fetch_optional(&db).await.ok().flatten().unwrap_or(0.0);

        let last: i64 = /* 读 last_used_at */;
        let new_score = update_ewma(old, now, last);

        sqlx::query(
            "INSERT INTO tool_usage (tool_name, plugin_id, call_count, success_count, last_used_at, ewma_score)
             VALUES (?1, ?2, 1, ?3, ?4, ?5)
             ON CONFLICT(tool_name) DO UPDATE SET
                call_count = call_count + 1,
                success_count = success_count + ?3,
                last_used_at = ?4,
                ewma_score = ?5"
        )
        .bind(tool_name)
        .bind(&event.plugin_id)
        .bind(if success { 1 } else { 0 })
        .bind(now)
        .bind(new_score)
        .execute(&db).await.ok();
    });
}
```

---

## 6\. 完整暴露流水线

```
┌─────────────────────────────────────────────────────┐
│ 1. 收集所有 Active 插件的所有工具                     │
│    （来自 PluginRegistry，过滤掉停用插件）             │
└──────────────────┬──────────────────────────────────┘
                   ▼
┌─────────────────────────────────────────────────────┐
│ 2. 分层过滤                                          │
│    - always 集合 → 直接入选（不占额度）               │
│    - hidden 集合 → 直接排除                           │
│    - on_demand   → 进入候选池                         │
└──────────────────┬──────────────────────────────────┘
                   ▼
┌─────────────────────────────────────────────────────┐
│ 3. 候选池打分                                        │
│    - 会话内用过 → +1.0                               │
│    - 全局 EWMA   → +0.7 * ewma                       │
│    - 语义检索   → +0.5 * similarity                  │
└──────────────────┬──────────────────────────────────┘
                   ▼
┌─────────────────────────────────────────────────────┐
│ 4. Top-N 截断（N = 20）                              │
│    超出的工具进"待激活池"                             │
└──────────────────┬──────────────────────────────────┘
                   ▼
┌─────────────────────────────────────────────────────┐
│ 5. 注入 LLM 请求                                     │
│    tools = always ∪ top_n(on_demand)                 │
│    加上 search_tools 元工具                          │
└─────────────────────────────────────────────────────┘
```

### 6.1 执行位置

挂在 `agent/request` waterfall 上。

```rust
pub struct ToolExposureMiddleware {
    usage: Arc<UsageStore>,
    index: Arc<ToolVectorIndex>,
    max_tools: usize,           // 默认 20
}

#[async_trait]
impl WaterfallPlugin for ToolExposureMiddleware {
    async fn on_request(
        &self,
        ctx: &Context,
        req: &mut LlmRequest,
    ) -> WaterfallDecision {
        let all_tools = ctx.tool_registry().enabled_specs();
        let query = req.messages.last_n_user(3);
        let selected = self.select(all_tools, &query, &req.session_id).await;
        req.tools = selected;
        WaterfallDecision::Continue
    }
}
```

### 6.2 选中逻辑

```rust
impl ToolExposureMiddleware {
    async fn select(
        &self,
        all: Vec<ToolSpec>,
        query: &str,
        session_id: &SessionId,
    ) -> Vec<ToolSpec> {
        // 1. always 无条件入选
        let (always, rest): (Vec<_>, Vec<_>) = all.into_iter()
            .partition(|t| t.exposure == Exposure::Always);

        // 2. hidden 直接排除
        let candidates: Vec<_> = rest.into_iter()
            .filter(|t| t.exposure != Exposure::Hidden)
            .collect();

        // 3. 打分
        let session_hot = self.session_hot_tools(session_id);
        let retrievals: HashMap<_, _> = self.index
            .search(query, 15).await.into_iter().collect();

        let mut scored: Vec<_> = candidates.into_iter().map(|t| {
            let s_session = if session_hot.contains(&t.name) { 1.0 } else { 0.0 };
            let s_global = self.usage.ewma(&t.name) * 0.7;
            let s_retrieval = retrievals.get(&t.name).copied().unwrap_or(0.0) * 0.5;
            let score = s_session + s_global + s_retrieval;
            (t, score)
        }).collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        // 4. Top-N 截断
        let quota = self.max_tools.saturating_sub(always.len());
        let top_n: Vec<_> = scored.into_iter()
            .take(quota)
            .map(|(t, _)| t)
            .collect();

        // 5. 加上 search_tools 元工具
        let mut result = always;
        result.extend(top_n);
        result.push(self.search_tools_spec());
        result
    }
}
```

### 6.3 调试面板

做一个可观测的调试面板，显示：

```
本轮暴露：20 / 87 个工具
├─ always (3): fs_read, fs_list, search_tools
├─ session_hot (2): git_diff, fs_grep
├─ global_hot (8): ...
└─ retrieval (7): ...
节省上下文：约 18,500 token
```

### 6.4 验证指标

* 暴露数量稳定在 N ± 3
* 会话内用过 3 次以上的工具，下一轮 100% 在暴露列表里
* 检索召回的工具，用户下一轮真的用了（命中率 > 30%）

---

## 7\. 与 ctx-compact 协同

两个中间件都挂在 `agent/request`，顺序很重要：

```
Tracing → ToolExposure → ctx-compact → Timeout → ... → LLM
```

**ToolExposure 在前：** 先决定暴露哪些工具，压缩才知道要保留多少空间。

**ctx-compact 在后：** 它负责压缩历史消息。如果 ToolExposure 已经把 100 个工具的 schema 砍到 20 个，压缩压力会小很多。

**反过来会出问题：** 如果 ctx-compact 先跑，它可能把重要的工具描述也压掉了，导致 ToolExposure 拿到的是残缺的 tools 列表。

---

## 8\. 插件闲置统计与停用

### 8.1 停用 vs 卸载 vs 启用

| 操作 | 文件 | lockfile | active 指针 | Effect 撤销 | 重启后 |
|---|---|---|---|---|---|
| 停用 | 保留 | 保留，enabled=false | 移除 | 不撤销 | 保持停用 |
| 启用 | 已在 | 改 enabled=true | 指向 | — | 保持启用 |
| 卸载 | 删除 | 删除 | 移除 | 执行 | 不再存在 |

**关键决策：停用不撤销 Effect。**

* 停用目的：减少上下文负担和误触发概率
* 释放内存是卸载的职责
* 不撤销 Effect 让"再次启用"变成瞬时操作

### 8.2 停用后 LLM 看不到工具

停用状态的插件，其所有工具不进入 `ToolExposureMiddleware` 的候选池，也不在 `search_tools` 的索引里。

**但保留在注册表里**，随时可以重新启用。

**停用 = 从上下文里移除，等于立刻释放 token 空间。** 这是停用的核心价值。

### 8.3 闲置等级分类

```rust
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IdleLevel {
    Fresh,          // 7 天内用过
    Normal,         // 7-30 天
    Idle,           // 30-90 天
    Stale,          // 90-180 天
    Abandoned,      // 180 天以上
    NeverUsed,      // 装了 ≥ 14 天从未用过
}
```

| 等级 | 触发条件 | 视觉 | 建议 |
|---|---|---|---|
| Fresh | ≤ 7 天 | 无标记 | 无 |
| Normal | 7-30 天 | 无标记 | 无 |
| Idle | 30-90 天 | 灰色小字 | 无 |
| Stale | 90-180 天 | 黄色徽章 | 可考虑停用 |
| Abandoned | > 180 天 | 红色徽章 | 建议停用或卸载 |
| NeverUsed | 装了 ≥ 14 天从未用 | 黄色徽章 | 可考虑卸载 |

**试用期保护：** 装了 < 14 天且从未使用 → 显示"刚装上"，不给任何提示。

### 8.4 人类尺度的时间文案

**永远用"多久没用"，不用"多少天未使用"。**

| ❌ 错误 | ✅ 正确 |
|---|---|
| 3 天前 | 本周用过 |
| 14 天未使用 | 上周用过 |
| 47 天未使用 | 1 个多月没用了 |
| 94 天未使用 | 3 个多月没用了 |
| 187 天未使用 | 半年多没用了 |
| 412 天未使用 | 一年多没用了 |

### 8.5 结构化输出

```rust
pub struct IdleInfo {
    pub level: IdleLevel,
    pub value: Option<u32>,       // 数字，如 3
    pub unit: Option<IdleUnit>,   // Week / Month / Year
}

pub enum IdleUnit {
    Today,        // 今天用过
    ThisWeek,     // 本周用过
    LastWeek,     // 上周用过
    LastMonth,    // 上个月用过
    Month(u32),   // N 个多月
    HalfYear,     // 半年多
    Year(u32),    // N 年多
    NeverUsed,    // 从未使用
}
```

### 8.6 完整文案表

| 天数区间 | level | value | unit |
|---|---|---|---|
| 0-1 | Fresh | — | Today |
| 2-6 | Fresh | — | ThisWeek |
| 7-13 | Fresh | — | LastWeek |
| 14-29 | Normal | — | LastMonth |
| 30-59 | Idle | 1 | Month |
| 60-89 | Idle | 2 | Month |
| 90-179 | Stale | 3 | Month |
| 180-364 | Abandoned | — | HalfYear |
| ≥ 365 | Abandoned | 1+ | Year |

### 8.7 从未使用的情况

按"安装后多久"分级：

| 装了多久 | 文案 |
|---|---|
| 0-13 天 | 刚装上 |
| 14-30 天 | 装上两周了，还没用过 |
| 31-90 天 | 装了 1 个多月，一次没用过 |
| 91-180 天 | 装了 3 个多月，一次没用过 |
| > 180 天 | 装了半年多，一次没用过 |

**"从未使用"和"长时间未用"必须区分。** 前者是"你可能不需要"，后者是"最近没用"。

### 8.8 只提示，不自动执行

**三条理由：**

1. 用户可能就是那段时间不需要，将来会需要
2. 自动停用会让用户觉得应用背着自己做事
3. 状态切换应该永远由用户显式触发

**唯一例外：** 设置项里的"自动停用 N 天未使用的插件"，默认关闭。

**提示出现的时机：**

* 用户**主动打开**"已安装"页面时
* **不要**在应用启动时弹 toast
* **不要**在聊天过程中打断用户

**闲置提示是"用户来找"的信息，不是"推给用户"的通知。**

---

## 9\. UI 呈现

### 9.1 已安装列表

```
大小写转换      v1.1.0   ✓ 运行中                   [停用] [卸载]
网页抓取        v0.3.0   ✓ 运行中   上周用过         [停用] [卸载]
Shell 执行      v0.9.0   ⏸ 已停用   3 个多月没用了   [启用] [卸载]
已废弃工具      v0.1.0   ⏸ 已停用   半年多没用了 ⚠️   [启用] [卸载]
某测试插件      v0.1.0   ⏸ 已停用   装了 3 个月，一次没用过  [启用] [卸载]
```

**只显示一句"多久没用"。** 没有次数、没有趋势、没有图表。

### 9.2 筛选器

```
[全部] [运行中] [已停用] [超过 1 个月没用] [超过 3 个月没用] [半年以上没用]
```

筛选器文案也用"多久"，不用"N 天"。

### 9.3 顶部提示条

```
💡 有 4 个插件超过 3 个月没用了，[去看看]
```

**不是**"有 4 个插件超过 90 天未使用"。中文里，3 个月比 90 天更好懂。

### 9.4 徽章配色

```css
.idle-fresh       { color: transparent; }         /* 无标记 */
.idle-normal      { color: transparent; }         /* 无标记 */
.idle-idle        { color: var(--text-muted); }   /* 灰色小字 */
.idle-stale       { background: #FACC15; color: #1a1d21; }  /* 黄色 */
.idle-abandoned   { background: #F87171; color: #1a1d21; }  /* 红色 */
.idle-never-used  { background: #FACC15; color: #1a1d21; }  /* 黄色 */
```

### 9.5 批量操作

多选 + 批量停用/卸载：

```
☐ 全选

[停用选中] [启用选中] [卸载选中]
```

**卸载批量操作要二次确认**，且明确列出将被删除的插件名。

### 9.6 工具暴露调试面板

面向开发者的可观测面板（设置里的"开发者选项"）：

```
本轮暴露：20 / 87 个工具
├─ always (3): fs_read, fs_list, search_tools
├─ session_hot (2): git_diff, fs_grep
├─ global_hot (8): fs_write, git_status, ...
└─ retrieval (7): http_post, sql_query, ...
节省上下文：约 18,500 token
```

---

## 10\. 后端 API

### 10.1 扩展 PluginUiState

```rust
#[derive(Serialize)]
pub struct PluginUiState {
    pub id: String,
    pub version: String,
    pub state: PluginState,           // installed / active / loading / error
    pub enabled: bool,                // 启用/停用状态

    // 元数据
    pub display_name: Option<HashMap<String, String>>,
    pub description:  Option<HashMap<String, String>>,
    pub icon: Option<String>,

    // 使用情况
    pub first_installed_at: i64,
    pub last_used_at: Option<i64>,
    pub idle: IdleInfo,               // 结构化闲置信息

    pub side_effects: Vec<SideEffectInfo>,
}
```

**前端不做任何时间计算。**

### 10.2 命令列表

```rust
/// 列出所有已安装插件（含闲置信息）
#[tauri::command]
async fn plugin_list_with_usage(...) -> Result<Vec<PluginUiState>, String>;

/// 启用/停用插件
#[tauri::command]
async fn plugin_set_enabled(
    id: String,
    enabled: bool,
    ...
) -> Result<(), String>;

/// 批量停用
#[tauri::command]
async fn plugin_batch_disable(
    ids: Vec<String>,
    ...
) -> Result<BatchResult, String>;

/// 批量卸载
#[tauri::command]
async fn plugin_batch_uninstall(
    ids: Vec<String>,
    ...
) -> Result<BatchResult, String>;

/// 工具暴露调试信息（开发者选项）
#[tauri::command]
async fn tool_exposure_debug(
    session_id: String,
    ...
) -> Result<ExposureDebugInfo, String>;
```

### 10.3 停用必须落盘

```rust
pub async fn set_enabled(&self, id: &str, enabled: bool) -> Result<(), String> {
    if enabled {
        let ver = self.lockfile.lock().await.version_of(id)?;
        self.registry.load().activate(id, &ver)?;
    } else {
        self.registry.load().deactivate(id);
    }

    // 落盘 enabled 状态
    self.lockfile.lock().await.set_enabled(id, enabled)?;

    self.state_tx.send(PluginStateChange::EnabledChanged {
        id: id.into(), enabled,
    }).ok();
    Ok(())
}
```

启动时只加载 `enabled: true` 的插件：

```rust
for entry in lockfile.lock().plugins.values() {
    if entry.enabled {
        registry.load().load(&entry.id, &entry.version, &path)?;
    }
}
```

---

## 11\. i18n 处理

### 11.1 后端只返回结构化数据

```rust
pub struct IdleInfo {
    pub level: IdleLevel,
    pub value: Option<u32>,
    pub unit: Option<IdleUnit>,
}
```

**不返回中文文案。** 让前端根据当前 locale 渲染。

### 11.2 前端渲染

```typescript
// desktop/src/lib/idle-text.ts
export function renderIdle(
  info: IdleInfo,
  t: (k: string, p?: any) => string,
): string {
  switch (info.unit) {
    case 'never_used': return t('idle.never_used');
    case 'today':      return t('idle.today');
    case 'this_week':  return t('idle.this_week');
    case 'last_week':  return t('idle.last_week');
    case 'last_month': return t('idle.last_month');
    case 'half_year':  return t('idle.half_year');
    case 'month':      return t('idle.month', { n: info.value });
    case 'year':       return t('idle.year', { n: info.value });
    default:           return '';
  }
}
```

### 11.3 翻译文件

`locales/zh-CN.json`：

```json
{
  "idle": {
    "today": "今天用过",
    "this_week": "本周用过",
    "last_week": "上周用过",
    "last_month": "上个月用过",
    "month": "{n} 个多月没用了",
    "half_year": "半年多没用了",
    "year": "{n} 年多没用了",
    "never_used": "装了这么久，一次没用过"
  }
}
```

`locales/en-US.json`：

```json
{
  "idle": {
    "today": "Used today",
    "this_week": "Used this week",
    "last_week": "Used last week",
    "last_month": "Used last month",
    "month": "Unused for over {n} month(s)",
    "half_year": "Unused for over 6 months",
    "year": "Unused for over {n} year(s)",
    "never_used": "Installed but never used"
  }
}
```

**切换语言无需重新请求。**

---

## 12\. 落地路径

### 12.1 阶段划分

| 阶段 | 内容 | 复杂度 | 依赖 |
|---|---|---|---|
| 1 | ToolSpec 加 exposure 字段，默认 on_demand | ★ | — |
| 2 | always 集合直接暴露，on_demand 全暴露 | ★ | 阶段 1 |
| 3 | plugin_usage 表 + 后台写入 task | ★★ | ToolCall 事件补 plugin_id |
| 4 | IdleLevel 分类 + IdleInfo 结构 | ★ | 阶段 3 |
| 5 | plugin_list_with_usage 命令 + 前端渲染 | ★ | 阶段 4 |
| 6 | 筛选器 + 批量操作 | ★ | 阶段 5 |
| 7 | tool_usage 表 + EWMA 更新 | ★★ | 阶段 3 |
| 8 | Top-N 截断 + 调试面板 | ★★ | 阶段 7 |
| 9 | 会话内热度统计 | ★ | 阶段 8 |
| 10 | search_tools 元工具 | ★★ | 阶段 8 |
| 11 | 向量检索（embedding 索引） | ★★★ | 阶段 10 |
| 12 | 可选：自动停用设置项 | ★★ | 阶段 6 |

**1-6 一天能做完。7-10 两天能做完。11 单独排期。**

### 12.2 前 6 个阶段做完，你已经拥有

* 完整的工具暴露控制（`always` / `on_demand` / `hidden`）
* 插件闲置统计和停用机制
* 用户能做决策的 UI

### 12.3 后 5 个阶段做完，你拥有

* 高频工具自动优先
* LLM 主动检索工具
* 语义级别的工具推荐
* 上下文开销可控

---

## 13\. 常见坑

| 坑 | 原因 | 解法 |
|---|---|---|
| 停用后重启又变运行中 | 只改内存没落盘 lockfile | set_enabled 必须写 lockfile |
| 统计延迟高 | 每次调用同步写 DB | 5 秒批量刷盘 |
| 从未使用显示成"0 天前" | 没处理 last_used_at IS NULL | 单独走 NeverUsed 分支 |
| 语言切换文案不更新 | 后端返回中文文案 | 只返回结构化数据 |
| 提示太频繁 | 每次启动都弹 | 只在已安装页显示 |
| 批量卸载误操作 | 没有二次确认 | 列出待删插件名 |
| 停用后工具还在候选池 | 中间件没过滤 enabled | ToolExposure 过滤停用插件 |
| 试用期用户被打扰 | 装完 3 天就提示"未使用" | 14 天试用期保护 |
| EWMA 分数爆炸 | 半衰期设错 | decay = exp(-days/7)，半衰期约 5 天 |
| 工具暴露抖动 | 每轮打分变化大 | 加"上一轮已暴露"作为额外信号 |
| embedding 模型加载慢 | 阻塞主循环 | 异步加载，未就绪时降级到关键词 |
| search_tools 结果为空 | 索引未建或查询太窄 | 兜底返回 top-10 高频工具 |
| 向量索引内存占用大 | 工具数量增长 | 量化到 f16 或 int8 |

---

## 14\. 设计哲学

### 14.1 用户需要的不是数据，是决策依据

"用了 47 次" 是数据。 "3 个月没用了" 是决策依据。

在 UI 上，永远优先展示后者。

这条原则不仅适用于闲置统计，也适用于所有 Agent 相关的界面：

* 不显示 "Token 消耗 8723"，显示 "接近上下文上限"
* 不显示 "工具调用 5 次"，显示 "正在分析、搜索、整理"
* 不显示 "延迟 342ms"，显示 "响应稍慢"

**人是来做事的，不是来看数字的。**

### 14.2 工具越多，模型越差

GitHub Copilot 团队的经验：50 个工具砍到 15 个，成功率上升。

这告诉我们：**暴露策略的核心不是"让 LLM 看到所有工具"，而是"让 LLM 只看它需要的那几个"。**

`search_tools` 元工具是这个哲学的极致体现——平时不占上下文，需要时主动拉取。

### 14.3 提示而非自动

自动停用插件，看起来是"贴心"，实际是"越界"。

用户的插件列表是他的领地。应用可以建议，不能代劳。

**唯一的例外是安全**：如果一个插件被标记为 `Irreversible` 且长期未使用，可以强制弹窗确认是否保留。这不是"越界"，是"保护"。

### 14.4 双时间尺度

* **秒级**：工具暴露（每次请求都重算）
* **天级**：插件闲置（每次打开页面才更新）

两者不互相干扰，各用各的时间尺度。

不要因为"用户今天用过一次"就把闲置等级从 `Abandoned` 变成 `Fresh`。**插件级的闲置统计要平滑**，用 `last_used_at` 就够了，不需要 EWMA。

---

*文档版本：v1.0* *配套项目：Locowrk* *前置文档：《Agent 内核设计》《插件元数据与打包设计 v2》《Agent 必备插件清单》*