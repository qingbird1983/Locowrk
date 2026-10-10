# A 域 · 根 workspace `crates/`（死代码 / 垃圾代码 / 优化）

**审计对象**：工作树当前状态（v2.145 未提交改动在位）。
**扫描面**：`crates/` 全部 8 个成员 crate（cordis-core / plugin-api / plugin-host / plugin-service / kernel-agent / kernel-bin / session-store / memory-tree）＝ 136 个 `.rs` 文件。
**跨仓验证面**（用于判「有没有消费者」）：`desktop/src-tauri/`、`plugins-src/`、`market/`、`config/`、`scripts/`、`docs/`、根 `Cargo.toml`；排除 `target/`、`node_modules/`、`.git/`。
**方法**：① `mod` 声明 ↔ 文件系统双向比对（孤儿模块）；② 591 个 `pub` 项 + 268 个 `pub` 字段全量枚举 → 全仓标识符词边界计数，按「出现次数 == 定义处数」判无消费者，再逐条人读确认；③ 8 个 crate 的 `Cargo.toml` 逐依赖 × 逐 crate 源码文本比对；④ 退役语义关键词全仓 grep；⑤ 热点路径人读。

**与 Lead `0-mechanical.md` 的关系**：重叠项不重复计数，统一收在 §5「复核结论」。本文件的价值集中在 Lead 未覆盖的 A 域深读项（§1–§4）。

---

## 1. 高价值 · 整簇零接线的子系统

### A1【死子系统 · 高】session-store 磁盘快照 + upcaster 演进链：整簇无生产入口

- **证据（定义面）**：`crates/session-store/src/snapshot.rs:16` `pub const SNAPSHOT_SCHEMA_VERSION`、`:21` `pub struct V1ToV2Upcaster`、`:34` `pub struct Snapshot`、`:41` `pub trait Upcaster`、`:48` `pub struct Upcasters`、`:59` `pub fn to_current`、`:84` `pub fn snapshot_at`、`:95` `pub fn from_snapshot`；`crates/session-store/src/lib.rs:29` 全部 re-export。
- **证据（消费面）**：全仓 grep `session_store::Snapshot` / `session_store::Upcaster` / `session_store::V1ToV2Upcaster` = **0 命中**；`snapshot_at` 全仓仅 `snapshot.rs:84`（定义）+ `snapshot/tests.rs:33,122`；`from_snapshot` 仅 `snapshot.rs:95` + `snapshot/tests.rs:35,56,104,124`；`Upcasters` / `V1ToV2Upcaster` 仅 `session-store` 自身。落盘面 `crates/session-store/src/persist.rs:80-98` `load_session` 是**逐行 JSONL 回放**，`:59-75` `rewrite_log` 是**全量文本重写**——两条路径都不碰 `Snapshot`。
- **为什么是死代码**：DESIGN §9.5 / `docs/design/DESIGN.md:1364-1374` 规划的「快照 + upcaster 加速 replay」写完了实现与单测，但恢复路径从未接上；唯一活着的只有内存态 `Session.snapshots: BTreeMap`（`session.rs:285,308-310,351-362`，那是另一套，**不要连带删**）。整簇 ≈ `snapshot.rs` 全 111 行生产代码 + `snapshot/tests.rs` 122 行测试常驻编译与维护面。
- **置信度**：高（计数 + 人读双证）。
- **建议动作**：三选一 —— ①在 `load_session` 接上「读到 `<id>.snapshot` 就 `from_snapshot` 起跳」（设计意图兑现）；②标注 `// RESERVED: DESIGN §9.5，未接线` 并挂 ROADMAP；③整簇删除（含 `lib.rs:29` re-export 与 `snapshot/tests.rs`）。
- **删除风险**：中。`docs/design/CHANGELOG.md:28`（v2.130）与 `docs/design/ROADMAP.md:872` 明确记档「快照 schema 1→2 + `V1ToV2Upcaster`」为**已交付里程碑**，`desktop/src-tauri/src/assets/gc.rs:2` 的 GC 通配面还写着 `sessions/*.snapshot`——删除等于撤回一条归档记录，需产品/Lead 裁定；接线则需一并改 gc 注释。

### A2【死子系统 · 高】cordis-core `serial` 监听器机制：零生产注册者、零生产派发

