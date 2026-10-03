//! 触发引擎（⑭）：1s tick 扫 `run_at` 到期的调度，逐条注入目标会话。
//!
//! 目标解析（跨重启语义）：调度绑定创建会话（`CallCtx.session_id`，
//! runner 走 `step_as` 带出）；但前端每次启动生成新会话 id、会话又是内存
//! 的——重启后旧行不在会话表里，此时落**最近聊过的活会话**（last_chat，
//! agent_chat 每轮记一次），不再落僵尸会话。两者皆无（刚启动还没聊过）
//! = Wait：30s 顺延等用户开口，一次性累计顺延 2 小时放弃并系统通知。
//!
//! 忙判定 = 会话锁 try_lock 失败（用户正在该会话聊天）：顺延不让轮，
//! 周期任务让位上限 20 次后跳过该槽（不追帧、不连发）。启动补跑没有
//! 单独路径——错过期间的 run_at 落在过去，第一次 tick 的 `due()` 天然
//! 覆盖（周期任务补跑一次后滚到 now + interval）。

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use session_store::Session;
use tauri::AppHandle;

use kernel_agent::Kernel;

use crate::desktop_notify;
use crate::events::{AgentEvent, EventSink};
use crate::runner::{self, UsagePolicy};
use crate::scheduler::store::{self, Kind, Schedule, ScheduleDb};
use crate::settings::SettingsStore;
use crate::usage::UsageRecorder;

/// 目标解析的纯决策（单测友好）。
pub(crate) enum Target {
    /// 存的会话条目还活着 → 原地注入。
    Stored,
    /// 行已不在（重启过）→ 落最近聊过的会话。
    Active(String),
    /// 没有任何活会话（刚启动未聊天）→ 顺延等待。
    Wait,
}

pub(crate) fn resolve_target(stored_live: bool, last_chat: Option<&str>) -> Target {
    if stored_live {
        Target::Stored
    } else {
        match last_chat {
            Some(id) => Target::Active(id.to_owned()),
            None => Target::Wait,
        }
    }
}

/// 忙/无目标时的让位决策（纯函数）。
pub(crate) enum OnDefer {
    /// 30s 后重试。
    Retry(i64),
    /// 一次性顺延超限：放弃并通知。
    GiveUp,
    /// 周期让位超限：跳过本槽滚到下一周期（deferrals 归零）。
    SkipSlot(i64),
}

pub(crate) fn defer_or_give_up(kind: Kind, deferrals: i64, now: i64, interval_ms: i64) -> OnDefer {
    let cap = match kind {
        Kind::Once => store::ONCE_MAX_DEFERRALS,
        Kind::Interval => store::INTERVAL_MAX_DEFERRALS,
    };
    if deferrals < cap {
        OnDefer::Retry(now + store::DEFER_STEP_MS)
    } else {
        match kind {
            Kind::Once => OnDefer::GiveUp,
            Kind::Interval => OnDefer::SkipSlot(now + interval_ms),
        }
    }
}

/// 周期任务触发后的下槽：不追帧——补跑/长时间阻塞后一律从"现在"起算，
/// 但保证不早于旧槽 + 间隔（防 tick 密集重入）。
pub(crate) fn next_interval_run(now: i64, old_run_at: i64, interval_ms: i64) -> i64 {
    (now + interval_ms).max(old_run_at + interval_ms)
}

/// 引擎依赖快照（全部 Arc 克隆，spawn 用——同 extract::ExtractDeps 模式）。
pub(crate) struct EngineDeps {
    pub db: Arc<ScheduleDb>,
    pub kernel: Arc<Kernel>,
    pub sink: Arc<dyn EventSink>,
    pub usage: Arc<UsageRecorder>,
    pub settings: Arc<SettingsStore>,
    pub sessions: Arc<std::sync::Mutex<HashMap<String, Arc<tokio::sync::Mutex<Session>>>>>,
    /// ⑲ 会话落盘目录：触发的轮次与手聊同一镜像（run_turn log 参数）。
    pub sessions_dir: std::path::PathBuf,
    pub last_chat: Arc<RwLock<Option<String>>>,
    pub app: AppHandle,
}

/// 起引擎后台任务（组装点调用；db 打不开 = 不起，调度缺席已降级）。
/// 组装点 setup 在主线程、没有 tokio 上下文，必须走 tauri 全局运行时
/// （同 events / usage 先例；裸 tokio::spawn 会 panic "no reactor running"）。
pub(crate) fn spawn(deps: EngineDeps) {
    tauri::async_runtime::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(1));
        loop {
            tick.tick().await;
            let now = store::now_ms();
            for sched in deps.db.due(now) {
                fire(&deps, sched, now).await;
            }
        }
    });
}

