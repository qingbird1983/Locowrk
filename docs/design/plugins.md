# Locowrk Agent 必备插件清单

> 判断"必备"的唯一标准：**没有它，Agent 就只能聊天。**
> 按这个标准，真正必备的其实很少。

---

## 一、总览

| 优先级 | 插件 | 一句话作用 |
|---|---|---|
| P0 | `llm-openai` / `llm-ollama` | 真实模型适配 |
| P0 | `tool-fs` | 读/写/列目录/搜索文件 |
| P0 | `tool-shell` | 执行命令（危险） |
| P0 | `tool-fetch` | HTTP GET / 网页抓取 |
| P1 | `tool-edit` | 精确代码编辑 |
| P1 | `tool-git` | Git 操作 |
| P1 | `tool-http` | POST / PUT / 通用请求 |
| P1 | `memory-sqlite` | 会话持久化 + 跨会话记忆 |
| P1 | `memory-vector` | 向量检索（RAG） |
| P1 | `ctx-compact` | 上下文超长自动压缩（中间件） |
| P1.5 | `mw-approval` | 拦截不可逆工具，弹审批（中间件） |
| P1.5 | `mw-audit` | 审计日志，所有工具调用落盘 |
| P1.5 | `mw-ratelimit` | 限制单位时间调用次数 |
| P2 | `tool-mcp` | 桥接 MCP 生态（**强烈推荐**） |
| P2 | `tool-browser` | 操作网页（Playwright / CDP） |
| P2 | `tool-sandbox` | shell 在容器/WASM 里跑 |
| P2 | `agent-sub` | 子 Agent 调度 |
| P2 | `tool-schedule` | 定时任务 |
| P2 | `tool-notify` | 桌面通知 / Webhook |

---

## 二、P0：没有这些，Agent 不成立

| 插件 | 能力 | SideEffect | 依赖 |
|---|---|---|---|
| `llm-openai` / `llm-ollama` | 真实模型适配 | `Pure` | — |
| `tool-fs` | 读/写/列目录/搜索文件 | 读=`Pure`，写=`Compensable` | — |
| `tool-shell` | 执行命令 | **`Irreversible`** | — |
| `tool-fetch` | HTTP GET / 网页抓取 | `Irreversible`（2026-10-06 真实测试复测裁定：GET 也对外发起网络请求，服务端留痕、URL 可能带敏感 token，"只读"标法与事实不符；原 `Pure` 作废） | — |

**这四个是地基。** 把 `tool-echo` 换成这四个，Agent 立刻从"能聊天"变成"能干活"。

### `tool-fs` 的 SideEffect 分级（关键设计）

- `fs_read` / `fs_list` / `fs_grep` → `Pure`，随便调，回退免费
- `fs_write` → `Compensable { undo_tool: "fs.restore" }`，写入前自动备份原内容，撤销时恢复
- `fs_delete` → `Irreversible`，走审批

### `tool-shell` 的安全策略

**这是整个系统里最危险的插件，必须标 `Irreversible`。**

第一版建议：

- 默认只允许白名单命令（`ls` / `cat` / `grep` / `git status`）
- 危险命令（`rm` / `curl | sh` / `sudo`）**直接拒绝，不进审批流程**
- 想放开就显式配置

---

## 三、P1：让它真正好用

| 插件 | 能力 | SideEffect |
|---|---|---|
| `tool-edit` | 精确代码编辑（apply patch / 行替换） | `Compensable` |
| `tool-git` | status / diff / commit / log | commit=`Compensable`，push=`Irreversible` |
| `tool-http` | POST / PUT / 通用请求 | `Irreversible` |
| `memory-sqlite` | 会话持久化 + 跨会话记忆 | `Pure` |
| `memory-vector` | 向量检索（RAG） | `Pure` |
| `ctx-compact` | 上下文超长时自动压缩 | `Pure`（中间件） |

### `tool-edit` 为什么比 `fs_write` 重要

Agent 改代码时，整文件重写会：

- 丢失未预期的改动
- 浪费 token

`tool-edit` 应该支持：

- 按行号替换
- 按字符串匹配替换（`old_string` → `new_string`）
- 应用 unified diff
- **每次编辑前读原文件，生成精确的 undo 数据**（写进 `SideEffect::Compensable`）