- **证据（定义面）**：`crates/cordis-core/src/event_bus.rs:27` `pub trait SerialListener`、`:71` `pub enum SerialOutcome`、`:88` `serials: DashMap<String, ListenerList<dyn SerialListener>>`、`:113` `pub fn on_serial`、`:155` `off` 的 serial 分支、`:198` `pub async fn serial(&self, key: &str, payload: &Value) -> SerialOutcome`；`crates/cordis-core/src/lib.rs:27` re-export `SerialListener, SerialOutcome`。
- **证据（消费面）**：全仓 `on_serial` 命中 6 处 = 定义 1 + `event_bus/tests.rs:121,122,123,136,249`（全测试）；`bus.serial(` 命中仅 `event_bus/tests.rs:125,144`；`SerialOutcome` 仅 `event_bus.rs` + 测试 + `lib.rs` re-export。对照：`waterfall` 生产可达（`crates/kernel-agent/src/kernel/request.rs:39,80,206`）、`emit` 生产可达（`crates/cordis-core/src/runtime.rs:300`、`crates/plugin-service/src/service.rs:180,244`）。
- **为什么是死代码**：`serials` map 在生产中永远是空表（无人注册），`serial()` 在生产中永不被调用；注释自述用途「顺序通知、可短路（§3.5 表：审计链、安全过滤器）」已被宿主侧中间件取代——见 `crates/kernel-agent/src/middleware/audit.rs:5`「mw-audit 当前以**宿主侧中间件**落地」。即：整类被架构演进架空，注释没跟上。
- **置信度**：高。
- **建议动作**：优先「删除整簇」（`SerialListener` / `SerialOutcome` / `serials` / `on_serial` / `serial()` / `off` 的 serial 分支 / `lib.rs:27` re-export，≈ 45 行 + 测试）；若为插件预留，则加 `// RESERVED` 并在 DESIGN §3.5 表注明「暂无注册者」。
- **删除风险**：低-中。`EventBus` 是 `cordis-core` 的公开面，删 trait 属 API 收窄；仓库内零消费者，但 DESIGN.md §3.5 表列了该类决策点（文档需同步修订）。

---

## 2. 高价值 · 哑 API / 幻影项

### A3【哑 API · 高】plugin-host `kv_put` 写入一个永不可读的仓库（静默无效的持久化承诺）

- **证据**：`crates/plugin-host/src/host_vtable.rs:16` `use std::collections::HashMap;`、`:21-23` `static KV: LazyLock<Mutex<HashMap<Vec<u8>, Vec<u8>>>>`（注释「kv_put 的真实后端；kv_get 待所有权协议后开放」）、`:36` `kv_get: host_kv_get`、`:65-73` `host_kv_get` 函数体**只有** `code::ERR_UNSUPPORTED`（常量 stub）、`:75-93` `host_kv_put` 真实 `KV.lock().insert(key.to_vec(), val.to_vec()); code::OK`。
- **证据（无读者）**：全仓 `kv_put` 调用方 = **0**（命中仅 `plugin-api/src/ffi.rs:97` 契约字段、`plugin-host/src/host_vtable.rs` 定义与注册、`host_vtable/tests.rs:7,38` 测试）；`KV` 静态只有 :87 一处写入，**没有任何读取点**（唯一潜在读者 `host_kv_get` 是常量 stub）。
- **为什么是垃圾代码**：这是一个「调用方拿到 `OK`、数据被分配后永久不可达」的写口——插件作者按 ABI 文档写 `kv_put` 会得到成功回执却毫无持久化/回读能力，属最坏的一类哑承诺（静默而非如实报错）。对照同一文件 `host_notify`（`:116-123`）在无桌面环境时**如实**返回 `ERR_UNSUPPORTED`，`host_kv_get` 同款——`kv_put` 是唯一不对称者。
- **置信度**：高。
- **建议动作**：让 `host_kv_put` 与 `host_kv_get` 一致返回 `code::ERR_UNSUPPORTED`（诚实化，保留 ABI 签名不动），并保留 `KV` 静态待 `kv_free` 追加后启用；或反向决定——此刻就补 `kv_free` 槽并开放 `kv_get`。**不要**只留注释。
- **删除风险**：低（改函数体返回值，签名与 ABI 冻结面不动；plugin-host 现有 2 条 kv 测试需按新语义改断言）。

### A4【幻影 ABI 码 · 中】`code::ERR_UNKNOWN_TOOL` 无任何生产者

- **证据**：`crates/plugin-api/src/ffi.rs:16` `pub const ERR_UNKNOWN_TOOL: i32 = -2;`（文档「请求的工具名在本插件中不存在」）。全仓 3 处命中：`ffi.rs:16`、`crates/plugin-api/src/ffi/tests.rs:24`、`crates/plugin-host/src/wasm/tests.rs:82`——**全部是测试**。
- **为什么是幻影项**：真实「工具名不存在」路径不产生它——`crates/plugin-api/src/plugin_side.rs:137-155` `state.call(...)` 返回 `Err` 时统一落 `code::ERR_CALL_FAILED`（`:151`）；宿主侧 `crates/plugin-host/src/instance/native.rs:97-107` 对非 OK 只做泛化映射（`format!("plugin returned code {code}")`），既不产也不认 `-2`。即：手写 C/Zig 插件按文档返回 `-2` 时，宿主只会打印裸码，模型侧拿不到「工具不存在」语义。
- **置信度**：高。
- **建议动作**：二选一 —— ①在 `native.rs` / `wasm` 返回码映射里为 `-2` 补一条人类可读报文（低成本，兑现「错误信息要教模型下一步」的项目纪律）；②保留常量但改注释为 `RESERVED（无生产者）`。**不建议直接删**：`ffi.rs` 顶部 :1-3 声明「冻结契约：这里的一切一经发布不可再改签名」，删常量等于 ABI 收窄。
- **删除风险**：中（ABI 面）。

