//! 调度工具面（⑭）：宿主内置三件（search_tools 同款归宿主先例——调度器
//! / 存储 / 触发都是宿主关切，插件无后台常驻能力）。经
//! `KernelToolSource::with_builtin` 进目录，与插件同名时内置赢。
//!
//! 会话绑定：`CallCtx.session_id`（runner 走 `step_as` 带出）——缺会话
//! 身份（非桌面路径）直接报错，不猜默认值。跨会话隔离：list / delete
//! 只见本会话的行，删别人的调度不是这个工具的事。
//!
//! 分级：create 标 Irreversible（宁严勿松——创建的是"未来会替模型说话
//! 的定时器"，审批即告知用户）；list / delete 纯本地读删 = Pure。

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};

use async_trait::async_trait;
use plugin_api::{CallCtx, Exposure, SideEffect, Tool, ToolError, ToolSpec};
use serde_json::{json, Value};

use crate::scheduler::store::{Kind, Schedule, ScheduleDb, MAX_SCHEDULES};
use crate::text::clip_trimmed;
use crate::timeutil::now_ms;

static VERSION: std::sync::LazyLock<semver::Version> =
    std::sync::LazyLock::new(|| semver::Version::new(1, 0, 0));

static ID_SEQ: AtomicU64 = AtomicU64::new(0);

/// id：时间戳 + 进程内序号（无 uuid 依赖；唯一性在同进程内够用）。
fn new_id() -> String {
    let n = ID_SEQ.fetch_add(1, Ordering::Relaxed);
    format!("sch-{:x}-{n}", now_ms())
}

fn cap_hint(e: String) -> ToolError {
    // 表满等存储拒绝如实回给模型（含 MAX_SCHEDULES 提示，见 store.rs）
    ToolError::Failed {
        tool: "schedule_create".into(),
        message: e,
    }
}

// ---- schedule_create ----

fn create_spec() -> &'static ToolSpec {
    static SPEC: OnceLock<ToolSpec> = OnceLock::new();
    SPEC.get_or_init(|| ToolSpec {
        name: "schedule_create".into(),
        description: "Schedule a future turn to be injected into THIS session (a scheduled \
                      reminder/task). Modes, exactly one of: `interval_secs` (recurring, \
                      minimum 10), `delay_secs` (one-shot after N seconds), `at` (one-shot, \
                      unix epoch milliseconds). Pass `prompt` — what the agent should do \
                      when it fires — and optional short `title`. Requires approval. Use \
                      schedule_list to review, schedule_delete to cancel."
            .into(),
        schema: json!({
            "type": "object",
            "properties": {
                "prompt": { "type": "string",
                            "description": "the turn content to inject when the schedule fires" },
                "title": { "type": "string",
                           "description": "short label for notifications (default: prompt prefix)" },
                "interval_secs": { "type": "number", "description": "recurring every N seconds (min 10)" },
                "delay_secs": { "type": "number", "description": "one-shot after N seconds" },
                "at": { "type": "number", "description": "one-shot at unix epoch milliseconds" }
            },
            "required": ["prompt"]
        }),
        side_effect: SideEffect::Irreversible,
        exposure: Exposure::Always,
    })
}

pub(crate) struct ScheduleCreateTool {
    db: Arc<ScheduleDb>,
}

impl ScheduleCreateTool {
    pub(crate) fn new(db: Arc<ScheduleDb>) -> Arc<Self> {
        Arc::new(Self { db })
    }
}

#[async_trait]
impl Tool for ScheduleCreateTool {
    fn spec(&self) -> &ToolSpec {
        create_spec()
    }

    fn plugin_version(&self) -> &semver::Version {
        &VERSION
    }

    async fn call(&self, ctx: &CallCtx, args: Value) -> Result<Value, ToolError> {
        let session_id = ctx.session_id.clone().ok_or_else(|| ToolError::Failed {
            tool: "schedule_create".into(),
            message: "no session context; scheduling requires a chat session".into(),
        })?;
        let prompt = string_arg(&args, "prompt", 8000)?;
        let title = match args.get("title").and_then(Value::as_str) {
            Some(t) if !t.trim().is_empty() => clip_trimmed(t, 100),
            _ => clip_trimmed(&prompt, 40),
        };
        let now = now_ms();
        let interval_secs = opt_num(&args, "interval_secs")?;
        let delay_secs = opt_num(&args, "delay_secs")?;
        let at = opt_num(&args, "at")?;
        // 模式互斥校验（真实测试 2026-10-06 P2-1）：描述承诺 exactly one of，
        // 旧实现是静默优先级（interval > at > delay）——多模式同给时模型以为
        // 的语义与落库行为分叉。显式拒绝，不猜优先级。
        let provided: Vec<&str> = [
            ("interval_secs", interval_secs.is_some()),
            ("delay_secs", delay_secs.is_some()),
            ("at", at.is_some()),
        ]
        .iter()
        .filter(|(_, present)| *present)
        .map(|(name, _)| *name)
        .collect();
        if provided.len() != 1 {
            return Err(bad(&match provided.is_empty() {
                true => "one of interval_secs / delay_secs / at is required".into(),
                false => format!(
                    "pass exactly ONE of interval_secs / delay_secs / at (got {})",
                    provided.join(" + ")
                ),
            }));
        }
        let (kind, run_at, interval_secs) = if let Some(secs) = interval_secs {
            if !(10..=31_536_000).contains(&secs) {
                return Err(bad("interval_secs must be 10..=31536000"));
            }
            (Kind::Interval, now + secs as i64 * 1000, Some(secs as i64))
        } else {
            let run_at = match at {
                Some(ms) => ms as i64,
                None => now + delay_secs.unwrap_or(0) as i64 * 1000,
            };
            if run_at <= now {
                return Err(bad(
                    "`at` is in the past (or `delay_secs` is 0); pass a future time",
                ));
            }
            (Kind::Once, run_at, None)
        };
        let sched = Schedule {
            id: new_id(),
            session_id,
            kind,
            title,
            prompt,
            run_at,
            interval_secs,
            deferrals: 0,
            created_ms: now,
        };
        self.db.create(&sched).map_err(cap_hint)?;
        Ok(json!({
            "id": sched.id,
            "kind": kind_str(sched.kind),
            "run_at": sched.run_at,
            "note": "fires as a turn injected into this session; you will see its output in the conversation"
        }))
    }
}

