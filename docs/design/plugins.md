# Locowrk Agent 必备插件清单

> 判断"必备"的唯一标准：**没有它，Agent 就只能聊天。**
> 按这个标准，真正必备的其实很少。

---

## 一、总览

> **⚠ v2.149 批 E 加「现役状态」列（此前本表只有"该建什么"、没有任何一列说明"现在是什么"，
> 于是照本表实现的 agent 会去重建 `tool-sandbox`、把 LLM 写成插件、给已存在的功能再造一份）**。
> 图例：✅ 现役插件（`plugins-src/` 动态库，版本号 = 最新暂存档）· 🔧 已交付但**不是插件**
> （宿主内置，经组装点注册，无 manifest 面）· ◐ 部分 / 形态已变 · ❌ 已退役 · ⬜ 未做。
> 唯一进度看板仍是 [ROADMAP.md](ROADMAP.md)，本列只回答"这份清单里哪些已经存在、以什么形态存在"。

| 优先级 | 插件 | 一句话作用 | 现役状态（v2.149 核） |
|---|---|---|---|
| P0 | `llm-openai` / `llm-ollama` | 真实模型适配 | ❌ **插件形态作废**——v2.75 裁定 LLM 是宿主内置服务（`desktop/src-tauri/src/llm/` 自建 OpenAI 兼容客户端，一条线协议，genai 依赖全删；kind 只决定鉴权头与端点归一） |
| P0 | `tool-fs` | 读/写/列目录/搜索文件 | ✅ 0.1.5（六工具；G3 revision 守卫在位） |
| P0 | `tool-shell` | 执行命令（危险） | ✅ 0.1.6（`shell` + `shell_read`；v2.145 起无命令级禁令，见 §三修订注） |
| P0 | `tool-fetch` | HTTP GET / 网页抓取 | ✅ 0.1.2（`Irreversible`，SSRF 防线） |
| P1 | `tool-edit` | 精确代码编辑 | ✅ 0.2.1（容错匹配 + revision 链） |
| P1 | `tool-git` | Git 操作 | ✅ 0.1.3（根提交显式拒绝 + stderr 末 6 行） |
| P1 | `tool-http` | POST / PUT / 通用请求 | ⬜ 未做（backlog；`fetch` 只 GET，webhook 外发在 `tool-notify` 里） |
| P1 | `memory-sqlite` | 会话持久化 + 跨会话记忆 | ✅ 0.1.5（双层记忆 + 文件化双写） |
| P1 | `memory-vector` | 向量检索（RAG） | ◐ 从未作独立插件——向量基建以**宿主共享件**存在（`llm/embed.rs` + `SharedEmbed`），服务 `search_titles` / ToolExposure 第三路打分 / `search_tools`；未配置 embedding 时整条智能层优雅降级 |
| P1 | `ctx-compact` | 上下文超长自动压缩（中间件） | ❌ **已退役**——桌面压缩链 v2.137 整面退役（记忆树整理接管），本体 v2.147 批 C 摘除（净摘 893 行）；仅存 `CompactionOutcome`（旧日志重放面）+ `estimate_tokens`（全仓粗估尺） |
| P1.5 | `mw-approval` | 拦截不可逆工具，弹审批（中间件） | 🔧 已交付但**不是洋葱插件**——审批 = 内核 `consult` 门 + 宿主 `ApprovalHub` 七序决策表（DESIGN §7.2），带 UI 决策卡、会话豁免、项目粘性规则 |
| P1.5 | `mw-audit` | 审计日志，所有工具调用落盘 | 🔧 已交付（v2.9）= kernel-agent 内置 `Audit` 洋葱件（现役链 Tracing → Audit → Timeout → CircuitBreaker），非 dlopen 插件 |
| P1.5 | `mw-ratelimit` | 限制单位时间调用次数 | ⬜ 未做（backlog；现有的只有 LLM 传输层 429 退避，不是工具调用限流） |
| P2 | `tool-mcp` | 桥接 MCP 生态（**强烈推荐**） | ✅ 0.1.0（stdio JSON-RPC，`mcp_<server>_<tool>` 装载时冻结） |
| P2 | `tool-browser` | 操作网页（Playwright / CDP） | ◐ 未做插件；v2.144 交付的是宿主内置 `browser_open`/`browser_console`（**工作面板预览 + 页面报错回读**，iframe + `lwpreview://`），不是"操作网页"的自动化面 |
| P2 | `tool-sandbox` | shell 在容器/WASM 里跑 | ❌ **已整目录退役**（v2.145）——曾以插件形态交付（v2.31 受限令牌 + JobObject），因不隔离 fs/网络即非安全边界而拆除；**不要重建**，见 §六退役注 |
| P2 | `agent-sub` | 子 Agent 调度 | 🔧 已交付（v2.30）= `kernel-agent/subagent/` + 宿主内置 `subagent_run`/`subagent_verify`（v2.145 起 `subagent_run` = Pure，派生不再弹卡），非插件 |
| P2 | `tool-schedule` | 定时任务 | 🔧 已交付（v2.29）= `desktop/scheduler/`（sqlite + tick 引擎）+ 宿主内置 `schedule_*` 工具面，非插件 |
| P2 | `tool-notify` | 桌面通知 / Webhook | ✅ 0.1.0（channel 二选一；桌面走 `HostVtable::notify` 槽） |

