//! specs/13 刀⑥ 的判据（AC-20…AC-22）：**唯一落盘口** + 游标单调 + 失败轮次也落盘。
//!
//! 这一族盯的是"落盘"这件事的**两面**（老教训：界面只显示一面，磁盘是另一面）——
//! 内存里对的东西，磁盘上可能**没有**（缺口）或**写了两遍**（重影）。两者都**不报错、
//! 不变红、界面照常**，只有重开应用才看得见（那一轮 LLM 的钱花了、内容没了）。
//!
//! 改前：落盘游标**每个调用点自己算**（runner 的 `log_start` / `before` / `len-1`、
//! 压缩层的 `from`、heal 的 `seq`）⇒ "磁盘上到哪儿了"有好几个出处 ⇒ 漏一处是缺口、
//! 算重一处是重影；**失败的轮次**里那一步追加的事件干脆不落盘（`finish_turn` 只落
//! `TurnEnd`）。改后：游标住会话（`Session::persisted`），**唯一落盘口**
//! `session_log::persist_pending`。
//!
//! **变异**（改坏哪一处，本族要立刻红）：
//! · 游标推进写成 `len - 1`（少推进一条）⇒ 下一趟重写最后一条 ⇒ **重影**（AC-20 红）。
//! · `persist_pending` 无视游标、每次都写整段 ⇒ 落两次 ⇒ **重影**（AC-20 红）。
//! · `finish_turn` 退回"只落 `TurnEnd`" ⇒ 失败轮次的步内事件丢失 ⇒ **缺口**（AC-22 红）。
//! · 压缩层不落盘（`try_compact` 末尾那句删掉）⇒ 压缩事件不在盘上 ⇒ **缺口**
//!   （AC-21 / AC-22 红）。

use super::*;

use crate::runner::run_turn;
use session_store::SessionEvent;

/// 磁盘日志的行数（= 落了几条事件；空行不算）。
fn log_lines(log: &std::path::Path, id: &str) -> usize {
    let path = session_store::session_file(log, id).expect("会话日志路径可算");
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    text.lines().filter(|l| !l.trim().is_empty()).count()
}

/// 读回磁盘上的事件序列（按盘上原序）。
fn log_events(log: &std::path::Path, id: &str) -> Vec<SessionEvent> {
    let path = session_store::session_file(log, id).expect("会话日志路径可算");
    let (restored, _) = session_store::load_session(&path).expect("磁盘上有这份日志");
    restored.events().to_vec()
}

/// **AC-20**：落盘口是唯一的 ⇒ 游标单调 ⇒ **同一段落落两次不重影**。
///
/// 这一条是"重影"那一半的判据：改前每个调用点自己算起点，两处各写一遍就是同一条
/// 事件落两遍（磁盘上多一行，重放时那条事件出现两次）。改后起点住会话 ⇒ 第二次
/// 落盘是**空段**（游标已到末尾）⇒ 一个字节都不多。
#[test]
fn the_persist_path_is_the_only_one_and_twice_is_not_a_double_write() {
    let root = crate::testutil::work_dir("s13-persist-unique");
    let log = root.join("sessions");
    let mut session = Session::new();
    session.append(SessionEvent::UserMessage {
        content: "一".into(),
        parts: Vec::new(),
    });
    session.append(SessionEvent::AssistantMessage {
        content: "答".into(),
        tool_calls: vec![],
    });
    session.append(SessionEvent::UserMessage {
        content: "二".into(),
        parts: Vec::new(),
    });

    crate::session_log::persist_pending(&log, "s-idem", &mut session);
    assert_eq!(log_lines(&log, "s-idem"), 3, "第一趟把三条都写下去");

    // **再落一次**（没有新东西）：游标单调 ⇒ 一个字节都不许多。
    crate::session_log::persist_pending(&log, "s-idem", &mut session);
    assert_eq!(
        log_lines(&log, "s-idem"),
        3,
        "同一段落落两次不许出现重复行（重影）"
    );

    // 追加一条再落：只多一行，且与内存逐条相等。
    session.append(SessionEvent::AssistantMessage {
        content: "再答".into(),
        tool_calls: vec![],
    });
    crate::session_log::persist_pending(&log, "s-idem", &mut session);
    assert_eq!(log_lines(&log, "s-idem"), 4, "新的一条只写一次");
    assert_eq!(
        log_events(&log, "s-idem"),
        session.events().to_vec(),
        "盘上与内存逐条相等"
    );
}