### A5【pub 无消费者 · 中】cordis-core 的「UI 观测面 + 作用域助手」七个 `pub fn` 仅自家测试在用

- **证据（Runtime 观测面，注释自称 UI 来源）**：`crates/cordis-core/src/runtime.rs:52` `pub fn deps_of`（「调试/校验用」，全仓**零**调用，连自家测试都不用）、`:83` `pub fn state_of`（「UI 投影的运行态来源，§11.6」）、`:88` `pub fn failure_of`（「UI 投影用，§11.6」）、`:93` `pub fn provider_of`、`:98` `pub fn dependents_of`——后四者出现次数统计：crate 内生产引用 0，全仓外部引用 0，命中全部落在 `runtime/tests/lifecycle.rs` 与 `runtime/tests/reload.rs`。
- **证据（Context 作用域助手）**：`crates/cordis-core/src/context.rs:106` `pub fn provide_scoped`（**仅** `context/tests.rs:160,189,190`）、`:159` `pub fn use_service_scoped`（**仅** `context/tests.rs:161,193,197,199`）。对照：`provide_dyn_scoped`/`use_dyn_scoped`（trait-object 孪生体）有真实生产消费者（`crates/kernel-agent/src/kernel.rs:134,140,145` 与 `:161`）——**按具体类型的孪生体无人用**。
- **为什么是死代码**：§11.6 的「UI 投影」消费方从未落地（desktop 侧无任何引用）；`deps_of` 连测试都没用，是纯残留。
- **置信度**：高（含 `deps_of` 零命中，Lead M5 已独立复核该项）。
- **建议动作**：删 `deps_of`（零风险）；其余四项连同 `provide_scoped`/`use_service_scoped` 一并裁定——要么降到 `pub(crate)`/加 `// RESERVED: §11.6 UI 投影待接`，要么删除。`provide_dyn_scoped`/`use_dyn_scoped` **保留**。
- **删除风险**：低（仓库内零外部消费者；但 `Runtime`/`Context` 是内核公开 API，若 DESIGN §11.6 有排期需先对文档）。

### A6【pub 无消费者 · 中低】`LockFile::clear_pending_delete` 被 `startup_reconcile` 绕过

- **证据**：`crates/plugin-service/src/lockfile.rs:154-157` `pub fn clear_pending_delete(&mut self, dir: &Path) { self.pending_delete.retain(...) }`；唯一调用点是 `crates/plugin-service/src/lockfile/tests.rs:81`。真正的清理在 `crates/plugin-service/src/service.rs:277-287`：`lf.pending_delete.retain(|dir| match std::fs::remove_dir_all(dir) { Ok(()) => false, ... })`——**直接内联 retain，不走这个 API**。
- **为什么是死代码**：同一语义（清 tombstone）有两个实现，公开的那个零调用；注释「清理已成功删除的 tombstone」描述的行为实际由调用方自己实现，读者会被误导去改这个方法而无效。
- **置信度**：高。
- **建议动作**：让 `startup_reconcile` 复用 `clear_pending_delete`（按 dir 逐个），或删除该方法（`pending_delete` 是 pub 字段，内联 retain 不算破坏封装）。
- **删除风险**：低。

### A7【pub 无消费者 · 低】`PluginRegistry::graveyard_len` 仅测试

- **证据**：`crates/plugin-host/src/registry.rs:210-216`，注释自述「graveyard 长度（**诊断/测试用**：确认逻辑卸载只挪不移除）」；命中仅 `registry.rs:211` + `registry/tests.rs:19,23`。
- **置信度**：高。**建议**：保留（有诊断价值）但降到 `#[cfg(test)]` 或 `pub(crate)`；**风险**：低。

### A8【pub 只被测试用 · 低】`Kernel::with_retry_budget` 仅测试调用

- **证据**：`crates/kernel-agent/src/kernel.rs:109-113` `pub fn with_retry_budget(mut self, budget: RetryBudget)`；唯一调用点 `crates/kernel-agent/tests/waterfall.rs:102`。`RetryBudget` 类型本身在生产可达（`kernel.rs:75,91,152` 字段与传递）。
- **关联**：与 Lead M3「Retry 中间件零接线」同源——重试预算的**外部调节入口**没有任何宿主使用（desktop 用 `RetryBudget::default()`）。
- **置信度**：高。**建议**：随 M3 一并裁定（接口保留则挂 ROADMAP，否则删）。
- **删除风险**：低。

### A9【幻影字段 · 中】`memory-tree::TriggerInput.anchor_source` 只写不读

