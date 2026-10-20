//! 批 F-24b（审计 M-18 的 c/d/e①/f/g 五条）的等值与行为变化判据。
//!
//! 形状 = 历批口径：**被换掉的那一段原文抄成 `ref_*_old` 参考实现**，在夹具网格上逐格
//! 对账；参考实现随每次 `cargo test` 都跑，等价证明不依赖 `tmp/`（别当代码膨胀删掉——
//! 删了判据就退化成自证）。
//!
//! 三条例的分工：
//! - [`turn_id_keeps_monotonic_when_the_log_lost_a_turn_start_row`] = **d 项的行为变化专钉**。
//!   它驱动生产本体 `run_turn`，不是在测参考实现的副本；旧算法（全量数 `TurnStart` 条数
//!   +1）在这副夹具下会发给出**与日志里已有轮号重复**的号，新算法（反向找最后一个
//!   `TurnStart` 自带的 id +1）不会。
//! - [`lazy_call_event_table_does_not_change_the_projection`] = e① 的等值（懒建那张
//!   `has_call_event` 表**不改变**任何一条 `AgentEvent`）。
//! - [`batch_append_is_byte_identical_to_line_by_line`] = g 的等值（`append_events` 与
//!   逐条 `append_event` 的磁盘字节完全相同）。
//!
//! c 项（`with_llm_config`）的等值不在这里：`llm/tests/dispatch.rs` 已有 4 条
//! `effective_cfg` 用例（会话覆盖 / 子代理路由 / 决策环改写 / 无覆盖），本批改的是
//! **取锁与克隆次数**而不是解析顺序。⚠ 这句原先写着"由门禁 `no_llm_clone_hotpath`
//! 静态计数钉住"，而那个门禁**全仓查无此文**（台账 T5 复核时实测：`no_llm_clone_hotpath`
//! 在本仓库只出现在这一行注释里）——次数从来没人盯，所以 `prepare` 里那次多余的
//! `llm_config()` 克隆一直活着。现在真有一条了：`check-single-source.mjs` 的 **SS3**
//! （`with_llm_config(` 1 次、`.llm_config()` / `.llm()` 各 0 次，配一条内存变异）。

use session_store::{append_event, append_events, Session, SessionEvent, TurnId};

use crate::runner::project_events;
use crate::runner::run_turn;
use crate::tests::{kernel_with_tools, Collector};

/// HEAD 版 `runner/mod.rs` 的轮号算法原文（只把 `session.events()` 收成切片参数，
/// 其余一字未改）。
fn ref_turn_id_old(events: &[SessionEvent]) -> TurnId {
    TurnId(
        events
            .iter()
            .filter(|e| matches!(e, SessionEvent::TurnStart { .. }))
            .count() as u64
            + 1,
    )
}

/// 新算法在生产里的形状（本文件只用来做"网格等值"，行为变化那条例走生产本体）。
fn turn_id_new(events: &[SessionEvent]) -> TurnId {
    TurnId(
        events
            .iter()
            .rev()
            .find_map(|e| match e {
                SessionEvent::TurnStart { turn_id, .. } => Some(turn_id.0),
                _ => None,
            })
            .unwrap_or(0)
            + 1,
    )
}

fn user(text: &str) -> SessionEvent {
    SessionEvent::UserMessage {
        content: text.into(),
        parts: Vec::new(),
    }
}

fn assistant() -> SessionEvent {
    SessionEvent::AssistantMessage {
        content: "答".into(),
        tool_calls: vec![],
    }
}

fn turn_start(id: u64) -> SessionEvent {
    SessionEvent::TurnStart {
        turn_id: TurnId(id),
        ts: 1,
    }
}

/// 投影里的 `tool_uncertain` 卡，取回 `(tool, args)` 两列：空 `tool` = "只有声明位"
/// 那一支的形状，非空 = 有宿主工具行的那一支（`runner/projection.rs` 两支的真实差异）。
fn uncertain_cards(s: &Session) -> Vec<(String, serde_json::Value)> {
    project_events(s.events(), "s-hotpath")
        .iter()
        .filter_map(|e| match e {
            crate::events::AgentEvent::ToolUncertain { tool, args, .. } => {
                Some((tool.clone(), args.clone()))
            }
            _ => None,
        })
        .collect()
}