/// 触发一条：目标解析 → 会话忙判定 → 通知与注入 → 收尾（删行/滚槽）。
async fn fire(deps: &EngineDeps, sched: Schedule, now: i64) {
    let stored_live = deps
        .sessions
        .lock()
        .expect("sessions poisoned")
        .contains_key(&sched.session_id);
    let last_chat_cell = deps.last_chat.read().ok().and_then(|g| g.clone());
    let target = resolve_target(stored_live, last_chat_cell.as_deref());
    let session_id = match target {
        Target::Stored => sched.session_id.clone(),
        Target::Active(id) => id,
        Target::Wait => {
            defer(deps, &sched, now);
            return;
        }
    };

    // 会话锁在触发的整轮内持有：忙（用户正在聊）= try_lock 失败 → 顺延
    let lock = deps
        .sessions
        .lock()
        .expect("sessions poisoned")
        .entry(session_id.clone())
        .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(Session::new())))
        .clone();
    let Ok(mut session) = lock.try_lock() else {
        defer(deps, &sched, now);
        return;
    };

    // 告知先行：会话内 SystemNotice（UI 投影）+ 窗口不在位补系统通知。
    // 之后才跑轮（与"先事件后执行"同精神——用户先知道发生了什么）。
    deps.sink.emit(&AgentEvent::ScheduleFired {
        session_id: session_id.clone(),
        schedule_id: sched.id.clone(),
        title: sched.title.clone(),
    });
    if !desktop_notify::window_attended(&deps.app) {
        let body: String = sched.prompt.chars().take(200).collect();
        if let Err(e) = desktop_notify::show(&deps.app, &sched.title, &body) {
            tracing::warn!("{e}");
        }
    }

    let policy = UsagePolicy {
        threshold_tokens: deps.settings.effective_threshold_tokens(),
        window_tokens: deps.settings.effective_window().unwrap_or(0),
    };
    // 失败也照常收尾：一次性删行（重试多半还是错），周期滚到下槽；
    // 错误经系统通知兜底告知（轮内 LLM 报错用户未必在看）。
    if let Err(e) = runner::run_turn(
        &deps.kernel,
        &mut session,
        &session_id,
        sched.prompt.clone(),
        Some(policy),
        deps.sink.as_ref(),
        Some(deps.usage.as_ref()),
        Some(&deps.sessions_dir),
        deps.settings.max_turn_steps(),
    )
    .await
    {
        tracing::warn!("scheduled turn {} failed: {e}", sched.id);
        if let Err(e) = desktop_notify::show(
            &deps.app,
            &sched.title,
            &format!("scheduled turn failed: {e}"),
        ) {
            tracing::warn!("{e}");
        }
    } else {
        // v2.57 自动标题（与 agent_chat 同一入口）：触发的轮次也算首轮
        crate::title::spawn_auto_title(
            deps.app.clone(),
            Arc::clone(&deps.settings),
            deps.sessions_dir.clone(),
            session_id,
            session.derive_messages(),
        );
    }
    finish(deps, &sched, now);
}

/// 顺延（Wait / 会话忙公共路径）：30s 后重试，超限按类别让位。
fn defer(deps: &EngineDeps, sched: &Schedule, now: i64) {
    let interval_ms = sched.interval_secs.unwrap_or(0) * 1000;
    match defer_or_give_up(sched.kind, sched.deferrals, now, interval_ms) {
        OnDefer::Retry(run_at) => deps.db.reschedule(&sched.id, run_at, sched.deferrals + 1),
        OnDefer::GiveUp => {
            tracing::warn!("schedule {} gave up after deferrals; notifying", sched.id);
            let _ = desktop_notify::show(
                &deps.app,
                &sched.title,
                "schedule missed: the session stayed busy / no chat for too long",
            );
            deps.db.delete(&sched.id);
        }
        OnDefer::SkipSlot(run_at) => deps.db.reschedule(&sched.id, run_at, 0),
    }
}

/// 成功触发后收尾：一次性删除（证据在会话日志），周期滚到下槽。
fn finish(deps: &EngineDeps, sched: &Schedule, now: i64) {
    match sched.kind {
        Kind::Once => {
            deps.db.delete(&sched.id);
        }
        Kind::Interval => {
            let interval_ms = sched.interval_secs.unwrap_or(0) * 1000;
            deps.db.reschedule(
                &sched.id,
                next_interval_run(now, sched.run_at, interval_ms),
                0,
            );
        }
    }
}