### `ctx-compact` 是中间件，不是工具

挂在 `agent/request` waterfall 上，检测到 token 超阈值就自动摘要早期消息。

**这个插件不做，长对话必崩。**

---

## 四、P1.5：最容易忽略但极其关键

这三个是**中间件类插件**，不是工具，但重要性不亚于 P0。

| 插件 | 挂载点 | 作用 |
|---|---|---|
| `mw-approval` | `agent/request` | 拦 `Irreversible` 工具，弹审批 |
| `mw-audit` | `serial` on `tool/call` | 审计日志，所有工具调用落盘 |
| `mw-ratelimit` | `tool` 中间件链 | 限制单位时间调用次数 |

- `mw-approval` 已经有（审批弹窗 UI 阶段六做过）
- `mw-audit` **建议第一版就加**——出问题时能回溯 Agent 到底干了什么

---

## 五、P2：生态兼容的"作弊"路径 —— MCP

**这是最重要的一条建议。**

不用自己写所有工具。**接一个 `tool-mcp` 桥接插件，就能拿到 MCP 生态里成百上千个现成工具**（文件系统、数据库、浏览器、Slack、GitHub、Notion……）。

### 架构

```
tool-mcp 插件
  ├─ 读 MCP 配置文件（mcp.json）
  ├─ 启动 MCP server 子进程（stdio / SSE）
  ├─ 握手，list_tools
  └─ 把每个 MCP tool 适配成本地 Tool
       调用时 → JSON-RPC → MCP server → 返回
```

### 好处

- 一个插件解决 80% 的工具需求
- MCP 生态在爆发，跟着社区走比自己写快
- 每个 MCP tool 的 `side_effect` 由配置声明，**不写就默认 `Irreversible`**（安全兜底）

> **这一条如果做，优先级应该排在 `tool-edit` 之前。**
> 自己写 5 个工具的时间，接上 MCP 能拿到 50 个。

---

## 六、P2：特定场景才需要

| 插件 | 场景 |
|---|---|
| `tool-browser` | 需要操作网页（Playwright / CDP） |
| `tool-sandbox` | shell 需要在容器/WASM 里跑 |
| `agent-sub` | 子 Agent 调度（多 Agent 协作） |
| `tool-schedule` | 定时任务 |
| `tool-notify` | 桌面通知 / Webhook 推送 |

**`tool-browser` 建议晚做。** 它比 `tool-fetch` 复杂一个数量级，而且很多场景 `tool-fetch` + `tool-mcp` 就够了。

### `tool-sandbox` 方案定稿（ROADMAP ⑯，v2.31）

> 原文一行定义"shell 在容器/WASM 里跑"过薄，不足以直接动码；本节是批首交付的
> 方案取舍与能力矩阵（ROADMAP 纪律：批首先定方案再动码）。候选三选一：
> ① Windows 受限令牌 + JobObject 限额；② wasmtime WASI 内建命令集；③ 检测
> Docker/WSL 走容器。

**定案 = ①。** 理由（按静态事实）：

- **② 弃**：只能跑 wasm 化命令——白名单里的真实 CLI（git/grep…）都不是
  wasm，要么放弃真实命令要么自建 wasm 工具链，实用性窄、成本高。且 ⑧ 的
  wasmtime 是**插件**沙箱（guest 不可信），这是 **shell** 沙箱（宿主 spawn
  的原生子进程不可信），信任模型不同，不是一回事。记录为远期可选内建命令集。
