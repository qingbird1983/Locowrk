//! AC-13（手动那一趟的产物**落进会话日志**）。
//!
//! 判据的由来：手动压缩是**独立入口**（不在 `run_turn` 里，它有自己的命令层
//! 入口），以前它自己没落盘 ⇒ 压完看着一切正常（断层条、摘要、界面全对），但
//! 磁盘上还是压前那一份历史 ⇒ **重开应用，压缩连同摘要一起消失**：那轮 LLM 的
//! 钱花了、窗口又回到压前大小。
//!
//! 自动路径那条（`runner` 的 step 边界）另有判据（`tests/compact_wiring.rs`）——
//! 两个触发路径各落各的，漏一个就是一个静默的数据丢失。**变异**：把压缩层
//! `try_compact` 末尾那句落盘删掉 ⇒ 内存里那条压缩事件还在、日志里没有
//! ⇒ 本条立刻红。
//!
//! 落盘口收成一处之后（specs/13 刀⑥）：压缩**自己**落盘（压缩器认日志目录），
//! 三条触发共用这一处 ⇒ 本条钉的是"手动那一趟也在盘上"。

use super::*;

/// **AC-13**：手动 `/压缩` 压成之后，那条压缩事件（连同用量）**进了会话日志**；
/// 磁盘上重放出来的窗口头一条 = 那条 checkpoint（保留区读时重建）。
#[tokio::test]
async fn the_manual_compaction_is_written_to_the_session_log() {
    let script = Script::new(vec![Reply::Text(valid_summary())]);
    let (cp, _root, log) = compactor_logging("s13-manual-log", Arc::clone(&script), true, 50);
    // 六轮够大的对话 ⇒ 保留区切得进中段（与 `compact_wiring` 那份同形）。
    let mut session = session_with_usage(900_000, 6, 2500);
    crate::tests::seed_log(&log, "s-manual", &mut session);
    let outcome = cp
        .run_manual(&mut session, "s-manual", &policy(1_000_000, 50), &hooks())
        .await;
    assert!(
        matches!(outcome, crate::compact::Outcome::Compacted { .. }),
        "手动这一趟该压成（它不受门槛与开关限制），实测 {outcome:?}"
    );
    assert_eq!(script.calls(), 1, "手动那一趟只花一条摘要往返");
    // 内存里那条压缩事件（老口径就成立）——真正要钉的是**磁盘镜像**。
    assert_eq!(
        crate::tests::compacted_count(&session),
        1,
        "内存会话里要有一条压缩事件"
    );
    let path = session_store::session_file(&log, "s-manual").expect("会话日志路径可算");
    let (restored, _) = session_store::load_session(&path).expect("磁盘上有这份日志");
    assert_eq!(
        crate::tests::compacted_count(&restored),
        1,
        "手动压完的产物必须落进会话日志（不然重启后压缩凭空消失）"
    );
    // 重放出来的窗口头一条 = 那条 checkpoint（保留区读时重建；AC-5/AC-6 另有专测）。
    let msgs = restored.derive_messages();
    let Message::User { content, .. } = &msgs[0] else {
        panic!(
            "重放出来的窗口头一条该是那条 checkpoint user 消息，实测 {:?}",
            msgs[0]
        )
    };
    assert!(
        content.starts_with(session_store::CHECKPOINT_TAG),
        "重放出来的窗口头一条该是那条 checkpoint，实测 {content:?}"
    );
    assert!(
        content.contains(&valid_summary()),
        "摘要正文逐字进 checkpoint：{content}"
    );
}
