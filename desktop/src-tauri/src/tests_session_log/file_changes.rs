//! 本轮文件改动（后审计队列 Q12·C1）的投影用例：±N 的权威源在**插件结果载荷**
//! 里（`tool-edit` 三个写工具 + `tool-fs` 的 fs_write 新建支），宿主投影只搬运
//! 与聚合，不读盘补算。这里锁四件事：区间、NULL 档、聚合口径、实时与重放同形。
//!
//! 夹具一律照生产载荷形状写（`plugins-src/tool-*/src/*.rs` 里那几个 `json!`），
//! 不凭想象编字段——字段名错一个，测试全绿而真跑恒为空（本仓立过的判据）。

use plugin_api::{CallId, SideEffect};
use serde_json::json;
use session_store::{Session, SessionEvent, TurnId};

use crate::events::AgentEvent;
use crate::runner::projection::project_events;
// 聚合口径自 v2.205 起住在 `runner::turn_summary`（轮末汇总独立成模块）；
// 本簇跟着改引用路径，函数体与期望值一字未动 = 搬移的等价性由这 4 条用例给。
use crate::runner::turn_summary::collect_file_changes;

fn tool_result(output: serde_json::Value) -> SessionEvent {
    // ToolCall 先行（纪律 4），投影才认得这条结果属于哪个工具
    SessionEvent::ToolResult {
        call_id: CallId("c1".into()),
        output,
        error: None,
    }
}

fn edit_output(path: &str, additions: i64, deletions: i64) -> serde_json::Value {
    json!({
        "path": path,
        "bytes": 42,
        "edits": 1,
        "additions": additions,
        "deletions": deletions,
        "match_type": "exact",
        "byte_range": [0, 3],
        "existed": true,
        "backup": "backups/x",
        "revision": "sha256:abc",
    })
}

fn fs_write_created(path: &str, lines: i64) -> serde_json::Value {
    json!({
        "path": path,
        "bytes": 10,
        "existed": false,
        "additions": lines,
        "deletions": 0,
        "backup": null,
        "revision": "sha256:def",
    })
}

fn fs_write_overwritten(path: &str) -> serde_json::Value {
    // 覆盖写：插件**不给** ±N（值 = null），因为整文件重写算不出 diff
    json!({
        "path": path,
        "bytes": 20,
        "existed": true,
        "additions": null,
        "deletions": null,
        "backup": "backups/y",
        "revision": "sha256:ghi",
    })
}

fn turn(start: u64, events: Vec<SessionEvent>) -> Vec<SessionEvent> {
    let mut v = vec![
        SessionEvent::TurnStart {
            turn_id: TurnId(start),
            ts: 1_700_000_000_000 + start as i64,
        },
        SessionEvent::ToolCall {
            call_id: CallId("c1".into()),
            tool: "edit_replace".into(),
            args: json!({"path": "a.rs"}),
            plugin_id: Some("tool-edit".into()),
            plugin_version: semver::Version::new(0, 1, 0),
            side_effect: SideEffect::Compensable {
                undo_tool: "fs_restore".into(),
            },
        },
    ];
    v.extend(events);
    v.push(SessionEvent::TurnEnd {
        turn_id: TurnId(start),
        outcome: session_store::TurnOutcome::Completed,
        ts: 1_700_000_009_000 + start as i64,
    });
    v
}

/// 一条文件改动的可比对元组（path, kind, additions, deletions）——`clippy::type_complexity`
/// 对本批这条判据的意见，正解是**给它名字**而不是 `#[allow]`：用例里比的是这四列。
type ChangeRow = (String, String, Option<i64>, Option<i64>);
/// 一次 `FileChanges` 投影的可比对形状：`(turn_id, 行表)`。
type ChangesEvent = (Option<u64>, Vec<ChangeRow>);

/// 找投影里的 FileChanges 载荷（至多一条）。
fn changes_of(events: &[SessionEvent]) -> Vec<ChangesEvent> {
    project_events(events, "s1")
        .into_iter()
        .filter_map(|e| match e {
            AgentEvent::FileChanges { turn_id, files, .. } => Some((
                turn_id,
                files
                    .into_iter()
                    .map(|f| (f.path, f.kind, f.additions, f.deletions))
                    .collect(),
            )),
            _ => None,
        })
        .collect()
}

