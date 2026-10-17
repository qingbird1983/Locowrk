//! 审批门 consult 范围测试（㉘·B）：非 Pure（含 Compensable）都过决议者；
//! 无决议者时 Compensable 照旧放行、Irreversible fail-safe 拒绝（headless
//! 零变化）。㉘·C：决议者在场 = Pure 也过决策表（隐私门要能拦住只读工具
//! 读凭据），headless 的 Pure 照旧静默放行。目录/审批全用进程内假件，不走 FFI。

//! 拆分说明（v2.160 批 F-10）：本模块只留**跨簇共享件**——顶层 `use`、以及被两个以上
//! 簇的用例引用的假件（`spec` / `specs` / `call` / `ProbeTool` / `ProbeSource` /
//! `DriftingMemory` / `Spy` 七件，连各自的 `impl` 块一起）；
//! 12 条用例按被测面分派到 3 个子文件——`approval`（审批门 consult 范围）· `prefix`
//! （M1 相邻轮前缀一致）· `tree`（M4 记忆树切流）。形状照 `settings/tests/` 与
//! `organize/tests/`、`llm/tests/` 先例：共享件是 tests 的私有项，子簇经 `use super::*;`
//! 取用 ⇒ **可见性零改动**；正文里 `super::` / `self::` 出现 0 次 ⇒ 无相对路径漂移面。
//! 用例本体逐字搬运，断言一条未增删。

mod approval;
mod prefix;
mod tree;

use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use cordis_core::{Context, WaterfallDecision, WaterfallListener};

use plugin_api::{CallCtx, CallId, SideEffect, Tool, ToolError, ToolSpec};

use semver::Version;

use serde_json::{Value, json};

use session_store::{Session, SessionEvent};

use crate::approval::{ApprovalDecision, ApprovalResolver};

use crate::kernel::{
    Kernel, LlmError, LlmRequest, LlmResponse, LlmService, ToolCallReq, ToolSource,
};

use crate::kernel::{TreeContextSource, TreePlan, collapse_expanded, tail_user_start};

use crate::waterfall::points;

use crate::{
    GlobalEwma, MemoryEntry, MemoryInject, MemoryInjectConfig, MemorySource, ToolExposure,
    ToolExposureConfig,
};

fn spec(name: &str, side: SideEffect) -> ToolSpec {
    ToolSpec {
        name: name.into(),
        description: "test".into(),
        schema: json!({ "type": "object" }),
        side_effect: side,
        exposure: Default::default(),
    }
}

/// 可执行假工具：执行即回显工具名（断言"放行后真的跑了"）。
struct ProbeTool(ToolSpec);

#[async_trait]
impl Tool for ProbeTool {
    fn spec(&self) -> &ToolSpec {
        &self.0
    }

    fn plugin_version(&self) -> &Version {
        static V: Version = Version::new(0, 1, 0);
        &V
    }

    async fn call(&self, _ctx: &CallCtx, _args: Value) -> Result<Value, ToolError> {
        Ok(json!({ "ran": self.0.name }))
    }
}

struct ProbeSource(Vec<ToolSpec>);

#[async_trait]
impl ToolSource for ProbeSource {
    async fn resolve(
        &self,
        _session_id: Option<&str>,
        name: &str,
    ) -> Result<Arc<dyn Tool>, ToolError> {
        self.0
            .iter()
            .find(|s| s.name == name)
            .map(|s| Arc::new(ProbeTool(s.clone())) as Arc<dyn Tool>)
            .ok_or_else(|| ToolError::NotAvailable(name.to_string()))
    }

    async fn list(&self, _session_id: Option<&str>) -> Vec<ToolSpec> {
        self.0.clone()
    }
}

fn call(id: &str, tool: &str) -> ToolCallReq {
    ToolCallReq {
        id: CallId(id.into()),
        tool: tool.into(),
        args: json!({}),
        session_id: Some("s1".into()),
    }
}

fn specs() -> Vec<ToolSpec> {
    vec![
        spec(
            "fs_write",
            SideEffect::Compensable {
                undo_tool: "fs_restore".into(),
            },
        ),
        spec("shell", SideEffect::Irreversible),
        spec("fs_read", SideEffect::Pure),
    ]
}

/// 逐次漂移的记忆源：每次召回 value = 调用序号——预取生效 = 只查一次。
struct DriftingMemory(std::sync::atomic::AtomicUsize);

impl MemorySource for DriftingMemory {
    fn recall(&self, _session_id: Option<&str>, _limit: usize) -> Vec<MemoryEntry> {
        let n = self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        vec![MemoryEntry {
            key: "drift".into(),
            value: n.to_string(),
            tags: String::new(),
            updated_at: 0,
            scope: String::new(),
        }]
    }
}

/// 末位监听器：记录决策环出口 payload（LLM 实际收到的请求形状）。
///
/// 台账 T2 之后它顺带成了"守卫有没有还回载荷"的活体探针：注入环若漏写回，
/// 这里记到的是 `null` 而不是带 system/tools 的完整请求。
struct Spy(Mutex<Vec<String>>);

#[async_trait]
impl WaterfallListener for Spy {
    async fn handle(&self, key: &str, payload: &mut Value) -> WaterfallDecision {
        if key == points::REQUEST {
            self.0.lock().unwrap().push(payload.to_string());
        }
        WaterfallDecision::Continue
    }
}