- **证据**：`crates/memory-tree/src/trigger.rs:76` `pub anchor_source: AnchorSource,`；生产唯一写入点 `desktop/src-tauri/src/organize/mod.rs:273-275`（`let (anchor_tokens, anchor_source) = Self::anchor(session); ... TriggerInput { anchor_source, ... }`）；**唯一决策函数** `trigger.rs:89-99` `should_auto_organize` 完全不读该字段（只读 `tail_events` / `pacing` / `chunk_tokens` / `threshold_tokens` / `anchor_tokens`）。全仓无 `.anchor_source` 读取点：desktop 的观测面板自己另算一份（`organize/debug.rs:52-65` 调 `Self::anchor(session)` 后自 match）。
- **为什么是幻影**：字段被构造、被 serde 传递形状、却对唯一消费者（`should_auto_organize`）不可见——「锚点来源」这条事实在结构里兜了一圈又丢了，读者会误以为决策依赖它。
- **置信度**：高（crates 内该标识符出现 2 次 = 定义 + 测试构造，Desktop 侧 5 次全部是 `AnchorSource` 枚举与本地变量，无字段读取）。
- **建议动作**：删字段（`AnchorSource` 枚举本身仍被 desktop 用，保留）；或让 `should_auto_organize` 在 `Estimate` 来源时走更保守的分支（把字段变成真语义——需产品裁定）。
- **删除风险**：低（字段是 crate 内结构体，非 ABI；但需同步删 `trigger.rs` 测试构造里的两行）。

### A10【无用 allow · 低，待一条命令定论】`wasm/backend.rs:42` 的 `clippy::too_many_arguments` 疑为多余

- **证据**：`crates/plugin-host/src/wasm/backend.rs:41-51` `#[allow(clippy::too_many_arguments)] pub(crate) fn new(store, memory, alloc, free, call, shutdown, handle)`——计数为**恰好 7** 个值参数。clippy 该 lint 缺省阈值 7（`> 7` 才报），故该 allow 应属历史残留（函数曾更长）。
- **置信度**：中（阈值是 clippy 缺省约定，未在本仓实跑验证）。
- **定论命令**（**只读验证，不落盘修改**）：`cargo clippy -p plugin-host --all-targets --message-format short 2>&1 | Select-String too_many_arguments` —— 当前输出为 0 行不足以定论；需临时摘掉该 `#[allow]` 后复跑一次（届时若仍 0 告警则可删）。**建议**：交 Lead 在门禁批里顺带验证；**风险**：极低。

---

## 3. 中低价值 · 注释腐烂 / 重复实现

### B1【注释腐烂 · 中低】`persist.rs` 仍宣称旁车透出「整理模式」

- **证据**：`crates/session-store/src/persist.rs:166-168` 注释「…+ 模型覆盖透出（v2.58）+ **整理模式透出（M4）**：旁车一次读…」；但 `crates/session-store/src/persist/meta.rs:16-17` 明确记档「v2.137：`organizer` 整理模式键随 legacy 整理面退役——旧旁车里的存量键为幻影键，serde 忽略」，`SessionMetaFile`（`meta.rs:20-32`）**已无 organizer 字段**，`list_sessions` 读取列表（`persist.rs:169-175`）也无对应项。
- **为什么是垃圾**：旁白声称存在的数据通道已不存在（与 AGENTS v2.137「一切会话皆树」落地一致），后续读者会去找一个不存在的字段。
- **置信度**：高。**建议**：删「+ 整理模式透出（M4）」五字。**风险**：零。

### B2【注释腐烂 · 低】「三个 request 监听器共用」实为七个

- **证据**：`crates/kernel-agent/src/middleware/mod.rs:62-63` `/// waterfall 监听器的 Replace 收口（to_value 失败按 fail-open 放行）—— ToolExposure / Memory-Inject / ctx-compact 三个 request 监听器共用。` 实际 `replace_decision` 调用点 = **7 个模块共 9 处**：`base_system.rs:70`、`compact.rs:257`、`fragments.rs:79`、`instruction_files.rs:65`、`memory_inject.rs:283,333`、`tool_exposure.rs:332,375`、`tree_context.rs:45`（v2.139 提示词分层批新增 BaseSystemInject / InstructionFilesInject / FragmentInject / TreeDirectoryInject 后注释未更新）。
- **置信度**：高（`:64 pub(crate) fn replace_decision` 的 doc 与调用点计数不符）。
- **建议**：改成「七个 request 监听器共用」或删枚举、改「链上监听器共用」。**风险**：零。

### B3【注释腐烂 · 低】`kernel-bin` 头注释仍在说它用 `PluginService`

- **证据**：`crates/kernel-bin/src/main.rs:7`「阶段七补：profile.toml 加载、**PluginService 安装态目录与会话持久化**」；但 kernel-bin 全 crate 对 `plugin_service` 的引用 = **0**（唯一命中 `crates/kernel-bin/Cargo.toml:12` 的依赖声明，= Lead M1 的未使用依赖之一）。
- **置信度**：高。**建议**：删该句，或按 M1 删依赖时一并订正。**风险**：零。