#[test]
fn write_results_aggregate_into_one_turn_header() {
    let events = turn(
        7,
        vec![
            tool_result(edit_output("F:/w/a.rs", 2, 1)),
            tool_result(fs_write_created("F:/w/new.rs", 5)),
        ],
    );
    let got = changes_of(&events);
    assert_eq!(got.len(), 1, "一轮一条汇总（不是每条结果一发）");
    let (turn_id, files) = &got[0];
    assert_eq!(
        *turn_id,
        Some(7),
        "轮号从 TurnStart 原样带出（u64，不是新造串）"
    );
    assert_eq!(
        files.as_slice(),
        &[
            (
                "F:/w/a.rs".to_string(),
                "modified".to_string(),
                Some(2),
                Some(1)
            ),
            (
                "F:/w/new.rs".to_string(),
                "created".to_string(),
                Some(5),
                Some(0)
            ),
        ],
        "顺序 = 首次出现顺序，kind 由宿主定形"
    );
}

#[test]
fn missing_diff_source_stays_null_instead_of_zero() {
    // 覆盖写给不出 diff ⇒ 两个数留 None 而**不是** Some(0)：把"没读出来"报成
    // "改了 0 行"是假数据，前端据此不渲染数字。
    let events = turn(3, vec![tool_result(fs_write_overwritten("F:/w/b.rs"))]);
    let (_, files) = &changes_of(&events)[0];
    assert_eq!(
        files.as_slice(),
        &[(
            "F:/w/b.rs".to_string(),
            "overwritten".to_string(),
            None,
            None
        )]
    );

    // 同一文件先算得出、后被覆盖 ⇒ 整条不留数（有一半是猜的就不报），kind 取最后
    let mixed = turn(
        4,
        vec![
            tool_result(edit_output("F:/w/c.rs", 9, 4)),
            tool_result(fs_write_overwritten("F:/w/c.rs")),
        ],
    );
    let (_, files) = &changes_of(&mixed)[0];
    assert_eq!(
        files.as_slice(),
        &[(
            "F:/w/c.rs".to_string(),
            "overwritten".to_string(),
            None,
            None
        )],
        "聚合后 ±N 归 None，kind = 最后一次"
    );
}

#[test]
fn read_only_tools_and_empty_turns_produce_nothing() {
    // fs_read 带 path 但**不带**那两个键 ⇒ 不是文件改动（识别按形状不按工具名）
    let read = turn(
        5,
        vec![SessionEvent::ToolResult {
            call_id: CallId("c1".into()),
            output: json!({"path": "F:/w/a.rs", "content": "hi", "revision": "sha256:x"}),
            error: None,
        }],
    );
    assert!(changes_of(&read).is_empty(), "只读结果不进清单");

    // 本轮没写任何文件 ⇒ 一条都不发（宁缺勿假，不发空清单）
    let bare = turn(6, vec![]);
    assert!(changes_of(&bare).is_empty(), "空轮不发 FileChanges");
}

#[test]
fn live_and_replay_share_one_ruler() {
    // 实时路径（runner::finish_turn 调 `collect_file_changes(&events[turn_start..])`）
    // 与重放路径（project_events 的 TurnEnd 支）必须逐字节同一个数。这里同一段
    // 事件走两条通路，比序列化后的载荷——跨文件的关系只能由跨文件判据钉住。
    let events = turn(
        8,
        vec![
            tool_result(edit_output("F:/w/d.rs", 3, 2)),
            tool_result(edit_output("F:/w/e.rs", 1, 0)),
        ],
    );
    let replay = project_events(&events, "s1")
        .into_iter()
        .find(|e| matches!(e, AgentEvent::FileChanges { .. }))
        .expect("重放侧产出 FileChanges");
    let live = AgentEvent::FileChanges {
        session_id: "s1".into(),
        turn_id: Some(8),
        files: collect_file_changes(&events[0..events.len() - 1]),
    };
    assert_eq!(
        serde_json::to_string(&replay).unwrap(),
        serde_json::to_string(&live).unwrap(),
        "两处必须同一把尺（区间或口径一漂，重放就和当场不一样）"
    );

    // Session 形态的同一件事：投影吃的是切片，实时吃的是 session.events() 区间
    let mut session = Session::new();
    for e in &events {
        session.append(e.clone());
    }
    assert_eq!(
        collect_file_changes(&session.events()[0..session.len() - 1]).len(),
        2,
        "同一区间在 Session 上与在数组上读数一致"
    );
}
