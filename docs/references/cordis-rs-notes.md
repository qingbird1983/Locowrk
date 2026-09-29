# cordis-rs 借鉴笔记

> 评估日期：2026-09-29；本地副本 `F:\Warehouse\Github\cordis-rs`。
> 结论服务于 DESIGN.md v2.1 的实施（阶段一/二为主）。所有源码行号均经当日抽查核实。

## 1. 概况

- **仓库**：github.com/dshbox/cordis-rs；cordiverse/cordis（Koishi 4 TS 内核）的 Rust 重写。
  **注意**：本地另有一份 `F:\Warehouse\Gitee\cordis-rs`，是 2026-08-21 的旧快照（v0.1.0，单提交；与 Github 副本在 crates 下 15 个文件不同、127 个文件单侧存在）。**勿作参考，一切以 Github 副本（0.3.27）为准。**
- **许可**：MIT，双版权（dshbox contributors 2026 + Shigma 2024）。借鉴思路自由；整段复制须保留双版权声明。
- **工具链**：MSRV 1.88 / edition 2024（我们声明 1.85，注意点见 §4）；workspace 0.3.27，pre-1.0 活跃开发（评估当日仍有提交）。
- **结构**：4 个产品 crate（cordis-rs 门面 / cordis-core 内核 / cordis-timer / cordis-loader）+ 7 个示例 crate；约 4.5 万行（286 个 rs 文件）。
- **依赖极简**：cordis-core 运行时依赖仅 4 个——tokio（刻意不带 time feature，ADR 0036）、parking_lot、thiserror 2、futures；零 async-trait（原生 AFIT），无 DashMap。
- **工程纪律**：约 440 个测试（含约 40 个集成测试文件）+ trybuild 编译期误用测试 + loom 模型检测 + criterion 基准；docs/adr/ ADR 库 + 多份并发评审文档；missing_docs = warn、deny `todo!`/`dbg_macro!`。

## 2. 六个借鉴点

### 2.1 Effect 两阶段撤销（effect.rs:157-164，ADR 0010）

`DisposableList::remove` 在锁内只做 `map.remove` 领取所有权（remove-returns-ownership），清理在**锁外**执行；consume 型 move-only 的 `EffectRegistration`（`disarm`/`dispose` 返回 bool，重复领取返回 `Ok(false)`）实现 at-most-once。清理失败/panic 归一化为不透明 `EffectFailure`（kind + 诊断串，禁 downcast），防止清理路径二次爆炸。

**对我们的落点**：阶段一 effect.rs 重审 v2.1 的 `revoke(id)` 报错式 API——优先考虑领取式形态；若保留 `revoke(id)`，"先出账放锁再执行"纪律与他们 ADR 0010 完全同构，可放心沿用。

### 2.2 StatePublication 状态发布（fiber/mod.rs:92-139、479）

`Mutex` + 每态修订号 `[u64; 6]` + 全局 `sequence` + `Notify`：等待方靠修订号证明"两次观察之间没有状态落地"，`wait_for` 不会错过瞬时状态（这是 `ready()` 可靠性的基础）。**error 载荷拆独立字段**（`error: Mutex<Option<Arc<PluginFailure>>>`，fiber/mod.rs:479），状态枚举本体保持 `Copy`。

**对我们的落点**：v2.1 因 `Failed(FailureKind)` 带载荷把 AtomicU8 改成了 `RwLock<FiberState>`——照此拆分可以回到更轻的状态存储。阶段一 fiber.rs 落地时采用，并同步回写 DESIGN.md §3.3。

### 2.3 依赖漂移的提交次序（service.rs:291-445，ADR 0031）

`ServiceStore` 锁内**先提交受影响 Fiber 的义务集合**（`CommittedServiceDrift::commit`，在改槽位可见性之前），锁释放后才由 kick 驱动重检查——避免持锁执行用户清理/重载代码。

**对我们的落点**：阶段二 runtime.rs"卸载 A 时 B 自动卸载、重载 A 时 B 自动恢复"的次序论证直接参考此形态。

### 2.4 反向依赖索引 + 回退扫描（deps.rs）