### C1【重复实现 · 中低】`valid_session_id` 两份实现且规则不一致（跨 crate 潜在失配）

- **证据（两份）**：`crates/session-store/src/persist.rs:23-29`＝ `!id.is_empty() && chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))`；`crates/memory-tree/src/storage.rs:15-22`＝ `!id.is_empty() && id.len() <= 64 && chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')`（**无 `.`、多 64 长度上限**），且注释自称「（与 session_log 落盘命名同源收紧）」。
- **为什么是问题**：同名同责的安全守卫双份维护、规则已分叉：任何含 `.` 的会话 id 在 session-store 合法（日志能落盘）却在 memory-tree 被拒（**树投影静默失效**，`TreeStore::load/save` 返回 `None`/`Err`，`memory-tree/src/storage.rs:99-110`「投影可丢」会把它当不存在）。今日桌面 id 形态 `desktop-<毫秒>`（`persist.rs:23` 注释）恰好都合法，故**当前不可达**，属陷阱而非现网缺陷。
- **置信度**：高（代码双读 + 规则逐字符比对）。**建议**：下沉到 `plugin-api`（项目已有 `fsutil` 共享基座先例），单点定义 + 单测锁规则。
- **删除风险**：低。**附带**：`crates/memory-tree/src/material.rs:44` `clip(s,n)` 与 `crates/kernel-agent/src/middleware/memory_inject.rs:234` `clip(value)` 同居「截断」之名但语义不同（前者 trim+省略号、后者定长+`…(truncated)`），跨 crate 无法共享，仅登记为命名混淆，不建议为它建共享层。

---

## 4. 优化建议（按收益排序）

### D1【热点冗余克隆 · 高】7 个 request waterfall 每步无条件深拷贝 + 全量反序列化整个 payload

- **证据（7 个同构点）**：`crates/kernel-agent/src/middleware/base_system.rs:62`、`compact.rs:177`、`fragments.rs:62`、`instruction_files.rs:51`、`memory_inject.rs:263`、`tool_exposure.rs:278`、`tree_context.rs:32` —— 形状全为 `serde_json::from_value::<RequestPayload>(payload.clone())`。
- **代价面**：`RequestPayload`（`crates/kernel-agent/src/waterfall.rs:46-63`）含 `messages: Vec<Message>`（**整个会话未整理尾巴**，长会话数百 KB JSON）+ `tools: Vec<ToolSpec>`（全工具目录含 schema）。每个 LLM step 触发 7 次 `Value` 深拷贝 + 7 次全量 `from_value`；其中多数随后经 `replace_decision`（`middleware/mod.rs:64-69`）**再全量 `to_value`**。100K token 级会话 ≈ 每 step 多次 MB 级分配/解析。
- **设计意图被架空（关键）**：`crates/cordis-core/src/event_bus.rs:31-32` 明确写「要改写就克隆 payload 返回 `Replace`——**借用签名让 Continue / Reject / Retry 都不付克隆**」；但 7 个监听器**全部在真正决定是否改写之前**就完成克隆 + 全量解析（`memory_inject.rs:260` 仅多一道 `cfg.enabled` 布尔闸、`tool_exposure.rs:278` 的 cfg 判在克隆之后、`tree_context.rs:32` 解析完于 `:36` 立刻 `Continue`），所以「Continue 不付克隆」在实践中不成立。
- **置信度**：高（形状 7 处一致 + 类型定义人读）。
- **建议动作（落到函数）**：给 `WaterfallListener::handle` 增一条「先按 key 粗筛（多数监听器真正关心的只有 `session_id`/`tree_directory` 的有无）」的快路径，或把 `RequestPayload` 拆成 `serde_json::value::RawValue` 懒解析视图；最小改动版 = 在 `tree_context.rs:32`（唯一「只为读一个字段」的监听器）改用手工 `payload.get("tree_directory")`，并为 3 个「偶发改写」者（fragments / instruction_files / memory_inject）加前置判空后再解析。
- **风险**：中（涉及 waterfall 公共签名或 7 个监听器逐一改；有 `tests/prompt_layering.rs` 与各 `*/tests.rs` 可作回归网）。

### D2【O(n²) · 高】`SessionState::pending_slot` 反向全扫 → `apply` / `state_at` / `load_session` 二次方

- **证据**：`crates/session-store/src/session.rs:141-149`
  ```rust
  fn pending_slot(&mut self, call_id: &CallId) -> Option<&mut ToolCallMsg> {
      self.messages.iter_mut().rev().find_map(|m| match m {
          Message::Assistant { tool_calls, .. } => tool_calls.iter_mut().find(|c| &c.id == call_id),
          _ => None,
      })
  }
  ```
  调用点 `:110`（`ToolCall` 分支）与 `:121`（`ToolResult` 分支）。`find_map` 在**最近一条 assistant 不含该 id 时会继续往回扫到日志开头**（语义注释只说「最近一条 assistant 消息」）。上层：`apply` 由 `state_at`（`:351-362` 逐事件 fold）与 `persist.rs:92`（`load_session` 逐行回放）逐事件调用。
