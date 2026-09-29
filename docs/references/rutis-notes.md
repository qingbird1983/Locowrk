# rutis 借鉴笔记

> 评估日期：2026-09-29；本地副本 `F:\Warehouse\Gitee\rutis`。
> 与 [cordis-rs-notes.md](cordis-rs-notes.md) 并列：cordis-rs 提供"纪律与并发模式"，rutis 是与 Locowrk **同路**（TypeId 服务键）的平行实现，阶段二参考价值最高。所有源码行号均经当日抽查核实。

## 1. 概况

- **仓库**：eric8810/rutis；自述"Cordis 核心范式的 Rust 惯用实现（自 min-cordis 的 Rust 工作区独立成库）"。与 dshbox/cordis-rs 是**不同的实现**；与 TS 上游有逐条对拍文档（docs/cordis-spec-parity-*.md + tests/parity.rs，2647 行）。
- **许可**：MIT，双版权（Shigma 2021-present "Cordis" + eric8810 2026 "rutis"）。
- **工具链**：MSRV **1.85**（与我们声明一致，无版本摩擦）/ edition 2021。
- **结构**：6 crate——rutis（内核，15 文件 9061 行）、rutis-agent（agent 层 + TUI）、rutis-cordis（TS 桥接协议）、rutis-dsh（组合根）、aimux-llm、rutis-cli；全仓约 2.1 万行。
- **质量**：约 264 个测试函数（内核含 parity.rs 对拍测试）；30+ 篇设计/评审文档（多轮评审），代码注释逐处标注决策编号（D6/D21/D32…）与对拍出处；内核依赖仅 tokio + tokio-util + thiserror。
- **活跃度**：243 提交（2026-08-18 ~ 09-22），提交信息带多轮评审修复痕迹，非一次性灌入。

## 2. 与 Locowrk 五支柱对照

| 支柱 | rutis | 与我们的关系 |
|---|---|---|
| 服务键 | `TypeKey { type_id, qualifier: Option<Qualifier> }`（key.rs:47-51），TypeId 主键 + 可选限定名（Static/Dynamic 双轨，按字符串内容等值，key.rs:10-42） | **同路**（对比 cordis-rs 的 const NAME 字符串方案）；qualifier 是我们目前没有的"同接口多实例"通道 |
| trait 对象注册 | `T: ?Sized` 全程支持；存储用内层 `Box<Arc<T>>` 擦除（registry.rs:11-24） | 与我们 provide_dyn/use_dyn 的装箱思路同构，可直接对照 |
| Scope | isolate 按 **label 字符串**等值合并（TS 语义） | **分歧**：我们是指针身份 + 手写 ptr hash；模型不同 |
| Effect | 仅清理句柄（`Effect { Done, Disposer, AsyncDisposer, Many }`），EffectRecord 恰好一次 + 严格 LIFO + 双 panic 边界（effect.rs:43-153） | 名同实异：**无可逆修改/两阶段撤销**；只有清理机制细节可借鉴 |
| Fiber | 六态 Copy 枚举（fiber.rs:16-31）+ error 载荷独立字段（fiber.rs:38）；意图邮箱单驱动；`last_deps` 等值合并跳过重载；摘除三步协议 | **阶段二最高价值**，竞态处理全部有测试背书 |
| EventBus | emit/parallel/serial/waterfall 四语义齐全；waterfall 是 TS 式隐式 veto（不调 next 即拦截） | 四语义同构；决策协议**分歧**（见 §4） |
| FFI/ABI | 无（纯 Rust trait；跨进程桥接走 JSON-RPC over TCP） | 对阶段三零参考价值 |
| 会话 | 单 JSON 快照 + truncate/compact/sanitize，非事件溯源 | 对 session-store 无参考；仅回滚/修复思想可迁移 |

三实现收敛佐证：cordis-rs、rutis 的 Fiber 状态枚举都保持 `Copy`、error 载荷都拆独立字段——印证借鉴笔记 2.2 的方向。

## 3. 可借鉴点（按落地阶段）

### 阶段一（Context / Effect / kernel-agent）

