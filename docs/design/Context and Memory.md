# Locowrk 上下文与记忆管理设计

> 配套项目：Locowrk
> 版本：1.0
> 前置文档：《Agent 内核设计》《工具暴露策略与使用统计设计》《Agent 必备插件清单》

---

## 目录

1. [设计目标](#1-设计目标)
2. [核心概念区分](#2-核心概念区分)
3. [上下文管理](#3-上下文管理)
4. [记忆管理](#4-记忆管理)
5. [协同工作流](#5-协同工作流)
6. [数据模型](#6-数据模型)
7. [后端 API](#7-后端-api)
8. [i18n 处理](#8-i18n-处理)
9. [落地路径](#9-落地路径)
10. [常见坑](#10-常见坑)
11. [设计哲学](#11-设计哲学)

---

## 1. 设计目标

| 目标 | 含义 |
|---|---|
| **不崩** | 长对话不因 token 超限而失败 |
| **不贵** | 控制 Token 消耗，避免二次方级成本爆炸 |
| **不失忆** | 跨会话能记住用户偏好和项目上下文 |
| **用户可控** | 阈值可调、策略可选、压缩可撤销 |
| **透明** | 用户随时知道当前上下文占用和压缩历史 |
| **可降级** | embedding 模型未就绪时，降级到关键词检索 |

### 非目标

- 不做跨设备记忆同步（第一版）
- 不做记忆的自动学习（隐式提取放最后）
- 不做记忆的可视化编辑（只做查看和删除）
- 不做上下文的多级分层缓存（第一版用扁平结构）

---

## 2. 核心概念区分

**上下文管理和记忆管理是两件事，不是一件事的两个名字。**

| | 上下文管理 | 记忆管理 |
|---|---|---|
| **时间尺度** | 当前这一轮请求 | 跨会话、跨天、跨月 |
| **处理对象** | 当前 Session 的消息历史 | 长期存储的知识和经历 |
| **核心问题** | "窗口装不下了怎么办" | "下次怎么想起来" |
| **典型动作** | 压缩、裁剪、摘要 | 存储、检索、注入 |
| **生命周期** | 会话内 | 永久 |
| **形态** | 中间件（`ctx-compact`） | 插件（`memory-*`） |
| **不做的后果** | 长对话直接崩 | Agent 每次都是失忆症 |
| **优先级** | **P0（立刻做）** | P1（下一步） |

**一句话：上下文管理是"给 LLM 看的这一屏"，记忆管理是"放在书架上的那本书"。**

---

## 3. 上下文管理

### 3.1 为什么需要

#### 3.1.1 二次方级成本

Agent 每一轮请求都要把完整历史重新发一遍。

假设每轮平均产生 1000 token：

| 轮次 | 本轮新增 | 发送给 LLM 的累计 | 累计总消耗 |
|---|---|---|---|
| 1 | 1,000 | 1,000 | 1,000 |
| 5 | 1,000 | 5,000 | 15,000 |
| 10 | 1,000 | 10,000 | 55,000 |
| 20 | 1,000 | 20,000 | 210,000 |
| 50 | 1,000 | 50,000 | 1,275,000 |
| 100 | 1,000 | 100,000 | 5,050,000 |

**N 轮对话的总消耗是 O(N²)。** 这就是用户体感上"指数级消耗"的原因。

#### 3.1.2 窗口限制

Ollama 上的 7B 模型窗口是 8k token。聊 5 轮工具调用，加上工具 schema，就爆了。爆了之后要么报错，要么被截断，模型开始胡言乱语。

#### 3.1.3 结论

**上下文管理不做，长对话直接不可用。** 这是 P0，不是 P1。

### 3.2 触发阈值

#### 3.2.1 可配置滑条

**档位：10% - 90%，10% 一级。**

```
┌──────────────────────────────────────────────┐
│  上下文压缩阈值                                │
│                                              │
│  10%  20%  30%  40%  50%  60%  70%  80%  90% │
│   ●────●────●────●────●────●────●────●────●   │
│                    ▲                         │
│                    │                         │
│                  当前: 40%                    │
│                                              │
│  当前模型: deepseek-flash (128k 窗口)         │
│  触发压缩: 51,200 token                       │
│                                              │
│  ⓘ 建议值: 25-35%                             │
└──────────────────────────────────────────────┘
```

> **v2.141 注记（Organize Summary Design v1）**：整理收束额外产出一份**轮级总结**
> （该轮全部章节的浓缩叙事，`===SUMMARY===` 尾随标记段，rule 降级无总结宁缺勿假），
> 在下一轮开场排空成 `SessionEvent::ContextSummary` 落日志——历史消息面常驻、
> 断层缝合处、随下次整理滚动收编；system 目录与前缀缓存零扰动。总结链跨会话
> 聚合即项目进展叙事锚点（tree.md / 项目联合视图）。详见《Organize Summary Design v1》。

#### 3.2.2 阈值语义（v2.137 修订：绝对 token，不再按窗口百分比折算）

> **v2.137 修订（2026-10-09，用户裁定「上下文根本不压缩了」）**：本节原「百分比给判断、绝对 token 给感知」的双显示规范随百分比阈值一并退役——整理阈值改**绝对 token**（`context.thresholdTokens`，「对话上下文达到多少 K 就触发记忆树整理」），滑块 100K–800K 每 100K 一档、滑动气泡跟随显示当前档位；`thresholdPct`/`strategy`/`stabilityRounds` 与 ctx-compact 桌面链同批退役（settings 存量键幻影化）。窗口不再参与阈值折算；窗口本身由模型档**自动识别**（会话模型行 `context_window` ?? models.dev 目录 ?? 档位 `window_tokens`，`session_window` 四层链——v2.137 前只认手填档位窗口，用量环算不出百分比的本批根因），喂用量环百分比与阈值用量进度条的比例段。整理阈值需低于所用模型窗口，否则上下文超限先于整理发生。下方旧表保留作历史依据（§8 的 i18n 样例键同为历史 mock）。
>
> **v2.138 注记（2026-10-09，压力实测口径分裂修复）**：「上下文用量」的**分子**统一为供应商真实 input（日志最后一条 `llm_usage.input`，即 `prompt_tokens` 含缓存命中）——用量环、阈值用量进度条、浮层「已用」与整理触发/自动收起同一把尺（`runner::reported_input_tokens`，**「看到的 = 触发用的」**：进度条到线必然伴随整理）。背景：压力实测 UI 显 110K（字符估算 +18%）而真实 95K 未过 100K 线，进度条 112% 全红整理却不触发。无供应商上报（一步未跑/不报用量）回落估算（树组装口径 → 全量字符估算）；真实值滞后最后一条 assistant 回复一步（下次请求真算），如实边界。

**原规范（v2.137 前有效）：百分比是给用户判断的，绝对 token 数是给用户感知的。**

同样 40%，在不同窗口下含义完全不同：

| 模型窗口 | 40% 对应的 token |
|---|---|
| Ollama 7B (8k) | 3,200 |
| GPT-4o (128k) | 51,200 |
| Claude (200k) | 80,000 |
| Gemini (1M) | 400,000 |

UI 上必须两行都显示，不然用户会误判。

#### 3.2.3 动态建议值

**窗口越大，建议值越低。** 因为绝对 token 数是二次方增长的。

```rust
fn suggested_threshold(window_size: u64) -> u32 {
    match window_size {
        0..=16_000       => 30,   // 本地小模型
        16_001..=64_000  => 35,   // 中等窗口
        64_001..=256_000 => 25,   // 大窗口
        _                => 15,   // 1M 及以上
    }
}
```

| 场景 | 建议阈值 | 理由 |
|---|---|---|
| **本地小模型**（Ollama 7B/8B） | 30% | 窗口本来就小，压缩要早 |
| **中等窗口**（32k-128k） | 35% | 平衡成本和体验 |
| **大窗口**（200k+） | 25% | 绝对值太大，压缩要早 |
| **1M 窗口** | 15% | 20% 时二次方成本已经显著 |
| **省钱优先** | 15% | 频繁压缩，成本最低 |
| **质量优先** | 60% | 尽量保留上下文 |

**UI 上的"建议值"应该动态计算，而不是写死。**

#### 3.2.4 下限警告

**阈值不能设得太低。**

每次压缩调 LLM 做摘要，本身也要发一遍当前上下文。如果每 2 轮就压一次，压缩成本会超过节省的成本。

```
压缩成本 = 发送当前上下文 + 接收摘要
```

**建议下限：20%。** 再低就得给用户警告：

> ⚠️ 更低的阈值可能导致压缩开销超过节省的成本。

### 3.3 压缩策略

三种策略，用户可选：

| 策略 | 行为 | 信息损失 | 成本 |
|---|---|---|---|
| **滑动窗口** | 保留最近 N 轮，丢掉更早的 | 高 | 低（不调 LLM） |
| **分层保留**（默认） | 系统提示 + 摘要 + 最近 3-5 轮原文 | 低 | 中 |
| **仅摘要** | 全部压缩成一段摘要 | 中 | 高（要调 LLM） |

#### 3.3.1 滑动窗口

最简单、最快、最省。纯本地裁剪，不调 LLM。

```
[System Prompt]          ← 保留
[最近 N 轮原文]           ← 保留
[本轮用户消息]            ← 保留
[更早的所有历史]           ← 丢弃
```

**适用**：本地小模型用户、成本极度敏感、对上下文连续性要求不高。

#### 3.3.2 分层保留（推荐默认）

```
[System Prompt]          ← 永远完整
[早期对话摘要]            ← 用 LLM 压缩成一段话
[最近 3-5 轮原文]         ← 完整保留
[本轮用户消息]            ← 完整保留
```

**适用**：绝大多数场景。它是最平衡的。

#### 3.3.3 仅摘要

```
[System Prompt]
[全部历史压缩成一段摘要]
[本轮用户消息]
```

**适用**：极端长对话、需要保持全局连贯性、不在乎压缩成本。

### 3.4 压缩目标

**不是压到阈值以下，是压到阈值的 50%。**

```rust
let threshold = window_size * user_threshold_pct / 100;
let target = threshold / 2;   // 压到一半
```

**理由**：如果只压到刚好低于阈值，下一轮马上又超了，压缩会变得非常频繁，每次都要调 LLM 做摘要，反而更贵。

**示例**：阈值 40%（51,200 token），触发时压到 25,600 token。给下一轮留足够的缓冲。

### 3.5 执行位置

挂在 `agent/request` waterfall 上。

```rust
pub struct ContextCompactMiddleware {
    settings: Arc<SettingsStore>,
}

#[async_trait]
impl WaterfallPlugin for ContextCompactMiddleware {
    async fn on_request(
        &self,
        ctx: &Context,
        req: &mut LlmRequest,
    ) -> WaterfallDecision {
        let window_size = ctx.model().window_size().await;
        let threshold_pct = self.settings.context_threshold();
        let threshold = window_size * threshold_pct / 100;

        let current = estimate_tokens(&req.messages);
        if current > threshold {
            let target = threshold / 2;
            let strategy = self.settings.compact_strategy();
            match compact(&mut req.messages, target, strategy).await {
                Ok(result) => {
                    ctx.emit("context/compacted", result).await;
                }
                Err(e) => {
                    tracing::warn!("compact failed: {}", e);
                    // 降级到滑动窗口
                    fallback_sliding_window(&mut req.messages, target);
                }
            }
        }

        WaterfallDecision::Continue
    }
}
```

### 3.6 中间件顺序

```
Tracing → ToolExposure → Memory-Inject → ctx-compact → Timeout → LLM
```

**顺序很重要：**

1. **`ToolExposure`** 先跑——先决定暴露哪些工具，压缩才知道要保留多少空间
2. **`Memory-Inject`** 次跑——把相关记忆塞进上下文
3. **`ctx-compact`** 后跑——检查总 token，超了就压

**反过来不行。** 如果先压缩，记忆还没注入，压缩了也没用。

### 3.7 事件记录与撤销

压缩动作写入 `SessionEvent`：

```rust
SessionEvent::ContextCompacted {
    from_tokens: u64,
    to_tokens: u64,
    strategy: CompactStrategy,
    summary_preview: String,     // 摘要的前 200 字符
    ts: i64,
}
```

**用户能"回退到压缩前"**——Event Sourcing 的又一次应用。

```rust
session.rewind_to(compacted_event_seq - 1);
```

### 3.8 UI 呈现

#### 3.8.1 设置面板

```
┌─────────────────────────────────────────────┐
│  上下文管理                                  │
│                                             │
│  压缩阈值                                    │
│  10%  20%  30%  40%  50%  60%  70%  80%  90%│
│   ○────○────●────○────○────○────○────○────○  │
│                                             │
│  当前: 30% · 38,400 token                    │
│  窗口: 128,000 token                         │
│                                             │
│  ⓘ 建议 30-40%                              │
│     窗口越大，建议值越低                      │
│                                             │
│  ─────────────────────────────────────────   │
│                                             │
│  压缩策略                                    │
│  ○ 滑动窗口（只保留最近 N 轮）               │
│  ● 分层保留（系统提示 + 摘要 + 最近原文）      │
│  ○ 仅摘要（全部压缩）                        │
│                                             │
│  ─────────────────────────────────────────   │
│                                             │
│  ☑ 压缩时通知我                              │
│  ☐ 压缩前先问我                              │
│                                             │
└─────────────────────────────────────────────┘
```

#### 3.8.2 会话内实时指示

在对话界面顶部或底部，显示进度条：

```
上下文: ████████░░░░░░░░░░  40%  (51,200 / 128,000)
                                  ↑ 接近压缩阈值
```

**颜色分级：**
- 绿色（< 60% 阈值）
- 黄色（60-99% 阈值）
- 红色（≥ 阈值，即将触发）

#### 3.8.3 压缩事件提示

触发压缩时，在会话里插入一条系统消息：

```
📦 已压缩早期对话，节省 26,000 token
   [查看详情] [撤销]
```

"查看详情"展示摘要内容，"撤销"回到压缩前状态。

---

## 4. 记忆管理

### 4.1 两种记忆

| 类型 | 存什么 | 检索方式 | 插件 |
|---|---|---|---|
| **事实记忆** | 用户偏好、项目结构、常用路径 | 按 key 精确查 / 按标签查 | `memory-sqlite` |
| **经历记忆** | 聊过的话题、做过的决策 | 语义相似度检索 | `memory-vector` |

**第一版只做 `memory-sqlite` 就够。** 向量检索是第二版。

### 4.2 事实记忆（memory-sqlite）

#### 4.2.1 数据结构

```sql
CREATE TABLE memories (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    key         TEXT NOT NULL,          -- 如 "user.language_preference"
    value       TEXT NOT NULL,          -- 如 "Rust"
    tags        TEXT,                   -- JSON 数组，如 ["preference", "language"]
    source      TEXT NOT NULL,          -- "explicit" | "extracted"
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL,
    access_count INTEGER NOT NULL DEFAULT 0,
    last_accessed_at INTEGER
);

CREATE INDEX idx_memories_key ON memories(key);
CREATE INDEX idx_memories_tags ON memories(tags);
```

#### 4.2.2 写入时机

**第一版只做显式写入。**

用户说"记住：我偏好用 `thiserror` 而不是 `anyhow`"，Agent 调用 `memory_write` 工具：

```json
{
  "name": "memory_write",
  "description": "把一条事实写入长期记忆",
  "schema": {
    "type": "object",
    "properties": {
      "key": { "type": "string", "description": "唯一标识，如 user.preference.error_lib" },
      "value": { "type": "string", "description": "要记住的内容" },
      "tags": { "type": "array", "items": { "type": "string" } }
    },
    "required": ["key", "value"]
  },
  "side_effect": "Pure",
  "exposure": "always"
}
```

**隐式提取放最后做。** 它容易记一堆没用的东西，反而污染上下文。

> **⚠ v2.136：隐式提取（autoExtract）已按用户裁定整体退役**（v2.73 曾落地、实践确认无作用：召回无来源加权会被 auto.* 霸榜、与树章节内容两遍整理）。本小节按原设计留档。

> **⚠ v2.186（后审计队列 Q3 =《Memory Redesign (ZCode Alignment) v1》§5 提案 A）＝对上一行那条裁定的部分反转，反转面被刻意压到最小**：整理管道收束时记录员在「章节数组 + 轮级总结」之外**顺带**产出 ≤N 张事实卡并 upsert 进 `memories`（`source = "organized"`，管理页徽章单独一档）。它与 v2.136 退役的那件事不是同一件，五条差别逐条对齐原裁定的理由：①退役的是「**每轮**跑一条独立提取管线」，本件挂在「**整理收束**」上——触发频率 = 整理频率而不是轮频率，节奏/阈值/反弹熔断全部复用 `should_auto_organize`，**零新增调度器**；②不新增任何一次 LLM 调用（记录员本来就在读整段材料，只是让它多吐一段）；③不再造 `auto.*` 前缀族，键名由模型按**既有 key 清单**（带层标进 user 段）查重后自取 ⇒ 「被 auto.* 霸榜」的那个失效面没有回来的入口；④`memory.factCards` 可整面关掉，关掉 = 发给记录员的 user 段**逐字节回到 v2.185 形状**、一张都不落库（判据 `cards_switch_off_leaves_the_prompt_byte_identical_and_writes_nothing`）；⑤**注入面一字不动**——v2.136「项目侧全提醒制 + 正文不自动注入」与 v2.185 的 `<project_memory_index>` 指针节都原样有效，本批只给那节指针多了内容来源（这正是它治的问题：稿 §0-1 实证 `memories` 表 0 行 = 索引节有框架而没货）。实现真源 = `crates/memory-tree/src/manager/facts.rs`（`===FACTS===` 标记、契约文本、解析层四道钳）+ `desktop/src-tauri/src/organize/facts.rs`（`type`→层 的归层映射、查重清单读取、落库失败不反噬整理）。降级语义：规则切分路径**不产卡**（产不出就不产，不硬造）。设置两键 = `memory.factCards`（缺省开）+ `memory.factCardMax`（1–10，缺省 3）。同批兑现 Q2 留的欠账：`MemoryStore::write` 的 workspace 键位改走 `workspace::scope_key` 归一（宿主写路径与插件写路径从此同键位，md 目录名同源）。

#### 4.2.3 检索与注入

> **⚠ v2.136 注入口径修订（项目侧全提醒制）**：注入只取**全局层**（用户层面的喜好/全局要求，保持简短）；项目层事实不再自动注入（模型 memory_read / memory_search 按需查）；`<repo>/.Locowrk/` 项目记忆以**静态提醒段**指路（正文不注入，主模型按需读写）。

> **⚠ v2.185（后审计队列 Q2 =《Memory Redesign (ZCode Alignment) v1》§4）在本口径之上加一节指针，不反转它**：注入带新增 `<project_memory_index>`（位置 = `<persistent_memory>` 之后、`<memory_guidance>` 之前），内容只允许是本项目已存事实的**一行式指针**（`- <key> — <tags>（N 天前）`；优先取 `<数据根>/memory/projects/<项目>/MEMORY.md` 的指针行，md 缺失或"有文件但一行指针都没有"时由 sqlite 该键位的行现合成、**不回写文件**；200 行 / 25,000 字符双钳，超限附一行指路 WARNING）。**事实正文仍然不自动注入**——v2.136 禁的是把正文灌进 system，而本节治的是那条禁令的副作用：模型连"本项目记了哪些 key"都不知道，空结果容易被读成"这个项目没记过东西"。实现真源 = `desktop/src-tauri/src/memory_index.rs` + `memory.rs::project_index`（`MemorySource` 的默认方法返回 `None` = 内核不拼标签，未归属会话整节缺席）。同批三件：项目级**常驻进展行**（`organize/progress.rs`，解冻《Cross-Session Recall Design v1》批 2）拼在树目录段**之前**并按会话钉死；`memory_project_history` 的 exposure 由 `OnDemand` 提为 `Always`（否则被 Top-20 截断挤出目录，模型看不见工具存在）；`<memory_guidance>` 增两句「空结果 ≠ 没有历史」与把开场禁令限定为「没被问到时不主动翻」（措辞真源 = `memory_inject.rs` 的 `GUIDANCE_BLOCK`，《Prompt Assembly Layering Design》§5.2 的 After 文本已随本批订正）。

在 `agent/request` 里，根据最近用户消息提取 key/tag 候选，查询匹配的记忆。

**第一版用最简单的策略：按 `access_count` + `last_accessed_at` 排序，取 top-5。**

```rust
async fn inject_memories(
    db: &SqlitePool,
    messages: &[Message],
) -> Vec<Memory> {
    // 从最近用户消息里提取关键词
    let query = extract_keywords(messages);

    sqlx::query_as::<_, Memory>(
        "SELECT * FROM memories
         WHERE key LIKE ?1 OR tags LIKE ?1
         ORDER BY access_count DESC, last_accessed_at DESC
         LIMIT 5"
    )
    .bind(format!("%{}%", query))
    .fetch_all(db).await.unwrap_or_default()
}
```

### 4.3 经历记忆（memory-vector）

> **⚠ v2.136：经历记忆层整体退役**（唯一写入方 autoExtract 的 episode 随删；M5 起关缺省观察一个版本后由用户裁定移除）。本节按原设计留档。

**第二版再做。**

#### 4.3.1 数据结构

```sql
CREATE TABLE experiences (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    content     TEXT NOT NULL,          -- 一段经历描述
    embedding   BLOB NOT NULL,          -- f32 数组的二进制
    session_id  TEXT NOT NULL,
    created_at  INTEGER NOT NULL
);
```

#### 4.3.2 检索

```rust
async fn recall_experiences(
    index: &ExperienceIndex,
    query: &str,
    top_k: usize,
) -> Vec<Experience> {
    let q_emb = index.model.encode(query).await;
    index.search(&q_emb, top_k).await
}
```

**降级策略**：embedding 模型未加载时，退回到关键词匹配（BM25）。

### 4.4 记忆工具集

给 LLM 暴露三个工具：

| 工具 | 作用 | exposure |
|---|---|---|
| `memory_write` | 写入一条记忆 | `always` |
| `memory_read` | 按 key 读取 | `always` |
| `memory_search` | 按关键词/语义检索 | `always` |

**这三个工具应该是 `always`** —— 让模型始终能主动存取记忆。

### 4.5 记忆的可视化与删除

设置面板里加一个"记忆"页：

```
┌─────────────────────────────────────────────┐
│  长期记忆                                    │
│                                             │
│  🔍 搜索记忆...                              │
│                                             │
│  user.preference.error_lib                  │
│    值: 偏好使用 thiserror 而不是 anyhow       │
│    标签: preference, rust                    │
│    来源: 用户显式 · 3 天前                    │
│    [编辑] [删除]                             │
│                                             │
│  project.locowrk.structure                  │
│    值: 内核在 crates/cordis-core             │
│    标签: project, structure                  │
│    来源: 用户显式 · 1 周前                    │
│    [编辑] [删除]                             │
│                                             │
│  [清空全部记忆]                              │
└─────────────────────────────────────────────┘
```

**用户必须能看到并删除每一条记忆。** 这既是隐私要求，也是信任基础。

### 4.6 项目级隔离（㉘，用户 2026-10-04 指令："分项目 = 项目级上下文、项目级记忆，不能相互串、相互污染"）

**会话属于哪个项目，就只见哪个项目的记忆。** 归属 = 会话旁车 `workspace` 字段
（§2.3/UI shell 文档，v2.50 关联），未归属会话 = 独立的"未归属键位"。

* **双层记忆模型（㉘·A·fix，用户 2026-10-04 晚裁定："跨会话记忆要关闭——有那个记忆在整个工作区就会乱；记忆正常就两层：全局记忆记用户喜好偏好、全局起作用；项目记忆记这个项目的记录、不能跨项目读到"）**：
  * **全局层（`workspace=''`）**：只放**用户喜好偏好**（"永远用 pnpm"这类跨项目成立的约定），对**所有会话**可见——归属与未归属 alike。聊天工具可写：`memory_write` 显式 `scope="global"`；**归属会话缺省写不进全局**（宁严勿松），未归属会话缺省写**拒绝**（没有项目的"项目记忆"不许落地污染全局，模型该先归属或显式升格为偏好）；headless（无会话上下文）保持 `''` 单一宇宙语义不变。全局行聊天工具**禁删**（memory_delete 只作用本项目键位；全局行的删除是人的操作，走记忆管理页）。
  * **项目层（`(workspace, key)`）**：项目记录，**绝不跨项目可读**——读取（注入召回 / memory_read / memory_search / memory_list）= 本项目键位 ∪ 全局层，行带层标（`scope: "project" | "global"`）让模型知道来源层级；写入缺省落本项目。经历（§4.3）不设全局层，维持严格本项目。
* **隔离规则（严格对称）**：每条事实记忆 / 经历带 `workspace`；写入打标 = 写入会话的归属；项目行只见与读取会话归属**完全一致**的行（全局层是唯一例外，见上）。
* **键位即主键**：memory.sqlite3 主键 = `(workspace, key)`——同名 key 在不同
  项目下并存、互不覆盖（覆盖即污染）。旧库（key 单主键）首次访问整表迁移，
  旧行归入未归属键位；插件侧与宿主侧持等价迁移（探针 + BEGIN IMMEDIATE +
  二次探针，并发只有一方执行）。
* **键位归一化（v2.185 · 队列 Q2 · MR §6，趁 `memories` 表 0 行做掉）**：`(workspace, key)`
  的 workspace 分量**进记忆层之前**一律过 `plugin_api::wskey::normalize`（词法 canonicalize +
  剥 Windows verbatim 前缀 `\\?\` / `\\?\UNC\`；仅 Windows 折叠大小写并把 `/` 统一成 `\`，
  POSIX 只剥尾分隔符且大小写不动；空串/纯空白 = 未归属键位；路径不存在 = 回落词法形式，
  既不报错也**绝不归成空串**——那会把某项目的记忆污染进全局偏好层）。归一只落在**记忆键位
  边界**两处：插件 `memory-sqlite` 的 `WsCtx::from_parts`（写侧 scope、读侧过滤、md 目录名三处
  同源于此）与宿主 `workspace::scope_key`（注入键位 / 观测 `scope_id`），两者逐字同形；
  **不改 `CallContext.workspace` 的下发串**——权限围栏按 `starts_with` 比原始路径，改下发面会
  整体失配并要求重编全部插件。树投影目录名（`memory-tree/` 的 fnv8 编码）与 md 项目目录名
  的换名属 **M3（Q10，涉用户数据目录，动手前单独确认）**，本批未动、存量四个 md 目录照旧。
* **注入即隔离**：Memory-Inject / Experience-Inject 的召回契约带 `session_id`
  （kernel 只透传 id，workspace 是宿主概念）；宿主解析后按键位过滤。子 Agent
  会话没有旁车（⑮ 黑盒）→ **注入只见全局层、缺省写拒绝**（v2.68 起，见上；
  真偏好可 `scope="global"`）——不随父项目走是已知边界（诚实且跨项目零泄漏）。
* **归属先行（㉘·A·fix，v2.68）**：首聊挂起归属（`chat.pendingWorkspace`）随
  `agent_chat` 请求带入后端，**建会话即写旁车、先于首轮任何注入 / 围栏 /
  提取**——v2.66 之前"轮次收束后才 session_associate"的时序缝让首轮注入落在
  未归属键位、还被 M1 会话级预取缓存整场（Glance-main 会话引用 Locowrk 旧
  记忆的实机事故根因之一）。预取缓存键 = `(session_id, 键位身份)`（MemorySource
  新 `scope_id`，缺省空串 = 旧行为）：会话中途改归属 = 换键自动重算，M1 纪律
  （键位不变 = 前缀逐字节稳定）不破。
* **注入告知工作区（㉘·A·fix，v2.68）**：归属会话的注入块头部带一行当前项目
  工作区（MemorySource 新 `context_line`，缺省 `None`）——工作区此前只是权限
  边界、对模型不可见，"看下这个项目"只能靠旧记忆猜路径（实机事故根因之二）；
  未归属会话不带该行。该行逐会话稳定，同吃预取缓存、不伤前缀命中率。
* **工具面同隔离**（"模型看不见的数据才是真隔离"）：插件经 HostVtable 末尾
  追加的 `session_context` 槽（append-only，⑭ notify 先例；调用期线程上下文 +
  组装点注入解析器，headless 不注入 = 全局语义回落）反向查询本次调用归属，
  memory-sqlite 全工具面按键位过滤；undo 忠实还原原键位（快照带 workspace）。
* **管理页是人的管理面**：看全部键位并透出归属（「项目 X」徽章），跨项目可见
  不算串——隔离针对的是模型上下文，不是管理员。
* 借鉴来源：PI-Desktop 的项目模型（per-project memory blob + containment 跟
  会话走 + "memory is prompt-static, no cross-project retrieval surface"）；
  Locowrk 取"按行打标 + 键位过滤"形态（记忆是行式 KV 而非整块文档）。

---

## 5. 协同工作流

### 5.1 完整流程图

```
┌─────────────────────────────────────────────────────────┐
│  agent/request waterfall                                 │
│                                                         │
│  ① ToolExposure                                         │
│     决定本轮暴露哪些工具                                  │
│                       ↓                                 │
│  ② Memory-Inject                                        │
│     从 memory-sqlite / memory-vector 检索相关记忆         │
│     注入到 system prompt 或消息开头                       │
│                       ↓                                 │
│  ③ ctx-compact                                          │
│     估算 token，超阈值则压缩                              │
│                       ↓                                 │
│  ④ 发给 LLM                                             │
└─────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────┐
│  agent/post-step（对话结束后，异步）                      │
│                                                         │
│  ⑤ Memory-Extract（可选，后期）                          │
│     从本轮对话提取"值得记住的事实"                        │
│     写入 memory-sqlite                                   │
└─────────────────────────────────────────────────────────┘
```

### 5.2 顺序的理由

**为什么 Memory-Inject 在 ctx-compact 之前？**

因为记忆是"原料"，上下文管理是"分配预算"。原料要先放进来，才知道总量是多少，才能决定怎么压。

**为什么 ToolExposure 最先？**

因为工具 schema 占用的 token 也要计入总量。先确定工具集，再算总 token，才知道要压多少。

### 5.3 冲突处理

**情况 1：记忆太多，压缩又压不掉**

说明记忆检索太宽泛。收窄检索条件（top-3 而不是 top-5），或者按 `access_count` 优先。

**情况 2：压缩把记忆也压掉了**

不应该。`ctx-compact` 必须**保留 `Memory-Inject` 注入的记忆内容**，只压缩历史消息。

**实现**：注入的记忆放在系统提示里，系统提示永远不被压缩。

**情况 3：用户刚写了一条记忆，下一轮请求没带上**

记忆写入是异步的，可能在下一轮请求时还没落盘。用户会感知到"我说了记住，它却没记住"。

**解决**：`memory_write` 工具本身是 `Pure` 的，执行完立即同步写 DB（不走异步队列）。下一轮请求必然读到。

---

## 6. 数据模型

### 6.1 SQLite 表汇总

```sql
-- 事实记忆
CREATE TABLE memories (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    key               TEXT NOT NULL,
    value             TEXT NOT NULL,
    tags              TEXT,
    source            TEXT NOT NULL,
    created_at        INTEGER NOT NULL,
    updated_at        INTEGER NOT NULL,
    access_count      INTEGER NOT NULL DEFAULT 0,
    last_accessed_at  INTEGER
);

-- 经历记忆（第二版）
CREATE TABLE experiences (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    content     TEXT NOT NULL,
    embedding   BLOB NOT NULL,
    session_id  TEXT NOT NULL,
    created_at  INTEGER NOT NULL
);

-- 压缩历史
CREATE TABLE compaction_history (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id      TEXT NOT NULL,
    from_tokens     INTEGER NOT NULL,
    to_tokens       INTEGER NOT NULL,
    strategy        TEXT NOT NULL,
    summary_preview TEXT,
    ts              INTEGER NOT NULL
);

CREATE INDEX idx_compaction_session ON compaction_history(session_id);
```

### 6.2 设置项

```rust
pub struct ContextSettings {
    pub threshold_pct: u32,           // 10-90，默认 30
    pub strategy: CompactStrategy,    // 默认 Layered
    pub notify_on_compact: bool,      // 默认 true
    pub ask_before_compact: bool,     // 默认 false
}

pub struct MemorySettings {
    pub enabled: bool,                // 默认 true
    pub max_inject_count: usize,      // 默认 5
    pub auto_extract: bool,           // 默认 false
}
```

---

## 7. 后端 API

### 7.1 上下文相关

```rust
/// 获取当前会话的上下文占用
#[tauri::command]
async fn context_usage(session_id: String) -> Result<ContextUsage, String>;

#[derive(Serialize)]
pub struct ContextUsage {
    pub current_tokens: u64,
    pub window_size: u64,
    pub threshold_tokens: u64,
    pub threshold_pct: u32,
    pub percent: f32,
    pub strategy: CompactStrategy,
}

/// 获取压缩历史
#[tauri::command]
async fn context_compaction_history(
    session_id: String,
) -> Result<Vec<CompactionRecord>, String>;

/// 手动触发压缩
#[tauri::command]
async fn context_compact_now(session_id: String) -> Result<CompactResult, String>;

/// 撤销最后一次压缩
#[tauri::command]
async fn context_undo_compact(session_id: String) -> Result<(), String>;
```

### 7.2 记忆相关

```rust
/// 列出记忆
#[tauri::command]
async fn memory_list(
    query: Option<String>,
    limit: u32,
) -> Result<Vec<Memory>, String>;

/// 删除记忆
#[tauri::command]
async fn memory_delete(id: i64) -> Result<(), String>;

/// 编辑记忆
#[tauri::command]
async fn memory_update(id: i64, value: String) -> Result<(), String>;

/// 清空记忆
#[tauri::command]
async fn memory_clear_all() -> Result<u32, String>;
```

### 7.3 设置相关

```rust
#[tauri::command]
async fn settings_get_context() -> Result<ContextSettings, String>;

#[tauri::command]
async fn settings_set_context(settings: ContextSettings) -> Result<(), String>;

#[tauri::command]
async fn settings_get_memory() -> Result<MemorySettings, String>;

#[tauri::command]
async fn settings_set_memory(settings: MemorySettings) -> Result<(), String>;
```

---

## 8. i18n 处理

### 8.1 后端只返回结构化数据

```rust
pub enum CompactStrategy {
    SlidingWindow,
    Layered,
    SummaryOnly,
}
```

**不返回中文文案。** 让前端根据当前 locale 渲染。

### 8.2 前端渲染

```typescript
// desktop/src/lib/compact-text.ts
export function renderStrategy(
  s: CompactStrategy,
  t: (k: string) => string,
): string {
  return t(`compact.strategy.${s.toLowerCase()}`);
}

export function renderCompactionNotice(
  from: number,
  to: number,
  t: (k: string, p?: any) => string,
): string {
  return t('compact.notice', {
    saved: (from - to).toLocaleString(),
  });
}
```

### 8.3 翻译文件

`locales/zh-CN.json`：

```json
{
  "compact": {
    "title": "上下文管理",
    "threshold": "压缩阈值",
    "current": "当前: {pct}% · {tokens} token",
    "window": "窗口: {tokens} token",
    "suggest": "建议 {min}-{max}%",
    "suggest_hint": "窗口越大，建议值越低",
    "low_warning": "更低的阈值可能导致压缩开销超过节省的成本",
    "strategy": {
      "slidingwindow": "滑动窗口（只保留最近 N 轮）",
      "layered": "分层保留（系统提示 + 摘要 + 最近原文）",
      "summaryonly": "仅摘要（全部压缩）"
    },
    "notice": "已压缩早期对话，节省 {saved} token",
    "view_detail": "查看详情",
    "undo": "撤销"
  },
  "memory": {
    "title": "长期记忆",
    "search": "搜索记忆...",
    "source": {
      "explicit": "用户显式",
      "extracted": "自动提取"
    },
    "empty": "还没有记忆",
    "clear_all": "清空全部记忆"
  }
}
```

`locales/en-US.json`：

```json
{
  "compact": {
    "title": "Context Management",
    "threshold": "Compaction Threshold",
    "current": "Current: {pct}% · {tokens} tokens",
    "window": "Window: {tokens} tokens",
    "suggest": "Suggested {min}-{max}%",
    "suggest_hint": "Larger windows suggest lower thresholds",
    "low_warning": "Lower thresholds may cost more than they save",
    "strategy": {
      "slidingwindow": "Sliding window (keep last N turns)",
      "layered": "Layered (system + summary + recent)",
      "summaryonly": "Summary only (compress all)"
    },
    "notice": "Compacted early messages, saved {saved} tokens",
    "view_detail": "View details",
    "undo": "Undo"
  },
  "memory": {
    "title": "Long-term Memory",
    "search": "Search memories...",
    "source": {
      "explicit": "Explicit",
      "extracted": "Auto-extracted"
    },
    "empty": "No memories yet",
    "clear_all": "Clear all memories"
  }
}
```

---

## 9. 落地路径

### 9.1 阶段划分

| 阶段 | 内容 | 复杂度 | 优先级 |
|---|---|---|---|
| 1 | 设置面板加阈值滑条 + token 数显示 | ★ | **立刻** |
| 2 | `ctx-compact` 中间件（滑动窗口） | ★ | **立刻** |
| 3 | 会话内实时进度条 | ★ | **立刻** |
| 4 | 动态建议值（根据窗口大小） | ★ | 高 |
| 5 | 分层保留策略 + LLM 摘要 | ★★ | 高 |
| 6 | 压缩事件写入 SessionEvent + 撤销 | ★★ | 高 |
| 7 | `memory-sqlite` + `memory_write` 工具 | ★★ | 中 |
| 8 | `Memory-Inject` 中间件 | ★ | 中 |
| 9 | 记忆管理 UI（列表 + 删除） | ★ | 中 |
| 10 | `memory-vector` 语义检索 | ★★★ | 低 |
| 11 | 隐式提取（对话结束后自动） | ★★ | 低 |

### 9.2 关键节点

**阶段 1-3 一天能做完。** 这是最小可用版本，做完你现在的 Ollama 对话不再崩。

**阶段 1-6 两天能做完。** 此时上下文管理功能完整。

**阶段 7-9 两天能做完。** 此时记忆管理功能完整。

**阶段 10-11 单独排期。** 等真实需求出现再做。

### 9.3 现在该做什么

**做阶段 1-3。** 理由：

1. 你刚接上 Ollama，本地模型窗口小，不压缩聊 3 轮就崩
2. 这是"不做就不可用"的 P0，不是"做了更好"的 P1
3. 滑条 + 滑动窗口压缩，是投入产出比最高的一组

做完这 3 步，再回来做记忆管理。

---

## 10. 常见坑

| 坑 | 原因 | 解法 |
|---|---|---|
| 压缩后模型失忆 | 摘要丢掉了关键信息 | 分层保留，系统提示永不压缩 |
| 压缩太频繁 | 阈值设太低 | 下限 20%，加警告 |
| 压缩本身很贵 | 每次调 LLM 做摘要 | 阈值不能太低，或改用滑动窗口 |
| 压缩后下一轮又超 | 只压到阈值，没留缓冲 | 压到阈值的 50% |
| 记忆被压缩掉 | 记忆注入了但压缩没保护 | 记忆放在 system prompt，永不压缩 |
| 用户看不到压缩发生 | 没有 UI 反馈 | 插入系统消息 + 顶部进度条 |
| 压缩不可撤销 | 没有事件记录 | `ContextCompacted` 事件 + `rewind_to` |
| token 估算不准 | 用了错误的 tokenizer | 用与当前模型匹配的 tokenizer |
| 记忆太多污染上下文 | top-k 太大 | 默认 top-5，可配置 |
| 用户不知道记了什么 | 没有可视化 | 记忆管理页 + 删除按钮 |
| embedding 模型加载慢 | 阻塞主循环 | 异步加载，未就绪时降级到关键词 |
| 跨模型压缩失败 | 摘要模型和主模型不一致 | 用主模型做摘要，或允许配置独立摘要模型 |

---

## 11. 设计哲学

### 11.1 上下文是资源，不是垃圾

**压缩不是"清理垃圾"，是"分配预算"。**

用户的历史消息没有"垃圾"——每一条都曾经有意义。压缩的目的是在**有限的窗口**里，让**最重要的信息**留下来。

所以压缩策略的核心不是"丢什么"，是"**留什么**"。

### 11.2 用户要的是"别崩"，不是"精确控制"

绝大多数用户不需要知道 token 数、不需要知道压缩算法、不需要知道阈值。他们只想"别崩"。

所以：
- **默认值必须开箱即用**（分层保留 + 30% 阈值）
- **高级选项必须存在但不打扰**（滑条在设置里，不在主界面）
- **状态必须可见但不抢戏**（进度条在顶部，不弹窗）

**默认值设计得好，90% 的用户永远不会打开这个设置。**

### 11.3 记忆是"用户的东西"

**用户必须能看到、能编辑、能删除每一条记忆。**

这不是隐私合规要求，是信任基础。一个"偷记东西"的 Agent，会让用户不安。

**做法：**
- 记忆列表默认可见（不用藏）
- 每条记忆显示来源（显式/自动）
- 一键清空（不需要二次确认以外的任何阻力）
- 记忆写入时，可以在会话里显示一条系统消息："已记住：xxx"

### 11.4 透明优于聪明

**用户看到"已压缩早期对话，节省 26,000 token"，比看到"系统已优化性能"更有信任感。**

具体数字、具体动作、具体结果，永远比模糊的"优化"、"智能处理"更让人放心。

这也和 LoCo 的产品哲学一致：**Low Cost Communication 不只是"省钱"，也是"让用户看得清、算得明"。**

### 11.5 退路永远存在

**任何自动动作都要能撤销。**

- 压缩了 → 能撤销回压缩前
- 记忆写错了 → 能编辑、能删除
- 阈值调错了 → 随时能改回来

**Event Sourcing 给了这一切一个统一的实现路径。** 所有状态变化都写入日志，所有日志都能回放。这是 Locowrk 的底层能力，不只是给上下文管理用的。

---

*文档版本：v1.0*
*配套项目：Locowrk*
*前置文档：《Agent 内核设计》《工具暴露策略与使用统计设计》《Agent 必备插件清单》*