- **代价**：每次 `apply` O(消息数) → n 个事件回放 O(n²)；`SNAPSHOT_EVERY = 64`（`:16`）只把常数压低，不改变量级。长会话（数千事件）上是真实的启动/replay 热点（desktop 每次打开会话都走 `load_session`）。
- **置信度**：高（代码人读 + 调用链完整）。
- **建议动作（落到函数）**：`SessionState` 增 `open_calls: HashMap<CallId, (usize, usize)>`（messages 下标 + tool_calls 下标）在 `apply` 的 `AssistantMessage` 分支填充、`ToolResult` 消费后移除；或在 `pending_slot` 命中第一条 `Message::Assistant` 后即停（`find_map` → `find` + 内部 `find`），后者一行改动即可恢复 O(1) 均摊。
- **风险**：中（改折叠语义需 `session/tests.rs`（323 行）与 `kernel-agent` replay 断言背书；后者是低风险修法）。

### D3【热点冗余 clone/序列化 · 中】`tool_exposure`：每请求克隆整个工具目录 + 全目录 JSON 序列化两次

- **证据（克隆）**：`crates/kernel-agent/src/middleware/tool_exposure.rs:204` `let mut kept: Vec<ToolSpec> = always_specs.iter().map(|s| (*s).clone()).collect();` 与 `:217` `kept.push(spec.clone());` —— 每个入选 `ToolSpec`（含 `description` 与 `schema: Value`）整份克隆；钉死路径 `:309-318` 再克隆一遍。
- **证据（序列化）**：`:154-156` `fn spec_tokens(specs: &[ToolSpec]) -> u64 { serde_json::to_string(specs).map_or(0, |s| s.len() as u64) / 4 }`；调用点 `:319`（pin 路径：`spec_tokens(&req.tools).saturating_sub(spec_tokens(&kept))`）与 `:347`（正常路径）——**每请求把整个工具目录序列化成字符串两次**，只为求长度。
- **置信度**：高。**建议**：`spec_tokens` 改求和式估算（`spec.description.len() + serde_json::to_string(&spec.schema)?.len()` 逐项累加，或复用已算过的目录总量），`select` 返回 `Vec<usize>` 下标而非克隆 `ToolSpec`；`ExposureSnapshot` 只记名字，已足够。
- **风险**：低（纯函数，`tool_exposure/tests.rs` 387 行可回归）。

### D4【重复磁盘/JSON 反序列化 · 中】`search_sessions` 把每个会话文件读两遍、JSON 解析两遍

- **证据**：`crates/session-store/src/persist.rs:232-289` `search_sessions` 第一步 `for meta in list_sessions(dir)`（`:239`）；而 `list_sessions`（`:125-194`）**已经**逐个 `File::open` 并逐行 `serde_json::from_str::<SessionEvent>`（`:144-164`）以统计 `events` 与抓标题。随后 `search_sessions` 对每个会话**再次** `File::open` + 逐行 `from_str`（`:243-274`）。另 `:257-258` 每条消息都 `ci_fold`（`:293-301` 分配 `Vec<(char, usize)>`）**再**收集一份 `Vec<char>`，双重分配（`:258` 的 `chars` 与 `folded` 并存）。
- **置信度**：高。**建议**：给 `list_sessions` 加一个可选「顺带返回正文行」的回调/变体，让搜索共用同一遍 IO+解析；`ci_fold` 直接产出 `Vec<char>` 并把回映射偏移存进并行数组（或改 `folded.iter().map(|(c,_)| *c)` 为借用迭代，去掉 `chars` 分配）。
- **风险**：低-中（desktop 有 `session_search` 命令消费方，需回归搜索摘录断言）。

### D5【职责混合 · 中】`crates/session-store/src/session.rs`（415 行）= crates 内唯一超 400 行的生产文件，三个职责拧在一起

- **证据（规模口径）**：按 `scripts/check_length.sh` 同口径（含空行）复算，`crates/` 内 >400 行仅 5 个文件，其中 4 个是测试文件（`kernel/tests.rs` 853、`memory_inject/tests.rs` 492、`tool_exposure/tests.rs` 421、`compact/tests/mod.rs` 405）；`session.rs` 是**唯一生产文件**（415）。
- **耦合症状（不是行数问题）**：同文件里塞了三类互不依赖的关注点——
  ① **事件→状态折叠**：`SessionState::apply`（`:80-139`）+ `pending_slot`（`:142-149`，见 D2）+ `compact_messages`（`:267-278`）；
  ② **事件切片→消息投影**：`project_messages`（`:164-266`，含 G2 悬空调用诚实闭合 + 决策表，纯函数、不碰 `Session`）；
  ③ **会话生命周期**：`Session::new/append/state_at/derive_messages/derive_messages_from/rewind_to/rewind_before/fork`（`:290-395`）+ `Default` + `backfill` + 一条**冗余 re-export**（`:409-412`，见 §5 复核）。
  症状表现：`project_messages`（②）是「请求装配唯一真源」的独立纯逻辑，却与状态机（①）共享文件作用域，且 ③ 的 `#[allow(unused_imports)]`（`:411`）也是同一文件在代替 `lib.rs` 承担导出职责——三件事的修改者会在同一文件里互相撞车（已被项目自己的 400 行纪律命中）。
