//! 轮头（队列 Q12·第二刀 v2.205）的投影用例：本轮「真正干活多久」= 墙钟区间
//! 扣掉等待用户审批的那一段，等待的**时刻源在日志里**（`ApprovalDecided` 的
//! `asked_at` / `decided_at`，内核在 `approve()` 前后各取一次）⇒ 实时与重放算的是
//! 同一个数；旧日志读不出 ⇒ 整轮不给 `active_ms`（前端回落墙钟是**如实降级**，
//! 拿 0 冒充"一秒没干活"是本仓明令禁止的那种谎）。
//!
//! 夹具照生产形状写：两颗时刻由 `crates/kernel-agent/src/kernel.rs` 的审批支产出
//! （`asked_at = self.approval.is_some().then(now_ms)`），无决议者的直通路径就是
//! `None`——测试不许把它们混成一档。

use plugin_api::CallId;
use serde_json::json;
use session_store::{Session, SessionEvent, TurnId, TurnOutcome};

use crate::events::{AgentEvent, TurnState};
use crate::runner::projection::project_events;
use crate::runner::run_turn;
use crate::tests::{kernel_with_tools, Collector};

const TURN_START_MS: i64 = 1_700_000_000_000;
const TURN_END_MS: i64 = TURN_START_MS + 9_000; // 墙钟 9 秒

/// 一颗审批决策事件：两刻齐 = 新内核（v2.205 起），`None` = 旧日志或无决议者直通。
fn decided(asked_at: Option<i64>, decided_at: Option<i64>) -> SessionEvent {
    SessionEvent::ApprovalDecided {
        call_id: CallId("c1".into()),
        allowed: true,
        reason: Some("用户放行".into()),
        asked_at,
        decided_at,
    }
}

/// 一整轮的最小形状：TurnStart → 一次工具调用与结果 → 若干条给定事件 → TurnEnd。
fn turn(outcome: TurnOutcome, extra: Vec<SessionEvent>) -> Vec<SessionEvent> {
    let mut v = vec![
        SessionEvent::TurnStart {
            turn_id: TurnId(3),
            ts: TURN_START_MS,
        },
        SessionEvent::UserMessage {
            content: "改一下".into(),
            parts: Vec::new(),
        },
        SessionEvent::ToolCall {
            call_id: CallId("c1".into()),
            tool: "shell".into(),
            args: json!({"cmd": "ls"}),
            plugin_id: Some("tool-shell".into()),
            plugin_version: semver::Version::new(0, 1, 0),
            side_effect: plugin_api::SideEffect::Pure,
        },
    ];
    v.extend(extra);
    v.push(SessionEvent::ToolResult {
        call_id: CallId("c1".into()),
        output: json!({"stdout": ""}),
        error: None,
    });
    v.push(SessionEvent::TurnEnd {
        turn_id: TurnId(3),
        outcome,
        ts: TURN_END_MS,
    });
    v
}

/// 找投影出来的那一条轮头。
fn header_of(events: &[SessionEvent]) -> (TurnState, Option<i64>, Option<u64>) {
    project_events(events, "s1")
        .into_iter()
        .find_map(|e| match e {
            AgentEvent::TurnHeader {
                state,
                active_ms,
                turn_id,
                ..
            } => Some((state, active_ms, turn_id)),
            _ => None,
        })
        .expect("TurnEnd 必须投影出一条轮头")
}

#[test]
fn approval_wait_is_deducted_from_wall_clock_span() {
    // 墙钟 9000ms，其中等用户点了 2000ms ⇒ 真正干活 7000ms
    let (state, active, turn_id) = header_of(&turn(
        TurnOutcome::Completed,
        vec![decided(
            Some(TURN_START_MS + 1_000),
            Some(TURN_START_MS + 3_000),
        )],
    ));
    assert_eq!(
        (state, active, turn_id),
        (TurnState::Completed, Some(7_000), Some(3)),
        "扣等待的读数与轮号应由日志现推"
    );
}

#[test]
fn no_approval_means_zero_wait_not_missing_data() {
    // 本轮压根没弹审批 ⇒ 等待恒 0，active_ms = 墙钟差（与"有审批但读不出"不同档）
    let (_, active, _) = header_of(&turn(TurnOutcome::Completed, vec![]));
    assert_eq!(active, Some(9_000), "无审批 = 知道等待是 0，不是不知道");
}