// ---- schedule_list / schedule_delete ----

fn list_spec() -> &'static ToolSpec {
    static SPEC: OnceLock<ToolSpec> = OnceLock::new();
    SPEC.get_or_init(|| ToolSpec {
        name: "schedule_list".into(),
        description: "List the pending schedules of THIS session (id, kind, title, next \
                      fire time). Pure read."
            .into(),
        schema: json!({ "type": "object", "properties": {} }),
        side_effect: SideEffect::Pure,
        // Always（发现性补丁 §4.4）：能 schedule_create 就能查看自己建了什么
        exposure: Exposure::Always,
    })
}

pub(crate) struct ScheduleListTool {
    db: Arc<ScheduleDb>,
}

impl ScheduleListTool {
    pub(crate) fn new(db: Arc<ScheduleDb>) -> Arc<Self> {
        Arc::new(Self { db })
    }
}

#[async_trait]
impl Tool for ScheduleListTool {
    fn spec(&self) -> &ToolSpec {
        list_spec()
    }

    fn plugin_version(&self) -> &semver::Version {
        &VERSION
    }

    async fn call(&self, ctx: &CallCtx, _args: Value) -> Result<Value, ToolError> {
        let session_id = ctx.session_id.clone().ok_or_else(|| ToolError::Failed {
            tool: "schedule_list".into(),
            message: "no session context".into(),
        })?;
        let rows: Vec<Value> = self
            .db
            .list_session(&session_id)
            .iter()
            .map(|s| {
                json!({
                    "id": s.id,
                    "kind": kind_str(s.kind),
                    "title": s.title,
                    "run_at": s.run_at,
                    "interval_secs": s.interval_secs,
                    "deferrals": s.deferrals,
                })
            })
            .collect();
        Ok(json!({ "schedules": rows, "cap": MAX_SCHEDULES }))
    }
}

fn delete_spec() -> &'static ToolSpec {
    static SPEC: OnceLock<ToolSpec> = OnceLock::new();
    SPEC.get_or_init(|| ToolSpec {
        name: "schedule_delete".into(),
        description: "Cancel a pending schedule created in THIS session by its id (see \
                      schedule_list). Pure local removal — nothing fires."
            .into(),
        schema: json!({
            "type": "object",
            "properties": { "id": { "type": "string" } },
            "required": ["id"]
        }),
        side_effect: SideEffect::Pure,
        exposure: Exposure::OnDemand,
    })
}

pub(crate) struct ScheduleDeleteTool {
    db: Arc<ScheduleDb>,
}

impl ScheduleDeleteTool {
    pub(crate) fn new(db: Arc<ScheduleDb>) -> Arc<Self> {
        Arc::new(Self { db })
    }
}

#[async_trait]
impl Tool for ScheduleDeleteTool {
    fn spec(&self) -> &ToolSpec {
        delete_spec()
    }

    fn plugin_version(&self) -> &semver::Version {
        &VERSION
    }

    async fn call(&self, ctx: &CallCtx, args: Value) -> Result<Value, ToolError> {
        let session_id = ctx.session_id.clone().ok_or_else(|| ToolError::Failed {
            tool: "schedule_delete".into(),
            message: "no session context".into(),
        })?;
        let id = args
            .get("id")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| bad("missing string field `id`"))?;
        // 只删本会话的行：跨会话 id 一律 not found（不泄露存在性）
        let owned = self.db.list_session(&session_id).iter().any(|s| s.id == id);
        if !owned || !self.db.delete(id) {
            return Err(bad("schedule not found in this session"));
        }
        Ok(json!({ "deleted": id }))
    }
}

// ---- 参数 helpers（纯函数，单测覆盖） ----

fn bad(message: &str) -> ToolError {
    ToolError::Failed {
        tool: "schedule_create".into(),
        message: message.into(),
    }
}

fn kind_str(k: Kind) -> &'static str {
    match k {
        Kind::Once => "once",
        Kind::Interval => "interval",
    }
}

fn string_arg(args: &Value, key: &str, max_chars: usize) -> Result<String, ToolError> {
    let s = args
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| bad(&format!("missing non-empty string field `{key}`")))?;
    Ok(clip_trimmed(s, max_chars))
}

fn opt_num(args: &Value, key: &str) -> Result<Option<u64>, ToolError> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_f64()
            .filter(|n| *n >= 0.0 && n.fract() == 0.0)
            .map(|n| Some(n as u64))
            .ok_or_else(|| bad(&format!("field `{key}` must be a non-negative integer"))),
    }
}