- **置信度**：高（人读 + 与项目 `check_length` 基线一致）。
- **建议动作**：把 ② 抽为 `src/session/project.rs`（`project_messages` 103 行 + `compact_messages` 12 行 + 其专属测试），③ 的 re-export 删掉；`session.rs` 预计回落到 ~300 行内，且顺手消掉一个 `#[allow]`。**不要**按行数硬拆 ①（状态机与 `Session.snapshots` 强耦合）。
- **风险**：低（模块内拆分，`session_store::` 外部路径不变——`project_messages` 本就是私有函数）。

---

## 5. 与 Lead `0-mechanical.md` 的重叠项（复核结论，不重复计数）

以下项 Lead 已列，本域独立复核**全部成立**，此处只补证据/边界，避免报告间互相矛盾：

| Lead 项 | 本域复核补充 |
|---|---|
| **M1** 未使用依赖 3 处 | 成立。补一条**设计面反向证据**：`docs/design/DESIGN.md:1266` 明写依赖图 `cordis-core → plugin-api`，故 `crates/cordis-core/Cargo.toml:9` 这一行是「**文档要求但代码零使用**」——删依赖须同时改 DESIGN §8 依赖图（`docs/design/DESIGN.md:1264-1279`），否则文档与清单反向不符。`kernel-bin/plugin-service`、`memory-tree/tokio(dev)` 两行可直删。 |
| **M4/M5** `CallCtx` 取消设施 + `count_user_messages` + `deps_of` | 全部成立。补：`CallCtx.cancelled`（`crates/plugin-api/src/tool.rs:180,189`）全仓**从无置位路径**，故 `is_cancelled()`（`:193`）恒 `false`——与 §1 A5 的 `Runtime` 观测面同属「注释承诺 > 实现」族。`count_user_messages`（`crates/memory-tree/src/manager.rs:272-276`）我确认是纯转发，且 `material::user_count` 已由 `crates/memory-tree/src/lib.rs:33` 直接导出，转发层零收益。 |
| **M6** `session.rs:411` `#[allow(unused_imports)]` | 成立。补决定性证据：`crates/session-store/src/lib.rs:22` **已经**从 `event` 直接导出 `TurnId, TurnOutcome`；`session` 是私有模块（`lib.rs:15` `mod session;`），故 `session.rs:412` 的 `pub use` 连内部可达路径都不构成 → 纯冗余，删除零风险（`#[allow]` 同时消失）。 |
| **M9** 宿主集成测试道具 ×5 | 成立。补计数：`crates/plugin-host/tests/{echo_plugin,fs_fetch_plugins,tools_batch2,tools_batch3,wasm_plugins}.rs` 内 `build_lock`/`workspace_dir`/`artifact_name`/`build_and_stage` 各 **5** 份、`load_activate` **4** 份，逐函数同形（仅 `temp_root` 前缀 tag 不同，如 `tools_batch2.rs:97` `locowrk-batch2-{tag}`）。收敛路径照 `crates/kernel-agent/tests/common/mod.rs` 先例。 |
| **M7** 清洁面 | 我按 crates 口径独立复跑，结论一致（详见 §6）。 |
| **M2** ctx-compact 零接线 | 同意「不能整簇删除」的判断。补一条：`crates/kernel-agent/src/middleware/mod.rs:16` 与 `memory_inject.rs:15` 都已注明「桌面链 v2.137 退役，headless 保留」——**退役语义注释本身是干净的**，问题只在代码是否保留，不在措辞。 |

---

## 6. 已核查为健康的部分（覆盖率声明，避免 Lead 误判漏查）

以下面我**逐项跑过、结论为「没问题」**，未列入上文即为已确认无发现：

