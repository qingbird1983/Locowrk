# Locowrk 沙箱执行设计：集成 CubeSandbox

> **⚠ 归档说明（v2.149 批 E，2026-10-11；依据《CODE-AUDIT-v1》H9 / 《AUDIT-FIX-PLAN-v1》E6）**
> ① **本稿从未采纳、从未实施**：全仓 `grep -r "Sandbox Execution Design"` 除审计取证文档外**零引用**，
> `plugins-src/` 与 `crates/` 里没有任何 CubeSandbox 相关代码或依赖；它记录的是一条**外部沙箱集成路线**
> （CubeSandbox），与项目实际走的路线不同。
> ② **主题已整体退役**：实际交付的沙箱方案是 v2.31（⑯）的 **Windows 受限令牌 + JobObject 限额**
> （`plugins-src/tool-sandbox`，方案取舍记在 `docs/design/plugins.md` §六），而该插件又已随
> **v2.145 权限模型重设计整目录删除**（裁定：不隔离 fs/网络即非安全边界，第二入口只多撞锁面）。
> 因此本稿既不是「待办」也不是「现役规范」，只作**方向史**保留。
> ③ 现役的命令执行面与安全边界看：`docs/design/NEW/Permission Model Redesign (ZCode Alignment) v1.md`
> + `crates/plugin-api/src/policy.rs`（shell 政策两态）+ `plugins-src/tool-shell/src/proof.rs`（只读证明）
> + `desktop/src-tauri/src/privacy.rs`（隐私门）。**照本稿实现会造出与现网相反的东西。**

> 配套项目：Locowrk
> 版本：1.0
> 前置文档：《Agent 内核设计》《Agent 必备插件清单》《约束最小化原则》

---

## 目录