/// **AC-21**：磁盘镜像与内存会话**逐条相等**（无缺口、无重影）：手动压缩与主对话轮
/// 都走**同一个**落盘口。
///
/// 这一条盯的是"两处各落各的"那一半：改前自动那条在 runner、手动那条在压缩层，
/// 各自的起点各算各的 ⇒ 同一段可能被写两遍（重影），也可能被跳过一段（缺口）。
/// 改后两边都走 `persist_pending`，游标住会话 ⇒ 盘上就是内存那份日志。
#[tokio::test]
async fn the_disk_mirror_matches_the_session_event_for_event() {
    let script = Script::new(vec![Reply::Text(valid_summary())]);
    let (cp, _root, log) = compactor_logging("s13-persist-mirror", Arc::clone(&script), true, 50);
    // 六轮够大的对话 ⇒ 保留区切得进中段（与 `compact_wiring` 那份同形）。
    let mut session = session_with_usage(900_000, 6, 2500);
    crate::tests::seed_log(&log, "s-mirror", &mut session);

    // ① 手动那一趟：压缩**自己**落盘（压缩器认日志目录）。
    let out = cp
        .run_manual(&mut session, "s-mirror", &policy(1_000_000, 50), &hooks())
        .await;
    assert!(
        matches!(out, crate::compact::Outcome::Compacted { .. }),
        "手动这一趟该压成，实测 {out:?}"
    );
    assert_eq!(
        crate::tests::compacted_count(&session),
        1,
        "内存里有一条压缩事件"
    );
    let on_disk = log_events(&log, "s-mirror");
    let compacted_on_disk = on_disk
        .iter()
        .filter(|e| matches!(e, SessionEvent::ContextCompacted { .. }))
        .count();
    assert_eq!(
        compacted_on_disk, 1,
        "盘上也要有那条压缩事件（压缩自己落盘）"
    );

    // ② 主对话轮：runner 落盘（同一把尺、同一个游标）。
    let collector = crate::tests::Collector::default();
    run_turn(
        &crate::tests::kernel_with_tools(),
        &mut session,
        "s-mirror",
        "继续".into(),
        Vec::new(),
        Some(policy(1_000_000, 50)),
        &collector,
        None,
        Some(&log),
        crate::runner::DEFAULT_MAX_STEPS,
        None,
        None,
        Some(&cp),
    )
    .await
    .expect("这一轮照常跑完");

    // ③ 两面逐条相等：不多（重影）也不少（缺口）。
    assert_eq!(
        log_events(&log, "s-mirror"),
        session.events().to_vec(),
        "磁盘镜像必须与内存会话逐条相等（重影与缺口都不许有）"
    );
}

