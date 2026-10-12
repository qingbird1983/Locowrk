//! Q1b（《Memory Learning Mechanism》§2.3 的 (b)「检索回执」）：**结果采集件**。
//!
//! **这块治什么**：注入侧（Q1a）已经能答"给了模型哪些记忆"，但答不了"模型后来自己查回
//! 来了什么"。本件把三个记忆检索工具的每次成功调用落成一行（`returned` + `keys`），
//! [`crate::recall_log::InjectStats::injection_yield`] 的分子因此才有出处——没有它，
//! 下一刀 Q9（L2 改排序）依旧只能靠体感判断"注入到底有没有用"。
//!
//! **形状 = [`plugin_api::Tool`] 的装饰器**，挂在 [`crate::tool_source`] 的解析出口。
//! 为什么只有这一个位置可用（另两条已实测排除，细节见 `recall_log.rs` 头注）：
//!
//! * 洋葱中间件看不见工具名——`kernel_agent::middleware::Next` 的 `tool_name` /
//!   `tool_spec` / `args` 三个访问器全是 `pub(crate)`，宿主自制的中间件只能拿到
//!   `Result<Value>` 而不知道它是哪个工具给的；放宽内核 API 面不属于观测批。
//! * 插件本体不可改——`memory_read` / `memory_search` 住在 `memory-sqlite` 插件里，
//!   经 FFI 与冻结 ABI，插件写不了宿主的观测表。
//!
//! 装饰器则一次接线同时覆盖**内置工具**（`memory_project_history`）与**插件工具**
//! （另两个），因为它们在 `resolve` 之后是同一个 `Arc<dyn Tool>`。
//!
//! **键位（`scope_id`）为什么现算而不从结果里读**：实测两侧结果里都没有可用的键位串——
//! `memory-sqlite` 的 `row_scoped_json`（`plugins-src/memory-sqlite/src/store.rs:129-140`）
//! 只给**层标** `scope: "global" | "project"`，不带 workspace 路径，而一次 `memory_search`
//! 还可以跨层命中，"这一行属于哪个键位"根本没有单一答案。所以这里与注入档用**同一个出处、
//! 同一个归一函数**：`(scope_of)(session_id)` 过 [`crate::workspace::scope_key`]，与
//! `MemoryStore::scope_of_session` 逐字同形 ⇒ 两档的 `scope_id` 可以直接对表。层标是另一个
//! 量，本表没有它的列，本批也不加（加列 = 已存在的库不会重跑 DDL，静默丢数据），需要时另开一档。
//!
//! **不变量 = 零行为变更**（这一刀的等价判据，逐条有测试盯着）：
//!
//! 1. 返回值逐字节透传——`Ok` 的 payload、`Err` 的变体都不动；观测写失败只 `warn`
//!    （`recall_log.rs` 不变量 2 同款），**绝不让一行日志把工具调用打成失败**。
//! 2. 工具身份透传——`spec()` / `plugin_version()` / `plugin_id()` 都返回内部工具自己的，
//!    否则使用统计的插件归属（纪律 ⑩）与版本回退对表都会被这层壳改掉。
//! 3. 只包装看得见的三个工具名，其余工具**原对象返回**（不在目录里加一层无谓的壳）。
//! 4. 只记成功：`Err` 不落行——"检索失败"与"检索到 0 条"是两个不同的事实，后者才是
//!    `returned = 0` 想说的（`memory_read` 的 `found: false` 是成功返回 0 条，照记）。

use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use plugin_api::{CallCtx, Tool, ToolError, ToolSpec};
use serde_json::Value;

use crate::recall_log::RecallLog;
use crate::workspace::{scope_key, WorkspaceResolver};

/// 采集面：结果里带"模型查回了什么"的三个检索工具（`ToolSpec.name` 同串）。
/// 新增检索工具就加在这里——**别加进 `recall_log` 的 SQL**，那边的分子按
/// 「非注入档的 keys 并集」算，不需要按工具白名单（会漂）。
const OBSERVED: [&str; 3] = ["memory_read", "memory_search", "memory_project_history"];

/// 该工具是否进采集面。
fn is_observed(name: &str) -> bool {
    OBSERVED.contains(&name)
}

/// 采集件要的两个句柄：**观测表 + 键位解析器**，绑成一个件而不是两个可选参数——
/// 只给 log 就只能把 `scope_id` 写成空串，而那与注入档的同名字段就不是一个含义了
/// （空串在注入档里是「全局偏好层」）。不给"接了一半"的形态留口子。
pub(crate) struct ReceiptSink {
    log: Arc<RecallLog>,
    scope_of: WorkspaceResolver,
}

impl ReceiptSink {
    pub(crate) fn new(log: Arc<RecallLog>, scope_of: WorkspaceResolver) -> Arc<Self> {
        Arc::new(Self { log, scope_of })
    }
}

/// 需要观测的工具 ⇒ 套一层壳；其余 ⇒ **原样返回**（同一 `Arc`，不多一次分配）。
///
/// `resolve` 每次调用都会走这里，所以判断只看 `spec().name`（一次字符串比较），不查库。
pub(crate) fn observe(tool: Arc<dyn Tool>, sink: Arc<ReceiptSink>) -> Arc<dyn Tool> {
    if is_observed(tool.spec().name.as_str()) {
        Arc::new(ReceiptObserver { inner: tool, sink })
    } else {
        tool
    }
}