`DependencyIndex` 只是加速层，权威始终在 Fiber 自身的依赖边上；索引不完整（complete 标志为 false）时回退全量扫描——避免缓存一致性成为正确性问题。

**对我们的落点**：阶段二 runtime.rs 的 `dependents` 表按同原则实现：索引可失真，正确性不依赖它。

### 2.5 测试基建（可选招募）

trybuild（API 误用必须在编译期报错的 UI 测试）、loom（并发交错模型检测，feature 门控 + 独立 CI lane）、criterion 基准。

**对我们的落点**：阶段 0 CI 暂不引入；阶段二并发代码成型后再评估纳入 loom/trybuild。

### 2.6 手写指针 Hash 的旁证（service.rs:110、258，events/types.rs 的 ListenerRegistrationId）

三处 Arc 身份键都用 `std::ptr::hash(Arc::as_ptr(..))` 手写实现，不用 derive——与我们 ScopeKey 手写指针 Hash 的决定（DESIGN.md §3.4）互为旁证。

## 3. 三项正面冲突（保留我们的设计，不照抄）

| 分歧 | cordis-rs | 我们（DESIGN v2.1） | 结论 |
|---|---|---|---|
| 服务键 | `Service::NAME: &'static str` 字符串名 + RealmKey realm 隔离；TypeId 只做"名字-类型合同"的一次性校验 | TypeId 双轨：具体类型 `provide`/`use_service` + trait 对象 `provide_dyn`/`use_dyn` | **保留我们的**。字符串键跨 crate 靠人工对齐，撞名只能靠运行期合同报错兜底；trait 对象键是跨插件解耦的刚需 |
| 覆盖注册 | 槽位活跃时重复 provide 报 `DuplicatePublication`；owner 已死则静默驱逐旧槽；撤销直接删槽、**不恢复旧值** | `provide` 捕获旧值，`Unprovide` 撤销时**恢复旧值** | **保留我们的**。插件热插拔需要"卸载 B 恢复 A"的语义；可借鉴"被驱逐旧值在锁外 drop"的处理 |
| waterfall | Mapper（改写）+ Around（洋葱，`Next` 只能调一次），"拒绝" = 不调 Next；**无显式决策枚举、无 Retry** | `WaterfallDecision::{Continue, Replace, Reject, Retry{delay}}` + 重试预算 | **保留我们的**。Agent 审批/改写/重试场景需要显式决策协议；Around 洋葱可作 Replace 的实现载体 |

补充记录：

- cordis-rs **无 fire-and-forget 的异步 emit**（其 emit 是顺序 await）；我们遥测场景需要，勿照抄其四原语划分。
- 其 Scope 只管事件路由（parent 指针链 + ptr_eq 线性走，无需 Hash），服务隔离走 ServiceRealm，两者显式正交；我们的 Scope 管服务作用域，两者概念勿混淆。

## 4. 对实施计划的影响

- **阶段一 cordis-core**：
  - effect.rs：撤销 API 按借鉴点 2.1 的 consume-token 形态重审（若保留 `revoke(id)`，NotFound 错误语义不变）；
  - fiber.rs：状态存储采用"状态枚举 Copy + error 拆独立字段"（借鉴点 2.2），替代 v2.1 的 `RwLock<FiberState>` 整体方案，实现时回写 DESIGN.md §3.3；
  - 其余按 DESIGN v2.1 不变。
- **阶段二 cordis-core**：runtime.rs 反应式重载按借鉴点 2.3 的锁内外次序实现；dependents 索引按借鉴点 2.4（加速层 + 回退扫描）。
- **阶段 0 CI**：暂不引入 trybuild/loom；阶段二并发代码成型后评估（借鉴点 2.5）。
- **MSRV 注意**：cordis-rs MSRV 1.88，我们声明 1.85。AFIT 自 1.75 起可用，但其个别代码段可能用到 >1.85 的特性——借鉴时逐段核对，必要时改写；**不**为绕版本引入 async-trait 改写（阶段一 API 均为 v2.1 已定形态）。

## 5. 许可与引用纪律

- 借鉴思路、次序论证、测试做法：自由，无义务。
- 整段复制代码：保留双版权声明（dshbox contributors / Shigma），并在文件头注明来源文件与提交版本；MIT 义务仅此。