- **③ 弃（本批）**：可用性依赖用户环境（Docker daemon / WSL 发行版），
  路径翻译（`C:\` ↔ `/mnt/c`）、镜像与 daemon 生命周期都是新失败面；且
  `docker` 自身必须作为全权限子进程被 spawn——隔离承诺外包给了无法假设
  存在的组件。记录为"检测到容器环境时的远期可选后端"，不在本批。
- **① 取**：零新增第三方依赖（windows-sys 已在依赖树），Win8+ 即可，
  所交付能力全部可静态论证。

**能力矩阵（Windows，插件进程以普通用户运行时；如实分级，不做虚假安全声明）：**

| 维度 | 机制 | 隔离能力 |
|---|---|---|
| 进程树治理 | JobObject `KILL_ON_JOB_CLOSE`（S1 JobGuard 同源）+ 未开 BREAKAWAY（子进程不可脱离 Job） | ✅ 超时/收尾整树终止 |
| 内存上限 | `JOB_OBJECT_LIMIT_PROCESS_MEMORY` | ✅ 硬限，超限申请失败 |
| 进程数上限 | `JOB_OBJECT_LIMIT_ACTIVE_PROCESS` | ✅ 硬限 |
| CPU 占比 | `JOB_OBJECT_CPU_RATE_CONTROL` 硬顶（可选，缺省不启用） | ✅ 硬限 |
| 剪贴板 | `JOB_OBJECT_UILIMIT_READCLIPBOARD\|WRITECLIPBOARD` | ✅ 拒读写剪贴板（Job UI 限制没有全局钩子级，不虚报） |
| 特权削减 | `CreateRestrictedToken`（`DISABLE_MAX_PRIVILEGE` + deny `BUILTIN\Administrators`）经 `CreateProcessAsUserW`（自派生受限令牌无需特权；失败降级 job-only 并如实上报） | ✅ 新进程无任何特权、不可 UAC 提升（宿主提权运行时防继承） |
| 文件系统 | — | ❌ **不隔离**（与宿主同用户权限读写） |
| 网络 | — | ❌ **不隔离**（可正常联网，网络类命令靠策略硬锁挡） |
| 注册表等用户态 | — | ❌ **不隔离** |

**结论句**：沙箱 = 资源与权限的爆炸半径限制，**不是文件系统/网络的密封舱**。
因此策略面不可放松：与 `tool-shell` 同源三道闸（硬锁/高危/白名单，
`LOCOWRK_SANDBOX_ALLOW` 扩展、denylist 永远赢）原样保留；**高危解锁开关
不设**——fs 不隔离的前提下沙箱内解锁 rm 无增益，宁严勿松，引导走
`shell` 的解锁路径。与 tool-shell 的关系 = **并存不替换**（沙箱档位是
独立工具，shell 既有纪律原样保留）。

**工具面**：单工具 `sandbox_run`（`SideEffect::Irreversible` 一律审批），
参数 `command` 必填 + `cwd` / `timeout_secs` / `max_bytes` / `mem_mb` /
`max_procs` / `cpu_rate` 可选（arg 钳上限）；env 缺省
`LOCOWRK_SANDBOX_{TIMEOUT,MAX_BYTES,MEM_MB,MAX_PROCS,CPU_RATE}`（清单 §2.4
不静默纪律同 shell）。输出 JSON 增 **`isolation` 字段**如实报告生效档：
`restricted-token+job` / `job-only (restricted token unavailable)`（降级不
静默）。Unix 对称面：`pre_exec` setrlimit（`RLIMIT_AS`/`RLIMIT_NPROC`/
`RLIMIT_CPU`）+ 进程组树治理（NPROC 为 per-uid 近似，如实标注）。

---

## 七、落地顺序

### 第一批：把 Agent 变成能干活的东西

1. `llm-openai` 或 `llm-ollama` — 真实模型
2. `tool-fs` — 读文件（先只做读）
3. `tool-fetch` — 抓网页
4. `mw-audit` — 审计日志

### 第二批：让它真正好用

5. `tool-shell`（白名单模式）— 执行命令
6. `tool-fs` 写入能力 — 加 `Compensable` 备份
7. `tool-edit` — 精确代码编辑
8. `ctx-compact` — 上下文压缩

### 第三批：生态扩展

9. `tool-mcp` — 接 MCP 生态，一劳永逸
10. `memory-sqlite` — 会话持久化
11. `tool-git` — Git 操作

---

## 八、一个判断标准

每写一个插件前，问自己：

> **这个工具调用完，Agent 会不会做出用户看不懂的事？**

- 会 → 标 `Irreversible`，走审批
- 不会 → 标 `Pure` 或 `Compensable`，放开跑

**宁可标错成 `Irreversible`，也不要标错成 `Pure`。**

前者只是烦一点，后者可能删掉用户的东西。

---

## 九、下一步

先做第一批的 4 个。

做完你会发现 Locowrk 突然"活了"——从 `echo` 一下变成能读文件、能抓网页、能执行命令的真 Agent。

---