1. **孤儿模块**：`crates/` 全部 136 个 `.rs` 与所有 `mod` 声明双向比对——**无孤儿文件、无 `mod` 悬空**。`crates/kernel-agent/tests/*.rs`、`crates/plugin-host/tests/*.rs` 未被任何 `mod` 声明属正常（它们是独立 cargo 集成测试目标，非孤儿）。
2. **退役语义残留**：`crates/` 内 grep `sandbox|unlockRisky|LOCOWRK_SHELL_ALLOW|LOCOWRK_SHELL_UNLOCK_RISK|DENY_HARD|DENY_RISKY|BASE_ALLOW|NO_BLANKET|organizer_mode|organizer|autoExtract|auto_extract|experiences|handoff` = **0 命中**（`organizer`/`legacy` 仅出现在 v2.137 退役注记与「legacy 会话 = 无树计划」这一**仍然成立**的语义里，见 `session-store/src/persist/meta.rs:16`、`kernel-agent/src/kernel/services.rs:125`）→ v2.145/v2.137 的 crate 侧退役是干净的。
3. **标记类垃圾**：`TODO|FIXME|HACK|XXX|DEPRECATED`（含 `*.toml`）= **0**；`unimplemented!` = **0**；`todo!()` = 0 处代码（唯一命中是 `plugin-host/tests/tools_batch2.rs:353,366` 的**夹具字符串内容**，属被测数据）。
4. **注释掉的代码块**：启发式（`^\s*//\s*(let|if|for|while|match|return|fn|pub|use|self\.|ctx\.|let mut)`）在 `crates/` = **0 命中**。
5. **`#[allow]` 掩盖面**：`crates/` 共 5 处——`plugin-api/plugin_side.rs:99,176`（宏内裸指针，注释已说明理由）、`plugin-host/wasm/backend.rs:42`（见 A10）、`kernel-agent/tests/common/mod.rs:4`（多集成测试目标共享助手模块，标准做法）、`session-store/session.rs:411`（见 §5）→ 无「用 allow 盖住大块死代码」的情况。
6. **幻影 pub 字段**：268 个 `pub` 字段全量计数，`crates/` 内引用次数 ≤2 的**仅 1 个**（`anchor_source`，见 A9）→ 不存在成片的「serde 字段写完没人读」。
7. **pub 项消费者普查**：591 个 `pub` 项（含 `pub(crate)`）全仓计数 → 全仓**零引用 3 个**（`deps_of` / `count_user_messages` / `is_cancelled`）、「crate 内零生产引用 且 全仓零引用」16 个（已逐条人读，其中 4 个是测试助手模块内的定义，其余收在 §1–§3）。
8. **依赖逐个核对**：8 个 crate × `[dependencies]`/`[dev-dependencies]` 全部条目（含 `sha2`/`jiff`/`base64`/`libloading`/`wasmtime`/`dashmap`/`semver`/`async-trait`/`tracing` 等）逐一在**本 crate 源码文本**里比对 → 仅 3 处 0 命中（= Lead M1），其余全部有真实 `use`/路径/宏调用点。
9. **规模纪律**：`crates/` 内 >400 行文件 5 个（含空行口径），4 个为测试文件，唯一生产文件 `session-store/src/session.rs` 已单列 D5。
10. **workspace 成员一致性**：根 `Cargo.toml:3-22` 成员与实际目录一一对应，`plugins-src/tool-sandbox` **已从 members 移除且无残留引用**（与 AGENTS v2.145「整目录删」一致）。
11. **`#[cfg(test)]` 边界**：`crates/plugin-host/src/wasm/guests.rs`（115 行 WAT 夹具）与 `mod tests` 均正确置于 `#[cfg(test)]` 之后（`wasm/mod.rs:13-17`），未污染生产构建。

---

## 7. 不确定 / 待验证项（宁缺勿假，附定论命令）

| # | 待验证内容 | 确切检查命令（只读或最小改动） |
|---|---|---|
| V1 | A10：`wasm/backend.rs:42` 的 `too_many_arguments` allow 是否真属多余 | 临时注释掉 `crates/plugin-host/src/wasm/backend.rs:42` 后 `cargo clippy -p plugin-host --all-targets`；0 告警即可删（勿在审计批里落盘，交 Lead 门禁批）。 |
| V2 | A1：`Snapshot` 簇是否只是「尚未接线」而非「已废弃」 | 读 `docs/design/DESIGN.md:1360-1380`（§9.5 原文）与 ROADMAP 中「快照加速 replay」条目状态；若为 ✅ 已交付则应判**接线缺失（缺陷）**，若为空则判**预留**。 |
| V3 | A2：`SerialListener` 是否有仓外（第三方插件）消费者 | `rg -n "on_serial\|SerialListener" plugins/ market/ desktop/ --glob '!target'`（仓外插件为 C ABI 插件，本就不该用 Rust trait → 预期 0，但删除前值得一跑）。 |
| V4 | D1：payload 克隆的真实绝对成本 | 需实测（本次未跑）：在 `tool_exposure.rs:278` 前后打点，跑一轮 100K token 会话，量 `payload` JSON 字节数 × 7。属「必须有数字才好排期」的项。 |
| V5 | C1：含 `.` 的会话 id 在现网是否真的不可达 | `rg -n "desktop-|session_id" desktop/src/stores/*.ts desktop/src/lib/*.ts` 确认前端 id 生成式；若未来引入含点 id，C1 从「陷阱」升级为**真实缺陷**。 |

**未越界声明**：本次只读 `crates/` 之外的路径（`desktop/`、`plugins-src/`、`docs/`、根清单）用于 grep 验证；未修改任何源码；唯一写入文件 = 本报告。未运行 `cargo build/test/clippy` 全量（依指令交 Lead 门禁）。
