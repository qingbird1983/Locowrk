//! Q1b（L0·(b) 检索回执）验收判据。
//!
//! 主轴是**零行为变更**：这一刀在模型与工具之间加了一层壳，所以必须先证明壳不存在
//! （payload 逐字节、`Err` 变体、工具身份三件、未采集工具的原对象返回），再证明壳在的时候
//! 账落对了。全部驱动 [`crate::recall_receipt`] 的 `Tool` 实现本体与
//! [`crate::tool_source::KernelToolSource::resolve`]（接线面），不是手搓事件再测自己。
//!
//! ⚠ 夹具里三个 payload 是**照结果构造代码抄的**（`plugins-src/memory-sqlite/src/store.rs`
//! 的 `memory_read` / `memory_search` + `row_scoped_json`，与 `organize/project_history.rs`
//! 的输出），不是随手编的形状。抄这件事本身抓到一个真问题：命中行里**没有 `workspace`
//! 字段**，只有层标 `scope: "global" | "project"` ⇒ 键位必须由解析器现算（理由与后果写在
//! `recall_receipt.rs` 头注），下面 `scope_id_is_the_same_keyside_string_as_the_injection_side`
//! 钉住这条。
//!
//! 命中率的分子/分母口径判据在 `recall_log/tests.rs`（那是 `stats()` 的地盘）。

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use plugin_api::{CallCtx, Exposure, SideEffect, Tool, ToolError, ToolSpec};
use semver::Version;
use serde_json::{json, Value};

use super::{observe, ReceiptSink};
use crate::recall_log::RecallLog;
use crate::testutil::work_dir;

/// 采集面工具的替身：给定名字与要返回的 payload，可选插件身份与"必然失败"。
struct FakeRetrieval {
    spec: ToolSpec,
    payload: Value,
    fail: bool,
    plugin: Option<String>,
}

impl FakeRetrieval {
    fn ok(name: &str, payload: Value) -> Self {
        Self {
            spec: ToolSpec {
                name: name.into(),
                description: "替身检索工具".into(),
                schema: json!({"type": "object", "properties": {"query": {"type": "string"}}}),
                side_effect: SideEffect::Pure,
                exposure: Exposure::OnDemand,
            },
            payload,
            fail: false,
            plugin: Some("memory-sqlite".into()),
        }
    }
    fn failing(mut self) -> Self {
        self.fail = true;
        self
    }
    fn without_plugin(mut self) -> Self {
        self.plugin = None;
        self
    }
    /// 直接以 `Arc<dyn Tool>` 形态交付：测试里比较的都是 trait 方法，
    /// 拿具体类型再转换只会多一层无意义的写法。
    fn arc(self) -> Arc<dyn Tool> {
        Arc::new(self)
    }
}

#[async_trait]
impl Tool for FakeRetrieval {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }
    fn plugin_version(&self) -> &Version {
        static V: Version = Version::new(1, 2, 3);
        &V
    }
    fn plugin_id(&self) -> Option<&str> {
        self.plugin.as_deref()
    }
    async fn call(&self, _ctx: &CallCtx, _args: Value) -> Result<Value, ToolError> {
        if self.fail {
            return Err(ToolError::Failed {
                tool: "memory_search".into(),
                message: "库不可用".into(),
            });
        }
        Ok(self.payload.clone())
    }
}

/// 键位解析器：与 `recall_log/tests.rs` 同口径（`s1`/`sess-1`/`sess-9` 归 `F:/ws/a`，
/// 其余未归属 ⇒ `scope_key` 归一为空串）。
fn scope_of() -> crate::workspace::WorkspaceResolver {
    Arc::new(|sid: &str| match sid {
        "s1" | "sess-1" | "sess-9" => Some("F:/ws/a".to_string()),
        _ => None,
    })
}

/// 建好的空目录里的采集件 + 它背后的观测表句柄（读账要用后者）。
/// `RecallLog::at` 不碰盘，首行写入才建表。
fn obs(tag: &str) -> (Arc<ReceiptSink>, Arc<RecallLog>) {
    let (sink, _path, log) = obs_at(tag);
    (sink, log)
}

/// 同 [`obs`]，额外交出库路径（要断言"没落盘副作用"的判据用这一档）。
fn obs_at(tag: &str) -> (Arc<ReceiptSink>, PathBuf, Arc<RecallLog>) {
    let path = work_dir(&format!("recallreceipt-{tag}")).join("usage.sqlite3");
    let log = RecallLog::at(&path);
    let sink = ReceiptSink::new(Arc::clone(&log), scope_of());
    (sink, path, log)
}

fn ctx(session: Option<&str>) -> CallCtx {
    let mut c = CallCtx::new();
    c.session_id = session.map(str::to_string);
    c
}

/// 套壳后调用一次（采集面的生产形状）。
async fn call_through(
    tool: Arc<dyn Tool>,
    sink: Arc<ReceiptSink>,
    session: Option<&str>,
) -> Result<Value, ToolError> {
    observe(tool, sink)
        .call(&ctx(session), json!({"query": "rust"}))
        .await
}

/// `memory_search` 的真实结果形状（两行**跨两层**：一行全局层、一行项目层——
/// 正因为一次检索可以跨层，"这一行的键位"在结果里没有单一答案）。
fn two_layer_matches() -> Value {
    json!({
        "query": "rust",
        "matches": [
            {"key": "user.stack", "value": "Rust", "tags": "t", "updated_at": 5, "scope": "global"},
            {"key": "proj.lang", "value": "中文", "tags": "", "updated_at": 4, "scope": "project"}
        ]
    })
}

/// `memory_read` 命中的真实结果形状（注意：**没有 `workspace` 这个键**）。
fn read_hit(key: &str, layer: &str) -> Value {
    json!({"key": key, "value": "v", "tags": "", "updated_at": 1, "scope": layer, "found": true})
}

/// 落账形状那一族（mode = 工具名、键位同源、不去重、形状漂移不出账）。
mod ledger;
/// 零行为变更那一族（payload 逐字节透传 / `Err` 原样 / 未采集工具不被套壳 / 观测失败不反噬）。
mod passthrough;
/// 接线承重那一族（生产出口 `KernelToolSource::resolve` 侧的覆盖与未接线对照）。
mod wiring;