/// **AC-23**：**重开应用**（恢复路径）之后，老历史**不被重写一遍**。
///
/// 这一条盯的是"游标在恢复路径上落定"：`load_session` 读回来的事件**盘上已经有了**
/// ⇒ 游标 = 事件数。不落定的话，重开一次应用就会把整段历史**再写一遍**（重影）——
/// 而"重开应用"恰恰是用户最常做的动作（切会话、退出再进）。
///
/// **变异**：`load_session` 里那句 `mark_persisted` 删掉 ⇒ 本条立刻红（盘上多出一整段）。
#[test]
fn a_restored_session_does_not_rewrite_its_history() {
    let root = crate::testutil::work_dir("s13-persist-restore");
    let log = root.join("sessions");
    let mut session = Session::new();
    session.append(SessionEvent::UserMessage {
        content: "一".into(),
        parts: Vec::new(),
    });
    session.append(SessionEvent::AssistantMessage {
        content: "答".into(),
        tool_calls: vec![],
    });
    crate::session_log::persist_pending(&log, "s-restore", &mut session);
    assert_eq!(log_lines(&log, "s-restore"), 2, "前史在盘上");

    // 模拟"重开应用"：从磁盘恢复回内存（`restore_all` 走的就是 `load_session`）。
    let map: std::sync::Arc<
        std::sync::Mutex<
            std::collections::HashMap<String, std::sync::Arc<tokio::sync::Mutex<Session>>>,
        >,
    > = std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::new()));
    assert_eq!(
        crate::session_log::restore_all(&log, &map),
        1,
        "恢复到一个会话"
    );
    let arc = map
        .lock()
        .unwrap()
        .get("s-restore")
        .cloned()
        .expect("恢复出来的会话在表里");
    let mut restored = arc.blocking_lock();
    assert_eq!(restored.len(), 2, "恢复出来两条");

    // 恢复之后再落一条 ⇒ 盘上 = 原来那两条 + 新的一条（**老历史没被重写**）。
    restored.append(SessionEvent::UserMessage {
        content: "三".into(),
        parts: Vec::new(),
    });
    crate::session_log::persist_pending(&log, "s-restore", &mut restored);
    assert_eq!(
        log_lines(&log, "s-restore"),
        3,
        "重开应用之后老历史被重写了一遍（重影）"
    );
    assert_eq!(
        log_events(&log, "s-restore"),
        restored.events().to_vec(),
        "盘上与内存逐条相等"
    );
}

/// **AC-22**：**失败轮次**的步内事件进日志。
///
/// 这一条是改前最刺眼的那个缺口：供应商按"太长"拒收 → 压一次（**压成了**，那两条
/// 事件真的发生了）→ 重试仍拒收 ⇒ 轮以 `Failed` 收束。改前 `finish_turn` 只落
/// `TurnEnd` ⇒ **压缩那两条永远不落盘**：压过了、花了钱、重开应用却看不见发生过。
#[tokio::test]
async fn a_failed_turn_keeps_the_step_events_in_the_log() {
    let script = Script::new(vec![Reply::Text(valid_summary())]);
    let (cp, _root, log) = compactor_logging("s13-failed-turn", Arc::clone(&script), true, 50);
    // 占用 100K < 门槛 500K ⇒ 自动那一路不起步 ⇒ 这一轮走的是"供应商拒收 → 压一次"。
    // 历史 ~20K token（> 保留区 15K ⇒ 切得出保留指针；≤ 溢出目标 70K ⇒ 装得下）。
    let mut session = session_with_usage(100_000, 8, 1700);
    crate::tests::seed_log(&log, "s-fail", &mut session);
    let kernel = failing_kernel("context length exceeded");
    let collector = crate::tests::Collector::default();

    let err = run_turn(
        &kernel,
        &mut session,
        "s-fail",
        "来一句".into(),
        Vec::new(),
        Some(policy(1_000_000, 50)),
        &collector,
        None,
        Some(&log),
        crate::runner::DEFAULT_MAX_STEPS,
        None,
        None,
        Some(&cp),
    )
    .await
    .expect_err("重试仍被拒收 ⇒ 这一轮以失败收束");
    assert!(
        err.contains("context length exceeded"),
        "冒泡的是原始错误：{err}"
    );
    assert_eq!(script.calls(), 1, "溢出那一趟压过一次（压成了）");

    let on_disk = log_events(&log, "s-fail");
    assert!(
        on_disk
            .iter()
            .any(|e| matches!(e, SessionEvent::ContextCompacted { .. })),
        "压成那条必须在盘上（压过了、钱花了，不能重开就没了）"
    );
    assert!(
        on_disk.iter().any(|e| matches!(
            e,
            SessionEvent::TurnEnd {
                outcome: session_store::TurnOutcome::Failed,
                ..
            }
        )),
        "失败收束必须在盘上（重放才知道这轮停在哪）"
    );
    assert_eq!(
        on_disk,
        session.events().to_vec(),
        "失败轮次也一样：磁盘镜像与内存会话逐条相等"
    );
}
