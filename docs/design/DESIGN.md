# Agent 内核完整设计方案（Rust）

> 一份从零构建的可热插拔、可回退、带桌面端的 Agent 系统设计文档。
> 核心思想：借鉴 DeepSeek Harness / Cordis 的"一切皆插件"哲学，用 Rust 的所有权模型实现零开销的服务组合、精确的副作用撤销、多 Agent 进程内隔离，并用 Tauri 2 提供插件管理桌面端。

---

## 目录

**第一部分：内核设计**
1. [设计目标与约束](#1-设计目标与约束)
2. [为什么是 Rust](#2-为什么是-rust)
3. [Cordis 五支柱](#3-cordis-五支柱)
4. [会话日志：唯一真相源](#4-会话日志唯一真相源)
5. [插件分发与 ABI 边界](#5-插件分发与-abi-边界)
6. [回退的三层粒度](#6-回退的三层粒度)
7. [中间件链与 Agent 主循环](#7-中间件链与-agent-主循环)

**第二部分：工程组织**
8. [Crate 工作区结构](#8-crate-工作区结构)
9. [关键实现细节](#9-关键实现细节)

**第三部分：桌面端**
10. [桌面端技术栈选型](#10-桌面端技术栈选型)
11. [PluginService 与插件管理](#11-pluginservice-与插件管理)
12. [Tauri IPC 层设计](#12-tauri-ipc-层设计)
13. [UI 功能模块设计](#13-ui-功能模块设计)

**第四部分：交付**
14. [落地路径](#14-落地路径)
15. [风险与权衡](#15-风险与权衡)
16. [附录](#附录-a关键类型索引)

---

# 第一部分：内核设计

## 1. 设计目标与约束

### 1.1 目标

| 目标 | 含义 | 优先级 |
|---|---|---|
| **一切皆插件** | 模型适配器、工具、会话存储、沙箱、Agent 循环本身都是插件，无特权模块 | P0 |
| **热插拔** | 插件可运行时加载/卸载/替换，旧版本可回退，微秒级切换 | P0 |
| **可回退** | 代码版本、会话状态、副作用三个层面都可回退 | P0 |
| **多 Agent 隔离** | 单进程运行多个异构 Agent，互不污染服务视图 | P1 |
| **可观测** | 全链路事件可审计，崩溃可恢复 | P1 |
| **可运维** | 桌面端可管理插件安装/卸载/更新，无需手改配置文件 | P1 |
| **性能** | 内核热路径无全局锁，服务查询 O(1) | P2 |

### 1.2 非目标

- 不追求跨语言插件（C ABI + WASM 双路线已覆盖）
- 不追求分布式 Agent 编排（本设计聚焦单机内核）
- 不追求零成本抽象到极致（可读性和正确性优先于纳秒级优化）

### 1.3 关键约束

- **ABI 冻结**：`plugin-api` crate 一旦发布，改动即破坏兼容，加字段只能追加到末尾
- **永不 dlclose**：生产环境物理保留已卸载库，避免残留函数指针导致段错误
- **FFI 边界必须 catch_unwind**：panic 跨 FFI 是 UB
- **先写日志再执行**：进程崩溃时可 replay 识别悬空调用
- **UI 不直接操作 Registry**：只通过 `PluginService`，保证业务逻辑集中

---

## 2. 为什么是 Rust

### 2.1 Go 的根本性障碍

Go 官方 `plugin` 包存在难以绕过的限制：

| 限制 | 具体表现 |
|---|---|
| 版本强耦合 | 插件与主程序必须在完全相同 Go 版本下编译，主程序升级需全量重编 |
| 无法卸载 | plugin 包没有提供任何卸载 API（只有 `Open`/`Lookup`，不存在 `Close`），旧版本无法释放，内存只增不减 |
| 路径缓存 | `plugin.Open` 对同一路径返回同一指针，无法重新加载 |
| 平台受限 | 仅支持 Linux/macOS，且文档列出 8 项重大 Warning |
| 依赖版本绑定 | go.mod 中所有共享依赖版本必须一致 |

社区实际方案是 **子进程 + gRPC**（`hashicorp/go-plugin`），代价是每插件独立进程，IPC 延迟毫秒级，远高于进程内调用。

### 2.2 Rust 的优势

| 维度 | Rust 表现 |
|---|---|
| 动态库卸载 | `libloading::Library::drop` 可真正卸载，配合引用计数实现安全热插拔 |
| 零开销服务查询 | `TypeId` + `DashMap` 提供服务键，无字符串比较 |
| 生命周期精确 | 所有权模型天然表达"在途调用持有实例引用" |
| 现成参考 | `cordis-rs`（Cordis 内核移植）、`dsh-rs`（DSH 完整移植） |
| 跨平台 | dlopen 在 Linux/macOS/Windows 均可用 |
| 桌面端集成 | Tauri 2 后端就是 Rust，内核 crate 直接复用 |

### 2.3 结论

**Agent 内核偏向 Rust。** 核心原因是"可逆效应"的实现需要精确的资源生命周期控制，以及桌面端 Tauri 2 需要后端与内核同语言。Go 路线可行，但热插拔与卸载能力显著弱于 Rust。

---

## 3. Cordis 五支柱

整套设计的地基。以下全部用 Rust 惯用法实现。

### 3.1 Context：服务的唯一载体

内核没有特权模块，所有能力都挂在同一个 `Context` 上，以**类型键**（`TypeId`）区分。

```rust
use dashmap::DashMap;
use std::any::{Any, TypeId};
use std::sync::Arc;

/// 服务标记 trait：Send + Sync + 'static 即可，TypeId 就是天然服务键
pub trait Service: Send + Sync + 'static {}
impl<T: ?Sized + Send + Sync + 'static> Service for T {}

pub struct Context {
    services: DashMap<TypeId, Arc<dyn Any + Send + Sync>>,
    // 账本锁只保护内存操作（push/remove），从不 await——用 std 锁即可（见 §9.1）
    effects:  std::sync::Mutex<Vec<EffectEntry>>,
    scope:    Scope,
    events:   EventBus,
}

impl Context {
    /// 按具体类型注册服务，返回 EffectId 用于后续撤销
    pub fn provide<S: Service>(&self, svc: S) -> EffectId {
        let key = TypeId::of::<S>();
        // 捕获旧值：撤销时恢复旧注册而不是直接删除——
        // 否则"覆盖注册后撤销"会误删第一次注册的服务
        let previous = self.services.insert(key, Arc::new(svc));
        let entry = EffectEntry {
            id: EffectId::new(),
            fiber_id: current_fiber_id(),
            disposer: Box::new(Unprovide { key, previous }),
        };
        self.effects_push(entry);
        entry.id
    }

    /// 按 trait 对象注册服务：键 = TypeId::of::<T>()，T 是 trait 本身。
    /// 存储时装箱为 Arc<dyn Any>（内层才是 Arc<dyn T>）
    pub fn provide_dyn<T: ?Sized + Service>(&self, svc: Arc<dyn T>) -> EffectId {
        let key = TypeId::of::<T>();
        let previous = self.services.insert(key, Arc::new(svc));
        let entry = EffectEntry {
            id: EffectId::new(),
            fiber_id: current_fiber_id(),
            disposer: Box::new(Unprovide { key, previous }),
        };
        self.effects_push(entry);
        entry.id
    }

    /// 按具体类型查询服务，返回 Arc 快照
    pub fn use_service<S: Service>(&self) -> Result<Arc<S>, ServiceMissing> {
        self.services
            .get(&TypeId::of::<S>())
            .and_then(|e| e.clone().downcast::<S>().ok())
            .ok_or_else(ServiceMissing::new::<S>)
    }

    /// 按 trait 查询服务。跨插件解耦一律走这对 API：
    /// 模型插件 provide_dyn::<dyn LlmService>(adapter)，内核 use_dyn::<dyn LlmService>()。
    /// 注意 use_service::<dyn LlmService>() 永远查不到——
    /// provide::<S> 的 S 是 Sized，注册键是具体类型的 TypeId，两者不相等
    pub fn use_dyn<T: ?Sized + Service>(&self) -> Result<Arc<dyn T>, ServiceMissing> {
        self.services
            .get(&TypeId::of::<T>())
            .and_then(|e| e.clone().downcast::<Arc<dyn T>>().ok())
            .map(|boxed| (*boxed).clone())
            .ok_or_else(ServiceMissing::new::<T>)
    }

    /// 显式撤销某个 Effect。
    /// 与 revoke_by_fiber 同一纪律：先出账、放锁、再执行逆操作——
    /// Disposer 可能回调 Context，持锁 await 会死锁（tokio Mutex 不可重入）
    pub async fn revoke(&self, id: EffectId) -> Result<(), EffectError> {
        let entry = {
            let mut effects = self.effects.lock().expect("effects lock poisoned");
            let pos = effects.iter().position(|e| e.id == id)
                .ok_or(EffectError::NotFound(id))?;
            effects.remove(pos)
        };
        entry.disposer.dispose().await;
        Ok(())
    }
}
```

**关键设计点**：

- `Service` 是标记 trait（空实现 + blanket impl），不需要 `const KEY`。Rust 的 `TypeId` 就是天然服务键。
- 服务键有两种：具体类型键（`provide` / `use_service`）与 trait 对象键（`provide_dyn` / `use_dyn`）。**跨插件边界一律用 trait 对象键**，否则消费方与提供方无法解耦（见 `use_dyn` 的注释）。
- `use_service` / `use_dyn` 返回 `Arc` 快照，调用方只在这一瞬间持有分片读锁，后续长耗时操作不持锁。
- `DashMap` 是分片读写锁（并非无锁）：读只锁命中的分片，写只锁对应分片，热路径没有全局锁。
- `provide` 捕获被覆盖的旧值，`Unprovide` 撤销时恢复旧值而非删除，保证覆盖注册的撤销语义正确。

### 3.2 Effect：每次修改携带逆操作

**整个设计中最关键的一条纪律。** 任何对 `Context` 的修改都必须返回一个 `Disposer`，运行时按 LIFO 顺序维护撤销链。

Cordis 论文将一次 Effect 定义为：接收当前上下文，返回修改后的新上下文以及逆操作。多个 Effect 按顺序执行时，逆操作按相反顺序组合。

```rust
#[async_trait::async_trait]
pub trait Disposer: Send + Sync + 'static {
    async fn dispose(self: Box<Self>);
}

pub struct EffectEntry {
    pub id: EffectId,
    pub fiber_id: FiberId,           // 用于交错 Effect 的独立撤销
    pub disposer: Box<dyn Disposer>,
}

impl Context {
    /// 执行一次可逆注册：f 做修改，同时返回逆操作
    pub fn effect<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&Self) -> (R, Box<dyn Disposer>),
    {
        let (result, disposer) = f(self);
        self.effects_push(EffectEntry {
            id: EffectId::new(),
            fiber_id: current_fiber_id(),
            disposer,
        });
        result
    }
}
```

#### 交错 Effect 的独立性

**工程难点**：多个组件的 Effect 可能交错发生，只卸载其中一个组件时，需撤销它的 Effect 而保留其他组件的修改。

**解决方案**：为每个 Effect 打上 `FiberId` 标签。`revoke_by_fiber(fid)` 只过滤匹配 `FiberId` 的条目，并按 LIFO 顺序执行，而非简单从尾部弹出。

```rust
impl Context {
    pub async fn revoke_by_fiber(&self, fid: FiberId) {
        let mine = {
            let mut effects = self.effects.lock().expect("effects lock poisoned");
            // 提取该 Fiber 的所有 Effect，保持原有顺序
            let (mine, rest): (Vec<_>, Vec<_>) = effects
                .drain(..)
                .partition(|e| e.fiber_id == fid);
            *effects = rest;
            mine
        };  // 出作用域即放锁，与 revoke 的纪律一致

        // 逆序执行撤销（不持锁，Disposer 可安全回调 Context）
        for entry in mine.into_iter().rev() {
            entry.disposer.dispose().await;
        }
    }
}
```

### 3.3 Fiber：插件生命周期容器

Fiber 是插件的一个运行实例，采用六态状态机。

```rust
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FiberState {
    Pending,            // 等待依赖满足
    Loading,            // 正在执行 apply
    Active,             // 正常运行
    Unloading,          // 正在执行撤销链
    Unloaded,           // 已卸载，可重新激活
    Failed(FailureKind),
}

pub struct Fiber {
    id: FiberId,
    plugin: Box<dyn Plugin>,
    // Failed(FailureKind) 携带载荷，压不进 AtomicU8；用 RwLock 存完整状态
    state: Arc<std::sync::RwLock<FiberState>>,
    deps: Vec<TypeId>,                  // 声明的依赖（inject）
    // Effect 账本唯一存于 Context（按 FiberId 打标签），Fiber 不再维护副本
}

#[async_trait::async_trait]
pub trait Plugin: Send + Sync + 'static {
    /// 声明依赖的服务类型
    fn inject(&self) -> Vec<TypeId> { vec![] }

    /// 插件激活：所有依赖满足后调用
    async fn apply(&self, ctx: &Context) -> Result<(), PluginError>;

    /// 插件卸载：撤销链执行完毕后调用
    async fn dispose(&self, ctx: &Context) -> Result<(), PluginError>;
}
```

#### Fiber 的状态转换

```
        ┌──────────────────────────────────────┐
        │                                      │
        ▼                                      │
    Pending ──[依赖满足]──> Loading ──[apply成功]──> Active
        ▲                     │                      │
        │                     │[apply失败]           │[依赖消失]
        │                     ▼                      ▼
        │                  Failed               Unloading
        │                                           │
        │                                           │[撤销链完成]
        │                                           ▼
        └──────────[依赖恢复]──────────────────── Unloaded
```

#### 依赖图与反应式重载

运行时维护三张表：

```rust
pub struct Runtime {
    dependents:       HashMap<FiberId, Vec<FiberId>>,   // 反向依赖
    service_providers: HashMap<TypeId, FiberId>,        // 服务提供者
    graph:            DependencyGraph,                   // 拓扑排序
}
```

**替换流程**（例如 `ctx.llm` 被替换）：

1. 沿反向依赖链找到所有 `inject(llm)` 的 Fiber
2. 让下游 Fiber 进入 `Unloading`，执行撤销链
3. 替换 Provider
4. 按拓扑序重新激活下游 Fiber

这就是"反应式余效应"的工程落地：**依赖变化不是轮询检测，而是运行时主动通知**。

### 3.4 Scope：一进程多 Agent 隔离

`Scope` 让同一个进程可以运行多个异构 Agent，每个 Agent 有独立的注册层。

```rust
#[derive(Clone)]
pub struct ScopeKey(Arc<()>);

impl PartialEq for ScopeKey {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)   // 零开销身份比较
    }
}
impl Eq for ScopeKey {}

// 必须手写指针哈希：DashMap 键要求 Eq 与 Hash 一致。
// 绝不能 derive(Hash)——Arc<()> 的自动 Hash 对内层 () 求值，所有 ScopeKey
// 哈希相同而 Eq 不同，既违反契约又把所有作用域挤进同一分片
impl std::hash::Hash for ScopeKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::ptr::hash(Arc::as_ptr(&self.0), state)
    }
}

pub struct ScopedLayers<L> {
    global: L,
    scoped: DashMap<ScopeKey, L>,       // 按作用域惰性创建
}

impl<L: Default> ScopedLayers<L> {
    /// Agent 本地注册只写自己的 scoped layer
    pub fn provide_scoped<S: Service>(&self, key: ScopeKey, svc: S) {
        self.scoped
            .entry(key)
            .or_default()
            .provide(svc);
    }

    pub fn use_scoped<S: Service>(&self, key: &ScopeKey) -> Option<Arc<S>> {
        self.scoped
            .get(key)
            .and_then(|l| l.use_service::<S>().ok())
            .or_else(|| self.global.use_service::<S>().ok())
    }
}
```

**单 Agent 场景下 scoped map 始终为空，开销只有一次 `DashMap` 的空查询。**

> **实现落点（v2.3）**：`ScopedLayers` 不是独立于 Context 的第二张表，而是
> Context 服务表的**载体**——`layers.global()` 即 §3.1 的服务表，scoped 层
> 按 `ScopeKey` 惰性创建。注册入口为 `provide` / `provide_dyn`（global）与
> `provide_scoped` / `provide_dyn_scoped`（scoped），后者同样走 Effect 账本
> 可撤销；查询入口 `use_service_scoped` / `use_dyn_scoped` 先查 scoped 层、
> 未命中回退 global。`Kernel::scoped(key)` 使主循环的服务查询作用域化
> （§14 阶段四"两 Agent 工具视图隔离"的落点）。

### 3.5 事件总线：四种分发语义

事件总线同时承担协作和拦截两个职责。

| 模式 | 语义 | 典型用途 |
|---|---|---|
| `emit` | 异步通知，不等待返回值 | 日志、指标、遥测 |
| `parallel` | 并发通知所有监听者 | 缓存刷新、索引更新 |
| `serial` | 顺序通知，可短路 | 审计链、安全过滤器 |
| `waterfall` | 顺序经过中间件，每个可改写/拒绝 | Agent 循环插桩、请求改写、审批 |

```rust
pub enum WaterfallDecision {
    Continue,
    Replace(serde_json::Value),
    Reject(RejectReason),
    Retry { delay: Duration },
}

pub struct EventBus {
    listeners: DashMap<String, Vec<Arc<dyn Listener>>>,   // key = 事件名
}

impl EventBus {
    pub async fn emit(&self, key: &str, payload: Value);
    pub async fn parallel(&self, key: &str, payload: Value);
    pub async fn serial(&self, key: &str, payload: Value) -> SerialOutcome;
    pub async fn waterfall(&self, key: &str, payload: Value) -> WaterfallDecision;
}
```

> **返回值精化（v2.3）**：伪代码让 `waterfall` 只返回 `WaterfallDecision`，
> 但内核需要**改写后的 payload** 才能继续（§7.2 的 `parse_request`），故实现
> 返回 `WaterfallOutcome { payload, decision }`：决策环顺序执行，`Continue`
> 传原 payload、`Replace` 传改写后的、`Reject` / `Retry` 当场短路并作为终态
> 决策返回；无监听器 = 原样放行。监听器签名 `handle(&key, &payload) ->
> WaterfallDecision`（借用传入，要改写就克隆返回 `Replace`）；serial 监听器
> 返回 `bool`（false = 短路），结局为 `SerialOutcome::{Completed, Stopped{at}}`。

**`waterfall` 是"连 Loop 都能热插拔"的工程实现。** 插件可接管三个决策点：

- `agent/pre-step`：处理用户消息前
- `agent/request`：发 LLM 请求前
- `agent/request-error`：请求失败后

---

## 4. 会话日志：唯一真相源

会话是一份**仅追加**的类型化事件日志，所有其他视图都是它的投影。参考 DSH 的设计，日志涵盖系统提示、思维链、工具调用与结果、子 Agent 调度。

### 4.1 事件定义

```rust
#[derive(Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionEvent {
    TurnStart        { turn_id: TurnId, ts: i64 },
    UserMessage      { content: String },
    AssistantChunk   { delta: String },
    AssistantMessage { content: String, tool_calls: Vec<CallId> },
    ToolCall         { call_id: CallId, tool: String, args: Value,
                       plugin_version: Version, side_effect: SideEffect },
    ToolResult       { call_id: CallId, output: Value, error: Option<String> },
    EffectApplied    { effect_id: EffectId, description: String },
    PluginActivated  { plugin_id: String, from: Option<Version>, to: Version },
    ManualIntervention { call_id: CallId, reason: String },
    ApprovalDecided  { call_id: CallId, allowed: bool, reason: Option<String> },
    TurnEnd          { turn_id: TurnId, outcome: TurnOutcome },
}
```

### 4.2 会话结构

```rust
pub struct Session {
    log: Vec<SessionEvent>,              // 只追加，追加时深冻结
    cursor: usize,                       // 当前回放位置
    snapshots: BTreeMap<usize, SessionState>,  // 每 N 个事件一个快照
    schema_version: u32,
}

impl Session {
    pub fn append(&mut self, e: SessionEvent) -> usize {
        self.log.push(e);
        self.log.len() - 1
    }

    /// 纯函数投影：从日志派生 LLM 消息历史。
    /// 两个要点：只看游标之前的事件（回退后不包含已撤销的轮次）；
    /// AssistantMessage 只存 CallId，工具名与参数从 ToolCall 事件回填
    /// （真实工具协议要求 assistant 消息携带完整 tool_calls，光有 id 不够）
    pub fn derive_messages(&self) -> Vec<Message> {
        let visible = &self.log[..self.cursor];

        // 第一遍：收集 ToolCall（call_id → tool / args）
        let mut calls = HashMap::new();
        for e in visible {
            if let SessionEvent::ToolCall { call_id, tool, args, .. } = e {
                calls.insert(call_id, (tool.clone(), args.clone()));
            }
        }

        // 第二遍：拼装消息
        visible.iter().filter_map(|e| match e {
            SessionEvent::UserMessage { content } =>
                Some(Message::user(content.clone())),
            SessionEvent::AssistantMessage { content, tool_calls } =>
                Some(Message::assistant_with_tools(content.clone(),
                    tool_calls.iter().map(|id| {
                        let (tool, args) = calls.get(id)
                            .cloned()
                            .unwrap_or_else(|| (String::new(), Value::Null));
                        ToolCallMsg { id: id.clone(), tool, args }
                    }).collect())),
            SessionEvent::ToolResult { call_id, output, .. } =>
                Some(Message::tool(call_id, output.clone())),
            _ => None,
        }).collect()
    }

    /// 状态 = fold(events from last snapshot)
    pub fn state_at(&self, seq: usize) -> SessionState {
        let (base_seq, mut st) = self.snapshots
            .range(..=seq).next_back()
            .map(|(s, v)| (*s, v.clone()))
            .unwrap_or((0, SessionState::default()));
        for e in &self.log[base_seq..seq] {
            st.apply(e);
        }
        st
    }

    /// 回退：移动游标，不删事件
    pub fn rewind_to(&mut self, seq: usize) {
        self.cursor = seq;
    }

    /// 分叉：从已有日志创建新会话
    pub fn fork(&self, at: usize) -> Session {
        Session {
            log: self.log[..at].to_vec(),
            cursor: at,
            // 只保留切断点之前的快照：超出的快照指向已不存在的事件，
            // 会让 state_at 的日志切片越界 panic
            snapshots: self.snapshots.range(..=at)
                .map(|(s, v)| (*s, v.clone())).collect(),
            schema_version: self.schema_version,
        }
    }
}
```

### 4.3 关键纪律

- **事件只追加不删除**：回退 = 移动游标 + replay，天然可审计、可分叉
- **先写日志再执行**：`ToolCall` 事件在调用发出前追加。崩溃后 replay 能识别"悬空调用"，触发超时回收或补偿
- **崩溃尾部修复**：`interruptedTurnClosers` 合成闭合事件恢复
- **快照版本化**：快照加 `schema_version` + upcaster 支持结构演进

---

## 5. 插件分发与 ABI 边界

### 5.1 ABI 契约层

**唯一冻结的 crate。** 一旦发布，改动即破坏兼容。

```rust
// plugin-api/src/lib.rs
pub const PLUGIN_ABI_VERSION: u32 = 1;

#[repr(C)]
pub struct HostVtable {
    pub log: extern "C" fn(level: u32, ptr: *const u8, len: usize),
    pub kv_get: extern "C" fn(k: *const u8, klen: usize,
                              out: *mut *mut u8, olen: *mut usize) -> i32,
    pub kv_put: extern "C" fn(k: *const u8, klen: usize,
                              v: *const u8, vlen: usize) -> i32,
}

// 插件必须导出的符号
pub type DescribeFn = extern "C" fn() -> *const u8;   // 返回 JSON 元数据
pub type InitFn     = extern "C" fn(host: *const HostVtable) -> *mut c_void;
pub type CallFn     = extern "C" fn(
    handle: *mut c_void,
    tool: *const u8, tool_len: usize,
    args: *const u8, args_len: usize,
    out: *mut *mut u8, out_len: *mut usize,
) -> i32;
pub type ShutdownFn = extern "C" fn(handle: *mut c_void);
pub type FreeBufFn  = extern "C" fn(ptr: *mut u8, len: usize);

pub const SYM_DESCRIBE: &[u8] = b"agent_plugin_describe\0";
pub const SYM_INIT:     &[u8] = b"agent_plugin_init\0";
pub const SYM_CALL:     &[u8] = b"agent_plugin_call\0";
pub const SYM_SHUTDOWN: &[u8] = b"agent_plugin_shutdown\0";
pub const SYM_FREE:     &[u8] = b"agent_plugin_free_buffer\0";
```

**长度协议补充**（v2.2 实现时确定）：`DescribeFn` 只返回 `*const u8`，无长度
参数——约定 describe 返回 **NUL 结尾的 UTF-8 JSON**（JSON 文本不含裸 NUL，
serde 会转义），宿主按 C 字符串读取。同理，call 的输出缓冲由**插件**分配
（`std::alloc`，对齐 1）、宿主用完必须经该插件的 `agent_plugin_free_buffer`
释放——两侧可能是不同分配器，禁止跨边界 free；缓冲区约定详见 `plugin-api::ffi`
的模块注释。

### 5.2 元数据契约

```rust
#[derive(Serialize, Deserialize)]
pub struct PluginManifest {
    pub id: String,
    pub version: String,             // semver
    pub abi_version: u32,
    pub kind: PluginKind,            // Tool / Middleware / Memory / Model
    pub tools: Vec<ToolSpec>,        // 每个工具自带 side_effect，不再有 manifest 级冗余字段
}

#[derive(Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub schema: serde_json::Value,   // JSON Schema
    pub side_effect: SideEffect,
}

#[derive(Serialize, Deserialize, PartialEq, Clone)]
pub enum SideEffect {
    Pure,                              // 只读，回退免费
    Compensable { undo_tool: String }, // 有补偿工具
    Irreversible,                      // 必须审批
}

/// 宿主侧工具 trait。定义在 plugin-api 是为了让 kernel-agent（消费方）
/// 与 plugin-host（适配方）共用同一类型，但它只活在宿主进程内，
/// 绝不进入 C ABI——FFI 侧的对应物是 CallFn（§5.1）
#[async_trait::async_trait]
pub trait Tool: Send + Sync + 'static {
    fn spec(&self) -> &ToolSpec;
    fn plugin_version(&self) -> &Version;
    async fn call(&self, ctx: &CallCtx, args: Value) -> Result<Value, ToolError>;
}

/// 一次工具调用的宿主侧上下文（session_id / call_id / deadline / 取消信号）
pub struct CallCtx { /* ... */ }
```

### 5.3 跨语言能力

C ABI + JSON 边界是**语言中立**的。任何能编译成动态库并导出 C 符号的语言都能写插件：

| 语言 | 可行性 | 方式 |
|---|---|---|
| C / C++ | 原生 | 直接实现符号 |
| Zig | 原生 | `export fn`，体验极好 |
| Rust | 一等公民 | 直接用 `plugin-api` crate |
| Go | 可行 | `-buildmode=c-shared`，启动 runtime 有开销 |
| Python | 可行 | 用 C 写 shim 转调 CPython C API |
| Nim | 原生 | `exportc` 编译成 dylib |

**Rust 的优势不是排他，而是最省事**：`plugin-api` 直接给类型定义，serde 序列化，一个 `declare_plugin!` 宏生成五个符号，panic 跨 FFI 的 `catch_unwind` 封装。其他语言需要几百行胶水代码，一次性投入。

### 5.4 插件侧骨架

```rust
// plugins/tool-http/src/lib.rs
use plugin_api::*;
use std::ffi::c_void;

struct State { host: *const HostVtable }

#[no_mangle]
pub extern "C" fn agent_plugin_describe() -> *const u8 {
    MANIFEST_JSON.as_ptr()   // 静态 JSON 字节串
}

#[no_mangle]
pub extern "C" fn agent_plugin_init(host: *const HostVtable) -> *mut c_void {
    Box::into_raw(Box::new(State { host })) as *mut c_void
}

#[no_mangle]
pub extern "C" fn agent_plugin_call(
    handle: *mut c_void,
    tool: *const u8, tool_len: usize,
    args: *const u8, args_len: usize,
    out: *mut *mut u8, out_len: *mut usize,
) -> i32 {
    // 必须 catch_unwind，panic 跨 FFI 是 UB
    let result = std::panic::catch_unwind(|| {
        let state = unsafe { &*(handle as *const State) };
        let tool = unsafe { std::slice::from_raw_parts(tool, tool_len) };
        let args = unsafe { std::slice::from_raw_parts(args, args_len) };
        // ... 执行，结果序列化后写 out
        write_out(json_bytes, out, out_len);
    });
    match result {
        Ok(_) => 0,
        Err(_) => -1,
    }
}
```

### 5.5 宿主侧适配器

```rust
pub struct SabiToolAdapter {
    inner: Arc<PluginInstance>,     // 持有 dlopen 句柄
    spec: ToolSpec,
    sem: Arc<Semaphore>,            // 并发限制
}

#[async_trait::async_trait]
impl Tool for SabiToolAdapter {
    fn spec(&self) -> &ToolSpec { &self.spec }
    fn plugin_version(&self) -> &Version { &self.inner.manifest.version }

    async fn call(&self, ctx: &CallCtx, args: Value) -> Result<Value, ToolError> {
        let _permit = self.sem.acquire().await?;
        let inst = self.inner.clone();
        let name = self.spec.name.clone();
        // 同步 FFI 调用丢到 blocking 池，避免阻塞 reactor
        tokio::task::spawn_blocking(move || inst.call_raw(&name, args))
            .await
            .map_err(|e| ToolError::Panic(e.to_string()))?
    }
}
```

### 5.6 热插拔的三个工程纪律

**纪律一：永不 dlclose。** `dlclose` 后代码段消失，残留函数指针会导致段错误。做法是**逻辑卸载 + 物理保留**：从注册表移除即可，库一直留在进程里，进程重启才真正释放。代价是内存，收益是零崩溃风险。IDE 和数据库插件系统普遍采用这个策略。唯一例外是"加载即失败"（§5.7 的 ABI 校验失败等）：init 从未执行、没有任何代码指针流出时，可直接释放句柄。

**纪律二：在途调用用 Arc 引用计数解决。** `deactivate` 只是把 `Arc<PluginInstance>` 从注册表拿走，调用方的 Arc 还在，`Drop` 自然延后。需要主动等待时轮询 `Arc::strong_count == 1`。

**纪律三：FFI 调用放 `spawn_blocking`。** 插件是同步 FFI，必须放 blocking 池。插件内必须 `catch_unwind`。

### 5.7 注册表与版本管理

```
plugins/
  tool-http/
    1.2.0/libtool_http.so
    1.3.0/libtool_http.so
  tool-shell/
    0.9.0/libtool_shell.so
```

多版本并存是回退的基础。

```rust
pub struct PluginRegistry {
    // key = plugin_id，value = 所有已加载版本。
    // 内部可变性：方法全部 &self，外层 Arc<PluginRegistry> 共享即可
    versions:  RwLock<HashMap<String, BTreeMap<Version, Arc<PluginInstance>>>>,
    active:    RwLock<HashMap<String, Version>>,
    graveyard: Mutex<Vec<Arc<PluginInstance>>>,   // 逻辑卸载但物理保留
}

impl PluginRegistry {
    pub fn load(&self, id: &str, ver: &Version, path: &Path)
        -> Result<Arc<PluginInstance>, LoadError>
    {
        if let Some(inst) = self.versions.read().get(id).and_then(|m| m.get(ver)) {
            return Ok(inst.clone());
        }
        let inst = PluginInstance::dlopen(path)?;
        if inst.manifest.abi_version != PLUGIN_ABI_VERSION {
            // 加载即失败的唯一豁免：init 从未执行、无代码指针流出，
            // 句柄随 inst drop 直接释放（"永不 dlclose"的例外，见附录 C）
            return Err(LoadError::AbiMismatch { .. });
        }
        self.versions.write()
            .entry(id.into()).or_default()
            .insert(ver.clone(), inst.clone());
        Ok(inst)
    }

    pub fn activate(&self, id: &str, ver: &Version) -> Result<(), LoadError> {
        let inst = self.versions.read().get(id).and_then(|m| m.get(ver))
            .ok_or(LoadError::NotLoaded)?.clone();
        self.active.write().insert(id.into(), ver.clone());
        Ok(())
    }

    /// 版本回退：本质就是 activate 到旧版本
    pub fn rollback(&self, id: &str) -> Result<Version, LoadError> {
        let cur = self.active.read().get(id).cloned().ok_or(LoadError::NotLoaded)?;
        let prev = self.versions.read().get(id)
            .and_then(|m| m.range(..&cur).next_back().map(|(v, _)| v.clone()))
            .ok_or(LoadError::NoPreviousVersion)?;
        self.activate(id, &prev)?;
        Ok(prev)
    }

    /// 逻辑卸载：Arc 交给 graveyard，在途调用结束后自动释放
    pub fn deactivate(&self, id: &str) {
        if let Some(ver) = self.active.write().remove(id) {
            if let Some(inst) = self.versions.write()
                .get_mut(id).and_then(|m| m.remove(&ver))
            {
                self.graveyard.lock().expect("graveyard lock poisoned").push(inst);
            }
        }
    }

    /// 当前激活版本的实例（读路径：拿到 Arc 即放锁）
    pub fn current(&self, id: &str) -> Option<Arc<PluginInstance>> {
        let ver = self.active.read().get(id)?.clone();
        self.versions.read().get(id)?.get(&ver).cloned()
    }

    pub fn has_version(&self, id: &str, ver: &Version) -> bool {
        self.versions.read().get(id).map_or(false, |m| m.contains_key(ver))
    }

    /// 列出所有已加载插件状态（供 UI 展示）
    pub fn list_states(&self) -> Vec<PluginUiState>;

    /// 订阅状态变化（供 UI 实时更新）
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<PluginStateChange>;

    /// 强制回退到指定版本（UI 的"回退"按钮）
    pub fn rollback_to(&self, id: &str, version: &Version) -> Result<(), LoadError>;
}
```

**调用侧不持锁**：

```rust
// 只在这一瞬间拿读锁，拿到 Arc 就放锁
let tool = registry.current("tool-http")
    .and_then(|i| i.tool("http_get"))
    .ok_or(ToolError::NotAvailable)?;
let result = tool.call(ctx, args).await;   // 长耗时不持锁
```

注册表自身用**内部可变性**：`RwLock` 读写分离，写临界区只有一次 map 操作；外层 `Arc<PluginRegistry>` 共享即可。**不要用 `ArcSwap<PluginRegistry>` 整体换值**——`ArcSwap::load()` 只给出共享引用，`&mut self` 方法调不进去；整体换值也要求 Registry 可廉价复制，与多版本 HashMap 结构不匹配。

#### activate 与 Fiber 生命周期的衔接

Registry 只管版本与 `active` 指针，Fiber/Runtime 只管依赖反应（§3.3 的替换流程），两者通过事件解耦，由编排层（`PluginService`）缝合：

```
PluginService::update / rollback
  → PluginRegistry::activate(id, ver)         # 1. 切 active 指针（微秒级）
  → PluginService 向 EventBus 发 plugin/swapped 事件
  → Runtime 订阅该事件：
      2. 由 service_providers 找出新版本提供（或撤掉）的服务键
      3. 沿 dependents 反向依赖链，让下游 Fiber 进入 Unloading（执行撤销链）
      4. 新实例 apply 完成后，按拓扑序重新激活下游 Fiber
```

这条链路是 §3.3"反应式余效应"的触发端：依赖变化不是轮询检测，而是 activate 之后的一条事件。

### 5.8 WASM 扩展路线

如果真想让"任何语言都能写插件"变成一等公民，把 `PluginLoader` 换成 wasmtime：

```rust
pub trait PluginLoader: Send + Sync {
    fn load(&self, path: &Path) -> Result<Arc<PluginInstance>>;
}

pub struct DylibLoader;   // dlopen + C ABI（第一版）
pub struct WasmLoader;    // wasmtime + WASI（未来）
```

WASM 的好处：内存安全沙箱、任何能编译到 `wasm32-wasi` 的语言都能写、天然可卸载、跨平台一份产物。代价是性能损失 1.5~3 倍和 WASI 能力限制。**内核只依赖 `PluginLoader` trait，第一版用 `DylibLoader`，把 WASM 作为第二实现预留。**
>
> **落地注记（v2.16）**：`WasmLoader` 已实现（plugin-host `wasm/` 模块），
> registry 按 `.wasm` 扩展名分发，manifest 与 C ABI 同 schema。与本文
> 草图的两处分野：宿主能力经 **`locowrk.log` 导入**注入（不是 vtable
> 指针——wasm 拿不到宿主指针），未注入的导入一律实例化失败；同实例
> 调用经 Mutex 串行（guest 是单线程线性内存），并发语义较 native 收窄。
> 沙箱与时间预算见 §14 v2.16 上注。

---

## 6. 回退的三层粒度

回退要分清是回退**代码**还是回退**状态**，机制完全不同。

### 6.1 第 1 层：插件版本回退

就是 `rollback()`。多版本并存 + 永不 dlclose，切换微秒级，旧版本一定可用。

```rust
registry.rollback("tool-http")?;   // 一行搞定
```

### 6.2 第 2 层：会话状态回退

事件只追加不删除，回退 = 移动游标 + replay。每 N 个事件存快照，避免 replay 全量历史。

```rust
session.rewind_to(seq);                    // 回到某个事件
session.rewind_before(&call_id)?;          // 回到某次工具调用前
let forked = session.fork(seq);            // 分叉：试试另一条路
```

### 6.3 第 3 层：副作用回退（补偿事务）

工具声明副作用等级，内核据此决定策略。

```rust
pub enum SideEffect {
    Pure,                              // 纯函数，回退免费
    Compensable { undo_tool: String }, // 有补偿工具
    Irreversible,                      // 不可逆，调用前审批
}

pub async fn rollback_effects(&self, from_seq: usize) -> Result<()> {
    let events = self.log.events_after(from_seq);
    // 逆序补偿
    for e in events.iter().rev() {
        if let SessionEvent::ToolResult { call_id, error, .. } = e {
            // 失败的调用不补偿：副作用未必发生，盲目补偿反而制造新副作用
            if error.is_some() { continue; }
            let req = self.log.find_request(call_id)?;
            match req.side_effect {
                SideEffect::Pure => {}
                SideEffect::Compensable { undo_tool } => {
                    // 幂等键用 call_id，防止重复补偿
                    self.invoke_tool(&undo_tool,
                        json!({ "call_id": call_id })).await?;
                }
                SideEffect::Irreversible => {
                    self.log.append(SessionEvent::ManualIntervention {
                        call_id: call_id.clone(),
                        reason: "irreversible side effect".into(),
                    });
                }
            }
        }
    }
    Ok(())
}
```

**补偿工具必须幂等**（用 `call_id` 做去重键），因为补偿本身也可能失败重试。

### 6.4 第 2 层与第 3 层是互补的

- **Event Sourcing** 管的是"会话状态可回放"
- **可逆效应** 管的是"运行时资源可精确撤销"

两者叠加，Agent 才能既回到某个对话节点，又让插件卸载不留残余。

---

## 7. 中间件链与 Agent 主循环

### 7.1 中间件链

超时、重试、熔断、限流、审批、审计通过类 tower 的洋葱模型接入。

```rust
#[async_trait::async_trait]
pub trait Middleware: Send + Sync {
    async fn handle(&self, ctx: &mut CallCtx, next: Next<'_>)
        -> Result<Value, ToolError>;
}

pub struct Next<'a> {
    chain: &'a [Arc<dyn Middleware>],
    idx: usize,
    tool: Arc<dyn Tool>,
}

/// 组装：Tracing → Timeout → Retry → CircuitBreaker → Approval → Tool
pub fn build_chain(mw: Vec<Arc<dyn Middleware>>, tool: Arc<dyn Tool>) -> Service;
```

审批中间件拦 `Irreversible` 工具，把人机确认做成一个异步点，正好和 Event Sourcing 对上：审批结果本身也是一条事件。

> **审批门位置（v2.3）**：审批决策必须作为 `ApprovalDecided` 事件写入会话
> 日志，而洋葱中间件没有会话句柄——所以审批门落在 Kernel 的 step 循环里
> （§7.2 的 `await_approval`，经 `ApprovalResolver` trait 解析，无决议者时
> fail-safe 拒绝）。§7.1 链上的 Approval 位保留给未来无会话上下文的独立
> 调用路径；本阶段链上实装的是 Tracing → Timeout → Retry → CircuitBreaker。

> **审计落点（v2.9，阶段八③）**：plugins.md 的 `mw-audit` 以**宿主侧中间件**
> 落地（`kernel-agent::middleware::audit`：`AuditEntry` / `AuditSink` /
> `JsonlAuditSink` / `Audit`）——C ABI 的 `PluginKind::Middleware` 仍是占位
> 类别（§5.2），中间件插件化（经 FFI 挂链）推迟到 ABI 扩展时再做。链序
> Tracing → **Audit** → Timeout → CircuitBreaker：审计在超时/熔断外侧，
> 被拒的调用也留痕；审批拒绝的调用不进链，以 `ApprovalDecided` 事件留在
> 会话日志。记录为 JSONL 追加写（desktop 数据根 `audit.jsonl`，一行一次
> 调用，含 args / side_effect / ok / error / 耗时），文件只增不改，与会话
> 日志同一纪律；单条写失败只告警不上抛。

> **熔断粒度（v2.21 搭车修复，2026-09-30 冒烟测试反馈）**：CircuitBreaker
> 从全局单计数器改为**按工具名隔离**——每个工具独立 Closed/Open/HalfOpen
> 状态机，故障工具不再连坐健康工具（曾出现从未失败的工具一起被
> `circuit open (cooling down)` 拒绝）。阈值/冷却仍是组装参数（desktop
> 5 次 / 30s），成功即删键回落 Closed，半开探测失败直接重回打开。manifest
> 声明的非法工具名在 plugin-host 目录合并处剔除（plugin-api
> `is_valid_tool_name`，OpenAI 兼容接口硬约束），不再打爆整个请求。

### 7.2 Agent 主循环

```rust
/// 工具目录服务：kernel-agent 只依赖这个抽象。
/// plugin-host 负责把 PluginRegistry 适配成 ToolSource 注册进 Context——
/// 主循环对 dlopen / Registry 一无所知，"工具目录"也只是个服务
#[async_trait::async_trait]
pub trait ToolSource: Send + Sync + 'static {
    async fn resolve(&self, name: &str) -> Result<Arc<dyn Tool>, ToolError>;
}

pub struct Kernel {
    ctx: Arc<Context>,                     // 所有服务（含 ToolSource、LlmService）从这里获取
    mw:  Arc<Vec<Arc<dyn Middleware>>>,    // 会话由调用方持有（SessionManager），Kernel 无会话状态
}

impl Kernel {
    pub async fn step(&self, session: &mut Session) -> Result<StepOutcome> {
        let state = session.state_at(session.cursor);

        // waterfall: pre-step，插件可拒绝/改写用户消息
        let decision = self.ctx.events
            .waterfall("agent/pre-step", state.to_value()).await;
        let messages = apply_decision(state.messages(), decision)?;

        // waterfall: request，插件可改 provider/model/参数
        let req = build_llm_request(&messages);
        let req = self.ctx.events
            .waterfall("agent/request", req.into_value()).await;
        let req = parse_request(req)?;

        // 模型适配器不特殊，也只是插件：
        // 以 trait 对象服务获取（插件用 provide_dyn::<dyn LlmService> 注册）
        let llm = self.ctx.use_dyn::<dyn LlmService>()?;
        let resp = match llm.complete(req).await {
            Ok(r) => r,
            Err(e) => {
                // waterfall: request-error，插件可决定 retry
                match self.ctx.events
                    .waterfall("agent/request-error", e.into_value()).await
                {
                    WaterfallDecision::Retry { delay } => {
                        tokio::time::sleep(delay).await;
                        return Ok(StepOutcome::Retry);
                    }
                    _ => return Err(e.into()),
                }
            }
        };

        session.append(SessionEvent::AssistantMessage {
            content: resp.content.clone(),
            tool_calls: resp.tool_calls.iter().map(|c| c.id.clone()).collect(),
        });

        let tools = self.ctx.use_dyn::<dyn ToolSource>()?;

        for call in resp.tool_calls {
            let tool = tools.resolve(&call.tool).await?;

            // 先写日志再执行：审批与执行都发生在 ToolCall 事件之后。
            // 被拒绝的调用立即以 denied 的 ToolResult 闭合——
            // replay 只会把"执行中崩溃"视为悬空，被拒调用不会留下悬空
            session.append(SessionEvent::ToolCall {
                call_id: call.id.clone(),
                tool: call.tool.clone(),
                args: call.args.clone(),
                plugin_version: tool.plugin_version().clone(),
                side_effect: tool.spec().side_effect.clone(),
            });

            // 不可逆工具先审批；决策本身作为 ApprovalDecided 事件写入日志
            if matches!(tool.spec().side_effect, SideEffect::Irreversible) {
                if !self.await_approval(&call, session).await? {
                    session.append(SessionEvent::ToolResult {
                        call_id: call.id.clone(),
                        output: Value::Null,
                        error: Some("denied by user".into()),
                    });
                    continue;
                }
            }

            // 经中间件链执行
            let out = self.chain(&tool)
                .call(&mut CallCtx::new(), call.args.clone()).await;

            session.append(SessionEvent::ToolResult {
                call_id: call.id.clone(),
                output: out.clone().unwrap_or(Value::Null),
                error: out.err().map(|e| e.to_string()),
            });
        }

        Ok(StepOutcome::Continue)
    }
}
```

**顺序很关键**：先写 `ToolCall` 再执行。进程崩在调用中途时，replay 能知道"有一个调用悬空了"，可以做超时回收或补偿；被拒绝的调用则当场以 `denied` 的 `ToolResult` 闭合，不留悬空。

**重试预算**：`agent/request-error` 返回的 `Retry` 决策受内核重试预算约束（同一 step 内累计次数与累计延迟上限，超限转为失败），防止决策点形成无限循环。

> **实现落点（v2.3）**：预算约束要求"同一 step 内累计"，若把 `Retry` 返回给
> 调用方再重入 step，每次都是新 step、预算无法累计——所以重试在 step 内部
> 闭环：`Retry { delay }` → sleep → 重新走 request 决策环，局部计数器累计
> 次数与延迟，超限即返回 `RetryBudgetExhausted` 错误；`StepOutcome` 不再有
> `Retry` 变体（对外只剩 `Continue`）。

> **实现落点（v2.8，阶段八②真实模型）**：`LlmRequest` 增加 `tools` 字段
> （本 step 的工具目录快照，`ToolSpec` 复用 plugin-api 定义），随
> `agent/request` 决策环进 payload（`serde(default)` 保持与旧决策方互通）——
> 真实模型要看到工具定义才会发起调用（LLM.md）。相应地 `ToolSource` 增加
> `list()`：目录服务缺失时视为空表，纯聊天不要求装配 ToolSource，真调到
> 不存在的工具仍走 `resolve` 的 `NotAvailable` 失败路径。桌面端经 genai
> 三合一适配器（LLM.md 方案一）实现 openai / anthropic / ollama，配置来自
> 设置面板（§13.1），每次请求前重读、保存即生效；`ServiceTargetResolver`
> 按设置的 provider 强制路由（不做模型名前缀魔法），base_url 覆盖端点，
> api_key 只在后端读取。model 未配置时回落演示替身（开箱即用）。

---

# 第二部分：工程组织

## 8. Crate 工作区结构

```
agent-kernel/
├── Cargo.toml                    # workspace 根
├── crates/
│   ├── plugin-api/               # 冻结 ABI（唯一，改动即破坏兼容）
│   │   ├── src/lib.rs
│   │   └── Cargo.toml
│   ├── cordis-core/              # Context / Effect / Fiber / Scope / EventBus
│   │   ├── src/
│   │   │   ├── context.rs
│   │   │   ├── effect.rs
│   │   │   ├── fiber.rs
│   │   │   ├── scope.rs
│   │   │   ├── event_bus.rs
│   │   │   └── runtime.rs        # 依赖图 + 拓扑调度
│   │   └── Cargo.toml
│   ├── kernel-agent/             # Agent 循环 / Waterfall / 中间件
│   │   ├── src/
│   │   │   ├── kernel.rs
│   │   │   ├── middleware.rs
│   │   │   ├── waterfall.rs
│   │   │   └── chain.rs
│   │   └── Cargo.toml
│   ├── plugin-host/              # Loader / Registry / SabiToolAdapter
│   │   ├── src/
│   │   │   ├── loader.rs         # dlopen 封装
│   │   │   ├── registry.rs
│   │   │   └── adapter.rs        # FFI → Tool 适配
│   │   └── Cargo.toml
│   ├── plugin-service/           # 新增：安装/卸载/更新编排（CLI 与桌面端共用）
│   │   ├── src/
│   │   │   ├── service.rs
│   │   │   ├── lockfile.rs
│   │   │   ├── market.rs         # 市场客户端
│   │   │   └── installer.rs      # 下载 + 校验 + 解压
│   │   └── Cargo.toml
│   ├── session-store/            # Session 日志 / 快照 / 回放 / 补偿
│   │   ├── src/
│   │   │   ├── event.rs
│   │   │   ├── session.rs
│   │   │   ├── snapshot.rs
│   │   │   └── compensation.rs
│   │   └── Cargo.toml
│   └── kernel-bin/               # 纯 CLI 可执行入口
│       ├── src/main.rs
│       └── Cargo.toml
├── desktop/                      # 桌面端（Tauri 2）
│   ├── src-tauri/                # Rust 后端
│   │   ├── src/
│   │   │   ├── commands/
│   │   │   │   ├── agent.rs
│   │   │   │   ├── plugin.rs
│   │   │   │   ├── session.rs
│   │   │   │   └── settings.rs
│   │   │   ├── events.rs         # 内核事件 → Tauri event 桥接
│   │   │   ├── state.rs          # 共享状态（Kernel、PluginService）
│   │   │   ├── lib.rs
│   │   │   └── main.rs
│   │   ├── Cargo.toml
│   │   └── tauri.conf.json
│   ├── src/                      # 前端
│   │   ├── views/
│   │   │   ├── ChatView.vue
│   │   │   ├── PluginMarket.vue
│   │   │   ├── PluginManager.vue
│   │   │   ├── ApprovalModal.vue
│   │   │   └── Settings.vue
│   │   ├── stores/
│   │   ├── lib/api.ts
│   │   └── App.vue
│   └── package.json
├── plugins/
│   ├── llm-openai/               # LlmService 插件
│   ├── tool-http/
│   │   ├── 1.2.0/libtool_http.so
│   │   └── 1.3.0/libtool_http.so
│   ├── tool-shell/
│   │   └── 0.9.0/libtool_shell.so
│   └── session-sqlite/           # 会话持久化插件
└── config/
    └── profile.toml              # 声明 active 版本 + 插件组合
```

### 依赖方向（严格单向；`X → Y` 表示 X 依赖 Y）

```
kernel-bin        → kernel-agent + plugin-service + session-store   （组装点）
desktop/src-tauri → kernel-agent + plugin-service + session-store   （组装点）
kernel-agent      → cordis-core + plugin-api + session-store
plugin-service    → plugin-host + cordis-core
cordis-core       → plugin-api
plugin-host       → plugin-api
session-store     → plugin-api
```

- `plugin-api`：不依赖任何其他 crate
- `cordis-core`：只依赖 `plugin-api`
- `kernel-agent`：依赖 `cordis-core` + `plugin-api` + `session-store`（§7.2 的 `Kernel::step` 直接消费会话日志 `Session`；session-store 不反向依赖 kernel-agent，无环）
- `plugin-host`：依赖 `plugin-api`
- `plugin-service`：依赖 `plugin-host` + `cordis-core`
- `session-store`：只依赖 `plugin-api`
- `kernel-bin` / `desktop/src-tauri`：仅有的两个组装点，依赖以上全部

**kernel-agent 与 plugin-host 互不依赖**：两者共用的 `Tool` / `ToolSpec` / `CallCtx` 定义在 plugin-api 的宿主侧（不进入 C ABI，见 §5.2）；内核通过 `ToolSource` 服务 trait（kernel-agent 定义、plugin-host 实现并注册进 Context，见 §7.2）解析工具，因此主循环不需要知道 dlopen 和 Registry 的存在。

**关键设计**：`plugin-service` 是独立 crate，CLI 和桌面端共用同一套插件管理逻辑。UI 换框架、换技术栈都不影响它。

### 关键依赖

```toml
[workspace.dependencies]
tokio        = { version = "1", features = ["full"] }
async-trait  = "0.1"
serde        = { version = "1", features = ["derive"] }
serde_json   = "1"
dashmap      = "5"
libloading   = "0.8"
semver       = "1"
thiserror    = "1"
tracing      = "0.1"

# 桌面端
tauri        = { version = "2", features = ["protocol-asset"] }
tauri-plugin-shell = "2"
tauri-plugin-dialog = "2"
reqwest      = { version = "0.12", features = ["json", "stream"] }
sha2         = "0.10"
tar          = "0.4"
flate2       = "1"
```

---

## 9. 关键实现细节

### 9.1 Effect 撤销链与 tokio 生命周期对齐

**难点**：`Disposer` 是异步的，撤销链的执行需要在 tokio runtime 内。但 `Context::provide` 是同步方法。

**方案 A（推荐）**：`Context::provide` 保持同步，只把 `EffectEntry` push 到队列；异步撤销由 `Context::revoke` / `revoke_by_fiber` 异步执行。

**方案 B**：`Context` 内部持有一个 `tokio::runtime::Handle`，在 `Drop` 时 `handle.spawn` 异步撤销。风险是 runtime 可能已经关闭。

**结论**：采用方案 A。调用方显式 `await ctx.revoke(id)` 或 `await runtime.deactivate(fiber)`，生命周期清晰可控。配套决定：Effect 账本用 `std::sync::Mutex`——临界区只做内存操作（push / remove），从不 await；异步只出现在 dispose 执行段，且执行时必须已放锁（见 §3.1 / §3.2 的实现）。

### 9.2 交错 Effect 的独立性

已在 [3.2](#32-effect每次修改携带逆操作) 详述。核心是 `FiberId` 标签 + `partition` + 逆序执行。

### 9.3 Scope 零开销实现

```rust
// 单 Agent 场景
if self.scoped.is_empty() {
    return self.global.use_service::<S>();   // 直接走全局，无额外开销
}
```

`DashMap::is_empty()` 是 O(1) 的分片计数查询。

### 9.4 FFI panic 保护

```rust
pub extern "C" fn agent_plugin_call(...) -> i32 {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // 实际调用逻辑
    }));
    match result {
        Ok(code) => code,
        Err(_) => {
            // 记录 panic 信息到 host vtable 的 log
            ERROR_CODE
        }
    }
}
```

宿主侧必须 catch，插件侧也必须 catch，双保险。

### 9.5 快照的 schema 演进

```rust
pub struct Snapshot {
    pub schema_version: u32,
    pub seq: usize,
    pub state: serde_json::Value,   // 用 Value 而非强类型，便于演进
}

pub trait Upcaster {
    fn from_version(&self) -> u32;
    fn upcast(&self, v: Value) -> Value;
}

// 加载时逐级 upcast 到当前版本
fn load_snapshot(raw: Value, current: u32) -> Result<Snapshot> {
    let mut v = raw;
    let mut ver = v["schema_version"].as_u64().unwrap_or(0) as u32;
    while ver < current {
        v = UPCASTERS[&ver].upcast(v);
        ver += 1;
    }
    serde_json::from_value(v)
}
```

### 9.6 在途调用等待

```rust
pub async fn drain_instance(inst: Arc<PluginInstance>, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Arc::strong_count(&inst) > 1 {
        if Instant::now() > deadline {
            return false;   // 超时，放弃等待
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    true
}
```

因为永不 dlclose，超时放弃也安全。

---

# 第三部分：桌面端

## 10. 桌面端技术栈选型

### 10.1 为什么是 Tauri 2

你的 Agent 内核是 Rust 写的，桌面端需要同时满足三个条件：**能直接复用 Rust crate、能管理插件生命周期、UI 开发效率足够高**。Tauri 2 是唯一同时满足这三条的方案。

| 维度 | Tauri 2 | egui | iced |
|---|---|---|---|
| Rust 内核集成 | 直接依赖 crate | 直接依赖 crate | 直接依赖 crate |
| UI 开发效率 | HTML/CSS/JS 生态 | 立即模式，适合工具面板 | Elm 架构，脚手架多 |
| 插件管理界面 | Web 组件丰富 | 需自绘 | 需自绘 |
| 包体积 | 最小（系统 WebView） | 小 | 中 |
| 跨平台移动端 | 官方支持 | 无 | 实验性 |

egui 适合"内部调试面板"，iced 适合"纯 Rust 长期演进"，但 Agent 桌面端需要**插件市场、会话列表、配置表单、审批弹窗**这些典型业务界面，Web 前端生态的生产力优势不可替代。

### 10.2 推荐组合

```
┌─────────────────────────────────────────────┐
│  Frontend: Vue 3 + Vite + Pinia + Tailwind  │  ← Web 技术栈，快速迭代
│             （或 React / Svelte）             │
├─────────────────────────────────────────────┤
│  Tauri 2 IPC Layer                          │
│    #[tauri::command]  ← 内核命令暴露         │
│    emit / listen       → 流式事件推送        │
├─────────────────────────────────────────────┤
│  Rust Backend = 你的 Agent 内核              │
│    cordis-core / kernel-agent / plugin-host  │
│    plugin-service / session-store            │
└─────────────────────────────────────────────┘
```

前端框架的选型不影响内核设计，选团队最熟的即可。Tauri 2 + Vue 3 + Pinia + TailwindCSS 是被验证过的组合，命令封装在 `src/lib/api.ts`，store 管理状态，后端命令在各模块的 `commands.rs` 中实现、统一在 `lib.rs` 注册。

### 10.3 分层架构

```
┌──────────────────────────────────────┐
│  UI Shell（Tauri WebView）            │
│    聊天视图 / 插件市场 / 设置 / 审批   │
└──────────────┬───────────────────────┘
               │ invoke(cmd) / listen(event)
┌──────────────▼───────────────────────┐
│  Bridge Layer（Tauri Commands）       │
│    薄适配层：参数转换 + 权限校验       │
│    不写业务逻辑                        │
└──────────────┬───────────────────────┘
               │ 直接调用 Rust API
┌──────────────▼───────────────────────┐
│  Kernel + PluginService               │
│    Context / Session / PluginRegistry │
└──────────────────────────────────────┘
```

**关键纪律：Bridge Layer 是薄适配层。** 所有 `#[tauri::command]` 函数只做三件事：解析前端参数、调用内核 API、把结果转成 `Result<Value, String>`。业务逻辑一行不写。这样内核可以独立测试，UI 换框架也不动内核。

---

## 11. PluginService 与插件管理

### 11.1 插件管理命令集

这是桌面端最核心的功能组。

```rust
// desktop/src-tauri/src/commands/plugin.rs

/// 列出已安装插件（从本地 lockfile 读取）
#[tauri::command]
async fn plugin_list(
    plugins: tauri::State<'_, Arc<PluginService>>,
) -> Result<Vec<InstalledPlugin>, String> {
    plugins.list_installed()
}

/// 从注册源拉取市场目录
#[tauri::command]
async fn plugin_registry_list(
    plugins: tauri::State<'_, Arc<PluginService>>,
) -> Result<Vec<MarketplaceItem>, String> {
    plugins.fetch_registry().await
}

/// 从市场安装插件（下载 + 校验 + 注册）
#[tauri::command]
async fn plugin_install(
    id: String,
    plugins: tauri::State<'_, Arc<PluginService>>,
) -> Result<String, String> {
    plugins.install(&id).await
}

/// 启用 / 禁用已安装插件
#[tauri::command]
async fn plugin_set_enabled(
    id: String,
    enabled: bool,
    plugins: tauri::State<'_, Arc<PluginService>>,
) -> Result<(), String> {
    plugins.set_enabled(&id, enabled).await
}

/// 卸载插件（删目录 + 更新 lockfile + 触发内核卸载）
#[tauri::command]
async fn plugin_uninstall(
    id: String,
    plugins: tauri::State<'_, Arc<PluginService>>,
) -> Result<bool, String> {
    plugins.uninstall(&id).await
}

/// 检查更新
#[tauri::command]
async fn plugin_check_updates(
    plugins: tauri::State<'_, Arc<PluginService>>,
) -> Result<Vec<UpdateInfo>, String> {
    plugins.check_updates().await
}

/// 更新插件（带自动回退）
#[tauri::command]
async fn plugin_update(
    id: String,
    plugins: tauri::State<'_, Arc<PluginService>>,
) -> Result<(), String> {
    plugins.update(&id).await
}

/// 回退到指定版本
#[tauri::command]
async fn plugin_rollback(
    id: String,
    version: String,
    plugins: tauri::State<'_, Arc<PluginService>>,
) -> Result<(), String> {
    plugins.rollback(&id, &version).await
}
```

### 11.2 PluginService 与内核的对接

`PluginService` 是桌面端的插件管理器，它**包装**内核里的 `PluginRegistry`，不替代它。

```rust
pub struct PluginService {
    /// 内核注册表（多版本管理 + 热插拔；内部可变性，方法都是 &self）
    registry: Arc<PluginRegistry>,
    /// 插件安装根目录（安装态，见 §11.3）
    install_root: PathBuf,
    /// lockfile：记录已安装插件及其版本（std Mutex，临界区只有内存操作）
    lockfile: Arc<Mutex<LockFile>>,
    /// 市场客户端
    market: MarketClient,
    /// 内核上下文（effect 撤销 + plugin/swapped 事件）
    ctx: Arc<Context>,
    /// 状态广播（供 UI 实时更新）
    state_tx: broadcast::Sender<PluginStateChange>,
}

impl PluginService {
    pub async fn install(&self, id: &str) -> Result<String, String> {
        // 1. 从市场下载
        let pkg = self.market.download(id).await?;

        // 2. 校验签名 / hash
        self.verify(&pkg)?;

        // 3. 解压到 install_root/{id}/{version}/
        let install_path = self.install_root.join(id).join(&pkg.version);
        extract(&pkg, &install_path)?;

        // 4. 加载到内核注册表
        let _inst = self.registry.load(id, &pkg.version, &install_path)?;

        // 5. 通过 effect 注册，返回 EffectId 以便回退
        let effect_id = self.ctx.effect(|ctx| {
            let undo = Box::new(UninstallEffect {
                id: id.to_string(),
                version: pkg.version.clone(),
                install_path: install_path.clone(),
                registry: self.registry.clone(),
            });
            ((), undo)
        });

        // 6. 更新 lockfile
        self.lockfile.lock().expect("lockfile poisoned").add(id, &pkg.version, effect_id);

        // 7. 广播状态变化
        self.state_tx.send(PluginStateChange::Installed {
            id: id.into(),
            version: pkg.version,
        }).ok();

        Ok(id.to_string())
    }

    pub async fn set_enabled(&self, id: &str, enabled: bool) -> Result<(), String> {
        if enabled {
            let ver = self.lockfile.lock().expect("lockfile poisoned").version_of(id)?;
            self.registry.activate(id, &ver)?;
        } else {
            self.registry.deactivate(id);
        }
        self.state_tx.send(PluginStateChange::EnabledChanged {
            id: id.into(),
            enabled,
        }).ok();
        Ok(())
    }

    pub async fn uninstall(&self, id: &str) -> Result<bool, String> {
        // 1. 从注册表逻辑卸载（Arc 交给 graveyard，物理保留）
        self.registry.deactivate(id);

        // 2. 执行该插件的所有 Effect 撤销（只对当前进程有效）
        let effect_ids = self.lockfile.lock().expect("lockfile poisoned").effects_of(id);
        for eid in effect_ids {
            self.ctx.revoke(eid).await?;
        }

        // 3. 物理删除两段式：Windows 上已加载的 DLL 被文件锁锁住，
        //    进程存活期间 remove_dir_all 可能失败——先写 tombstone，
        //    本次能删就删，删不掉留给下次启动在加载任何插件之前清理
        let dir = self.install_root.join(id);
        if dir.exists() && std::fs::remove_dir_all(&dir).is_err() {
            self.lockfile.lock().expect("lockfile poisoned").push_pending_delete(&dir);
            tracing::warn!(plugin = %id, "delete deferred to next startup");
        }

        // 4. 更新 lockfile + 广播
        self.lockfile.lock().expect("lockfile poisoned").remove(id);

        self.state_tx.send(PluginStateChange::Uninstalled { id: id.into() }).ok();
        Ok(true)
    }

    pub async fn rollback(&self, id: &str, version: &str) -> Result<(), String> {
        let ver = Version::parse(version).map_err(|e| e.to_string())?;
        // 检查该版本是否已加载
        if !self.registry.has_version(id, &ver) {
            // 未加载：从磁盘重新加载
            let path = self.install_root.join(id).join(version);
            if !path.exists() {
                return Err(format!("version {} not installed", version));
            }
            self.registry.load(id, &ver, &path)?;
        }
        self.registry.activate(id, &ver)?;
        // 通知 Runtime：下游 Fiber 撤销 + 拓扑重激活（见 §5.7）
        self.ctx.events.emit("plugin/swapped",
            serde_json::json!({ "id": id, "to": version })).await;
        self.lockfile.lock().expect("lockfile poisoned").update(id, &ver);
        Ok(())
    }
}
```

**关键设计**：安装时通过 `ctx.effect` 注册，`EffectId` 写入 lockfile，用于运行期撤销与审计。**崩溃恢复不依赖 EffectId**——Disposer 是内存闭包，进程重启后不可还原；恢复采用**对账式**：启动时比较 lockfile 与磁盘/注册表的实际状态（lockfile 有而磁盘无 → 清条目；磁盘有而 lockfile 无或已标记删除 → 清目录），配合 `pending_delete` tombstone 完成收尾。

> **实现备注（阶段七）**：本节代码即落地形态，落点 `crates/plugin-service/src/service.rs`。
> 与文档的四点偏差：① `install` 增加可选 `version` 参数（市场卡片可选版本安装，
> 缺省 latest）；`plugin_uninstall` 返回 `()` 而非 `bool`（前端不消费该值）。
> ② 不设独立 `plugin_check_updates` 命令——前端按 §13.3 用目录 latest 对比本地
> 版本即可。③ §5.7 的 `registry.subscribe()` 折叠为 PluginService 的单一
> `broadcast` 广播源（注册表变更在本架构下都经编排层发生；启动期开发态装载
> 发生在 UI 订阅之前，由 `plugin_list` 拉取兜底），桥接到
> `plugin://state-changed` 的转发器住在组装点 `events.rs`。④ `ctx.effect(...)`
> 是阶段七为 §3.2 补的公开原语（此前只有 `provide*` 这条特例路径可入账本）：
> 裸 Effect 归 `FiberId::HOST` 名下、不关联服务键，`LogicalUnload`（逆操作 =
> 逻辑卸载进 graveyard）由 plugin-service 实现并经它登记。市场客户端当前是
> `LocalDirMarket`（仓库 `market/` 目录，`scripts/build_market.sh` 暂存
> tool-shout 双版本），`Market` trait 就绪、HTTP 实现随真实市场服务补。

### 11.3 插件目录布局

```
~/.Locowrk/
├── plugins/                    # 已安装插件
│   ├── tool-http/
│   │   ├── 1.2.0/
│   │   │   ├── libtool_http.so
│   │   │   └── manifest.json
│   │   └── 1.3.0/
│   │       └── ...
│   └── tool-shell/
│       └── 0.9.0/
│           └── libtool_shell.so
├── lockfile.json               # 记录已安装插件、版本、EffectId
├── market-cache/               # 市场目录缓存
│   └── registry-snapshot.json
└── config/
    └── profile.toml            # 当前激活的插件组合
```

多版本并存是回退的基础，`install_root/{id}/{version}/` 的结构让版本回退只需切换 `active` 指针，不需要重新下载。

**开发态与安装态是两套目录**：开发态是仓库内的 `plugins/{id}/{version}/` 与 `config/profile.toml`（§8），内核开发与测试直接按 profile 声明加载，不经市场与 lockfile；安装态是上面的 `~/.Locowrk/`，存放用户从市场安装的插件。`PluginService.install_root` 指向安装态，两套目录互不重叠。

### 11.4 LockFile 结构

```rust
#[derive(Serialize, Deserialize, Default)]
pub struct LockFile {
    pub schema_version: u32,
    pub plugins: BTreeMap<String, InstalledEntry>,
    /// 物理删除 tombstone：本进程删不掉（Windows 文件锁）的目录，
    /// 下次启动在加载任何插件之前清理
    pub pending_delete: Vec<PathBuf>,
}

#[derive(Serialize, Deserialize)]
pub struct InstalledEntry {
    pub id: String,
    pub version: Version,
    pub effect_ids: Vec<EffectId>,      // 该插件注册的所有 Effect
    pub installed_at: i64,
    pub enabled: bool,
    pub source: InstallSource,          // Market | Local | Builtin
}
```

LockFile 是桌面端和内核之间的**契约文件**。桌面端读它渲染列表，内核启动时按它加载插件。崩溃恢复按 §11.2 的对账式流程执行：`effect_ids` 只做审计线索，`pending_delete` 与目录对账负责实际清理。

### 11.5 更新流程与回滚

更新是热插拔最复杂的场景，因为要保证失败可回退。

```rust
    pub async fn update(&self, id: &str) -> Result<(), String> {
        // 1. 快照当前状态
        let snapshot = self.snapshot(id).await?;

        // 2. 下载并直接解压到最终版本目录。
        //    不走 staging + rename：Windows 上已加载的 DLL 无法改名/移动，
        //    而新版本目录本来就不存在冲突，直接落位即可
        let new_pkg = self.market.download(id).await?;
        let final_path = self.install_root.join(id).join(&new_pkg.version);
        extract(&new_pkg, &final_path)?;

        // 3. 加载并激活新版本
        let new_ver = new_pkg.version.clone();
        match self.registry.load(id, &new_ver, &final_path)
            .and_then(|_| self.registry.activate(id, &new_ver))
        {
            Ok(()) => {
                self.lockfile.lock().expect("lockfile poisoned").update(id, &new_ver);
                // 通知 Runtime：下游 Fiber 撤销 + 拓扑重激活（见 §5.7）
                self.ctx.events.emit("plugin/swapped",
                    serde_json::json!({ "id": id, "to": new_ver })).await;
                Ok(())
            }
            Err(e) => {
                // 激活失败：旧版本从未离开 active 指针，"回滚"只是确认旧指针
                self.registry.activate(id, &snapshot.version)?;
                // 新版本目录清理同样受 Windows 文件锁约束：
                // 已 dlopen 过就删不掉，删不掉就 tombstone，下次启动清理
                if std::fs::remove_dir_all(&final_path).is_err() {
                    self.lockfile.lock().expect("lockfile poisoned")
                        .push_pending_delete(&final_path);
                }
                Err(format!("update failed, rolled back: {}", e))
            }
        }
    }
```

**旧版本永远保留**，直到用户主动清理。这样任何更新失败都是一次 `activate` 调用的事，微秒级回退。

### 11.6 热插拔的状态机（UI 可见）

桌面端把插件状态拆成两层展示：**安装态**来自 lockfile 投影（已安装、有更新），**运行态**来自内核 Fiber 状态机（§3.3）。`PluginState` 是两者的合并视图，与 `FiberState` 的映射关系：

- `FiberState::Pending / Loading → Loading`
- `FiberState::Active → Active`
- `FiberState::Unloading → Unloading`
- `FiberState::Unloaded → Installed`（已装未激活）
- `FiberState::Failed(e) → Error(e)`
- `UpdateAvailable` 不是运行态：由 lockfile 版本与 registry latest 比较得出

```rust
#[derive(Serialize)]
pub struct PluginUiState {
    pub id: String,
    pub name: String,
    pub version: String,
    pub latest_version: Option<String>,
    pub state: PluginState,
    pub error: Option<String>,
    pub side_effect_summary: Vec<SideEffectInfo>,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginState {
    Installed,       // 已安装但未激活
    Loading,
    Active,          // 正在运行
    Unloading,
    Error(String),   // 加载失败
    UpdateAvailable, // 有新版本
}
```

前端插件面板对应四类操作：**安装**（从市场）、**启用/禁用**（热插拔）、**更新**（版本升级）、**卸载**（删除）。参考 DeepSeek Harness Desktop 的市场设计，插件商店作为顶级入口，包含"发现"和"已安装"双页签。

---

## 12. Tauri IPC 层设计

### 12.1 命令暴露模式

```rust
// desktop/src-tauri/src/commands/agent.rs
#[tauri::command]
async fn agent_chat(
    session_id: String,
    message: String,
    kernel: tauri::State<'_, Arc<Kernel>>,
    sessions: tauri::State<'_, Arc<SessionManager>>,
) -> Result<AgentResponse, String> {
    let mut session = sessions.get_mut(&session_id)
        .ok_or("session not found")?;
    // 用户消息先进事件日志；step 的输入是日志投影，不是额外参数
    session.append(SessionEvent::UserMessage { content: message });
    let outcome = kernel.step(&mut session).await
        .map_err(|e| e.to_string())?;
    Ok(AgentResponse::from(outcome))
}

#[tauri::command]
async fn agent_approve(
    request_id: String,
    allowed: bool,
    resolver: tauri::State<'_, Arc<ApprovalResolver>>,
) -> Result<(), String> {
    if allowed {
        resolver.approve(&request_id).await.map_err(|e| e.to_string())
    } else {
        resolver.deny(&request_id, "user denied").await.map_err(|e| e.to_string())
    }
}

#[tauri::command]
async fn list_pending_approvals(
    resolver: tauri::State<'_, Arc<ApprovalResolver>>,
) -> Result<Vec<ApprovalRequest>, String> {
    Ok(resolver.pending().await)
}
```

审批命令必须独立暴露，因为 `Irreversible` 工具的审批是一个**阻塞点**，前端需要弹窗、用户确认、再回传结果。

### 12.2 流式事件推送

Agent 循环的流式输出（`AssistantChunk`）通过 Tauri 的 event 系统推送，不走 command 返回值：

```rust
// desktop/src-tauri/src/events.rs
use tauri::Emitter;   // Tauri 2 中 emit 定义在 Emitter trait，必须显式引入

pub fn bridge_kernel_events(
    app: AppHandle,
    kernel: Arc<Kernel>,
    plugins: Arc<PluginService>,
    approvals: Arc<ApprovalResolver>,
) {
    // 订阅内核的 chunk 流
    let mut rx = kernel.subscribe_chunks();
    let app1 = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Ok(chunk) = rx.recv().await {
            app1.emit("agent://chunk", ChunkPayload {
                session_id: chunk.session_id.clone(),
                delta: chunk.delta.clone(),
            }).ok();
        }
    });

    // 订阅插件状态变化
    let mut rx = plugins.subscribe();
    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Ok(change) = rx.recv().await {
            app2.emit("plugin://state-changed", change).ok();
        }
    });

    // 订阅审批请求
    let mut rx = approvals.subscribe();
    tauri::async_runtime::spawn(async move {
        while let Ok(req) = rx.recv().await {
            app.emit("agent://approval-request", req).ok();
        }
    });
}
```

前端侧：

```typescript
// desktop/src/lib/api.ts
import { listen } from '@tauri-apps/api/event';

export async function subscribeChunks(cb: (p: ChunkPayload) => void) {
  return listen<ChunkPayload>('agent://chunk', (e) => cb(e.payload));
}

export async function subscribePluginState(cb: (p: PluginStateChange) => void) {
  return listen<PluginStateChange>('plugin://state-changed', (e) => cb(e.payload));
}

export async function subscribeApprovals(cb: (p: ApprovalRequest) => void) {
  return listen<ApprovalRequest>('agent://approval-request', (e) => cb(e.payload));
}
```

**命令走请求-响应，事件走推送。** 不要把流式内容塞进 command 返回值——那会要求前端轮询，浪费 IO。

> **实现备注（阶段五）**：内核尚无 chunk 订阅总线（`kernel.subscribe_chunks()` 留待后续阶段）。
> 桌面端在轮次循环（`runner.rs`）里把会话日志新增段投影成事件发出：`agent://chunk`
> （非流式模型每步正文整块推送，真流式后只是 delta 变小、前端累积逻辑不变）、
> `agent://tool-call`、`agent://tool-result`、`agent://turn-done` 四通道。轮次失败以
> command reject 收尾、不发 turn-done，避免与成功路径的事件重复。事件出口抽象为
> `EventSink`，单元测试用收集器替换 Tauri 实现。
>
> **实现备注（阶段六）**：新增第五通道 `agent://approval-request`（`ApprovalHub`
> 实现 kernel 的 `ApprovalResolver`，推事件后以 oneshot 挂起等待 `agent_approve`）。
> 载荷统一 snake_case——本节与 §13.2 示例中的 camelCase 序列化以实现为准；
> `RiskLevel` 未引入（plugin-api 无此类型），风险度由 `side_effect` 呈现。
> 审批等待不设超时（工具链上的 Timeout 只包执行、不包审批）。

### 12.3 共享状态初始化

```rust
// desktop/src-tauri/src/state.rs
pub struct AppState {
    pub kernel: Arc<Kernel>,
    pub plugin_service: Arc<PluginService>,
    pub approval_resolver: Arc<ApprovalResolver>,
    pub sessions: Arc<SessionManager>,
}

pub fn init_state(app: &AppHandle) -> AppState {
    let ctx = Arc::new(Context::new());
    let registry = Arc::new(PluginRegistry::new());
    let lockfile = Arc::new(Mutex::new(LockFile::load_default().unwrap()));
    let market = MarketClient::new("https://plugins.example.com");

    let plugin_service = Arc::new(PluginService::new(
        registry.clone(), ctx.clone(), lockfile.clone(), market,
    ));

    // 启动顺序固定：先清 tombstone（此时尚未加载任何插件，文件锁不成立），
    // 再按 lockfile 恢复已启用的插件
    let lf = lockfile.lock().expect("lockfile poisoned");
    for dir in &lf.pending_delete {
        if std::fs::remove_dir_all(dir).is_err() {
            tracing::warn!(dir = ?dir, "pending delete failed again");
        }
    }
    for entry in lf.plugins.values() {
        if entry.enabled {
            let path = install_root().join(&entry.id).join(entry.version.to_string());
            if let Err(e) = registry.load(&entry.id, &entry.version, &path) {
                tracing::warn!(plugin = %entry.id, "restore failed: {}", e);
            }
        }
    }
    drop(lf);

    let kernel = Arc::new(Kernel::new(ctx));
    let approval_resolver = Arc::new(ApprovalResolver::new());
    let sessions = Arc::new(SessionManager::new());
    bridge_kernel_events(app.clone(), kernel.clone(),
        plugin_service.clone(), approval_resolver.clone());

    AppState { kernel, plugin_service, approval_resolver, sessions }
}
```

```rust
// desktop/src-tauri/src/lib.rs
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let state = init_state(&app.handle());
            app.manage(state.kernel);
            app.manage(state.plugin_service);
            app.manage(state.approval_resolver);
            app.manage(state.sessions);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // agent
            commands::agent::agent_chat,
            commands::agent::agent_approve,
            commands::agent::list_pending_approvals,
            // plugin
            commands::plugin::plugin_list,
            commands::plugin::plugin_registry_list,
            commands::plugin::plugin_install,
            commands::plugin::plugin_set_enabled,
            commands::plugin::plugin_uninstall,
            commands::plugin::plugin_check_updates,
            commands::plugin::plugin_update,
            commands::plugin::plugin_rollback,
            // session
            commands::session::session_list,
            commands::session::session_load,
            commands::session::session_fork,
            // settings
            commands::settings::settings_get,
            commands::settings::settings_set,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

> **实现备注（阶段五）**：本节代码是终态蓝图。阶段五只装配子集：PluginService /
> LockFile / 市场客户端尚未存在（阶段七），当前为 Context + PluginRegistry + Kernel +
> 会话表（`HashMap<String, Arc<tokio::sync::Mutex<Session>>>`，命令取会话锁、同会话
> 串行聊天，std Mutex 只护 map 存取不跨 await）。`plugins/` 目录由
> `LOCOWRK_PLUGINS_DIR` 或从 CWD / exe 逐级向上解析。
>
> **实现备注（阶段六）**：`agent_approve` / `list_pending_approvals` 已落地，
> `AppState` 增加 `approval: Arc<ApprovalHub>`（Kernel 构建时 `with_approval` 挂上）
> 与共享 `sink: Arc<dyn EventSink>`。工具目录加了组装点内置工具位（`with_builtin`，
> 内置优先于注册表，防插件顶掉演示替身）；演示工具 `send_email`
> （`SideEffect::Irreversible`）用于验收审批弹窗，真实实现后续由插件替换。

---

## 13. UI 功能模块设计

### 13.1 五大模块

| 模块 | 职责 | 核心命令 |
|---|---|---|
| **会话视图** | 聊天、流式输出、工具调用展示 | `agent_chat`, `listen("agent://chunk")` |
| **审批中心** | 不可逆工具的确认弹窗 | `agent_approve`, `list_pending_approvals` |
| **插件市场** | 浏览、搜索、安装、更新 | `plugin_registry_list`, `plugin_install` |
| **插件管理** | 已安装列表、启用/禁用、卸载、回退 | `plugin_list`, `plugin_set_enabled`, `plugin_uninstall`, `plugin_rollback` |
| **设置** | 模型配置、会话存储、快捷键 | `settings_get`, `settings_set` |

> **实现备注（阶段八·最小设置面板）**：三块落地——LLM 配置（provider
> openai/anthropic/ollama、model、base_url、api_key）、会话存储位置
> （只读展示数据根；桌面端会话当前在内存，重启清空，落盘持久化后续接）、
> 语言切换（§13.5）。存储 = 数据根 `settings.json`（`desktop/src-tauri/
> src/settings.rs`）；**API Key 只落盘不出 IPC**——命令返回投影仅含
> `api_key_set` 布尔；`base_url`/`api_key` 传空串 = 清除，字段缺省 =
> 保持不变。快捷键已接入（阶段八⑤）：settings.json `shortcut`（缺省
> Ctrl+Shift+L），设置面板「全局快捷键」块可改、保存即时重注册（§14
> v2.13 备注）。

### 13.2 审批弹窗的交互设计

这是 Agent 桌面端**最独特**的 UI 需求。当内核遇到 `Irreversible` 工具调用时，会通过 event 推送一个审批请求，前端弹窗让用户决策。

```rust
/// 审批请求载荷。推送给前端，字段序列化为 camelCase（与前端 req.requestId 对齐）
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalRequest {
    pub request_id: String,
    pub tool: String,
    pub args: serde_json::Value,
    pub side_effect: String,
    pub risk_level: RiskLevel,
}

// 内核侧：审批中间件 emit 请求
approval_resolver.request(call_id, ApprovalRequest {
    request_id: call_id.to_string(),
    tool: "send_email".into(),
    args: json!({ "to": "user@example.com", "subject": "..." }),
    side_effect: "irreversible".into(),
    risk_level: RiskLevel::High,
}).await?;
```

```vue
<!-- desktop/src/views/ApprovalModal.vue -->
<script setup lang="ts">
import { onMounted, ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { subscribeApprovals } from '../lib/api';

const pending = ref<ApprovalRequest[]>([]);

onMounted(async () => {
  await subscribeApprovals((req) => pending.value.push(req));
});

async function decide(req: ApprovalRequest, allowed: boolean) {
  await invoke('agent_approve', { requestId: req.requestId, allowed });
  pending.value = pending.value.filter(r => r !== req);
}
</script>

<template>
  <div v-for="req in pending" :key="req.requestId" class="modal">
    <h3>需要确认</h3>
    <p>工具：<code>{{ req.tool }}</code></p>
    <pre>{{ JSON.stringify(req.args, null, 2) }}</pre>
    <div class="risk-badge">{{ req.riskLevel }}</div>
    <div class="actions">
      <button @click="decide(req, false)">拒绝</button>
      <button @click="decide(req, true)" class="primary">允许</button>
    </div>
  </div>
</template>
```

审批结果本身也是一条 `SessionEvent`，写入事件日志，保证可审计。

> **实现备注（阶段六）**：审批门如 §7.1 所述在 Kernel：`ApprovalHub` 实现
> `ApprovalResolver`，被 Kernel 调用后推 `agent://approval-request` 并挂起等待
> `agent_approve`；`ApprovalDecided` 与被拒调用的失败 `ToolResult` 都由 Kernel 写入
> 日志。上例载荷里的 `riskLevel` 未实现（plugin-api 无 `RiskLevel`），风险度由
> `side_effect` 呈现；字段命名 snake_case（§12.2 阶段六备注）。前端组件为
> `desktop/src/components/ApprovalModal.vue`，文案全部走 `t()`（§13.5）。

### 13.3 插件市场的 UI 结构

核心是**双页签 + 卡片网格**：

- **发现页**：搜索 + 分类 chips + 卡片网格（头像 / 星标 / 分类 / 已弃用徽标 / 安装按钮）
- **已安装页**：按分类分组，每行显示版本、状态、更新按钮、卸载按钮、回退按钮

更新检测逻辑：只检查已安装且仍在目录中的插件，比较本地版本与 registry 的 latest，仅前向 semver 更新。

> **实现备注（阶段七）**：落点 `desktop/src/components/PluginMarket.vue`（发现/
> 已安装双页签 + 卡片网格 + 每行启停/更新/回退/卸载）与
> `desktop/src/stores/plugins.ts`。发现页卡片带版本下拉（选非 latest 安装，
> 服务于"装旧版 → 更新 → 回退"的全链验收）；已安装页按 id 首段分类分组。
> 实时同步从简：任何 `plugin://state-changed` 都整表重拉 `plugin_list` +
> `plugin_registry_list`（本地目录市场与 lockfile 均为毫秒级本地 IO）。
> 更新检测在前端做数值化 semver 前向比较（§13.3 逻辑），不设独立命令。

### 13.4 状态实时同步

前端订阅 `plugin://state-changed` 事件，Pinia store 收到后更新列表。这样用户点"安装"后，无需刷新页面，插件状态从 `installed` → `loading` → `active` 的转换会实时反映。

```typescript
// desktop/src/stores/plugins.ts
import { defineStore } from 'pinia';
import { invoke } from '@tauri-apps/api/core';
import { subscribePluginState } from '../lib/api';

export const usePluginStore = defineStore('plugins', {
  state: () => ({
    installed: [] as InstalledPlugin[],
    marketplace: [] as MarketplaceItem[],
  }),
  actions: {
    async loadInstalled() {
      this.installed = await invoke('plugin_list');
    },
    async initSubscriptions() {
      await subscribePluginState((change) => {
        switch (change.type) {
          case 'installed':
            this.loadInstalled();
            break;
          case 'enabled_changed':
            // 只更新对应项的状态
            const p = this.installed.find(i => i.id === change.id);
            if (p) p.enabled = change.enabled;
            break;
          case 'uninstalled':
            this.installed = this.installed.filter(i => i.id !== change.id);
            break;
        }
      });
    },
  },
});
```

> **实现备注（阶段七）**：通道已落地 `plugin://state-changed`，载荷就是
> plugin-service `PluginStateChange` 的序列化形态——`type` 标签 +
> installed / enabled_changed / uninstalled / active_version_changed
> （`Version` 随 serde 序列化为字符串），不经 `AgentEvent` 包装；转发器在
> 组装点 `events.rs::forward_plugin_states`（编排层不依赖 Tauri）。

### 13.5 国际化（i18n）

**需求（用户指定，2026-09-29）**：桌面端界面文案支持**中 / 英双语，默认中文**。

- 技术选型：`vue-i18n`；文案集中在 `desktop/src/locales/{zh-CN,en}.ts`，组件不硬编码文案
- 范围：UI 文案——导航、按钮、占位符、工具卡片标签、审批弹窗、错误条、状态徽标。
  会话内容（模型回复、工具输出、事件日志）属于数据，不在翻译范围
- 切换入口：设置面板（§13.1 设置模块）；当前语言持久化到本地设置，缺省跟随系统
  语言、无匹配回落中文
- 后端事件载荷（`agent://*` 的字段值）保持英文枚举原样（如 `pure` /
  `irreversible`），翻译只在渲染层做
- 落地节奏：阶段六新增界面（会话视图完善、审批弹窗）时**文案一律走 `t()`**，
  避免事后返工；语言切换入口与持久化随阶段八设置面板落地

---

# 第四部分：交付

## 14. 落地路径

| 阶段 | 内容 | 验收标准 |
|---|---|---|
| **阶段一（1-2 周）** | 内核骨架 + 事件日志：`Context::provide`/`use_service`、`Session` 追加/投影/回放，进程内工具跑通 ReAct | 一个进程内工具跑通 ReAct 循环，会话能 replay 到任意点 |
| **阶段二（2-3 周）** | Effect 撤销链 + Fiber 生命周期：所有 `Context` 修改走 `effect()`，Fiber 六态 + 依赖图 | 卸载 A 时 B 自动卸载，重加载 A 时 B 自动恢复 |
| **阶段三（2-3 周）** | C ABI 插件加载：`plugin-api` 符号约定、`plugin-host` dlopen、多版本管理、`rollback()` | 加载 1.2.0 → 激活 1.3.0 → 回退 1.2.0，全程不重启 |
| **阶段四（2 周）** | Waterfall 插桩 + Scope 隔离：三个决策点，`ScopedLayers` 惰性创建 | 审批插件通过 `agent/request` 改写参数；两 Agent 工具视图隔离 |
| **阶段五（1 周）** | Tauri 脚手架 + 最小命令：`agent_chat` 跑通，前端收流式 chunk | 桌面端能聊天，能看到工具调用 |
| **阶段六（1-2 周）** | 会话视图 + 审批弹窗：完整聊天体验 + 不可逆确认 | 全 UI 化操作，无需 CLI |
| **阶段七（1-2 周）** | PluginService + 管理命令 + 市场 UI | 安装/卸载/启用/禁用/更新/回退全部可用 |
| **阶段八（持续）** | 设置面板 + 快捷键 + 托盘 + WASM 加载器 + i18n 切换入口（§13.5） | 完整产品化：界面中/英可切换，默认中文 |

> **实现备注（阶段八·用户排期，2026-09-29）**：步骤顺序——① i18n 切换 +
> 最小设置面板 → ② 真实 LLM（**LLM.md 方案一**：`genai` 类统一库封装进
> 适配器，对外只暴露 `LlmService`；三合一 = openai/anthropic/ollama）→
> ③ 第一批工具（plugins.md：tool-fs 只读 / tool-fetch / mw-audit）→
> ④ 回头打磨市场细节（副作用徽章、回退下拉、更新按钮）→ ⑤ 托盘 + 快捷键
> （发版前）→ ⑥ WASM 加载器（独立里程碑，看需求，§5.8）。界面先骨架后
> 美化（用户指定）：功能按钮接通优先，视觉打磨放最后。
>
> **实现落点（v2.9，③已完成）**：`tool-fs@1.0.0`（`fs.read` / `fs.list` /
> `fs.grep`，全 `Pure`，只读先行；写入能力第二批按 `Compensable` 备份做）、
> `tool-fetch@1.0.0`（HTTP GET，`Pure`；4xx/5xx 按数据返回而非报错，传输层
> 错误才失败；响应体限长防灌爆上下文）——两者都是真 C ABI 插件
> （`plugins-src/`，`build_plugins.sh` 暂存），桌面端开发态装载后自动进
> 工具目录。`mw-audit` 以宿主侧中间件落地（落点见 §7.1 v2.9 备注）。E2E：
> plugin-host 集成测试真实 dlopen 两插件（fetch 打本进程 TCP 的 HTTP 服务，
> 全离线）；kernel-agent / desktop 单测含审计记录与 JSONL 落盘。
>
> **排期更新（v2.10，2026-09-29）**：新增权威文档
> `docs/design/Plug-in packaging.md`（插件元数据与打包 v2：LocalizedString
> 多语言、图标 data URL、author/homepage/license、manifest.json 落盘且
> dll 文件名不假设、前端 PluginCard 与 Fallback 矩阵、`locowrk new plugin`
> 脚手架）。**④ 重定义为"插件元数据 v2 + 市场打磨"**，子步骤：
> ④a plugin-api manifest v2 解析层——可选字段全 `#[serde(default)]`，
> `abi_version` 保持 1（新增字段只进 manifest JSON，符号约定不动，旧插件
> 免重编）；SideEffect 解析兼容"字符串"与"tagged"双形态（该文档 §2.1 与
> §8.1 两处示例自身不一致），序列化维持现行 `{"kind":"…"}` snake_case
> （describe JSON 不破坏旧插件）；工具级 `display_name` 只放 manifest.json
> 与投影层，**ToolSpec 不加字段**（模型侧描述保持单语言纯文本）；
> PluginManifest 追加可选字段属 append-only，合规。→ ④b 数据面：
> IconLoader（base64 data URL + canonicalize 防路径穿越 + 1MB 上限 + 缓存）、
> `InstalledPlugin` / `MarketplaceItem` / `plugin_list` 投影扩展、dev 态
> `plugins/` 补 manifest.json、dll 文件名发现规则改"manifest 为准"、坏
> manifest 跳过不崩整表。→ ④c 前端 PluginCard：pickLocalized 回退链
> （精确 → 主语言 → en-US → 任意 → id）、图标 data URL + 首字母兜底 +
> onerror 隐藏、副作用徽章配色；与原④范围（徽章 / 回退下拉 / 更新按钮）
> 合并实施。**已安装列表按 `origin` 分两大类**（Plug-in packaging.md §2.4，
> v2.1 增补）：`official`（官方内置，仓库自带插件显式声明）与
> `community`（第三方，**缺省值**，预留给未来市场开放；缺省即 community
> 防伪造官方徽章；分类只影响展示分组，不做信任边界）——④b 投影带出
> `origin`，④c 分组渲染。→ ④d Fallback 矩阵全覆盖测试 + 多版本并存/回退回归
> （1.1.0 → 1.1.1）。→ ④e `locowrk new plugin` 脚手架（kernel-bin 子命令
> + 模板）。**⑤ 托盘 + 快捷键（发版前）追加发布收尾一项：数据目录
> `~/.agent-kernel` → `~/.Locowrk`**（用户 2026-09-29 指定；尚无存量用户，
> 直接改名不做迁移，同步 §11、plugins/README.md 与 `data_root.rs`）。
> ⑥ WASM 不变。
>
> **实现落点（v2.11，④a–④c 已完成）**：依 docs/design/Plug-in packaging.md
> v2.1——
> **④a 解析层**（plugin-api）：`PluginManifest` 追加 v2 可选字段
> （display_name / description / icon / author / origin / homepage / license，
> 全 `serde(default)` 且 None 序列化省略，旧插件 describe 字节不变，
> `abi_version` 仍为 1）；`LocalizedString`（BTreeMap 底座，`pick` 回退链
> 精确 → 主语言 → en-US → 任意字典序最小）+ `LocalizedOrString`（v1 字符串 /
> v2 map 双形态）+ `Author` + `PluginOrigin`（缺省 community，
> skip_serializing_if 保持 v1 字节）；**SideEffect 解析双形态**（tagged 对象
> 或大小写不敏感纯字符串；`Compensable` 只收 tagged——undo_tool 只有对象
> 带得动），序列化维持 `{"kind":…}`；**ToolSpec 宽容解析**：description 兼容
> 多语言 map（就地 en-US → 任意压平成单语言 String，模型侧目录不做多语言），
> 除 name 外字段缺失取缺省、**side_effect 缺失按 Irreversible**（宁可标错成
> 不可逆也不标错成 Pure），工具级 display_name 等未知字段忽略；manifest 级
> `reversible` 不设字段（副作用模型按工具声明，文档式 manifest 靠未知字段
> 忽略照常解析）。仓库四插件 describe 全部补 v2 元数据（origin=official
> 显式声明）。
> **④b 数据面**：plugin-host 新增 `icon.rs` IconLoader（canonicalize 防路径
> 穿越 + 1MB 上限 + svg/png/webp 白名单 + DashMap 正负缓存）；`PluginInstance`
> 记录 dll 路径、`PluginStateSummary` 带展示目录（active 优先，否则最后
> 加载版本）；plugin-service 新增 `display.rs`（版本目录 manifest.json →
> `PluginDisplay` 投影，manifest 缺失/损坏降级缺省不崩列表），`InstalledPlugin`
> / `MarketplaceItem` 带出 display（多语言名/描述/icon data URL/origin/
> 副作用固定序清单）；market 与 installer 按 v2 全量 manifest 交叉校验，
> 坏 manifest 版本目录跳过；`origin` 分组后端数据即此（§2.4：缺省 community
> 防伪造官方徽章，分类非信任边界）。构建脚本暂存 manifest.json（sed 版本号）
> 与 assets/ 图标；桌面端开发态装载 dll 发现规则改"约定名优先，否则目录内
> 唯一平台动态库，多库歧义拒绝"（packaging.md §3.1 宿主不假设文件名）。
> **④c 前端**：`pickLocalized`（与 Rust pick 同链）+ `PluginIcon`（data URL
> + 首字母兜底 + onerror 隐藏）+ `SideEffectBadges`（§6.4 配色）+ `PluginCard`
> （发现页卡片）；已安装页按 origin 分"官方内置 / 第三方"两组（官方在前），
> 行内展示多语言名 + id + 副作用徽章；内置只读行与全部管理操作不变。
> **测试**：plugin-api 双形态/Fallback 矩阵用例、plugin-host 图标矩阵
> （缺失/坏格式/超限/穿越/缓存）、plugin-service 展示投影与坏 manifest 跳过、
> desktop find_artifact 矩阵；plugin-service 暂存固件升级 v2 后全绿。
> **④d Fallback 矩阵收尾（v2.12）**：§7 矩阵逐行对账补三块——① v1 极简
> manifest（无任何 v2 展示字段）投影全 None（前端回退 id / 首字母 / 不渲染，
> origin 缺省 community）；② 已装目录 manifest.json 损坏 → `list_installed`
> 照常列出该行、投影降级缺省，列表整体不崩；③ 展示投影跟随 active 指针
> 回归（安装 → 更新到新版 → 补丁改新版目录 display_name → 投影切新 →
> 回退 → 投影切回旧版；双版本目录并存 = 前端版本下拉数据源）。多版本
> 回退主链（install→update→rollback、更新失败保旧指针 + 清理新目录）此前
> 已有回归覆盖，本次不重复。
> **④e 脚手架（v2.12）**：`locowrk new plugin <name>`（kernel-bin 新增
> `scaffold.rs` + cli 接线）——按 packaging.md §8 生成
> `{Cargo.toml, src/lib.rs, manifest.json, assets/icon.svg}` 四件；manifest
> 为 §8.1 全量 v2 模板（工具级双语描述与 display_name、manifest 级
> reversible 写出，靠未知字段忽略前向兼容；origin 不写 = 缺省 community）；
> 名称清洗（§2.2：小写/数字/连字符，大小写归一，清洗后为空或已存在拒绝）；
> plugin-api 依赖路径自生成位置向上探测仓库根，仓库外写占位符；lib.rs
> 用 `declare_plugin!` 单工具 Pure 骨架。模板在 tmp/ 实测独立
> `cargo check` 通过（开箱即编译）。service/tests 超 400 行拆
> tests/{mod,lifecycle,fallback}（职责子模块）。
>
> **⑤ 托盘 + 快捷键 + 数据目录改名（v2.13）**：桌面端新增 `tray.rs`——
> 托盘图标（默认窗口图标）+ 菜单（显示主窗口 / 退出），左键单击 = 显示
> 聚焦主窗口；主窗口关闭 = 隐藏进托盘（CloseRequested 拦截），真退出只
> 走托盘菜单。`tauri-plugin-global-shortcut` 全局快捷键按下 = 切换主窗口
> 可见性；键位存 settings.json `shortcut`（字段级 serde 缺省，旧文件缺
> 字段取 SHORTCUT_DEFAULT 而非空串；缺省 Ctrl+Shift+L），`settings_set`
> 改键 → `parse::<Shortcut>` 校验 → 落盘 → `apply_shortcut` 重注册（先
> 挂新键成功才清场重挂，失败旧键保持；启动注册失败降级为无快捷键并
> 告警）。设置面板增「全局快捷键」块（§13.1 备注兑现），tauri 依赖开
> `tray-icon` feature。**数据目录按用户指定改名 `~/.agent-kernel` →
> `~/.Locowrk`**（直接改名不迁移：`data_root.rs` 与 §11.3、
> plugins/README.md、desktop/README.md、market/README.md 及相关 doc
> 注释同步）。托盘菜单文案暂硬编码中文（§13.5 默认中文；i18n 化随
> 菜单配置后续接）。
>
> **⑥ 第二批官方内置插件（v2.14，用户 2026-09-29 排期重排）**：⑥ 由
> "WASM 加载器"重定义为 **plugins.md §七落地顺序第二批**（WASM 顺延为
> ⑦，独立里程碑看需求，§5.8）。**安全底线同日指定**（对已落地工具回溯
> 生效）：① 文件类工具所有路径参数 canonicalize 后校验允许根前缀——
> plugin-api 新增 `fsutil`（Rust 便利层不进 ABI 面）：允许根 =
> `LOCOWRK_FS_ROOTS`（`;` 分隔，桌面端设工作区目录），缺省只放行数据根
> （备份目录在其下，fs.restore 天然可达）；tool-fs/tool-edit 共用，
> fs.grep 遍历跳过符号链接、fs.list 报 `link` 类型；② tool-fetch 加
> SSRF 防线（`ssrf.rs`）：私网/环回/链路本地（127/8、10/8、172.16/12、
> 192.168/16、169.254/16、0.0.0.0、::1、fc00::/7、fe80::/10 等）IP
> 字面量直判、域名解析后逐 IP 判，解析失败 fail-closed，重定向不跟随
> （302→内网是最短绕过），默认超时 20s→10s，
> `LOCOWRK_FETCH_ALLOW_PRIVATE=1` 显式后门（本机测试/开发）；③
> SideEffect 分级宁严勿松——**fs.restore 判 Irreversible**（恢复会覆盖
> 可能更新的当前内容，plugins.md §八），补偿执行器对 Irreversible undo
> 照走审批门。落点：**tool-fs@1.1.0**（fs.write
> `Compensable{undo_tool:"fs.restore"}` 备份先行——备份失败整体报错目标
> 不动；fs.delete Irreversible 仅文件；fs.restore Irreversible 且校验
> backup 必须出自数据根 `backups/`——防伪造 output 搬运任意文件；备份
> 布局 `<数据根>/backups/tool-fs/<纳秒>-<pid>/`；lib.rs 拆
> read.rs/write.rs——400 行纪律）；**kernel-agent compensation.rs**（§6.3
> 执行器：session-store `plan` 产出纯计划、内核逆序执行——undo 调用按
> 契约 `{"call_id","args","output"}` 传参（FFI 不传 CallCtx），新 call_id
> `{原id}-undo` 记日志（复用原 id 会产生同 id 双 ToolResult 破坏真实模型
> 协议），`CompensatedLedger` 成功后登记（失败可重试），Irreversible
> undo 过审批门，失败按纪律 5 闭合 ToolResult；复用主循环中间件链——
> 审计/超时/熔断对补偿同样生效）；**tool-shell@1.0.0**（plugins.md §二
> 安全策略：内置白名单 `ls/cat/grep/git status`，`LOCOWRK_SHELL_ALLOW`
> init 时读一次扩展 `prog [sub]` 条目；denylist（rm/sudo/curl/
> powershell/python 等约 50 项）永远赢、不进审批；元字符
> （`|&;<>\`$\n`）整条拒绝；无 shell 解析——引号感知切词后首词直接
> spawn，读取线程防管道死锁，轮询超时 20s（上限 120s）kill，输出限长；
> 一律 Irreversible）；**tool-edit@1.0.0**（edit.replace 强制唯一命中、
> edit.lines 1-based 闭区间严格不越界、edit.diff 严格 unified diff 无
> fuzz——试算全过才备份落盘，失配整体拒绝文件不动；全部
> `Compensable{undo_tool:"fs.restore"}` 跨插件 undo——官方捆绑保证同装，
> 备份经 fsutil 同布局）；**ctx-compact**（kernel-agent
> `middleware/compact.rs`，宿主侧——C ABI 插件够不到 LlmService；挂在
> `agent/request` waterfall：估算 token（序列化字节/4）超 32k 就把早期
> 消息压成单条 User 摘要（keep_recent=8），**切点对齐 User 消息起点**
> （不拆 tool_calls↔ToolResult 配对），摘要直连 LlmService 无递归，
> fail-open——任何一步失败放行原 payload；Replace 是逐跳改写、终态仍
> Continue（cordis-core 语义）；desktop state.rs 注册，阈值设置面板接线
> 归后续批次）。测试：fsutil/tool-fs/tool-shell 策略/tool-edit（含
> diff 矩阵）单测三态（正常/边界/恶意），plugin-host 真实 dlopen 集成
> （write→restore 全链路、白名单真实 spawn cargo --version、跨插件
> edit→fs.restore、SSRF 拒绝环回）；fs_fetch_plugins.rs 随 v1.1.0 契约
> 与根校验/后门 env 更新。
>
> **⑦ 第三批官方内置插件（v2.15，plugins.md §七第三批）**：三件 +
> 集成测试。**tool-git@1.0.0**：status/diff/log=Pure——porcelain 机读
> 解析（status `--porcelain=v1 -b --null` 是**全 NUL 字段流**，分支头也
> 以 NUL 结尾没有换行；log 用 \x1f 字段 / \x1e 记录分隔；空仓库按
> empty_repo 返回不报错）+ commit=`Compensable{git.undo_commit}`（输出
> 带新提交哈希；undo=soft reset 前校验 HEAD 仍指向原提交，动过即拒绝
> 转人工）+ push=Irreversible（远端/分支名字符白名单挡 `-oProxyCommand`
> 式 argv 注入；文件路径一律 `--` 之后）。git CLI argv 直 spawn 无 shell；
> 子进程环境剥 GIT_DIR / GIT_WORK_TREE 等重定向变量（`-C` 的允许根校验
> 不可被环境绕过）+ GIT_TERMINAL_PROMPT=0；repo 参数走 fsutil 允许根，
> add 相对路径按仓库根解析、canonicalize 后必须在仓库内。测试含本地
> bare 远端的离线 push 全链路。**memory-sqlite@1.0.0**：跨会话记忆库
> `<数据根>/memory.sqlite3` 固定路径（无路径参数即无逃逸面），rusqlite
> bundled（首个第三方重依赖，经 rsproxy 按次镜像拉取）；get/list/search=
> Pure；put/delete=`Compensable{memory.undo}`——plugins.md §三把本插件
> 整体标 Pure，按用户"宁严勿松"收紧一档（Compensable 无审批摩擦），
> undo 整条还原含旧 updated_at；LIKE 通配符转义按字面量；key≤512B /
> value≤256KiB；会话事件持久化仍归 session-store（§6），插件只做记忆。
> **tool-mcp@1.0.0**：§五 MCP 桥——读 `<数据根>/mcp.json`（用户手写，
> 本插件不提供改配置工具）→ spawn stdio JSON-RPC 子进程
> （newline-delimited，无 Content-Length 帧）→ initialize 握手 +
> tools/list → 每个远端工具适配成本地 ToolSpec `mcp.<server>.<tool>`
> （inputSchema 透传；工具名清洗、server 名 [a-z][a-z0-9_-] 且无点防
> 路由歧义）→ describe 时**冻结工具清单**进 manifest；连接经 static 在
> describe/make_state 间转交（不重复 spawn），死连接调用时懒重连，
> mcp.reload 只重连不增删（清单变更需重载插件）；side_effect 由配置
> 声明**缺省 Irreversible**（§五安全兜底），compensable 收紧为
> Irreversible（远端无机械 undo）；工具级 isError 作为数据返回
> （tool-fetch 惯例）；内容 1MiB 上限、握手 15s、调用超时配置化缺省
> 30s 上限 120s；单 server 配置坏 fail-soft 进 mcp.status 不拖垮整体。
> 测试替身 examples/fake_mcp_server（echo/boom/slow）——cdylib 无集成
> 测试目标故 CARGO_BIN_EXE 不可用，且 deps/ 哈希产物与 test-harness
> 版同名无法区分，example 目标无 harness 且产物名无哈希、路径确定。
> plugin-host tests/tools_batch3.rs 真实 dlopen 三链路：memory
> put/undo/delete/undo、git commit→undo→push 本地 bare、mcp 发现+调用
> （env 须在 load 前就位——describe 阶段完成发现）。
>
> **⑧ WASM 加载器（v2.16，§5.8 落地）**：`PluginLoader` trait 兑现 +
> `WasmLoader` 第二实现（wasmtime 34，feature 只留 cranelift/runtime/std
> ——不开 wasi/component-model/threads，沙箱面最小）。registry 按 `.wasm`
> 扩展名分发（其余仍 DylibLoader），manifest/ABI 校验/工具信封与 C ABI
> 完全同构——**manifest JSON 同 schema、abi_version 同值同义（工具信封
> 契约版本），变的只是传输层**。WASM ABI（wasm32，指针=i32 线性内存
> 偏移）：导出 memory / describe（静态 NUL 结尾 JSON，宿主不 free）/
> alloc / free / init（返回 guest 句柄）/ call（输出经两个 i32 cell
> 回传）/ shutdown；宿主导入只注入 `locowrk.log`（镜像 host_vtable
> 分级）——**能力面由宿主枚举**，客户机声明任何其他导入（含 wasi_*）
> 一律实例化失败。缓冲协议：每次 call 宿主 `alloc` 一块
> `[0..8)输出格|[8..8+tl)工具名|[8+tl..)args`，用完 free 两次；输出
> (ptr,len) 谎报越界被拒（先校验界内再拷贝）。沙箱：无 WASI（无文件/
> 网络/env/时钟）；Store limiter 内存 64MiB / 1 块内存 / 表 4×10000；
> **epoch interruption** 进程级心跳线程（20ms/格），握手与每次调用共用
> 同一预算 `LOCOWRK_WASM_TIMEOUT_MS`（缺省 10s 上限 600s）——**已知坑：
> epoch 开启后 Store 默认 deadline=0，心跳一走 describe/init 即 trap，
> 加载期必须显式设预算**；deadline 按次调用重置（alloc/call/free 同
> 窗口，上次耗尽不污染下一次）。host func 闭包返回
> `Result<_, anyhow::Error>`（WasmRet 约束——`Result<_, Trap>` 不满足
> IntoFunc）；单参 TypedFunc 直传值、一元组类型才传 tuple。
> instance.rs 拆 instance/{mod,native}.rs（`PluginBackend` 内部 trait，
> native Drop 仍是全进程唯一 dlclose 路径；wasm 同实例调用经 Mutex
> 串行——guest 单线程线性内存，语义收窄接口不变）。desktop
> `find_artifact` 认 `.wasm`（约定 `{id_underscored}.wasm` 优先于扫描，
> native 约定名并存时优先；多工件歧义仍拒绝）——开发态自动装载 WASM
> 插件直达；market 打包/分发 wasm 归后续。测试：WAT 客户机三态单测 9
> （echo/manifest/side_effect 双形态、错误信封/未知工具/trap 透传、
> epoch 预算杀死 spin 且下次调用自愈、缺导出点名、无 NUL 拒绝、wasi
> 导入拒绝、内存超限、registry 分发）+ 集成 wasm_plugins.rs 2（staging
> 布局、native+wasm 混载同 registry 工具目录共存）。
>
> **排期重排：壳批次 + ⑨–⑬（v2.17，用户 2026-09-29 确认）**：依两份新配套
> 文档落地——`docs/design/Tool exposure strategy and usage statistics.md`
> （工具暴露策略与使用统计 v1.0：§4 exposure 三层 / §5 EWMA / §8 闲置
> 分级 / §9 UI）与 `docs/design/Context and Memory.md`（上下文与记忆管理
> v1.0：§3 压缩策略 / §4 记忆管理 / §5 协同工作流）。**排序依据**：Context
> 文档把上下文管理标 P0"不做就不可用"、记忆 P1，且以工具暴露文档为前置；
> 中间件链序 ToolExposure → Memory-Inject → ctx-compact（先定工具集、
> 再注入记忆、再按总量压缩）决定 exposure 字段（⑩）先于 Memory-Inject
> （⑪）、compact 的 system-prompt 保护（⑨）先于⑪；⑫ 打分依赖⑩统计地基；
> 两文档末段向量/提取均 ★★★ 且共享 embedding 基建，归⑬看需求。**动因**：
> 功能越多 UI 越乱的根因是缺落位规则——顶层视图 3→5 在即（+记忆页/
> 开发者面板），⑩ 是最大 UI 变更波（PluginMarket.vue 291 行必拆），故壳
> 批次先行。**进度看板单列 `docs/design/ROADMAP.md`**（完成打钩 / 未完
> 排期，每批同步勾选），本节只记落点细节。
>
> **壳批次（UI 骨架，先于⑨）**：skeleton-first 的"骨架"而非"抛光"——壳
> 只锁结构与规则，内容留给功能批次。① 导航模型定稿：顶层视图集合与命名
> （对话/插件/记忆/设置），视图数超过 4 时 header pill nav 切侧边栏，
> 开发者调试面板挂设置内；② 页面模式库：列表页模式（插件页/记忆页共用
> ——搜索、筛选 chips、多选批量栏、行内徽章、空态；PluginCard 抽通用
> 模式）、设置分区模式（settings.json 分区 ↔ 面板分区一一对应：llm /
> context / memory / shortcut，⑨阈值/⑩自动停用/⑪记忆开关都落此模式）、
> 会话内系统消息模式（压缩通知[查看详情][撤销] / "已记住：xxx" / 审批
> 结果统一组件）、徽章体系（SideEffectBadges 并入闲置徽章灰/黄/红，
> exposure 文档 §9.4 配色）、上下文进度条模式；③ 事件→UI 流向图：
> `agent://` → store → 组件映射表 + command → 面板映射表，功能批次只
> 扩表；④ 弹层规则：modal（审批）/ 行内展开 / 独立页面的判据一次定死；
> ⑤ 视觉只定 design tokens（间距/圆角/色板 CSS 变量），不做全面抛光。
> Vue 文件同受 400 行纪律（PluginMarket 拆分在本批顺带完成）。
>
> **⑨ 上下文管理收口（Context 文档 §9 阶段 1–6，P0）**：现状 = compact.rs
> 已存在（固定 max_tokens 32k + keep_recent 8，分层保留近似形态），本批
> 改配置驱动。① 开工先定 LlmService 模型窗口来源（genai 未必暴露 → 设置
> 面板 context window 字段 + 预设表）——阈值按百分比计算的依赖；② 阈值
> 滑条接线（收尾 backlog 项）+ 百分比与绝对 token 双显示（§3.2.2：40% 在
> 8k/128k/1M 窗口含义完全不同）+ 下限 20% 警告（压缩成本 = 发送当前上下文
> + 接收摘要，太低得不偿失）；③ 压缩策略三档——滑动窗口（纯本地不调
> LLM）/ 分层保留（默认：系统提示 + 摘要 + 最近 3-5 轮原文）/ 仅摘要；
> 压缩目标 = 阈值的 50% 而非贴线（§3.4，留缓冲防频繁触发）；④ 动态建议
> 值 suggested_threshold(window)（§3.2.3：窗口越大建议越低——本地小模型
> 30% / 中等 35% / 大窗 25% / 1M 15%）；⑤ **ContextCompacted 事件进
> session-store**（from/to tokens、strategy、summary_preview 前 200 字符；
> serde default 向后兼容旧日志）+ 撤销 rewind_to（Event Sourcing 惯例：
> 任何自动动作可撤销）；⑥ 会话内实时进度条（事件推送，绿 <60% 阈值 /
> 黄 60-99% / 红 ≥阈值）+ 压缩系统消息。
>
> **⑨ 落地（v2.19，提交 92f298b）**：① 窗口来源 = settings.json
> `context.windowTokens`（预设 8k–1M + 自定义；genai 不暴露窗口元数据，
> 用户选择）；**窗口未知回落 32k 保守保护**（FALLBACK_WINDOW_TOKENS，
> 沿用 v2.14 行为——阈值 = 窗口×百分比要等窗口已知才成立）。②③④
> compact.rs 重写：`CompactConfig{threshold_tokens, strategy}` 经共享
> `RwLock` 注入（组装点持锁、settings_set 成功即热换入，kernel-agent
> 不依赖设置存储）；`CompactStrategy` serde snake_case 与设置区同形直接
> 复用；切点从 keep_recent 计数改为**预算制**（从尾部累计到阈值 50%，
> 回退到最近 User 边界；仅摘要切到最后一条 User）；分层/仅摘要的摘要
> 失败或 LlmService 缺失**就地降级滑动窗口**（§3.5 伪码语义，本地裁剪
> 仍保窗口不溢出；v2.14 的 fail-open 放行保留为最后兜底——找不到
> User 边界不硬压）；`suggested_threshold(window)` 导出，设置投影带
> `threshold_tokens`（生效值）与 `suggested_pct`，前端不做第二套换算。
> ⑤ `SessionEvent::ContextCompacted`（kept_messages / summary / from /
> to / strategy / ts，全字段 serde default 兼容旧日志）——**摘要存全文**
> （文档 §3.7 的 preview-only 无法支撑折叠重建，投影给 UI 时另截 200
> 字符）；折叠语义：derive_messages/state_at 把已累计消息留尾部
> kept_messages 条、其余替换为摘要 User 消息（压缩落在会话日志里，
> 后续 step 的投影与当轮 in-flight 改写一致）；压缩结果经
> `RequestPayload::compacted`（serde default 零破坏）从决策环带回，
> kernel.step 在本轮 assistant 消息**之前**落事件；重试轮幂等守卫
> （payload 已带 compacted 不再压）。撤销 = `context_undo_compact`
> 命令：rewind_to(压缩事件序号) + fork 原地换入（会话 id 不变）——
> 前缀游标语义下**其后的轮次一并回退**（"回到压缩前"，与文档
> rewind_to 语义一致；日志只追加不删除由"日志是内存副本"兜底，落盘
> 持久化在收尾 backlog）。⑥ `agent://context-usage` 每 step 推送
> （estimate_tokens 与压缩判定同一把尺）→ ContextProgressBar 接线；
> `agent://context-compacted` → SystemNotice [查看详情][撤销]（details
> = pushRawSystem 摘要展开，undo = 上述命令；失败推
> notice.compactUndoFailed）。前端 ContextSection 替换壳批次占位
> （UI shell doc §3.2/§4 已扩表）；中间件链位不变（ToolExposure /
> Memory-Inject 落地时按 §3.6 插到 ctx-compact 之前）。
>
> **⑩ 暴露声明 + 使用统计地基 + 闲置 UI（exposure 文档 §12 阶段 1–6）**：
> ① ToolSpec/manifest 加 `exposure` 字段（always / on_demand / hidden，
> 缺省 on_demand；`always` 总数 >10 加载告警=作者设计失误，§4.1）——
> manifest JSON 可选字段追加，现有 ToolSpec 宽容解析"未知字段忽略、缺省
> 取默认"，旧插件零破坏，PLUGIN_ABI_VERSION 不动；② always 直通 +
> on_demand 全暴露（行为暂时零变化，管线就位，截断归⑫）；③
> SessionEvent::ToolCall 补 plugin_id（现仅 plugin_version；serde default
> 兼容旧日志）+ **`<数据根>/usage.sqlite3`**（宿主侧自有，与 memory.sqlite3
> 分离——统计是宿主对插件的观测，不是插件数据）：tool_usage（含
> ewma_score）/ plugin_usage 两表，ToolResult 后异步更新，5s 批量刷盘
> （崩溃最多丢 5s，§3.3）；④ IdleLevel 六级 + IdleInfo 结构化输出
> （level+value+unit）——后端只返回结构化数据不返回文案，前端按 locale
> 渲染人类尺度"多久没用"（本周/上个月/3 个多月/半年多，§8.4–8.6）；
> 试用期 14 天保护；NeverUsed（装了 ≥14 天从未用）与长时间未用严格区分
> （§8.7）；⑤ plugin_list_with_usage 命令 + 前端闲置渲染 + 筛选器（全部/
> 运行中/已停用/超 1 月/超 3 月/半年，文案同用"多久"）+ 批量操作（批量
> 卸载二次确认列出插件名，§9.5）；⑥ 闲置提示只在用户主动打开已安装页时
> 出现，不弹 toast 不打断聊天（§8.8 提示而非自动；自动停用为默认关闭的
> 设置项归⑬）。已核对现状：lockfile `set_enabled` 落盘已存在
> （plugin-service/lockfile.rs），仅核对启动只加载 enabled:true。
>
> **⑪ 记忆注入 + 记忆管理 UI（Context 文档 §9 阶段 7–9）**：现状 =
> memory-sqlite@1.0.0 已存在（memory.get/list/search/put/delete/undo），
> 工具名与文档 memory_write/read/search 对齐 + `exposure: always` 声明
> （用⑩字段；记忆工具始终可存取，§4.4）。新增：① Memory-Inject 中间件
> ——读 memory.sqlite3 按最近用户消息关键词检索，access_count +
> last_accessed_at 排序 top-5（可配）注入 system prompt；链位
> Tracing → ToolExposure → Memory-Inject → ctx-compact → Timeout
> （ToolExposure 截断未落地前链位预留）；**ctx-compact 必须保护 system
> prompt 永不压缩**（§5.3 冲突 2：压缩压掉记忆是 bug）；② memory.put
> 同步落盘不走异步队列（§5.3 冲突 3："说了记住却没记住"最伤信任）；
> ③ 记忆管理页（搜索/列表/编辑/删除/清空；每条显示来源 explicit/
> extracted 与时间，§4.5——能看到并删除每条记忆是信任基础）。数据根
> memory.sqlite3 的 UI 访问归属实现时对齐：桌面端 command 直读数据根
> 文件（不碰 PluginRegistry，不违反 §10 UI 边界），落点补记 DESIGN.md。
>
> **⑫ 打分截断 + 元工具（exposure 文档 §12 阶段 7–10）**：① tool_usage
> EWMA 更新（decay = exp(-days/7)，半衰期约 5 天——纯计数会让三个月前的
> 旧热工具永远压榜，§5.1）；② Top-N 截断（N=20 可配）+ 暴露调试面板
> （开发者选项：本轮暴露 N/M、always/session_hot/global_hot/retrieval
> 分布、节省 token 估算，§6.3/§9.6）；③ 会话内热度（从 SessionEvent 实时
> 算不落盘，会话结束即失效；组合权重：会话内 1.0 > 全局 EWMA 0.7 >
> 检索 0.5，always 满分直通，§5.3）；④ search_tools 元工具（always +
> Pure，LLM 主动拉取兜底；空结果回退 top-10 高频，§4.3/§13）。触发时机：
> 内置 ~26 工具不截断也够用，接首个大 MCP server（+30）前落地正合适；
> EWMA 数据自⑩开始积累不浪费。
>
> **⑬ 智能层（看需求，两文档末段）**：向量检索（exposure 文档阶段 11，
> embedding 索引 + 未就绪降级 BM25/关键词）、memory-vector 经历记忆
> （Context 阶段 10）、隐式提取（Context 阶段 11，agent/post-step 异步
> 提取"值得记住的事实"——容易记垃圾故放最后）、自动停用设置项（exposure
> 文档阶段 12，默认关闭）。四者共享 embedding 基建，出现真实需求一起排。
> 收尾 backlog 不变（fs roots 目录选择器、desktop session 持久化、LLM
> 流式、市场 wasm 打包分发、kv_get 所有权、profile.toml 面板接线、托盘
> i18n），随批次搭车。
>
> **壳批次落点（UI 骨架，v2.18，提交 a2c82a5）**：落地规则文档
> `docs/design/UI shell and patterns.md`（§2 导航 / §3 模式库 / §4 流向图
> / §5 弹层规则 / §6 tokens / §7 扩展规则——功能批次照 §七"只扩表不改
> 骨架"执行；规则没落位的东西先改文档再写码）。代码：`App.vue` 顶层 4
> 视图定稿（对话/插件/记忆/设置；>4 再切侧边栏并届时评估 vue-router，
> 现不引入；chat `v-show` 保活，余 `v-if` 按需挂载）；`patterns/ListPage`
> 列表页模式（chips+搜索头部 → 空态优先内容 → `#batch` 批量栏插槽；
> 插件-已安装页 origin chips 与记忆页骨架为头两实例）；`chat/SystemNotice`
> 会话内系统消息模式 + chat store `system` 类别——**存 i18n key 而非渲染
> 文本**（文案是 UI 不是会话数据，语言切换对历史消息同样生效），
> `pushNotice()` 是唯一生产入口（⑨ 压缩通知 [查看详情][撤销]、⑪ "已
> 记住"接入即用，动作只上抛 id 由批次接线）；`IdleBadge`（六级 →
> fresh/normal 不显示、idle 灰 / stale 黄 / abandoned+never_used 红；
> 契约 = 后端只给 IdleInfo 结构化 `{level,value,unit}`，前端渲染"多久
> 没用"）与 `ContextProgressBar`（绿 <60% / 黄 60-99% / 红 ≥阈值；null
> 整个不渲染）挂载点就位、数据随⑨⑩接；设置面板拆 `settings/` 区组件
> （settings.json 分区 ↔ 面板分区一一对应：llm/sessions/language/
> shortcut 实装，context/memory/developer 虚线占位——**占位在对应批次
> 同批替换，不留永久空壳**）；PluginMarket 291→54 行（DiscoverTab /
> InstalledTab 子组件 + `lib/plugin-helpers` 纯助手；check_length.sh
> 纳入 `.vue`）；`style.css` `--lw-*` design tokens（表面三级/文本四级/
> 语义色五/圆角四/间距五，与 Tailwind 同值，唯一样式事实源，存量组件
> 不动）。搭车：托盘菜单 i18n——原生菜单无法复用 TS 文案表，tray.rs
> 自带双语小表，按 settings `locale` 取、`system` 经 sys-locale 探测
> （zh*→中文），settings_set 改语言成功即 `refresh_menu` 重挂（失败仅
> 日志）。验证：`pnpm build`（vue-tsc）过、desktop `cargo test` 23 过、
> check_length OK（最大 .vue = InstalledTab 193 行）。壳不自产数据：
> 徽章/进度条/批量栏/占位分区的数据接线全部归⑨⑩⑪⑫。
>
> **进度**：① 已完成（§13.1 备注）。② 已完成（v2.8，§7.2 备注）：桌面端
> `llm/` 模块以 genai 0.6 实现 `LlmService`，`LlmRequest` 携带工具目录快照
> （`ToolSource::list()`），配置读设置面板、保存即生效，model 未配置回落
> 脚本替身；真窗口 E2E（本机 Ollama 9B）全链路通过——模型按 schema 自选
> 参数调用 tool-echo 插件、工具结果回传后给最终回答。③ 已完成（v2.9，
> §7.1 备注）。④ 已完成（④a–④c v2.11，④d/④e v2.12）、⑤ 已完成
> （v2.13，上注）、⑥ 已完成（第二批官方内置插件 + 安全底线，v2.14，上注）、⑦ 已完成（第三批官方内置插件 tool-git / memory-sqlite / tool-mcp，v2.15，上注）、⑧ 已完成（WASM 加载器，v2.16，上注）。编号阶段①–⑧全部落地。**v2.17 排期已开跑：壳批次（UI 骨架）已完成（v2.18，上注，提交 a2c82a5）→ ⑨ 上下文管理收口（P0）已完成（v2.19，上注，提交 92f298b）→ ⑩ 暴露声明 + 使用统计地基 + 闲置 UI 已完成（v2.20，下注，提交 f6b1a44；搮车修复：插件工具名去点号，提交 9a5d39c）**。下一批 ⑪ 记忆注入 + 记忆管理 UI → ⑫ EWMA 打分截断 + search_tools 元工具 → ⑬ 智能层（向量/隐式提取/自动停用，看需求）；收尾 backlog 随批次搭车（托盘 i18n 已随壳批次完成）。

**阶段一和阶段二是试金石**：如果内核的服务组合和 Effect 撤销链设计干净，后续所有能力都是叠加。**阶段五和阶段六是桌面端的试金石**：如果 Tauri IPC 层能干净地桥接内核事件流，后续插件管理 UI 都是在这个模式上叠加。

---

## 15. 风险与权衡

### 15.1 ABI 稳定性

**风险**：`plugin-api` 一旦发布，任何改动都会破坏已编译插件。

**缓解**：
- 只暴露 C ABI + JSON 边界，不暴露 Rust 类型
- 加字段只能追加到末尾
- 用 `PLUGIN_ABI_VERSION` 做显式校验
- 长期方案：WASM 沙箱彻底解耦 ABI

### 15.2 内存增长

**风险**：永不 dlclose 导致已卸载插件内存不释放。

**缓解**：
- 逻辑卸载 + 物理保留是业界惯例（IDE、数据库插件系统）
- 真有内存压力时，drain 到 `Arc::strong_count == 1` 确认零在途引用后再 dlclose——这是纪律 3 之外的显式例外（运维动作，默认不做）
- 作为最后手段，定期重启进程

### 15.3 交错 Effect 的复杂度

**风险**：多个 Fiber 的 Effect 交错，撤销顺序容易出错。

**缓解**：
- 严格 LIFO + `FiberId` 过滤
- 每个 Effect 记录 `description`，便于调试
- 单元测试覆盖：A、B 交错注册，只卸载 A 后 B 仍正常

### 15.4 依赖图死锁

**风险**：两个插件互相 inject，形成循环依赖。

**缓解**：
- 加载时检测循环，拒绝注册
- 拓扑排序失败即报错，不静默降级

### 15.5 补偿工具不幂等

**风险**：补偿本身失败重试，产生重复副作用。

**缓解**：
- 强制要求 `call_id` 作为幂等键
- 补偿工具的实现方必须做去重
- 内核侧记录补偿执行日志，重复调用直接跳过

### 15.6 插件市场安全

**风险**：从市场安装的插件可能包含恶意代码。

**缓解**：
- 强制签名校验（`sha2` + 公钥验签）
- 首次安装弹窗提示权限（声明的工具和副作用等级）
- 高风险插件（`Irreversible`）默认禁用，需用户显式启用
- 长期方案：WASM 沙箱隔离

### 15.7 UI 与内核的耦合

**风险**：UI 直接操作 `PluginRegistry`，业务逻辑散落两端。

**缓解**：
- 强制纪律：UI 只通过 `PluginService` 和 Tauri command 操作内核
- Bridge Layer 保持薄，只做参数转换
- 内核 API 独立测试，不依赖 UI

---

## 附录 A：关键类型索引

| 类型 | 位置 | 职责 |
|---|---|---|
| `Context` | cordis-core | 服务载体 + Effect 账本 + 事件总线 |
| `EffectEntry` | cordis-core | 一次可逆修改的记录 |
| `Disposer` | cordis-core | 逆操作 trait |
| `Fiber` | cordis-core | 插件运行实例 |
| `Plugin` | cordis-core | 插件接口（inject / apply / dispose） |
| `ScopeKey` | cordis-core | 作用域身份 |
| `EventBus` | cordis-core | 四种分发语义 |
| `Session` | session-store | 会话事件日志 |
| `SessionEvent` | session-store | 事件类型（可扩展） |
| `PluginManifest` | plugin-api | 插件元数据 |
| `ToolSpec` | plugin-api | 工具规格 |
| `SideEffect` | plugin-api | 副作用等级 |
| `SabiToolAdapter` | plugin-host | FFI → Tool 适配器 |
| `PluginRegistry` | plugin-host | 多版本管理 |
| `PluginService` | plugin-service | 安装/卸载/更新编排 |
| `LockFile` | plugin-service | 已安装插件契约 |
| `MarketClient` | plugin-service | 市场客户端 |
| `Kernel` | kernel-agent | 主循环 |
| `Middleware` | kernel-agent | 洋葱模型中间件 |
| `WaterfallDecision` | kernel-agent | Waterfall 决策 |
| `AppState` | desktop | 桌面端共享状态 |
| `PluginUiState` | plugin-host | UI 展示的插件状态 |
| `PluginStateChange` | plugin-host | 状态变化广播 |
| `Tool` | plugin-api | 宿主侧工具 trait（不进入 C ABI） |
| `CallCtx` | plugin-api | 工具调用的宿主侧上下文 |
| `ToolError` | plugin-api | 工具错误类型 |
| `Service` | cordis-core | 服务标记 trait |
| `ToolSource` | kernel-agent | 工具目录服务（plugin-host 实现） |
| `LlmService` | kernel-agent | 模型服务 trait |
| `ApprovalResolver` | kernel-agent | 审批请求 / 决策 |
| `ApprovalRequest` | kernel-agent | 审批请求载荷（camelCase 序列化） |
| `SessionManager` | session-store | 多会话管理 |

## 附录 B：参考实现

- **cordis-rs**：cordiverse/cordis（Koishi 4 TS 内核）的 Rust 重写（dshbox/cordis-rs，MIT 双版权，pre-1.0 活跃开发）。cordis-core 含 Context、Effect（generation 清理账本）、Events（emit/parallel/query/waterfall 四原语）、Fiber（六态收敛）、Service（字符串名 + realm 槽）、依赖索引与反应式收敛、Registry、observation。**定位为参考实现而非移植对照**；其服务键是字符串名而非 TypeId，与我们 v2.1 的分歧见 [docs/references/cordis-rs-notes.md](../references/cordis-rs-notes.md)
- **rutis**：cordis 范式的另一 Rust 实现（eric8810/rutis，MIT 双版权，MSRV 1.85）。五支柱同源同名；服务键同为 TypeId 路线（TypeId 主键 + 可选限定名，`?Sized` trait 对象经 `Box<Arc<T>>` 擦除），Fiber 六态 + 意图邮箱驱动 + 依赖等值合并重载可直接对表；无 FFI/ABI、无事件溯源，阶段三与会话日志无参考。借鉴点与冲突见 [docs/references/rutis-notes.md](../references/rutis-notes.md)
- **dsh-rs**：DeepSeek Harness 的 Rust 完整移植
- **Cordis 论文**：时空可组合性的理论基础（效应 / 余效应）
- **Tauri 2 官方文档**：IPC、事件系统、插件系统
- **hashicorp/go-plugin**：子进程插件方案（Go 生态的替代路线）

## 附录 C：关键设计纪律速查

1. **`plugin-api` 的 FFI 边界只暴露 C ABI + JSON**；宿主侧 Rust trait（`Tool`、`CallCtx` 等）可定义其中，但绝不进入 C ABI
2. **所有 `Context` 修改走 `effect()`**，携带逆操作
3. **永不 dlclose**，逻辑卸载 + 物理保留；唯一例外是"加载即失败"（init 从未执行、无代码指针流出）可立即释放
4. **FFI 调用放 `spawn_blocking`**，插件内 `catch_unwind`
5. **先写 `ToolCall` 事件再执行工具**，崩溃可恢复
6. **UI 只通过 `PluginService` 操作内核**，Bridge Layer 保持薄
7. **多版本并存**，回退只需切换 `active` 指针
8. **更新失败自动回滚**，旧版本永远保留
9. **`Irreversible` 工具必须审批**，审批结果写入事件日志
10. **补偿工具必须幂等**，用 `call_id` 做去重键

---

*文档版本：v2.22（v2.22 补记：fsutil 守卫裁决一致性（冒烟复测澄清：守卫从未被绕过——`resolve_existing` 对 canonicalize NotFound 的路径改按最深存在祖先归一判根。第 4 轮实测修正措辞：守卫拒绝实为**三分支**，均为有意行为——①可解析但越界：报 outside the allowed roots（回显规范路径，本地调试有用）；②有祖先且根内但目标不存在：报原始 NotFound（打错字的正确提示）；③完全不可解析（如盘符不存在）：报 cannot resolve any existing ancestor（不泄露宿主路径）。三者都拒绝，fail-closed 不变；连带发现并堵上 `resolve_for_write` 缺失尾部 `..`/`.` 未归一、词法骗过 starts_with 的越界写口子）；tool-fs 1.2.1 / tool-edit 1.1.1 产物重建）；v2.21 补记：CircuitBreaker 按工具名隔离（冒烟测试反馈的全局连坐问题，§7.1 注）+ tool-fs 五工具 path 参数描述与实际必填对齐（10 处，产物已重建）；v2.20 补记⑩暴露声明 + 使用统计地基 + 闲置 UI 落点（提交 f6b1a44）：① plugin-api `Exposure` 枚举（always/on_demand/hidden，缺省 on_demand，serde snake_case；未知值报错与 side_effect 同策略）追加进 ToolSpec——manifest JSON 可选字段零 ABI 破坏，PLUGIN_ABI_VERSION 不动；always>10 预算告警挂在 RegistryToolSource::list_tools（merge_catalog 纯函数 + 单测：hidden 剔除 / 同名先声明者赢 / 被顶掉的 always 不计数，OnceLock 防刷屏）；② 直通态：list_tools 剔除 hidden、resolve 不过滤（⑫ 检索激活仍可达），行为与加字段前零变化；③ `Tool` trait 加默认方法 plugin_id()（宿主侧 trait 不进 C ABI，默认 None = 内置替身/测试 mock 零改动；SabiToolAdapter 覆写 = PluginInstance::id() 即 manifest.id）；SessionEvent::ToolCall 补 `#[serde(default)] plugin_id: Option<String>`——**与 exposure 文档 §3.4 的 `plugin_id: String` 有意偏差：内置替身没有插件身份，Option 才是事实形状**，旧日志解析为 None（event/tests.rs roundtrip）；desktop 新 usage.rs = `<数据根>/usage.sqlite3` 宿主自有（两表同 §3.1/§3.2，tool_usage 含 ewma_score 预留列供 ⑫ 积累）：runner 投影到 ToolResult 时 record() → mpsc → 后台任务 5s 批量刷盘，全部句柄 drop 收尾刷一次，库打不开降级为丢弃；隐私只记工具名/插件 id/时间戳；④ desktop 新 idle.rs = IdleLevel 六级 + IdleUnit 9 档 + classify/filter_key 逐行对照 §8.6/§8.7 全表单测；**有意偏差：never_used 的 value 编码安装月数档（None=两周内/1/3/6=半年多）**，前端 IdleBadge 按 locales idle.* 分档渲染"多久没用"（永不显示裸天数），never_used 黄档对齐 §9.4（修正壳批次红档）；试用期 14 天 = just_installed 档（fresh 无徽章自然隐藏）；filter_key（recent/gt1m/gt3m/gt6m 嵌套）后端裁决，闲置档与运行状态分两排 chips（ListPage subFilters 模式扩展，UI shell doc §3.1 已记）；⑤ `plugin_list_with_usage` = InstalledPlugin × plugin_times 的 attach 投影（builtin 开发态插件不评估闲置）；批量 `plugin_batch_set_enabled`/`plugin_batch_uninstall`（BatchResult 逐条成败不回滚）；InstalledTab：闲置徽章 + 第二排 chips + 多选批量栏（批量卸载内联二次确认列插件名，审批模态仍是唯一全局阻塞层）+ 顶部提示条（§9.3，§8.8 只在主动打开本页出现无 toast）；停用落盘核对：startup_reconcile 确有 .filter(enabled) 无需改动；**搮车修复（提交 9a5d39c，用户报"会话不通"）：DeepSeek 等 OpenAI 兼容接口强制 function.name ∈ ^[a-zA-Z0-9_-]+$，自家 22 个带点号工具名全被 400 拒收**——memory-sqlite 1.1.0（memory_get/put/list/search/delete/undo，提前落实 §4.4 命名方向，⑪ 剩 memory_write/read 对齐 + exposure=always）、tool-fs 1.2.0 / tool-git 1.1.0 / tool-edit 1.1.0（下划线全量改名 + undo_tool 同步，git.undo_commit→git_undo_commit）、tool-mcp 1.1.0（子工具 mcp.<server>.<tool> → mcp_<server>_<tool> 按首个下划线拆回，server 名字符集收紧 [a-z0-9-] 禁下划线保证无歧义）；版本真源 build_plugins.sh 同步 bump，集成测试版本串/断言同步，plugins/ 旧点号产物清重建；同提交修 tool-fetch SSRF 拒绝用例 ENV_LOCK 进程级 env 竞态（真实触发）；设置面板拆分触发点（用户 2026-09-30 确认）钉进 UI shell doc §3.2；验证 workspace cargo test 31 目标全绿 + clippy 零告警 + desktop 33 测 + pnpm build + check_length OK；v2.19 补记⑨上下文管理收口落点（提交 92f298b）：v2.19 补记⑨上下文管理收口落点（提交 92f298b）：compact.rs 配置驱动重写（CompactConfig 共享 RwLock 热换入 / 三策略 / 预算制切点压到阈值 50% / 摘要失败降级滑动 / suggested_threshold 分档建议）；settings.json `context` 区（windowTokens 预设+自定义，未知回落 32k 保守保护 / thresholdPct / strategy 与 kernel-agent CompactStrategy 同形复用）；SessionEvent::ContextCompacted（摘要存全文支撑折叠重建，UI 投影截 200 字符；derive/state 折叠留尾部 kept_messages；RequestPayload::compacted 带回，重试幂等守卫）；context_undo_compact（rewind_to+fork 原地换入，其后轮次一并回退）；agent://context-usage 每 step 推送 + ContextProgressBar 接线、agent://context-compacted → SystemNotice [查看详情][撤销] 真接线（pushRawSystem 原文条模式）；前端 ContextSection 替换壳批次占位，UI shell doc §3.2/§4 扩表；验证 workspace cargo test 全绿 + desktop 25 测 + clippy 零告警 + pnpm build + check_length OK；v2.18 补记壳批次（UI 骨架）落点（提交 a2c82a5）：落地规则文档 docs/design/UI shell and patterns.md（§2 导航模型 / §3 模式库（列表页 / 设置分区 / 会话内系统消息 / 徽章+进度条）/ §4 事件→UI 流向图（功能批次只扩表）/ §5 弹层规则 / §6 design tokens / §7 扩展规则）；代码——App.vue 顶层 4 视图定稿（对话/插件/记忆/设置，>4 再切侧边栏、暂不引 router，chat v-show 保活）、patterns/ListPage 列表页模式（已安装页+记忆页首两实例）、chat/SystemNotice + store system 类别（存 i18n key，pushNotice 唯一生产入口）、IdleBadge（六级→灰/黄/红，IdleInfo 结构化契约）+ ContextProgressBar（绿/黄/红，null 不渲染）、设置面板拆 settings/ 区组件（分区一一对应，context/memory/developer 占位=⑨⑪⑫同批替换）、PluginMarket 291→54 行（DiscoverTab/InstalledTab + plugin-helpers，check_length.sh 纳入 .vue）、style.css --lw-* tokens（唯一样式事实源）；搭车托盘菜单 i18n（sys-locale 探测 + 改语言即时重挂菜单）；验证 pnpm build + desktop cargo test 23 过 + check_length OK；v2.17 排期重排（用户 2026-09-29 确认）：两份新配套文档入库——docs/design/Tool exposure strategy and usage statistics.md（工具暴露与使用统计）与 docs/design/Context and Memory.md（上下文与记忆管理）；排为壳批次（UI 骨架：导航模型定稿 / 页面模式库 / 事件→UI 流向图 / 弹层规则 / design tokens，PluginMarket 拆分顺带）+ ⑨ 上下文管理收口 P0（LlmService 窗口来源先行 / compact 配置驱动三档策略 / 压到阈值 50% / ContextCompacted 事件 + rewind 撤销 / 实时进度条）+ ⑩ 暴露声明 + 使用统计地基 + 闲置 UI（ToolSpec exposure 字段零破坏追加 / ToolCall 补 plugin_id / usage.sqlite3 宿主自有 5s 刷盘 / IdleLevel 人类尺度文案 / 筛选批量）+ ⑪ 记忆注入 + 记忆管理 UI（Memory-Inject 注 system prompt 永不压缩 / memory.put 同步落盘 / 记忆管理页）+ ⑫ EWMA 打分截断 + search_tools 元工具 + 调试面板 + ⑬ 智能层（向量/隐式提取/自动停用，看需求）；进度看板独立 docs/design/ROADMAP.md（完成打钩，每批同步）；§14 补记阶段八⑧WASM 加载器落点：PluginLoader trait + WasmLoader（wasmtime 34，.wasm 扩展名分发、manifest 同构、locowrk.log 单导入枚举能力面、Store limiter 64MiB、epoch 时间预算按次重置、宿主 alloc/free 缓冲协议、输出越界拒绝）；instance 拆 mod/native（PluginBackend trait）；desktop find_artifact 认 .wasm；WAT 三态单测 9 + 集成 2；§5.8 落地注记；v2.15 补记阶段八⑦第三批官方内置插件落点：tool-git@1.0.0（porcelain 机读解析 / commit 可撤销 / push 审批 / GIT_* 环境重定向剥离）+ memory-sqlite@1.0.0（固定路径记忆库，写操作按宁严勿松收紧为 Compensable）+ tool-mcp@1.0.0（mcp.json 加载期发现冻结、缺省 Irreversible、懒重连、fail-soft、examples 测试替身）+ tools_batch3 集成测试；v2.14 补记阶段八⑥第二批官方内置插件落点（用户排期重排：⑥=plugins.md 第二批，WASM 顺延⑦）+ 同日安全底线（fsutil 允许根 canonicalize 校验、fetch SSRF 防线、fs.restore 判 Irreversible）：tool-fs@1.1.0 写入三件套（write 备份先行 / delete / restore 来源校验）+ kernel-agent compensation.rs（§6.3 补偿执行器，undo 契约 {"call_id","args","output"}、新 undo call_id、账本幂等）+ tool-shell@1.0.0（白名单+denylist+元字符拒绝+无 shell spawn）+ tool-edit@1.0.0（replace/lines/严格 diff，跨插件 undo）+ ctx-compact（agent/request 摘要压缩，切点对齐 User 起点，fail-open，desktop 已注册）；§14 补记阶段八⑤落点：托盘（左键显窗 / 菜单显示主窗口与退出 / 关窗隐藏进托盘）+ tauri-plugin-global-shortcut 全局快捷键切换主窗口（settings.json `shortcut` 字段级缺省 Ctrl+Shift+L、设置面板改键保存即重注册、启动失败降级告警）+ 数据目录改名 ~/.agent-kernel → ~/.Locowrk（直接改名不迁移，§11.3 与三个 README 同步）；§14 补记阶段八④d/④e 落点：Fallback 矩阵收尾——v1 极简 manifest 投影全 None、坏 manifest 不崩已装列表、展示投影跟随 active 指针回归；`locowrk new plugin` 脚手架——scaffold.rs 四件模板（§8.1 全量 v2 manifest + §8.2 占位图标）+ 名称清洗 + plugin-api 路径探测，tmp/ 实测独立 cargo check 通过，service/tests 拆 tests/{mod,lifecycle,fallback}；v2.11 补记阶段八④a–④c 落点：plugin-api manifest v2 解析层——LocalizedString/LocalizedOrString/Author/PluginOrigin 追加字段全 serde(default) 且 None 序列化省略、SideEffect 解析双形态而序列化维持 tagged、ToolSpec 宽容解析（description 多语言压平、side_effect 缺失按 Irreversible）、manifest 级 reversible 不设字段；plugin-host IconLoader + PluginInstance dll 路径；plugin-service display.rs 展示投影与 InstalledPlugin/MarketplaceItem 扩展、market/installer 全量 manifest 校验、坏 manifest 跳过；origin 官方内置/第三方分组数据面；前端 pickLocalized/PluginIcon/SideEffectBadges/PluginCard 与 origin 分组渲染；④d/④e 未开始；v2.10 排期更新：④ 重定义为插件元数据 v2 + 市场打磨——依 docs/design/Plug-in packaging.md，含 manifest v2 解析层 / IconLoader / 投影扩展 / PluginCard / Fallback 矩阵 / locowrk new plugin 脚手架；⑤ 追加发布收尾：数据目录 ~/.agent-kernel → ~/.Locowrk；v2.9 补记阶段八③第一批工具落点：§7.1 mw-audit 宿主侧中间件与 JSONL 审计落盘、tool-fs / tool-fetch 真 C ABI 插件与副作用标注；v2.8 补记阶段八②真实模型接入落点：§7.2 LlmRequest 携带工具目录快照与 ToolSource::list()、genai 三合一适配器与设置驱动分发；v2.7 补记阶段八①最小设置面板落点与 API Key 不出 IPC 纪律、阶段八用户排期与 LLM 方案一选型；v2.6 补记阶段七实现落点：§11.2 PluginService 编排与 ctx.effect 原语、市场=本地目录、install 带版本参数、广播收敛到编排层；§13.3 市场 UI 落点、§13.4 plugin://state-changed 载荷）*
*最后更新：2026-09-29*
*前序版本：v2.5（补记阶段六实现落点：§12.2 agent://approval-request 通道与 ApprovalHub、§12.3 审批命令装配与内置演示工具、§13.2 审批弹窗实现备注）、v2.4（新增 §13.5 国际化需求）、v2.3（补记阶段四实现落点：§3.4 ScopedLayers 为 Context 服务层载体、§3.5 waterfall 返回值精化为 WaterfallOutcome、§7.1 审批门在 Kernel、§7.2 重试预算 step 内闭环）、v2.2（补记 §5.1 长度/缓冲区协议）、v2.1（含桌面端）、v2.0（含桌面端）、v1.0（纯内核设计）*