/// 完整日志（每轮都有轮界）上两式必须逐格等值：轮号本来就等于「轮数 + 1」。
/// 阳性对照 = 网格必须跑满，且"相等"这一档不能是空集。
#[test]
fn both_turn_id_forms_agree_on_well_formed_logs() {
    let mut grids = 0usize;
    for rounds in 0..=6usize {
        let mut events = Vec::new();
        for r in 1..=rounds {
            events.push(turn_start(r as u64));
            events.push(user(&format!("问{r}")));
            events.push(assistant());
        }
        grids += 1;
        assert_eq!(
            ref_turn_id_old(&events),
            turn_id_new(&events),
            "完整日志第 {rounds} 轮两式不等（改前改后必须同号）"
        );
        assert_eq!(turn_id_new(&events).0, rounds as u64 + 1);
    }
    assert_eq!(grids, 7, "网格未跑满");
}

/// **d 项的行为变化专钉**（不在"零行为变化"的口径里，单独立一条）。
///
/// 夹具 = 崩溃恢复后 `load_session` 跳过坏行、日志里**少了一条 `TurnStart`** 的真实形态：
/// `[U, A, TurnStart(2), U, A]`（第一轮的轮界行损坏被跳过，第二轮的还在）。此时
/// 「`TurnStart` 条数」= 1 而「最后一个 id」= 2 ⇒
/// - 旧算法给 `1 + 1 = 2` = **与日志里已有的 `TurnStart(2)` 重号**（计时行与冻结读数都按
///   `TurnId` 配，两轮同号 = 前端把新轮的读数冻到旧轮上）；
/// - 新算法给 `2 + 1 = 3` = 单调不回退。
///
/// 这一条**驱动生产本体 `run_turn`**，不是在测上面的 `turn_id_new` 副本。
#[tokio::test]
async fn turn_id_keeps_monotonic_when_the_log_lost_a_turn_start_row() {
    let kernel = kernel_with_tools();
    let sink = Collector::default();
    let mut session = Session::new();
    // 手工铺一截"缺轮界"的日志（Session 不是 Clone，直接 append）
    for e in [
        user("第一问"),
        assistant(),
        turn_start(2),
        user("第二问"),
        assistant(),
    ] {
        session.append(e);
    }
    // 改前算法在这一夹具下会算出什么——写在这里是为了把"重号"摊开给读的人看
    let old_form = ref_turn_id_old(session.events());
    assert_eq!(
        old_form.0, 2,
        "夹具不对：旧算法应算出与日志已有轮号相同的 2（重号缺陷的形状）"
    );

    run_turn(
        &kernel,
        &mut session,
        "s-turn-id",
        "第三问".into(),
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

    let ids: Vec<u64> = session
        .events()
        .iter()
        .filter_map(|e| match e {
            SessionEvent::TurnStart { turn_id, .. } => Some(turn_id.0),
            _ => None,
        })
        .collect();
    // 新轮必须是全表最大且不与任何既有轮号重复
    assert_eq!(ids.last().copied(), Some(3), "新轮号应为 3，实测 {ids:?}");
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), ids.len(), "日志里出现重复轮号：{ids:?}");
}

