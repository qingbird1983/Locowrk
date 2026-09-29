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

### 11.3 插件目录布局

```
~/.agent-kernel/
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

**开发态与安装态是两套目录**：开发态是仓库内的 `plugins/{id}/{version}/` 与 `config/profile.toml`（§8），内核开发与测试直接按 profile 声明加载，不经市场与 lockfile；安装态是上面的 `~/.agent-kernel/`，存放用户从市场安装的插件。`PluginService.install_root` 指向安装态，两套目录互不重叠。

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

### 13.3 插件市场的 UI 结构

核心是**双页签 + 卡片网格**：

- **发现页**：搜索 + 分类 chips + 卡片网格（头像 / 星标 / 分类 / 已弃用徽标 / 安装按钮）
- **已安装页**：按分类分组，每行显示版本、状态、更新按钮、卸载按钮、回退按钮

更新检测逻辑：只检查已安装且仍在目录中的插件，比较本地版本与 registry 的 latest，仅前向 semver 更新。

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
| **阶段八（持续）** | 设置面板 + 快捷键 + 托盘 + WASM 加载器 | 完整产品化 |

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

*文档版本：v2.1（含桌面端；修订 v2.0 审查发现的机制性错误、Windows 平台问题与依赖图矛盾）*
*最后更新：2026-09-29*
*前序版本：v2.0（含桌面端）、v1.0（纯内核设计）*