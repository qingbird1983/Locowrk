//! 审计中间件（plugins.md P1.5 `mw-audit`，阶段八③落地）。
//!
//! 落点说明：DESIGN.md §7.1 约定审计经洋葱模型接入；§3.5 表中的
//! "serial 审计链"是事件总线语义，而 C ABI 的 `PluginKind::Middleware`
//! 仍是占位类别（§5.2）——所以 mw-audit 当前以**宿主侧中间件**落地，
//! 插件化（经 FFI 挂链）推迟到 ABI 扩展时再做。
//!
//! 覆盖范围：链序 Tracing → **Audit** → Timeout → CircuitBreaker，审计在
//! 超时/熔断**外侧**，被拒掉的调用也留痕。审批拒绝的调用不进中间件链，
//! 以 `ApprovalDecided` 事件留在会话日志（纪律 9）。
//!
//! 落盘：一行一条 JSON（JSONL，追加写）。文件只追加不修改，与会话日志
//! 同一纪律；单条写失败只告警不上抛——审计缺一条不能拖垮工具调用。

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use plugin_api::{CallCtx, SideEffect, ToolError};
use serde::Serialize;
use serde_json::Value;

use super::{Middleware, Next};

/// 一条审计记录。字段只增不改（演进只能加新键）；`ts` 是 RFC 3339 UTC。
#[derive(Debug, Clone, Serialize)]
pub struct AuditEntry {
    pub ts: String,
    pub tool: String,
    /// `pure` / `compensable` / `irreversible`（SideEffect 的扁平拼写）。
    pub side_effect: String,
    pub args: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub call_id: Option<String>,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub duration_ms: u64,
}

fn side_effect_str(se: &SideEffect) -> &'static str {
    match se {
        SideEffect::Pure => "pure",
        SideEffect::Compensable { .. } => "compensable",
        SideEffect::Irreversible => "irreversible",
    }
}

/// 审计出口。中间件只产生记录，写到哪由组装点注入
/// （desktop 数据根 / kernel-bin 演示 / 测试内存）。
pub trait AuditSink: Send + Sync + 'static {
    fn record(&self, entry: AuditEntry);
}

/// JSONL 文件出口：每次追加一行（打开-写-关；审计量级 = 工具调用量，
/// 常驻句柄反而是多余状态）。目录在构造时创建。
pub struct JsonlAuditSink {
    path: PathBuf,
}

impl JsonlAuditSink {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        if let Some(dir) = path.parent() {
            if let Err(e) = std::fs::create_dir_all(dir) {
                tracing::warn!("audit dir create failed ({}): {e}", dir.display());
            }
        }
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl AuditSink for JsonlAuditSink {
    fn record(&self, entry: AuditEntry) {
        let mut line = match serde_json::to_string(&entry) {
            Ok(l) => l,
            Err(e) => {
                tracing::warn!("audit serialize failed: {e}");
                return;
            }
        };
        line.push('\n');
        let write = || -> std::io::Result<()> {
            let mut f = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)?;
            f.write_all(line.as_bytes())
        };
        if let Err(e) = write() {
            tracing::warn!("audit write failed ({}): {e}", self.path.display());
        }
    }
}

/// 审计中间件：每次工具调用产出一条 [`AuditEntry`]（含参数、结果、耗时）。
#[derive(Clone)]
pub struct Audit {
    sink: Arc<dyn AuditSink>,
}

impl std::fmt::Debug for Audit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // sink 是 trait 对象（含内存出口/文件出口等），不外泄其内部形态
        f.debug_struct("Audit").finish_non_exhaustive()
    }
}

impl Audit {
    pub fn new(sink: Arc<dyn AuditSink>) -> Self {
        Self { sink }
    }
}

#[async_trait]
impl Middleware for Audit {
    async fn handle(&self, ctx: &mut CallCtx, next: Next<'_>) -> Result<Value, ToolError> {
        let started = Instant::now();
        let tool = next.tool_name().to_string();
        let side_effect = side_effect_str(&next.tool_spec().side_effect).to_string();
        let args = next.args().clone();
        let session_id = ctx.session_id.clone();
        let call_id = ctx.call_id.as_ref().map(|c| c.0.clone());

        let out = next.proceed(ctx).await;

        self.sink.record(AuditEntry {
            ts: jiff::Timestamp::now().to_string(),
            tool,
            side_effect,
            args,
            session_id,
            call_id,
            ok: out.is_ok(),
            error: out.as_ref().err().map(|e| e.to_string()),
            duration_ms: started.elapsed().as_millis() as u64,
        });
        out
    }
}