**四个 P0 是地基。** 把 `tool-echo` 换成这四个，Agent 立刻从"能聊天"变成"能干活"。

> **形态说明（v2.149）**：本清单写的是**能力清单**，不是"每项都必须是 dlopen 插件"。
> 实际交付里 9 个动态库插件 + 一批宿主内置工具面（审批、子 Agent、调度、记忆树工具、
> 浏览器预览、媒体工具、模型接入）并存——两者都经同一 `ToolSource`/目录机制进模型工具池，
> 区别只在装载通路（`plugins/` 暂存 vs 组装点 `with_builtin`）。"一切皆插件"约束的是
> **无特权模块**，不是"所有代码都必须打成 DLL"。

---

## 二、P0：没有这些，Agent 不成立

| 插件 | 能力 | SideEffect | 依赖 |
|---|---|---|---|
| `llm-openai` / `llm-ollama` | 真实模型适配 | `Pure` | — |
| `tool-fs` | 读/写/列目录/搜索文件 | 读=`Pure`，写=`Compensable` | — |
| `tool-shell` | 执行命令 | **`Irreversible`** | — |
| `tool-fetch` | HTTP GET / 网页抓取 | `Irreversible`（2026-10-06 真实测试复测裁定：GET 也对外发起网络请求，服务端留痕、URL 可能带敏感 token，"只读"标法与事实不符；原 `Pure` 作废） | — |

> **⚠ v2.149 批 E 补回 + 就地订正**：本节（含上表）在批 E 改 §一 表时被误删，已按
> `git show HEAD` 原文补回，并订正三处与代码实况不符的字面：① 上表 `llm-*` 行的能力
> **已交付但不是插件**（宿主内置，见 §一 状态列）；② 下方 `fs_write` 原写
> `undo_tool: "fs.restore"` = 点号旧写法，**现役真名 `fs_restore`**（v2.25 起因 OpenAI
> 兼容接口拒收点号，22 个工具名全表改下划线；实值见 `plugins-src/tool-fs/manifest.json`）；
> ③ `fs_delete` 自 v2.133（G3）也带 revision 守卫，而"走审批"的绝对措辞自 v2.145 起由
> §7.2 七序决策表决定（Auto 档与项目粘性规则可免卡）。

### `tool-fs` 的 SideEffect 分级（关键设计）

- `fs_read` / `fs_list` / `fs_grep` → `Pure`，随便调，回退免费
- `fs_write` → `Compensable { undo_tool: "fs_restore" }`（名字订正见上注），写入前自动备份原内容，撤销时恢复
- `fs_delete` → `Irreversible`，走审批

### `tool-shell` 的安全策略

**这是整个系统里最危险的插件，必须标 `Irreversible`。**

第一版建议：

- 默认只允许白名单命令（`ls` / `cat` / `grep` / `git status`）
- 危险命令（`rm` / `curl | sh` / `sudo`）**直接拒绝，不进审批流程**
- 想放开就显式配置

> **v2.145 修订（《Permission Model Redesign (ZCode Alignment) v1》）**：上行「第一版
> 建议」已成历史——白名单与 denylist 整批退役，政策层收敛两态：Deny 仅剩结构性拒绝
> （元字符/空命令/引号未闭合），其余一切命令放行进审批卡；免审面 `shell_read` 依赖
> 只读证明层（proof.rs，固定表 + EXACT_ARGV 精确表）。解释器安全语义 = 审批卡 + 项目
> 粘性规则（桌面 permission_rules，「本项目始终允许」落 `permissions/<hash>.json`）。
> `LOCOWRK_SHELL_ALLOW` / `LOCOWRK_SHELL_UNLOCK_RISK` env 幻影化（设置即启动 warn）。
> tool-shell 版本 0.1.5。

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

> **⚠ v2.149 批 E 加现状注**：本表六行 = ✅ `tool-edit` 0.2.1 · ✅ `tool-git` 0.1.3 ·
> ⬜ `tool-http`（未做）· ✅ `memory-sqlite` 0.1.5 · ◐ `memory-vector`（从未作插件，
> 向量基建 = 宿主共享件 `llm/embed.rs`）· ❌ **`ctx-compact` 已退役**（桌面压缩链
> v2.137 整面退役、本体 v2.147 批 C 摘除；压缩语义由**记忆树整理**承担 = 章节捆 +
> 段落总结 + 尾巴切片，见《Memory Tree Design v2》与《Organize Summary Design v1》）。
> 表内 `tool-git` 的 SideEffect 列也非终态：现役含 `git_undo_commit`（Irreversible）与
> 根提交显式拒绝，`git_status/diff/log` 另有无 shell 审批面 `shell_read` 同族工具。

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