#[test]
fn old_log_without_timestamps_gives_no_active_ms() {
    // 旧日志：有决策事件但没有两刻 ⇒ 不给 active_ms（宁缺勿假），状态照给
    let (state, active, _) = header_of(&turn(TurnOutcome::Completed, vec![decided(None, None)]));
    assert_eq!(state, TurnState::Completed);
    assert_eq!(active, None, "等待时长读不出时不得给数——更不得给 0");

    // 时钟回拨（decided < asked）同样整轮不给
    let (_, backdated, _) = header_of(&turn(
        TurnOutcome::Completed,
        vec![decided(
            Some(TURN_START_MS + 4_000),
            Some(TURN_START_MS + 2_000),
        )],
    ));
    assert_eq!(backdated, None, "负等待不是数据，是读数作废");

    // 两次审批里只有一次带时刻 ⇒ 有一半是猜的就不报（与 ±N 的 NULL 档同一条纪律）
    let (_, partial, _) = header_of(&turn(
        TurnOutcome::Completed,
        vec![
            decided(Some(TURN_START_MS + 500), Some(TURN_START_MS + 1_500)),
            decided(None, None),
        ],
    ));
    assert_eq!(partial, None, "任一次读不出 ⇒ 整轮不给");
}

#[test]
fn stopped_turn_projects_interrupted_not_completed() {
    // 取证批 G2 那条「停止也走 Completed」在这一刀修掉：日志记 interrupted 就投出 interrupted
    let (state, _, _) = header_of(&turn(TurnOutcome::Interrupted, vec![]));
    assert_eq!(state, TurnState::Interrupted, "停止必须与完成不同形");
    // 失败支同理（三态各自对位，映射是穷尽 From 不是字符串比较）
    let (failed, _, _) = header_of(&turn(TurnOutcome::Failed, vec![]));
    assert_eq!(failed, TurnState::Failed);
}

#[test]
fn turn_header_precedes_file_changes_and_turn_done() {
    // 顺序是前端的承重件：轮头与清单挂在轮尾、TurnDone 一到就冻结读数 ⇒
    // 三条事件的先后不能漂（实时 emit 与重放 push 同序，专测在下面那条）。
    let events = turn(
        TurnOutcome::Completed,
        vec![decided(Some(TURN_START_MS), Some(TURN_START_MS + 1_000))],
    );
    let kinds: Vec<&str> = project_events(&events, "s1")
        .iter()
        .map(|e| e.channel())
        .collect();
    let pos_header = kinds
        .iter()
        .position(|c| *c == "agent://turn-header")
        .expect("应有 turn-header 通道");
    let pos_done = kinds
        .iter()
        .position(|c| *c == "agent://turn-done")
        .expect("应有 turn-done 通道");
    assert!(pos_header < pos_done, "轮头必须早于 TurnDone：{kinds:?}");
}

#[tokio::test]
async fn realtime_and_replay_emit_the_identical_header() {
    // 「两处同一把尺」的跨文件判据：走组装点跑一轮（实时经 `emit_turn_summary`），
    // 再把同一份日志全量重放（经 project_events 的 TurnEnd 支）——两条轮头的
    // **序列化字节必须相同**（state 与 active_ms 都不许在两处各解释一遍）。
    // 调用形状照 `tests_session_log/mod.rs` 的既有 run_turn 用例，不自创参数序。
    let kernel = kernel_with_tools();
    let mut session = Session::new();
    let sink = Collector::default();
    run_turn(
        &kernel,
        &mut session,
        "s-th",
        "echo 你好".into(),
        Vec::new(),
        None,
        &sink,
        None,
        None,
        crate::runner::DEFAULT_MAX_STEPS,
        None,
        None,
        None,
        None,
    )
    .await
    .expect("turn ok");

    let live = sink
        .events()
        .into_iter()
        .find(|e| matches!(e, AgentEvent::TurnHeader { .. }))
        .expect("实时路径必须发出一条轮头");
    let replay = project_events(session.events(), "s-th")
        .into_iter()
        .find(|e| matches!(e, AgentEvent::TurnHeader { .. }))
        .expect("重放路径必须投出一条轮头");
    assert_eq!(
        serde_json::to_string(&live).unwrap(),
        serde_json::to_string(&replay).unwrap(),
        "实时与重放对同一轮给出不同的轮头读数 = 两处各算了一遍"
    );
    // 实时侧的 steps 只属于 TurnDone，轮头不许复制它（判据 S5 的实盘依据）
    assert!(
        !serde_json::to_string(&live).unwrap().contains("\"steps\""),
        "轮头里出现了 steps = 同一属性两个出处"
    );
}
