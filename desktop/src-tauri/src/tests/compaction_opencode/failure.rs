//! AC-8（失败处置三档）、AC-9（收缩矩阵）、AC-10（失败也照记 token 用量）。

use super::*;

use crate::runner::run_turn;
use session_store::SessionEvent;

/// **AC-8**：auto 失败 ⇒ 整步失败；overflow 失败 ⇒ 原始错误照常冒泡；
/// manual 失败 ⇒ 主轮继续（窗口没被动过）。
#[tokio::test]
async fn a_failed_compaction_does_not_break_the_queue() {
    // ① auto 失败 ⇒ 整步失败
    let script = Script::new(vec![Reply::Fail("供应商炸了".into())]);
    let (cp, _root) = compactor("ac8-auto", Arc::clone(&script), true, 50);
    let kernel = crate::tests::kernel_with_tools();
    let sink = crate::tests::Collector::default();
    let mut s = session_with_usage(150_000, 4, 200);
    let err = run_turn(
        &kernel,
        &mut s,
        "ac8",
        "来一句".into(),
        Vec::new(),
        Some(policy(200_000, 50)),
        &sink,
        None,
        None,
        4,
        None,
        None,
        Some(&cp),
    )
    .await;
    let err = err.expect_err("auto 失败 ⇒ 整步失败");
    assert!(
        err.contains("provider-error"),
        "失败要报判据名（前端按名查文案）：{err}"
    );
    // ② overflow 失败 ⇒ **不恢复**：原始错误照常冒泡
    //
    // 占用**不到线**（10K < 门槛 100K）⇒ 自动那一路不起步 ⇒ 这一轮走的是
    // "供应商拒收 → 压一次"那条路（overlap 那一路不看线）。
    let script2 = Script::new(vec![Reply::Fail("供应商炸了".into())]);
    let (cp2, _root) = compactor("ac8-overflow", Arc::clone(&script2), true, 50);
    let kernel2 = failing_kernel("context length exceeded");
    let sink2 = crate::tests::Collector::default();
    let mut s2 = session_with_usage(10_000, 4, 200);
    let err2 = run_turn(
        &kernel2,
        &mut s2,
        "ac8b",
        "来一句".into(),
        Vec::new(),
        Some(policy(200_000, 50)),
        &sink2,
        None,
        None,
        4,
        None,
        None,
        Some(&cp2),
    )
    .await
    .expect_err("overflow 压不成 ⇒ 原始错误照常冒泡");
    assert!(
        err2.contains("context length exceeded"),
        "冒泡的是**原始**错误：{err2}"
    );
    assert!(
        !err2.contains("context compaction failed"),
        "压缩那一趟的失败不顶替原始错误：{err2}"
    );
    // ③ manual 失败 ⇒ 主轮继续（会话窗口没被动过，也没有压成事件）
    let script3 = Script::new(vec![Reply::Fail("供应商炸了".into())]);
    let (cp3, _root) = compactor("ac8-manual", Arc::clone(&script3), true, 50);
    let mut s3 = session_with_usage(150_000, 4, 200);
    let before = s3.derive_messages();
    let out = cp3
        .run_manual(&mut s3, "ac8c", &policy(200_000, 50), &hooks())
        .await;
    assert_eq!(out.reason(), Some("provider-error"), "手动失败也报判据名");
    assert!(
        !s3.events()
            .iter()
            .any(|e| matches!(e, SessionEvent::ContextCompacted { .. })),
        "没压成就不许落压成事件"
    );
    assert_eq!(
        s3.derive_messages().len(),
        before.len(),
        "窗口没被动过 ⇒ 主轮可以继续"
    );
}

/// **AC-9**：收缩矩阵 —— 递减带 70% / 50% / 35%，用尽 ⇒ 失败（文案逐字）。
#[tokio::test]
async fn the_compaction_shrinks_in_opencode_steps() {
    // ① 递减带逐字（上游 `compaction.ts:93` `SHRINK_STEPS`）
    let mut shrink = crate::compact::deliver::Shrink::new(1000, 1000);
    assert_eq!(shrink.on_too_long(1000), Some(0.7), "第一档 70%");
    assert_eq!(shrink.target, 700);
    assert_eq!(shrink.on_too_long(1000), Some(0.5), "第二档 50%");
    assert_eq!(shrink.target, 500);
    assert_eq!(shrink.on_too_long(1000), Some(0.35), "第三档 35%");
    assert_eq!(shrink.target, 350);
    assert_eq!(shrink.on_too_long(1000), None, "递减带用尽 ⇒ 失败");
    // ② 那句省略标记逐字（上游 `compaction.ts:514-515`）
    assert_eq!(
        crate::compact::recent::omitted_note(3),
        "[3 older exchanges omitted]\n\n"
    );
    assert_eq!(
        crate::compact::recent::omitted_note(1),
        "[1 older exchange omitted]\n\n"
    );
    assert_eq!(crate::compact::recent::omitted_note(0), "");
    // ③ 端到端：被拒一次 ⇒ 缩一次再发（第二趟更小），压成
    let script = Script::new(vec![
        Reply::Fail("context length exceeded".into()),
        Reply::Text(valid_summary()),
    ]);
    let (cp, _root) = compactor("ac9", Arc::clone(&script), true, 50);
    let mut s = session_with_usage(150_000, 8, 400);
    let out = cp.run(&mut s, "ac9", &policy(200_000, 50), &hooks()).await;
    assert_eq!(script.calls(), 2, "被拒一次 ⇒ 缩一次再发");
    let reqs = script.requests();
    let first = crate::compact::deliver::estimate_request(&reqs[0]);
    let second = crate::compact::deliver::estimate_request(&reqs[1]);
    assert!(second < first, "第二趟更小：{first} → {second}");
    assert!(
        matches!(out, crate::compact::Outcome::Compacted { .. }),
        "缩过了就压成：{out:?}"
    );
}

/// **AC-10**：失败 / 中断的压缩照样把已发生的 token 记进会话用量。
#[tokio::test]
async fn a_failed_compaction_is_still_billed() {
    let script = Script::new(vec![Reply::Text("就一段没有标题的散文".into())]);
    let (cp, _root) = compactor("ac10", Arc::clone(&script), true, 50);
    let mut s = session_with_usage(150_000, 4, 200);
    let out = cp.run(&mut s, "ac10", &policy(200_000, 50), &hooks()).await;
    assert!(out.reason().is_some(), "这一趟没成：{out:?}");
    let usages: Vec<(u64, u64)> = s
        .events()
        .iter()
        .filter_map(|e| match e {
            SessionEvent::LlmUsage { input, output, .. } => Some((*input, *output)),
            _ => None,
        })
        .collect();
    assert!(
        usages.contains(&(222, 444)),
        "失败也照记 token 用量（这一趟发了两趟：原样 + 一次提醒，两趟都记）：{usages:?}"
    );
}