/// **e① 的等值**：懒建 `has_call_event` 表不改变投影的任何一条事件。
///
/// 读生产码得到的真实语义（`runner/projection.rs` 的 `ToolCall` 支与 `AssistantMessage`
/// 支）：悬空调用的卡有**两个出处**——有宿主工具行的那张带细节、只有声明位的那张是空细节。
/// `has_call_event` 唯一可观测的作用 = **让"只有声明位"那一支在有宿主行时不重复出卡**。
/// ⇒ 判据落点是**卡数**与**空细节计数**，不是"出不出卡"（我第一版按后者写，实测红了：
/// 档 A 确实出一张卡，只是来自另一支）。既有测试
/// `uncertain_projection_marks_dangling_and_resolves` 只覆盖"没有 ToolCall 事件 ⇒ 出卡"
/// 那一半，本条补的正是"有 ToolCall 事件 ⇒ **不多出**第二张"这一半。
#[test]
fn lazy_call_event_table_does_not_change_the_projection() {
    use plugin_api::{CallId, SideEffect};

    let call = |id: &str| SessionEvent::ToolCall {
        call_id: CallId(id.into()),
        tool: "fs_write".into(),
        args: serde_json::json!({"path": "a.txt"}),
        plugin_id: None,
        plugin_version: semver::Version::new(0, 1, 0),
        side_effect: SideEffect::Compensable {
            undo_tool: String::new(),
        },
    };
    let declared = |ids: &[&str]| SessionEvent::AssistantMessage {
        content: String::new(),
        tool_calls: ids.iter().map(|s| CallId((*s).into())).collect(),
    };

    // 档 A：声明 a1 且 `ToolCall(a1)` 在场、无 ToolResult ⇒ **只**一张卡（宿主行那一支）。
    // 表若失效（恒空）⇒ 声明位那一支重复出一张空细节卡，卡数变 2。
    let mut a = Session::new();
    for e in [user("go"), declared(&["a1"]), call("a1")] {
        a.append(e);
    }
    let cards_a = uncertain_cards(&a);
    assert_eq!(
        cards_a.len(),
        1,
        "有 ToolCall 事件的悬空调用只应出一张卡，实测 {cards_a:?}"
    );

    // 档 B：声明 b1 但 `ToolCall` 事件没落上（死在两次落盘之间）⇒ 一张**空细节**卡
    let mut b = Session::new();
    for e in [user("go"), declared(&["b1"])] {
        b.append(e);
    }
    let cards_b = uncertain_cards(&b);
    assert_eq!(cards_b.len(), 1, "声明位悬空应出一张卡：{cards_b:?}");
    assert!(
        cards_b[0].0.is_empty() && cards_b[0].1.is_null(),
        "档 B 的卡必须是无宿主行的空细节形态，实测 {:?}",
        cards_b[0]
    );

    // 档 C：两支混合（a1 有宿主行 + b1 只有声明）⇒ 恰两张卡、**恰一张**空细节。
    // 这一档最能测"表建了但建错"：错一个 id 就多半张或少半张。
    let mut c = Session::new();
    for e in [user("go"), declared(&["a1", "b1"]), call("a1")] {
        c.append(e);
    }
    let cards_c = uncertain_cards(&c);
    assert_eq!(cards_c.len(), 2, "两档混合应出两张卡，实测 {cards_c:?}");
    assert_eq!(
        cards_c.iter().filter(|(t, _)| t.is_empty()).count(),
        1,
        "恰一张空细节卡（另一张带宿主行细节）：{cards_c:?}"
    );

    // 阳性对照：「空细节卡数」这一读数必须**同时出现过 0 与正数**，否则上面几条断言
    // 可能全是同一形状（防"细节字段写反也照样绿"）。
    let empties: Vec<usize> = [cards_a, cards_b, cards_c]
        .iter()
        .map(|v| v.iter().filter(|(t, _)| t.is_empty()).count())
        .collect();
    assert!(
        empties.contains(&0) && empties.iter().any(|&n| n > 0),
        "三档的空细节卡计数必须既有 0 也有非 0，实测 {empties:?}"
    );
}

/// **g 的等值**：`append_events` 与逐条 `append_event` 写出的磁盘字节完全相同，
/// 且空切片不创建文件（改前的循环对空段本来就什么都不做）。
#[test]
fn batch_append_is_byte_identical_to_line_by_line() {
    let dir = crate::testutil::work_dir("hotpath-append");
    std::fs::create_dir_all(&dir).unwrap();
    let events: Vec<SessionEvent> = vec![
        turn_start(1),
        user("一"),
        SessionEvent::AssistantMessage {
            content: "二".into(),
            tool_calls: vec![],
        },
        user("三"),
    ];

    let per_line = dir.join("per_line.jsonl");
    let batched = dir.join("batched.jsonl");
    for e in &events {
        append_event(&per_line, e).unwrap();
    }
    append_events(&batched, &events).unwrap();

    let a = std::fs::read(&per_line).unwrap();
    let b = std::fs::read(&batched).unwrap();
    assert_eq!(a, b, "批量追加与逐条追加的磁盘字节必须逐字节相同");
    assert_eq!(
        a.iter().filter(|&&c| c == b'\n').count(),
        events.len(),
        "每条事件一行"
    );

    // 追加语义：再批量一段，前一段不动、总行数 = 两段之和
    append_events(&batched, &events[..2]).unwrap();
    let after = std::fs::read(&batched).unwrap();
    assert!(after.starts_with(&a), "批量追加必须是 append 而不是重写");
    assert_eq!(
        after.iter().filter(|&&c| c == b'\n').count(),
        events.len() + 2
    );

    // 空切片 = 连句柄都不开（既有语义：每轮末段可能为空）
    let untouched = dir.join("empty.jsonl");
    append_events(&untouched, &[]).unwrap();
    assert!(!untouched.exists(), "空切片不应创建文件");

    std::fs::remove_dir_all(&dir).ok();
}
