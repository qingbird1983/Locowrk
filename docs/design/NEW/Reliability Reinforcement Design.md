# Locowrk 可靠性补强设计

> 配套项目：Locowrk 版本：1.0 前置文档：《Agent 内核设计》《记忆树设计》《LLM 客户端与调度层设计》 来源：借鉴 Kiso 的"因果链"思路，补强 Locowrk 的可靠性边界

---

## 目录

 1. [设计目标](#1-%E8%AE%BE%E8%AE%A1%E7%9B%AE%E6%A0%87)
 2. [五组必须分开的东西](#2-%E4%BA%94%E7%BB%84%E5%BF%85%E9%A1%BB%E5%88%86%E5%BC%80%E7%9A%84%E4%B8%9C%E8%A5%BF)
 3. [Revision Guard：防止过期观察覆盖新现实](#3-revision-guard%E9%98%B2%E6%AD%A2%E8%BF%87%E6%9C%9F%E8%A7%82%E5%AF%9F%E8%A6%86%E7%9B%96%E6%96%B0%E7%8E%B0%E5%AE%9E)
 4. [Turn Commit：整轮干净结束才授予执行资格](#4-turn-commit%E6%95%B4%E8%BD%AE%E5%B9%B2%E5%87%80%E7%BB%93%E6%9D%9F%E6%89%8D%E6%8E%88%E4%BA%88%E6%89%A7%E8%A1%8C%E8%B5%84%E6%A0%BC)
 5. [Unknown Effect：崩溃恢复时不猜](#5-unknown-effect%E5%B4%A9%E6%BA%83%E6%81%A2%E5%A4%8D%E6%97%B6%E4%B8%8D%E7%8C%9C)
 6. [只读证明：从"白名单匹配"升级为"证明只读"](#6-%E5%8F%AA%E8%AF%BB%E8%AF%81%E6%98%8E%E4%BB%8E%E7%99%BD%E5%90%8D%E5%8D%95%E5%8C%B9%E9%85%8D%E5%8D%87%E7%BA%A7%E4%B8%BA%E8%AF%81%E6%98%8E%E5%8F%AA%E8%AF%BB)
 7. [子 Agent 独立验收](#7-%E5%AD%90-agent-%E7%8B%AC%E7%AB%8B%E9%AA%8C%E6%94%B6)
 8. [In-Band Summary：摘要复用现有前缀](#8-in-band-summary%E6%91%98%E8%A6%81%E5%A4%8D%E7%94%A8%E7%8E%B0%E6%9C%89%E5%89%8D%E7%BC%80)
 9. [内核行数门禁](#9-%E5%86%85%E6%A0%B8%E8%A1%8C%E6%95%B0%E9%97%A8%E7%A6%81)
10. [落地路径](#10-%E8%90%BD%E5%9C%B0%E8%B7%AF%E5%BE%84)
11. [常见坑](#11-%E5%B8%B8%E8%A7%81%E5%9D%91)
12. [设计哲学](#12-%E8%AE%BE%E8%AE%A1%E5%93%B2%E5%AD%A6)

---

## 1\. 设计目标

| 目标 | 含义 |
|---|---|
| 用户能信任 | Agent 做的事，系统说得清、追得到、恢复得了 |
| 不静默破坏 | 不会因为过期观察覆盖用户的修改 |
| 不半截执行 | 不会因为模型断流触发意外副作用 |
| 不猜 | 崩溃恢复时，不确定的事不自动重试 |
| 不伪造完成 | 模型说"完成了"不等于任务完成 |
| 边界可审计 | 任何一条因果链都能追溯 |

### 非目标

* 不追求"零崩溃"（崩溃是现实，要处理它，不是避免它）
* 不追求"全自动恢复"（不确定的情况就该问人）
* 不做分布式共识（单机场景，单写者模型）

---

## 2\. 五组必须分开的东西

**这五组区别，每一组都是用户会踩的坑。**

| 区别 | 含义 | 对应机制 |
|---|---|---|
| Observed ≠ Current | 模型看见过 ≠ 现实现在还是那样 | Revision Guard |
| Intent ≠ Effect | 模型产生意图 ≠ 现实已改变 | Turn Commit |
| Started ≠ Succeeded | 操作开始 ≠ 操作成功 | Unknown Effect |
| Process Completion ≠ Goal Satisfaction | Loop 结束 ≠ 目标达成 | 子 Agent 独立验收 |
| Memory ≠ Durable Fact | 进程记得 ≠ 崩了还在 | Event Log 唯一真相源 |

**这五条是设计的地基。所有具体机制都是它们的落地。**

---

## 3\. Revision Guard：防止过期观察覆盖新现实

### 3.1 问题场景

```
T0: AI 读取 main.rs，看到内容 C1
T1: AI 思考 30 秒，构造补丁
T2: 用户在此期间用 IDE 改了 main.rs → 内容变成 C2
T3: AI 提交补丁，基于 C1
T4: 补丁应用到 C2 上 → 用户的修改被静默覆盖
```

**结果：用户改了东西，AI 又给改回去了。用户不知道为什么。**

### 3.2 设计

**核心原则**：任何"基于观察的修改"，都必须验证观察还有效。

**read 时返回 revision**：

```json
{
  "path": "src/main.rs",
  "content": "...",
  "revision": "sha256:abc123...",   // 内容的哈希
  "mtime": 1790770863,
  "size": 1234
}
```

**edit 时必须带上 revision**：

```json
{
  "path": "src/main.rs",
  "expected_revision": "sha256:abc123...",
  "old_string": "...",
  "new_string": "..."
}
```

**edit 执行前重新读文件**：

```rust
let current = std::fs::read(path)?;
let current_rev = hash(&current);

if current_rev != expected_revision {
    return Err(ToolError::StaleObservation {
        expected: expected_revision,
        actual: current_rev,
        hint: "文件已被修改，请重新 fs_read 后再编辑",
    });
}

// revision 一致，执行修改
```

### 3.3 拒绝时的错误信息

```
错误：文件的当前状态和你观察时不一致。

你观察时的 revision: sha256:abc123
文件当前的 revision: sha256:def456

可能原因：
- 用户在 IDE 里修改了这个文件
- 其他进程写入了这个文件
- 另一个 Agent 会话修改了它

建议：
1. 重新 fs_read 这个文件，查看当前内容
2. 基于新内容重新构造编辑
```

**这条错误信息直接告诉 AI 下一步做什么。** AI 会自己重新读取。

### 3.4 例外：新建文件不需要 revision

```json
{
  "path": "src/new_file.rs",
  "expected_revision": null,   // null = 期望文件不存在
  "content": "..."
}
```

**如果文件已存在，拒绝。** 防止意外覆盖。

### 3.5 覆盖范围

**所有写操作都要带 revision**：

| 工具 | 是否需要 revision |
|---|---|
| fs_read | — （产生 revision） |
| fs_list | — |
| fs_write | ✅ 需要 |
| edit_replace | ✅ 需要 |
| edit_diff | ✅ 需要 |
| edit_lines | ✅ 需要 |
| fs_delete | ✅ 需要（防误删已改的文件） |

---

## 4\. Turn Commit：整轮干净结束才授予执行资格

### 4.1 问题场景

```
模型流式输出：
  "我要删除这个文件：" 
  [tool_call: fs_delete("/important.txt")]
  ... 断流了 ...
```

**如果系统一看到 `tool_call` 就执行，就删了。**

**但模型的输出可能没完成——它可能还会说"等等，先备份一下"。**

### 4.2 设计

**核心原则**：整轮 response 干净结束，工具调用才获得执行资格。

**两个阶段**：

```
┌─────────────────────────────────────┐
│  阶段 1：Draft（草稿）                │
│                                     │
│  模型流式输出中的一切                 │
│  - tool_call 只是"意图"              │
│  - 不写事件日志                      │
│  - 不执行任何工具                     │
└─────────────────────────────────────┘
                ↓
        整轮 response 完成
        格式校验通过
                ↓
┌─────────────────────────────────────┐
│  阶段 2：Commit（提交）              │
│                                     │
│  1. 写入 durable Turn Commit 事件    │
│  2. 工具调用获得执行资格              │
│  3. 逐个执行 + 写 Receipt            │
└─────────────────────────────────────┘
```

### 4.3 什么算"干净结束"

| 条件 | 说明 |
|---|---|
| 流式完整接收 | 收到 \[DONE\] 或等效终止符 |
| 格式合法 | JSON 解析通过 |
| 消息完整 | 有 content 或有 tool_calls，不是空响应 |
| 终止原因合法 | stop 或 tool_calls，不是 length（截断） |

**任何一个不满足 → 整轮作废，不执行任何工具。**

### 4.4 事件日志的记录

```rust
SessionEvent::AssistantMessage {
    content: String,
    tool_calls: Vec<ToolCall>,
    committed: bool,           // ← 关键字段
}
```

**只有 `committed: true` 的消息，其 tool_calls 才有执行资格。**

崩溃恢复时，扫描未 commit 的消息，全部丢弃。

### 4.5 截断的特殊处理

如果模型输出因为 token 限制被截断（`finish_reason: "length"`）：

* **不 commit**
* 把已输出的部分作为"未完成消息"记录
* 提示用户"模型输出被截断，是否需要继续"
* 不执行任何工具调用

**因为截断意味着模型还没说完，它的完整意图可能不同。**

---

## 5\. Unknown Effect：崩溃恢复时不猜

### 5.1 问题场景

```
T0: 写入 durable tool_execution_started
T1: 开始执行 shell("rm -rf build/")
T2: 进程被 SIGKILL
T3: 重启，发现只有 STARTED，没有 Receipt

问题：build/ 删了没有？
```

**无法从本地证据判断。**

### 5.2 设计

**核心原则**：不确定的副作用永远不自动重试。

**三种恢复状态**：

| 状态 | 证据 | 处理 |
|---|---|---|
| 未提交 | 有 Draft，无 Commit | 丢弃 draft，重新请求模型 |
| 已提交未执行 | 有 Commit，无 STARTED | 可以安全执行（副作用还没发生） |
| 执行中 | 有 STARTED，无 Receipt | 标记 uncertain，问用户 |
| 已完成 | 有 STARTED，有 Receipt | 正常继续 |

### 5.3 Uncertain 状态的 UI

恢复时，如果发现 uncertain 的操作，弹出处理界面：

```
⚠️ 检测到未完成的操作

  工具:  shell
  命令:  rm -rf build/
  开始时间: 3 分钟前
  状态: 执行中断，结果未知

  这个命令可能：
    - 完全执行了（build/ 已被删除）
    - 部分执行了（build/ 部分被删除）
    - 完全没有执行（build/ 仍然存在）

  请选择：
    [ ] 检查当前状态
    [ ] 重新执行
    [ ] 跳过，视为已完成
    [ ] 跳过，视为未执行

  选择后，将记录到事件日志，恢复继续。
```

### 5.4 事件日志记录

```rust
SessionEvent::ToolExecutionUncertain {
    call_id: CallId,
    tool: String,
    args: Value,
    started_at: i64,
    user_decision: Option<UncertainDecision>,   // 用户处理后才填充
}

pub enum UncertainDecision {
    Rerun,
    SkipAsCompleted,
    SkipAsNotExecuted,
    Inspected { result: String },
}
```

**用户处理完，事件日志记录决定，后续恢复不再重复询问。**

### 5.5 事件日志结构（完整因果链）

```rust
SessionEvent::ToolExecutionStarted {
    call_id: CallId,
    tool: String,
    args: Value,
    ts: i64,
}

SessionEvent::ToolExecutionReceipt {
    call_id: CallId,
    success: bool,
    result: Value,
    error: Option<String>,
    ts: i64,
}
```

**任何工具执行，都必须有 Started 和 Receipt 配对。**

---

## 6\. 只读证明：从"白名单匹配"升级为"证明只读"

### 6.1 问题

**黑名单永远不全会漏，白名单太死会烦。**

`ls` 在白名单里，但 `ls $(rm -rf /)` 呢？ `git status` 是无害的，但 `git -c core.pager="rm -rf /" status` 呢？

**"命令名字看起来对"不能作为放行依据。**

### 6.2 设计

**核心原则**：**只有能证明只读，才自动放行。证明不了，进审批。**

```
┌─────────────────────────────────────┐
│  命令解析                            │
│                                     │
│  1. 词法分析：切分成 token            │
│  2. 语法分析：识别管道、重定向、      │
│     subshell、命令替换等结构          │
│  3. 语义分析：判断每部分的读写意图     │
│                                     │
│  结论：可证明只读 / 不可证明          │
└─────────────────────────────────────┘
                ↓
      可证明只读 → 自动放行
      不可证明 → 进审批
```

### 6.3 "可证明只读"的条件

**一个命令可证明只读，当且仅当：**

| 条件 | 说明 |
|---|---|
| 命令在白名单内 | ls / cat / grep / git status 等 |
| 没有元字符 | 无 \`、$()、\|、>、>>、;、&&、\|\|、& |
| 没有 subshell | 无 (...) |
| 参数不涉及写 | 参数里没有 -exec、--output、-o 等写入标志 |
| 不访问敏感路径 | 参数不含 .env、.ssh、credentials 等 |
| 没有配置覆盖 | git -c 这类"能借配置触发外部程序"的形式，不放行 |

**任何一条不满足 → 进审批。**

### 6.4 解析器状态

```
解析器有三种结果：
  Safe     → 可证明只读
  Unsafe   → 可证明有副作用
  Unknown  → 无法判断

Unknown 不等于 Safe。
Unknown 按 Unsafe 处理（进审批）。
```

### 6.5 白名单的定义

白名单不是"命令名列表"，是"**已知只读的命令 + 参数模式**"：

```toml
[readonly]
ls        = "*"                    # 所有参数都安全
cat       = "*"
grep      = "*"
git.status = "*"
git.log   = "*"
git.diff  = "*"
```

**未来如果要加 `git show`，只需加一行 `git.show = "\*"`。**

### 6.6 和审批系统的衔接

```
可证明只读 → 直接执行（不计入审批）
不可证明 → 走 Irreversible 审批流程
白名单外 + 不在只读证明内 → 走 Irreversible 审批
```

**"可证明只读"是唯一能绕过审批的路径。**

---

## 7\. 子 Agent 独立验收

### 7.1 问题场景

```
主 Agent:  "让子 Agent 帮忙重构这个模块"
子 Agent:  "重构完成 ✓"
主 Agent:  "好的，任务完成"
用户:      跑测试 → 全部失败
```

**子 Agent 说"完成了"，不等于真的完成了。**

### 7.2 设计

**核心原则**：模型可以执行任务，但不能因为自己说"完成了"就成立。

**父 Session 必须做独立验收**。

### 7.3 验收方式

| 验收类型 | 适用场景 |
|---|---|
| 测试执行 | 有测试的项目，跑一遍测试 |
| 文件读取 | 读文件确认修改生效 |
| 状态检查 | 检查 git status 等 |
| LLM 复审 | 另一个模型/会话复审 |
| 用户确认 | 复杂任务让用户确认 |

### 7.4 验收协议

```json
{
  "name": "subagent_verify",
  "description": "对子 Agent 的执行结果做独立验收",
  "schema": {
    "type": "object",
    "properties": {
      "subagent_id": { "type": "string" },
      "acceptance_criteria": {
        "type": "array",
        "items": { "type": "string" },
        "description": "验收条件，比如'测试通过'、'文件已修改'"
      }
    }
  },
  "side_effect": "Pure"
}
```

### 7.5 子 Agent 的返回

```json
{
  "status": "claimed_complete",
  "summary": "重构完成",
  "verification": "pending"    // ← 不是 "completed"
}
```

**父 Session 拿到这个返回后，必须走一遍验收。**

**验收通过后，才写成 `completed`。**

### 7.6 事件记录

```rust
SessionEvent::SubagentResult {
    subagent_id: SubagentId,
    claimed_status: String,
    verified_status: VerificationStatus,
    verification_evidence: Option<String>,
}
```

---

## 8\. In-Band Summary：摘要复用现有前缀

### 8.1 问题

**如果摘要"另起一个请求"，Provider 侧的前缀缓存全废。**

```
主请求：   [System][Message1][Message2]...[MessageN]
摘要请求： [总结指令][Message1][Message2]...[MessageN]   ← 全新 prefix
```

**Provider 看到的是一个全新的 prefix，缓存全部 Miss。摘要本身就变成了昂贵操作。**

### 8.2 设计

**核心原则**：摘要时复用当前 session 的 prefix，只在末尾追加总结指令。

```
主请求：   [System][Message1][Message2]...[MessageN]
摘要请求： [System][Message1][Message2]...[MessageN][总结指令]
                                                    ↑
                                            只在末尾追加
```

**Provider 看到的前缀完全一致 → 缓存命中。**

### 8.3 实现要点

```rust
fn build_summary_request(session: &Session) -> LlmRequest {
    let mut messages = session.messages().to_vec();

    // 追加总结指令，而不是新建请求
    messages.push(Message::user(
        "请总结以上对话的要点，包括：\n\
         1. 已完成的工作\n\
         2. 关键决策\n\
         3. 未解决的问题\n\
         保持简洁，不超过 500 字。"
    ));

    LlmRequest {
        messages,
        stream: false,
        ..Default::default()
    }
}
```

### 8.4 成本对比

| 方式 | 前缀缓存 | 成本 |
|---|---|---|
| 另起请求 | 全部 Miss | 高 |
| In-Band | 前缀命中 | 约 1/50 |

---

## 9\. 内核行数门禁

### 9.1 目的

**不是"代码越少越好"，是"防止内核膨胀"。**

每一次有人想往内核塞东西，都要面对一个问题：

> **这个判断为什么属于内核？**

### 9.2 范围

**定义"内核"为**：一旦做错，就会改变 Locowrk 对事件顺序、行动资格、因果关系承诺的决定。

**内核之外的所有东西**：Provider 适配、工具实现、Extension、UI、存储……都往外长。

### 9.3 门禁机制

给 `cordis-core` 和 `kernel-agent` 分别设一个软上限：

| Crate | 软上限 | 触发动作 |
|---|---|---|
| cordis-core | 3000 行 | 提交前评审 |
| kernel-agent | 5000 行 | 提交前评审 |

**"触发评审"不是"禁止提交"，是"必须回答几个问题"**：

1. 这个改动为什么属于内核，而不是 Runtime / Tool / Extension / 产品层？
2. 它如果做错，会不会改变底层的执行语义？
3. 如果非加不可，内核里有没有东西应该先移出去？

### 9.4 门禁实现

CI 或提交前脚本：

```bash
# 统计行数
core_lines=$(tokei crates/cordis-core/src --output json | jq '.Rust.code')
kernel_lines=$(tokei crates/kernel-agent/src --output json | jq '.Rust.code')

# 超限警告
if [ "$core_lines" -gt 3000 ]; then
    echo "⚠️ cordis-core 已 ${core_lines} 行，超过 3000 行软上限"
fi
```

**不做硬性阻止，只做提醒。**

---

## 10\. 落地路径

### 10.1 阶段划分

| 阶段 | 内容 | 优先级 | 复杂度 |
|---|---|---|---|
| 1 | Revision Guard | P0 | ★★ |
| 2 | Turn Commit | P0 | ★ |
| 3 | Unknown Effect 处理 | P1 | ★★ |
| 4 | 只读证明升级 | P1 | ★★★ |
| 5 | 子 Agent 独立验收 | P2 | ★★ |
| 6 | In-Band Summary | P2 | ★ |
| 7 | 内核行数门禁 | P3 | ★ |

### 10.2 阶段 1：Revision Guard（P0）

**为什么最先做**：这是"用户信任"的核心。AI 静默覆盖用户修改是最容易踩、最难发现的坑。

**步骤**：

1. `fs_read` 返回 `revision`（内容哈希）
2. `fs_write` / `edit\_\*` 加 `expected_revision` 参数
3. 执行前校验 revision
4. 不一致时返回友好错误
5. 用打地鼠那个文件测试：读→改→在外部改→再改

**验收**：外部修改后，AI 的编辑会被拒绝，并提示重新读取。

### 10.3 阶段 2：Turn Commit（P0）

**为什么第二个做**：防止半截 response 触发副作用。

**步骤**：

1. 主循环里，`AssistantMessage` 加 `committed` 字段
2. 流式接收完成后，校验终止条件
3. 只有 `committed: true` 的消息，其 tool_calls 才执行
4. 崩溃恢复时，丢弃未 commit 的消息

**验收**：模拟断流，验证工具不被执行。

### 10.4 阶段 3：Unknown Effect 处理（P1）

**步骤**：

1. `ToolExecutionStarted` 和 `ToolExecutionReceipt` 事件
2. Session 加载时扫描"有 Started 无 Receipt"的操作
3. UI 上显示 uncertain 状态，等待用户决定
4. 用户决定写入事件日志

**验收**：真的 `kill -9`，验证恢复时显示 uncertain。

### 10.5 阶段 4：只读证明升级（P1）

**步骤**：

1. Shell 命令解析器（词法 + 语法）
2. 只读证明逻辑
3. 替换现有白名单匹配
4. 保留现有 denylist 作为兜底

**验收**：`ls` 自动放行，`ls $(rm)` 进审批。

### 10.6 阶段 5-7（P2-P3）

按需推进。

---

## 11\. 常见坑

| 坑 | 原因 | 解法 |
|---|---|---|
| Revision 计算太慢 | 大文件全量哈希 | 用 mtime + size + 前 1KB 哈希 |
| Revision 在不同平台不一致 | 换行符差异 | 统一成 LF 再哈希 |
| Turn Commit 漏判 | 没检查 finish_reason | 明确检查 stop / tool_calls |
| Uncertain 操作太多 | 每次崩溃都积累 | 用户处理后必须写入事件日志 |
| 只读证明太严 | 解析器看不懂就拒 | 这是对的，看不懂就进审批 |
| 只读证明太松 | 漏判危险命令 | 定期用危险命令测试集回归 |
| 子 Agent 验收太轻 | 只读一遍输出 | 必须跑独立的验证（测试/读文件） |
| In-Band Summary 破坏前缀 | 在中间插入指令 | 只在末尾追加 |
| 内核行数门禁误伤 | 合理的内核增长被拦 | 门禁是"提醒评审"，不是"禁止提交" |

---

## 12\. 设计哲学

### 12.1 模型负责"扩大问题空间"，Runtime 负责"限制什么算事实"

**这是 Kiso 这篇文章里最核心的一句话。**

模型可以越来越强，可以做越来越复杂的事。但 Runtime 必须保留**几条独立于模型的事实边界**：

* 模型说"我要删" ≠ 已经删了
* 模型说"我看到了" ≠ 现在还是那样
* 模型说"完成了" ≠ 目标达成了

**Runtime 不是限制模型，是让模型的强大建立在可信任的地基上。**

### 12.2 五组区别，是五种"用户会踩的坑"

* Observed ≠ Current → 用户改了东西，AI 又改回去了
* Intent ≠ Effect → AI 说要删，可能没删；说没删，可能删了
* Started ≠ Succeeded → AI 说执行了，可能只执行了一半
* Process Completion ≠ Goal Satisfaction → AI 说完成了，测试全红
* Memory ≠ Durable Fact → 崩溃重启，AI 忘了一切

**每一条都是用户真会遇到的问题。** 分开它们，不是为了工程优雅，是为了**用户能信任这个 Agent**。

### 12.3 不猜

**Ambiguity Never Auto-Repeats。**

不确定的副作用永远不自动重试。

**这是对用户的尊重。** 用户宁可多花 10 秒确认，也不愿意第二天发现系统在夜里删错了东西。

### 12.4 因果链可追溯

**每一个副作用，都必须能回答三个问题**：

1. 是谁提出的？（模型意图）
2. 什么时候获得资格执行的？（Turn Commit）
3. 执行的证据是什么？（Started + Receipt）

**这三条能回答，Agent 就是可信任的。**

---

*文档版本：v1.0* *配套项目：Locowrk* *关联文档：《Agent 内核设计》《记忆树设计》《LLM 客户端与调度层设计》* *灵感来源：Kiso 的因果链思路*