> **⚠ v2.149 批 E 就地订正**：这句判断的**结论**成立（长对话确实必须有收口机制），
> 但**承担者不是 `ctx-compact`**：桌面链自 v2.137 改由记忆树整理承担（绝对阈值
> `thresholdTokens` 缺省 100K 触发 → 章节捆 + 段落总结 → 请求组装只带 L0 目录 + 尾巴），
> 中间件本体 v2.147 批 C 已摘除（`middleware/compact.rs` 332→56 行，只剩
> `CompactionOutcome` 重放面 + `estimate_tokens` 粗估尺）。**不要按本节重新实现一个
> 摘要式压缩中间件**——那会与整理链打架（v2.136/v2.137 两轮裁定正是为此）。

---

## 四、P1.5：最容易忽略但极其关键

这三个是**中间件类插件**，不是工具，但重要性不亚于 P0。

| 插件 | 挂载点 | 作用 |
|---|---|---|
| `mw-approval` | `agent/request` | 拦 `Irreversible` 工具，弹审批 |
| `mw-audit` | `serial` on `tool/call` | 审计日志，所有工具调用落盘 |
| `mw-ratelimit` | `tool` 中间件链 | 限制单位时间调用次数 |

> **⚠ v2.149 批 E 加现状注（含一条挂载点订正）**：`mw-approval` = 🔧 已交付但**不挂
> waterfall**——审批是内核 `consult` 门 + 宿主 `ApprovalHub` 七序决策表（DESIGN §7.2，
> v2.145 起含项目 deny/allow 两级）；`mw-audit` = 🔧 已交付（v2.9）= kernel-agent 内置
> **洋葱件 `Audit`**（现役链 Tracing → Audit → Timeout → CircuitBreaker），表里原写的
> 挂载点「`serial` on `tool/call`」**不是现役通路**——`EventBus` 的 parallel/serial 两簇
> 全仓零生产调用点，已随 v2.148 批 D 在 DESIGN §3.5 标 **RESERVED**（审计 M-21）；
> `mw-ratelimit` = ⬜ 未做（backlog；现存的只有 LLM 传输层 429 退避）。
> 中间件类**不能**做成 dlopen 插件这点也在此说明：`agent/request` waterfall 与洋葱链
> 都在内核进程内、由组装点注册，插件契约层（C ABI）不承载中间件。

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

> **⚠ v2.149 批 E 加现状注**（本表原样读起来像"这五个都还没做"，实际五项形态各不相同）：
> `tool-browser` ◐（v2.144 交付的是宿主内置 `browser_open`/`browser_console` = 工作面板
> 预览 + 页面报错回读，**不是** Playwright 级网页自动化）· `tool-sandbox` ❌ **已整目录退役**
> （v2.145，见下方退役注，**不要重建**）· `agent-sub` 🔧 已交付为宿主内置子 Agent 调度
> （v2.30）· `tool-schedule` 🔧 已交付为宿主内置调度器 + `schedule_*` 工具面（v2.29）·
> `tool-notify` ✅ 现役插件 0.1.0。逐项依据见 §一「现役状态」列。

**`tool-browser` 建议晚做。** 它比 `tool-fetch` 复杂一个数量级，而且很多场景 `tool-fetch` + `tool-mcp` 就够了。

### ❌ `tool-sandbox` 方案定稿（ROADMAP ⑯，v2.31）——已随 v2.145 退役，本节保留为**方案史**

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

> **v2.145 退役注**：本节所述 tool-sandbox 已整批退役（用户裁定，ZCode 沙箱拆除
> 同款先例）——`plugins-src/tool-sandbox/` 与暂存产物删除，shell 成为唯一命令执行面。
> 理由：沙箱自认不隔离文件系统与网络、不构成真实安全边界，留第二入口只多一个「更易
> 撞锁」的面；能力面由 shell 审批 + 粘性规则承接。已存会话钉死集里的 sandbox_run 按
> 回映射 miss 语义自然消失。
>
> **盘上余留（v2.149 批 E 实测，写给后来读这份方案史的人）**：Rust 侧**零实现**
> （`grep -rn sandbox crates desktop/src-tauri/src plugins-src` = 2 命中，且两条都是
> 注释：`approval.rs:435` 记"移出扫描面"、`browser/mod.rs:4` 讲的是 iframe sandbox，
> 与本案无关）；前端 `desktop/src/lib/tool-detail.ts` 的 `sandbox_run` 映射**有意保留**
> = 历史会话日志重放面（用户 v2.146 批 C 裁定，勾选项只有"删孤儿 i18n 键"），
> 不代表该工具还存在。**下面这段"工具面"描述是退役前的实现记录，不要照它写码。**

**工具面**（退役前实现记录）：单工具 `sandbox_run`（`SideEffect::Irreversible` 一律审批），
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