/// `Tool` 装饰器：透传身份与结果，顺手在 `Ok` 分支落一行回执。
struct ReceiptObserver {
    inner: Arc<dyn Tool>,
    sink: Arc<ReceiptSink>,
}

#[async_trait]
impl Tool for ReceiptObserver {
    fn spec(&self) -> &ToolSpec {
        // 借内部工具的引用返回：壳不复制 spec，规格漂移（描述 / schema / exposure）
        // 不可能发生在这层。
        self.inner.spec()
    }

    fn plugin_version(&self) -> &semver::Version {
        self.inner.plugin_version()
    }

    fn plugin_id(&self) -> Option<&str> {
        self.inner.plugin_id()
    }

    async fn call(&self, ctx: &CallCtx, args: Value) -> Result<Value, ToolError> {
        let started = Instant::now();
        let outcome = self.inner.call(ctx, args).await;
        let name = self.inner.spec().name.as_str();
        if let Ok(payload) = &outcome {
            match extract(name, payload) {
                Some(receipt) => {
                    // 会话键与注入档同串（`ctx.session_id`，无会话 = 空串）：两个档要能在
                    // 同一张表里对上，否则"这次注入的 key 后来被查回了"根本无从关联。
                    let session_key = ctx.session_id.clone().unwrap_or_default();
                    // 键位现算 = 旁车一次读取，只在真要落行时才付这个成本（与注入档
                    // `MemoryStore::scope_of_session` 同函数同口径）。
                    let scope_id = scope_key(
                        ctx.session_id
                            .as_deref()
                            .and_then(|s| (self.sink.scope_of)(s))
                            .as_deref(),
                    );
                    if let Err(e) = self.sink.log.note_receipt(
                        name,
                        &session_key,
                        &scope_id,
                        receipt.returned,
                        &receipt.keys,
                        started.elapsed(),
                    ) {
                        tracing::warn!("recall observability: {e}");
                    }
                }
                // 读不出形状 = 不记，而不是记一条 0（把"解析失败"冒充"库里没货"）。
                // 这里见红说明**插件侧结果形状与 OBSERVED 的假设脱钩了**，是真实信号。
                None => {
                    tracing::warn!("recall observability: `{name}` 的结果形状读不出回执，本行未记")
                }
            }
        }
        outcome
    }
}

/// 一次成功回执里读出来的两个量（键位不在这里——它不来自结果 JSON，理由见头注）。
struct Receipt {
    /// 本次交付条数（`memory_project_history` = 会话条数，它没有"记忆条数"这个口径）。
    returned: usize,
    /// 交付的 memory key；`memory_project_history` 恒空表（结果里只有会话/章节标题）。
    keys: Vec<String>,
}

/// 从结果 JSON 里读回执。返回 `None` = 结果形状与假设不符（或工具名不在采集面）。
///
/// 三条支路的字段按**实测的结果构造代码**写（`memory-sqlite` 的 `store.rs::memory_read` /
/// `memory_search` 与 `organize/project_history.rs` 的输出），`tests.rs` 的夹具逐条照这三处
/// 的真实字段抄——包括「命中行只有 `scope` 层标、没有 `workspace`」这一条（写夹具时才发现，
/// 它直接改变了键位的取法，见头注）。
///
/// * `memory_read` → `{found: true, key, value, tags, updated_at, scope}` /
///   `{found: false, key}`：命中一行 ⇒ `returned = 1` 并带上 key；未命中是**成功返回零条**
///   ⇒ 记 `returned = 0`（这条是"模型问了但没有"的唯一出处，不许当成失败跳过）。
/// * `memory_search` → `{query, matches: [{key, value, tags, updated_at, scope}]}`
///   ⇒ `returned = matches.len()`（已过 limit 截断；稿里的 `n_candidates` 在这一侧没有出处，
///   故落 0）。
/// * `memory_project_history` → `{workspace, sessions: [...]}` ⇒ `returned` = 会话数、
///   `keys` 空（它给的是标题，不是 memory key——见队列 §5 的实测偏差）。
fn extract(name: &str, payload: &Value) -> Option<Receipt> {
    match name {
        "memory_read" => {
            let found = payload.get("found").and_then(Value::as_bool)?;
            let key = payload.get("key").and_then(Value::as_str).unwrap_or("");
            Some(Receipt {
                returned: usize::from(found),
                keys: match (found, key.is_empty()) {
                    (true, false) => vec![key.to_string()],
                    _ => Vec::new(),
                },
            })
        }
        "memory_search" => {
            let matches = payload.get("matches")?.as_array()?;
            Some(Receipt {
                returned: matches.len(),
                keys: matches
                    .iter()
                    .filter_map(|m| m.get("key").and_then(Value::as_str))
                    .map(str::to_string)
                    .collect(),
            })
        }
        "memory_project_history" => {
            let sessions = payload.get("sessions")?.as_array()?;
            Some(Receipt {
                returned: sessions.len(),
                keys: Vec::new(),
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests;