1. [设计目标](#1-设计目标)
2. [为什么需要沙箱](#2-为什么需要沙箱)
3. [CubeSandbox 是什么](#3-cubesandbox-是什么)
4. [与现有安全机制的关系](#4-与现有安全机制的关系)
5. [集成架构](#5-集成架构)
6. [工具设计](#6-工具设计)
7. [后端实现](#7-后端实现)
8. [生命周期管理](#8-生命周期管理)
9. [安全策略](#9-安全策略)
10. [UI 呈现](#10-ui-呈现)
11. [配置项](#11-配置项)
12. [落地路径](#12-落地路径)
13. [常见坑](#13-常见坑)
14. [设计哲学](#14-设计哲学)

---

## 1. 设计目标

| 目标 | 含义 |
|---|---|
| **安全执行任意代码** | Agent 生成的代码可以放心跑，不会威胁宿主 |
| **用完即走** | 每次任务独立沙箱，任务结束销毁 |
| **可回滚** | 长任务出错能恢复到之前的状态 |
| **用户可控** | 沙箱配置、网络策略、资源限制都可调 |
| **降级可用** | CubeSandbox 不可用时，功能优雅降级 |
| **不改变内核** | 作为一个插件接入，内核无感知 |

### 非目标

- 不做 CubeSandbox 本身的部署（用户自己部署或使用腾讯云托管版）
- 不替代 `tool-shell`（两者并存，用途不同）
- 不做跨会话的沙箱共享（每个会话独立）

---

## 2. 为什么需要沙箱

### 2.1 现有的安全层级

Locowrk 已有的安全机制：

| 层 | 机制 | 保护什么 |
|---|---|---|
| **路径守卫** | `fs_*` 工具的 workspace 根校验 | 防路径逃逸 |
| **命令白名单** | `tool-shell` 只允许白名单命令 | 防任意命令执行 |
| **副作用分级** | `SideEffect` 枚举 + 审批 | 防误操作 |
| **硬底线** | 元字符拒绝、解释器类拦截 | 防注入 |

**这些都运行在宿主机上，共享同一个内核。**

### 2.2 现有机制的边界

有些场景，现有机制覆盖不了：

| 场景 | 为什么现有机制不够 |
|---|---|
| **Agent 生成并运行代码** | 生成的 Python / JS / Shell 脚本无法预知，白名单挡不住 |
| **运行用户下载的脚本** | 从网上下载的代码，不可信 |
| **多语言运行时** | 需要 Python / Node / Go 等多种环境，不能全装在宿主上 |
| **长时间计算任务** | 跑在宿主上会占用资源、影响主程序 |
| **需要回滚的实验** | 试错型任务，需要"保存进度"的能力 |

**这些场景需要一个"可以随便折腾，但折腾坏了不影响宿主"的环境。**

### 2.3 沙箱是答案

沙箱的核心是**硬件级隔离**——每个任务跑在独立的操作系统内核上，就算代码是恶意的，也碰不到宿主的任何东西。

---

## 3. CubeSandbox 是什么

**CubeSandbox 是腾讯云开源的、专为 AI Agent 设计的安全代码执行沙箱服务。**

### 3.1 核心特性

| 特性 | 指标 | 对 Agent 的意义 |
|---|---|---|
| **极速冷启动** | < 60ms | 可以像"思考"一样快地创建和销毁沙箱 |
| **极低内存开销** | < 5MB / 实例 | 单机可跑 2000+ 沙箱 |
| **MicroVM 隔离** | 独立内核 | 硬件级隔离，无法越狱 |
| **状态快照/回滚** | snapshot / clone / rollback | 长任务可以"存档"和"读档" |
| **网络策略控制** | 可精细定义 | 控制 Agent 能访问哪些网络 |
| **E2B SDK 兼容** | 兼容主流 SDK | 迁移成本低 |

### 3.2 与 Docker 的区别

| | Docker | CubeSandbox |
|---|---|---|
| **隔离级别** | 共享内核（namespace） | MicroVM（独立内核） |
| **启动速度** | 秒级 | < 60ms |
| **内存开销** | 几十 MB 起 | < 5MB |
| **越狱风险** | 内核漏洞可越狱 | 硬件隔离，几乎不可越狱 |
| **适用场景** | 长期运行的服务 | 短平快的任务执行 |

**CubeSandbox 的定位不是替代 Docker，是补充 Docker 覆盖不了的"高频、短时、不可信"的执行场景。**

### 3.3 API 形态

CubeSandbox 对外提供 REST API，兼容 E2B SDK：

```
POST   /sandboxes                创建沙箱
POST   /sandboxes/{id}/execute   执行代码
GET    /sandboxes/{id}/files     读取文件
DELETE /sandboxes/{id}           销毁沙箱
POST   /sandboxes/{id}/snapshot  创建快照
POST   /sandboxes/{id}/rollback  回滚
```

---

## 4. 与现有安全机制的关系

**CubeSandbox 不是替代，是补充。**

| 工具 | 用途 | 执行环境 |
|---|---|---|
| `tool-shell` | 用户在白名单内的命令 | **宿主机** |
| `tool-fs` | 读写工作区文件 | **宿主机** |
| `tool-sandbox` | Agent 生成的任意代码 | **CubeSandbox** |

### 4.1 什么时候用 `tool-shell`，什么时候用 `tool-sandbox`

| 场景 | 用哪个 | 理由 |
|---|---|---|
| 跑 `git status` | `tool-shell` | 白名单内，宿主上跑更快 |
| 跑 `ls` / `cat` | `tool-shell` | 同上 |
| 跑 Agent 生成的 Python 脚本 | `tool-sandbox` | 代码不可预知，必须隔离 |
| 跑下载的第三方脚本 | `tool-sandbox` | 不可信代码 |
| 跑需要多语言环境的任务 | `tool-sandbox` | 沙箱自带环境，不污染宿主 |
| 长时间计算任务 | `tool-sandbox` | 不占用宿主资源 |

**判断标准**：**这段代码是不是"不可预知"的？**

- 可预知（白名单命令）→ `tool-shell`
- 不可预知（Agent 生成、用户下载）→ `tool-sandbox`

### 4.2 双层防护

`tool-sandbox` 和 CubeSandbox 的网络策略形成双层防护：

```
Layer 1: Locowrk 的 SideEffect 分级
  → tool-sandbox 标为 Irreversible
  → 用户审批

Layer 2: CubeSandbox 的网络策略
  → 沙箱内即使有恶意代码，也无法访问内网
  → 无法向外部发送数据
```

---

## 5. 集成架构

### 5.1 整体结构

```
┌─────────────────────────────────────────────────┐
│  Locowrk 内核                                    │
│                                                 │
│  Kernel ──> ToolRegistry ──> Tool trait         │
│                                    ↑            │
│                                    │            │
│                          ┌─────────┴─────────┐  │
│                          │  tool-sandbox 插件 │  │
│                          │  （DLL 或内嵌）    │  │
│                          └─────────┬─────────┘  │
└────────────────────────────────────┼────────────┘
                                     │ HTTP / SDK
                                     ▼
┌─────────────────────────────────────────────────┐
│  CubeSandbox 服务                                │
│  （独立部署 / 腾讯云托管）                        │
│                                                 │
│  MicroVM #1  MicroVM #2  MicroVM #3  ...        │
└─────────────────────────────────────────────────┘
```

### 5.2 tool-sandbox 插件的位置

**它是一个工具插件，不是中间件插件。**

理由：它是被 LLM 主动调用的（"我要执行这段代码"），不是拦截内核请求的。

```
crates/
└── plugin-tool-sandbox/
    ├── src/
    │   ├── lib.rs              ← 插件入口
    │   ├── client.rs           ← CubeSandbox 客户端
    │   ├── tools.rs            ← 工具定义
    │   └── session.rs          ← 沙箱会话管理
    └── Cargo.toml
```

**它是 DLL 还是内嵌？**

按之前的判断标准："第三方会不会写？"

- 如果 CubeSandbox 是官方推荐的沙箱方案 → 内嵌
- 如果未来可能支持多种沙箱后端（CubeSandbox / E2B / 自建）→ DLL

**建议：先做 DLL。** 因为它依赖外部服务，适合独立分发。

### 5.3 通信方式

Locowrk 通过 **HTTP REST API** 与 CubeSandbox 通信。

理由：
- CubeSandbox 的官方 SDK 是 Python / Go，Rust 没有官方 SDK
- REST API 稳定、简单、易于调试
- 不需要引入跨语言的 FFI 复杂度

```rust
pub struct CubeSandboxClient {
    base_url: String,
    api_key: Option<String>,
    http: reqwest::Client,
}

impl CubeSandboxClient {
    pub async fn create_sandbox(&self, config: SandboxConfig) -> Result<SandboxId>;
    pub async fn execute(&self, id: &SandboxId, code: &str) -> Result<ExecutionResult>;
    pub async fn destroy(&self, id: &SandboxId) -> Result<()>;
    pub async fn snapshot(&self, id: &SandboxId) -> Result<SnapshotId>;
    pub async fn rollback(&self, id: &SandboxId, snapshot: &SnapshotId) -> Result<()>;
}
```

---

## 6. 工具设计

### 6.1 工具清单

| 工具名 | 作用 | SideEffect |
|---|---|---|
| `sandbox_execute` | 在沙箱里执行代码 | **`Irreversible`** |
| `sandbox_read_file` | 读沙箱内的文件 | `Pure` |
| `sandbox_write_file` | 写文件到沙箱 | `Compensable` |
| `sandbox_snapshot` | 创建快照 | `Pure` |
| `sandbox_rollback` | 回滚到快照 | `Compensable` |
| `sandbox_status` | 查询沙箱状态 | `Pure` |

**`sandbox_execute` 必须标 `Irreversible`。** 它是所有工具里最危险的一个——代码可能是恶意的，可能读写沙箱内的所有文件，可能发起网络请求。

### 6.2 工具 schema

```json
{
  "name": "sandbox_execute",
  "description": "在隔离沙箱中执行代码。适用于运行不可预知的代码，例如 Agent 生成的脚本、用户下载的第三方代码。沙箱与宿主完全隔离。",
  "schema": {
    "type": "object",
    "properties": {
      "language": {
        "type": "string",
        "enum": ["python", "javascript", "bash", "go"],
        "description": "代码的语言"
      },
      "code": {
        "type": "string",
        "description": "要执行的代码"
      },
      "timeout_seconds": {
        "type": "integer",
        "minimum": 1,
        "maximum": 300,
        "default": 30,
        "description": "执行超时时间"
      },
      "reuse": {
        "type": "boolean",
        "default": true,
        "description": "是否复用当前会话的沙箱。false 则每次创建新的"
      }
    },
    "required": ["language", "code"]
  },
  "side_effect": "Irreversible",
  "exposure": "on_demand"
}
```

**`exposure` 设为 `on_demand`。** 只有当模型判断需要执行不可预知代码时，才通过 `search_tools` 拉取。平时不暴露，节省上下文。

### 6.3 返回结构

```json
{
  "exit_code": 0,
  "stdout": "...",
  "stderr": "...",
  "duration_ms": 234,
  "sandbox_id": "sbx_abc123",
  "artifacts": [
    { "path": "/output/chart.png", "size": 12480 }
  ]
}
```

**沙箱执行产出的文件（图片、数据、日志），应该作为 artifact 返回。**

UI 上可以展示，用户可以选择下载或保存到工作区。

---

## 7. 后端实现

### 7.1 插件入口

```rust
// plugin-tool-sandbox/src/lib.rs
use plugin_api::*;

#[derive(Default)]
pub struct SandboxPlugin {
    client: Option<CubeSandboxClient>,
    sessions: DashMap<SessionId, SandboxId>,
}

#[async_trait]
impl Plugin for SandboxPlugin {
    fn inject(&self) -> Vec<TypeId> { vec![] }

    async fn apply(&self, ctx: &Context) -> Result<(), PluginError> {
        // 读配置
        let config = ctx.config::<SandboxConfig>()?;
        let client = CubeSandboxClient::new(config.base_url, config.api_key)?;

        // 注册工具
        ctx.register_tool(SandboxExecute::new(client.clone(), self.sessions.clone()))?;
        ctx.register_tool(SandboxReadFile::new(client.clone()))?;
        ctx.register_tool(SandboxWriteFile::new(client.clone()))?;
        ctx.register_tool(SandboxSnapshot::new(client.clone()))?;
        ctx.register_tool(SandboxRollback::new(client.clone()))?;
        ctx.register_tool(SandboxStatus::new(client.clone()))?;

        Ok(())
    }

    async fn dispose(&self, ctx: &Context) -> Result<(), PluginError> {
        // 销毁所有活跃沙箱
        for entry in self.sessions.iter() {
            let _ = self.client.as_ref().unwrap().destroy(entry.value()).await;
        }
        Ok(())
    }
}
```

### 7.2 沙箱复用策略

**同一会话内，复用同一个沙箱。**

理由：
- 复用可以保持上下文（比如先 `cd` 再 `ls`）
- 复用可以避免反复创建销毁的开销
- 每个会话独立，互不干扰

```rust
impl SandboxExecute {
    async fn call(&self, ctx: &CallCtx, args: Value) -> Result<Value, ToolError> {
        let args: ExecuteArgs = serde_json::from_value(args)?;
        let session_id = ctx.session_id();

        // 检查是否复用
        let sandbox_id = if args.reuse {
            match self.sessions.get(&session_id) {
                Some(id) => id.clone(),
                None => {
                    let id = self.client.create_sandbox(SandboxConfig {
                        language: args.language,
                        timeout: args.timeout_seconds,
                        network: self.network_policy(),
                        ..
                    }).await?;
                    self.sessions.insert(session_id.clone(), id.clone());
                    id
                }
            }
        } else {
            self.client.create_sandbox(/* ... */).await?
        };

        // 执行代码
        let result = self.client.execute(&sandbox_id, &args.code).await?;

        // 如果是一次性沙箱，执行完销毁
        if !args.reuse {
            self.client.destroy(&sandbox_id).await?;
        }

        Ok(serde_json::to_value(result)?)
    }
}
```

### 7.3 会话结束的清理

**会话结束时，销毁对应的沙箱。**

```rust
// 订阅会话结束事件
ctx.events().subscribe("session/end", |session_id| async move {
    if let Some((_, sandbox_id)) = sessions.remove(&session_id) {
        let _ = client.destroy(&sandbox_id).await;
    }
});
```

**进程退出时，也要清理。** 通过 `Plugin::dispose` 或进程退出钩子。

### 7.4 超时与中断

**沙箱执行的超时要遵守用户配置。**

```rust
let timeout = args.timeout_seconds
    .unwrap_or(config.default_timeout)
    .min(config.max_timeout);  // 硬上限

tokio::time::timeout(
    Duration::from_secs(timeout),
    self.client.execute(&sandbox_id, &args.code)
).await
```

**超时后要主动 kill 沙箱内的进程。** CubeSandbox 的 API 应该支持中断执行，如果不支持，直接销毁沙箱重建。

---

## 8. 生命周期管理

### 8.1 沙箱的三种生命周期

| 模式 | 创建时机 | 销毁时机 | 适用场景 |
|---|---|---|---|
| **会话级**（默认） | 首次执行时 | 会话结束 | 多轮任务，需要保持状态 |
| **单次级** | 每次执行 | 执行结束 | 一次性任务，不需要状态 |
| **持久级** | 用户显式创建 | 用户显式销毁 | 跨会话的长任务 |

**默认会话级。** 用户在设置里可以改。

### 8.2 空闲回收

**如果沙箱一段时间（比如 10 分钟）没有活动，自动销毁。**

理由：CubeSandbox 虽然开销小，但长期挂着也是资源浪费。

```rust
// 后台任务：每分钟检查一次
async fn gc_idle_sandboxes(sessions: &DashMap<SessionId, SandboxEntry>) {
    let now = Instant::now();
    for entry in sessions.iter() {
        if now.duration_since(entry.value().last_activity) > Duration::from_secs(600) {
            let _ = client.destroy(entry.value().sandbox_id).await;
            sessions.remove(entry.key());
        }
    }
}
```

### 8.3 快照与回滚

**长任务用快照管理状态。**

```
1. 创建沙箱，跑一段代码
2. 创建快照 snapshot_1
3. 继续跑，出错了
4. 回滚到 snapshot_1
5. 换个方式重试
```

**这个能力对 Agent 特别有用**——它让 Agent 可以"试错"，而不需要每次都从头开始。

---

## 9. 安全策略

### 9.1 网络策略

**默认禁止沙箱访问网络。**

理由：Agent 生成的代码可能包含恶意网络请求。默认关闭网络，只有用户显式允许时才开启。

```rust
pub struct NetworkPolicy {
    /// 是否允许访问外网
    pub allow_outbound: bool,           // 默认 false

    /// 允许访问的域名白名单
    pub allowed_domains: Vec<String>,   // 默认空

    /// 是否允许访问内网
    pub allow_private_network: bool,    // 默认 false（硬底线）
}
```

**"允许访问内网"应该锁死为 false。** 这是硬底线——沙箱存在的意义就是隔离，允许访问内网等于白隔离。

### 9.2 资源限制

| 资源 | 默认值 | 用户可调 |
|---|---|---|
| CPU 核心数 | 1 | ✅ |
| 内存上限 | 512MB | ✅ |
| 磁盘上限 | 1GB | ✅ |
| 执行超时 | 30s | ✅ |
| 并发沙箱数 | 3 | ✅ |

**所有资源限制都要可配置。** 符合"约束最小化原则"。

### 9.3 与审批系统的集成

`tool-sandbox` 标为 `Irreversible`，所以每次调用都会触发审批。

**审批弹窗要展示**：
- 要执行的代码（截断显示）
- 语言
- 沙箱的网络策略
- 预计执行时间

```
⚠️ 即将在沙箱中执行代码

  语言: Python
  代码:
    import requests
    r = requests.get("https://api.example.com/data")
    print(r.json())
  
  沙箱配置:
    - 网络: 仅允许 api.example.com
    - 内存: 512MB
    - 超时: 30s
  
  [拒绝]  [仅本次允许]  [本会话始终允许]
```

---

## 10. UI 呈现

### 10.1 会话视图中的沙箱执行卡片

```
┌─────────────────────────────────────────────┐
│  ⚙ sandbox_execute            Python  v1.0.0│
│                                             │
│  ┌─────────────────────────────────────┐    │
│  │ import requests                     │    │
│  │ r = requests.get("...")             │    │
│  │ print(r.json())                     │    │
│  └─────────────────────────────────────┘    │
│                                             │
│  输出:                                       │
│  ┌─────────────────────────────────────┐    │
│  │ {"status": "ok", "count": 42}       │    │
│  └─────────────────────────────────────┘    │
│                                             │
│  ✓ 成功 · 234ms · 沙箱 sbx_abc123            │
│  [查看沙箱] [查看文件]                        │
└─────────────────────────────────────────────┘
```

### 10.2 沙箱管理面板

在设置里加一个"沙箱"分组：

```
设置 → 沙箱

CubeSandbox 服务地址
  [http://localhost:8080                    ]

API Key（可选）
  [***                                      ]

────────────────────────────────────

默认配置
  内存上限:   [512] MB
  磁盘上限:   [1] GB
  执行超时:   [30] 秒
  并发沙箱数: [3]

网络策略
  ☐ 允许访问外网
     域名白名单:
       [+ 添加]
  ☐ 允许访问内网
     ⚠️ 强烈不建议。开启后沙箱隔离失效。

生命周期
  ● 会话级（推荐）
  ○ 单次级
  ○ 持久级

空闲回收时间: [10] 分钟

────────────────────────────────────

活跃沙箱
  sbx_abc123  Python  内存 128MB  2 分钟前
  sbx_def456  Node.js 内存 64MB   5 分钟前
  
  [全部销毁]
```

### 10.3 开发者面板

```
沙箱统计

  本次会话:
    创建: 3
    复用: 12
    销毁: 2
    平均执行时间: 234ms
    总执行次数: 15

  资源占用:
    内存峰值: 256MB
    磁盘峰值: 12MB
    网络请求: 0（网络已禁用）
```

---

## 11. 配置项

### 11.1 插件配置

```toml
# plugins/tool-sandbox/config.toml

[service]
base_url = "http://localhost:8080"
api_key = ""                          # 可选
connect_timeout_seconds = 5
request_timeout_seconds = 60

[defaults]
memory_mb = 512
disk_mb = 1024
timeout_seconds = 30
max_concurrent = 3

[network]
allow_outbound = false
allowed_domains = []
allow_private_network = false         # 硬底线，建议锁死

[lifecycle]
mode = "session"                      # session | single | persistent
idle_recycle_minutes = 10

[snapshot]
enabled = true
max_snapshots_per_session = 5
```

### 11.2 配置的存储位置

```
~/.locowrk/plugins/tool-sandbox/
  ├── config.toml          ← 插件配置
  └── snapshots/           ← 快照元数据（如果本地缓存）
```

---

## 12. 落地路径

| 阶段 | 内容 | 复杂度 |
|---|---|---|
| 1 | 部署 CubeSandbox 服务（本地 Docker 或腾讯云） | ★ |
| 2 | 实现 `CubeSandboxClient`（REST API 封装） | ★★ |
| 3 | 实现 `sandbox_execute` 工具 | ★★ |
| 4 | 实现会话级沙箱复用 | ★★ |
| 5 | 实现会话结束的清理 | ★ |
| 6 | 实现 `sandbox_read_file` / `sandbox_write_file` | ★ |
| 7 | 审批弹窗的沙箱配置展示 | ★ |
| 8 | 设置面板的沙箱配置 | ★★ |
| 9 | 快照与回滚 | ★★ |
| 10 | 空闲回收 | ★ |
| 11 | 沙箱管理面板 | ★★ |

**1-7 是核心，一周能做完。8-11 是增强，再做一周。**

---

## 13. 常见坑

| 坑 | 原因 | 解法 |
|---|---|---|
| 沙箱创建失败 | 服务地址不通 | 启动时健康检查，失败则禁用工具 |
| 沙箱泄漏 | 会话结束没清理 | 订阅 `session/end` 事件 + 进程退出钩子 |
| 长任务超时 | 默认 30s 太短 | 用户可配，或者让模型自己指定 |
| 内存爆炸 | 沙箱内跑大任务 | 资源限制 + 监控 + 超限自动销毁 |
| 网络访问内网 | 用户误开 | 硬底线锁死为 false |
| 快照丢失 | 沙箱被回收了 | 快照要持久化，不能依赖沙箱本身 |
| 代码里有敏感信息 | 输出被记录到日志 | 沙箱输出不进主日志，只在会话里展示 |
| 沙箱 ID 泄漏到 LLM | 返回结构里包含 | 只在 UI 展示，不注入到下一轮上下文 |
| API Key 明文 | 配置文件里直写 | 支持环境变量 `CUBE_SANDBOX_API_KEY` |

---

## 14. 设计哲学

### 14.1 沙箱是"允许犯错"的基础设施

**没有沙箱，Agent 就只能做"确定安全"的事。** 所有需要试错、需要实验、需要运行不可预知代码的场景，都做不了。

**有了沙箱，Agent 才能真的"动手"。** 它可以写代码、跑代码、看结果、改代码、再跑——这才是真正的"自主执行"。

### 14.2 隔离不是为了限制，是为了放心

沙箱看起来是"限制 Agent"，实际上是**给 Agent 更多的自由**。

- 没有沙箱 → 只能跑白名单命令
- 有了沙箱 → 可以跑任何代码

**限制的是"影响范围"，不是"能力范围"。**

这和 Locowrk 的"约束最小化原则"完全一致——**给用户最大权限，只在真正影响安全的地方隔离。**

### 14.3 用完即走

CubeSandbox 的 60ms 冷启动，意味着**沙箱不是"长期资源"，是"临时工具"**。

用完就销毁，下一个任务创建新的。这是"函数式"的执行模型——无状态、可重复、易回滚。

### 14.4 与"捆稻草"的呼应

**沙箱的快照/回滚，和记忆树的"捆稻草"是同一个思路。**

- 记忆：原文不丢，只是整理成捆
- 沙箱：状态不丢，只是存成快照

**都是"保留全部，只改变访问方式"。**

---

*文档版本：v1.0*
*配套项目：Locowrk*
*关联文档：《Agent 内核设计》《约束最小化原则》《记忆树设计》*