1. **`Box<Arc<T>>` 类型擦除**（registry.rs:11-24）：`dyn Any` 不能直接装 trait 对象，内层 Box 再 downcast 回 `Arc<T>`——我们 `provide_dyn`/`use_dyn` 的现成实现手法。
2. **TypeId + 可选限定名**（key.rs:9-51）：将来需要"同接口多实例"（如多模型适配器并存）时的现成方案；Static/Dynamic 双轨 + 内容等值 Eq/Hash 的写法可直接抄。
3. **EffectRecord 清理机制**（effect.rs:43-153）：恰好一次、严格 LIFO、重复 dispose join 同一错误对象、panic 边界——补充我们两阶段撤销的执行细节。
4. **摘除三步协议**（ctx.rs:426-463 + registry.rs:88-101）：①标 `removing`（provider 清理期自访问豁免）→ ②取消/join 消费者 → ③按 **Arc ptr 身份**最终摘除（摘除窗口内被新 provide 替换的槽位不动）。
5. **Future 内联 panic 捕获**（event.rs:177-197，CatchUnwind）。
6. **工具三段管线 + 先发事件后执行**（rutis-agent/src/driver.rs:248-301）：先 emit `agent/tool-call` 事件 → waterfall 门控（fail closed）→ 执行 → post-execute 决策；与我们"先写 ToolCall 再执行"次序一致。

### 阶段二（Fiber / 依赖图）

7. **`last_deps` 等值合并**（fiber.rs:159-162、349-413）：依赖四元组集 `(PluginId, provider_gen, TypeKey, ScopeId)` 未变则跳过重载——消费者判定唯一事实源就是各 fiber 的 `last_deps`，不维护第二份反向索引（与我们 dependents 表的分工需在实现时协调）。
8. **意图邮箱驱动**（fiber.rs:259-301、575-643）：每 fiber 单驱动任务 + Intent mpsc 串行处理，apply 与卸载天然互斥；post_join/Settle 栅栏/退出排空处理全部 join 竞态；watch 常驻 receiver 保活的死等 bug 实录见 fiber.rs:66-74 注释。
9. **配置热更新 dry-run**（fiber.rs:855-919）：validate→build→validate 产物丢弃，通过后换代重装，两次终态检查封 dispose 竞态。
10. **对拍测试方法论**（tests/parity.rs）：与 TS 原版 spec 逐条对拍的测试组织方式；我们若要与 cordis 语义对拍可直接参考结构。

## 4. 冲突点（保留我们的设计，不照抄）

- **waterfall 语义**：TS 式隐式 veto（不调 next 即拦截）；我们的 `WaterfallDecision` 显式枚举 + 重试预算是刻意改进，勿被 CPS 续延风格带偏。serial 短路（`Ok(Some(v))`）同理。
- **isolate 作用域**：label 字符串等值合并；我们是指针身份。`scope_for` 父链回溯需改造后才可用。
- **Effect 定位**：仅清理句柄；照抄会丢失我们的可逆修改 + 两阶段撤销。
- **会话持久化**：单 JSON 快照；与事件溯源（只追加日志 + 游标 + fork）完全不同路线。
- **事件键**：TypeKey 类型作事件键、NAME 仅诊断；我们以事件名字符串为分发键，键体系不搬。

## 5. 对实施计划的影响

- **阶段一 cordis-core**：`use_service` 可见性规则对照 rutis（provider 须 Active 且未在摘除；访问方 Unloading/Disposed 不可见；provider 子树内自访问豁免，ctx.rs:167-199）；provide_dyn/use_dyn 用借鉴点 1。
- **阶段二 cordis-core**：fiber.rs/runtime.rs 以 rutis 为主参照（借鉴点 7/8），与 cordis-rs 的 ADR 0031 次序论证（锁内提交义务、锁外 kick）结合使用。
- **可编译性**：内核 crate（rutis）可裸 clone 编译；**rutis-agent 不可**（依赖仓库外 sibling checkout，CI 靠 symlink 绕过）——只参考内核，agent 层仅读代码。
- **MSRV 1.85 = 我们的声明**，借鉴代码无版本摩擦；edition 2021（我们 2024）复制时留意少量语法差异。

## 6. 许可与引用纪律

- MIT 双版权（Shigma + eric8810）；整段复制保留双版权声明并注明来源。
- **隐性价值**：30+ 篇评审文档记录了十几个并发边界 bug（逐个标注对拍出处与修复决策）——实现 Locowrk 阶段一/二时逐条对照排雷。
