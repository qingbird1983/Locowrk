//! AC-1（门槛按用户百分比 × 模型窗口）、AC-2（压缩请求 = 真实窗口 + 提示词）、
//! AC-6（保留区只存指针、读时重建）、AC-7（三种触发）。

use super::*;

use session_store::SessionEvent;

/// **AC-1**：门槛 = 窗口 × 用户百分比；不到线不压；**窗口未知不自动触发**。
#[tokio::test]
async fn the_auto_trigger_follows_the_user_threshold_against_the_model_window() {
    let script = Script::new(vec![Reply::Text(valid_summary())]);
    let (cp, _root) = compactor("compact-ac1", Arc::clone(&script), true, 50);
    // 窗口 200K + 阈值 50% ⇒ 门槛 = min(100K, 200K − 21K) = 100K
    // ① 到线（150K ≥ 100K）⇒ 压
    let mut s = session_with_usage(150_000, 4, 200);
    let out = cp.run(&mut s, "ac1", &policy(200_000, 50), &hooks()).await;
    assert!(
        matches!(out, crate::compact::Outcome::Compacted { .. }),
        "到线 ⇒ 压：{out:?}"
    );
    assert_eq!(script.calls(), 1, "压一趟就够");
    // ② 不到线（50K < 100K）⇒ 不压，**且不许发请求**
    let mut s2 = session_with_usage(50_000, 4, 200);
    let out2 = cp
        .run(&mut s2, "ac1b", &policy(200_000, 50), &hooks())
        .await;
    assert!(
        matches!(out2, crate::compact::Outcome::Skipped(_)),
        "不到线 ⇒ 不压：{out2:?}"
    );
    assert_eq!(script.calls(), 1, "不到线不许发请求");
    // ③ 窗口**未知** ⇒ 不自动触发（哪怕占用爆表）
    let mut s3 = session_with_usage(9_000_000, 4, 200);
    let out3 = cp.run(&mut s3, "ac1c", &policy_unknown(50), &hooks()).await;
    assert!(
        matches!(out3, crate::compact::Outcome::Skipped(_)),
        "窗口未知 ⇒ 不自动压：{out3:?}"
    );
    assert_eq!(script.calls(), 1, "窗口未知不许发请求");
}

/// **AC-1 的另一半**：同一窗口下，用户那一格真的在动线（25% ⇒ 50K 就在线上）。
#[tokio::test]
async fn the_user_percentage_moves_the_line() {
    let script = Script::new(vec![Reply::Text(valid_summary())]);
    let (cp, _root) = compactor("compact-ac1d", Arc::clone(&script), true, 25);
    let mut s = session_with_usage(50_000, 4, 200);
    let out = cp.run(&mut s, "ac1d", &policy(200_000, 25), &hooks()).await;
    assert!(
        matches!(out, crate::compact::Outcome::Compacted { .. }),
        "25% ⇒ 门槛 50K ⇒ 占用 50K 就在线上：{out:?}"
    );
    assert_eq!(script.calls(), 1);
}

/// **AC-2**：压缩请求 = 主对话那条管线的真实窗口 **+ 末尾追加提示词那条 user 消息**。
#[tokio::test]
async fn the_compaction_request_is_the_real_conversation_plus_the_summary_prompt() {
    let script = Script::new(vec![Reply::Text(valid_summary())]);
    let (cp, _root) = compactor("compact-ac2", Arc::clone(&script), true, 50);
    let mut s = session_with_usage(150_000, 4, 200);
    let before = s.derive_messages();
    let out = cp.run(&mut s, "ac2", &policy(200_000, 50), &hooks()).await;
    assert!(
        matches!(out, crate::compact::Outcome::Compacted { .. }),
        "这一趟压成了：{out:?}"
    );
    let reqs = script.requests();
    let req = &reqs[0];
    // ① 真实窗口：前 N 条**逐条**等于主对话轮的消息列
    assert_eq!(
        req.messages.len(),
        before.len() + 1,
        "只多一条：末尾那条摘要提示词"
    );
    assert_eq!(
        &req.messages[..before.len()],
        &before[..],
        "前 N 条逐条等于真实窗口（不是另拼一份）"
    );
    // ② 最后一条 = 摘要提示词（一条 user 消息）
    let last = req.messages.last().expect("有最后一条");
    assert!(
        user_text(last).starts_with("You MUST summarize the conversation above"),
        "最后一条是摘要提示词：{:?}",
        user_text(last)
    );
    // ③ 真实工具定义在册（上游同款：宣告真实工具，靠提示词禁调用）
    assert!(!req.tools.is_empty(), "压缩请求带着真实工具目录");
    // ④ 辅助标记（思考降最低档、不发 max_tokens）+ 会话身份
    //    （⇒ 吃会话级模型覆盖 = 上游「压缩用会话的模型」那条语义）
    assert_eq!(
        req.params.get("aux").and_then(|v| v.as_bool()),
        Some(true),
        "压缩请求带辅助标记"
    );
    assert_eq!(req.session_id.as_deref(), Some("ac2"), "会话身份照填");
}

/// **AC-2 的另一半**：hooks 看到的是**加提示词前**的请求（上游 A7）。
#[tokio::test]
async fn the_hooks_see_the_request_before_the_summary_prompt() {
    let seen = Arc::new(StdMutex::new(Vec::<Vec<Message>>::new()));
    let kernel = kernel_capturing_requests(Arc::clone(&seen));
    let script = Script::new(vec![Reply::Text(valid_summary())]);
    let (cp, _root) = compactor_with(
        "compact-ac2b",
        Arc::clone(&script),
        true,
        50,
        Arc::new(kernel),
    );
    let mut s = session_with_usage(150_000, 4, 200);
    let before = s.derive_messages();
    let _ = cp.run(&mut s, "ac2b", &policy(200_000, 50), &hooks()).await;
    let captured = seen.lock().expect("capture poisoned").clone();
    assert_eq!(captured.len(), 1, "决策环走了一趟");
    assert_eq!(
        captured[0].len(),
        before.len(),
        "决策环看到的是加提示词**前**的请求"
    );
    assert!(
        !captured[0]
            .iter()
            .filter_map(maybe_user_text)
            .any(|t| t.starts_with("You MUST summarize")),
        "决策环看不到摘要提示词"
    );
}

/// **AC-6**：保留区**只存指针**、读时重建；切点落在 user 消息边界。
#[tokio::test]
async fn the_kept_recent_text_is_rebuilt_from_the_pointer() {
    let script = Script::new(vec![Reply::Text(valid_summary())]);
    let (cp, _root) = compactor("compact-ac6", Arc::clone(&script), true, 50);
    let mut s = session_with_usage(150_000, 6, 200);
    let out = cp.run(&mut s, "ac6", &policy(200_000, 50), &hooks()).await;
    assert!(
        matches!(out, crate::compact::Outcome::Compacted { .. }),
        "这一趟压成了：{out:?}"
    );
    // ① 事件里**只有指针**（保留区条数 + 起点），没有保留区原文
    let (keep_from, kept) = s
        .events()
        .iter()
        .rev()
        .find_map(|e| match e {
            SessionEvent::ContextCompacted {
                keep_from,
                kept_messages,
                ..
            } => Some((*keep_from, *kept_messages)),
            _ => None,
        })
        .expect("有 ContextCompacted 事件");
    assert!(keep_from > 0, "保留区不是从第 0 条开始");
    assert!(kept > 0, "保留区非空");
    // ② 保留区原文**不进事件日志**（读时重建 ⇒ 日志不膨胀）
    let raw = serde_json::to_string(&s.events()).expect("事件可序列化");
    assert!(
        !raw.contains("[User]: "),
        "保留区文本不进事件日志（只存指针）：{raw}"
    );
    // ③ 重建出来的文本与保留段逐字相等，且切点落在 user 边界
    let msgs = s.derive_messages();
    assert_eq!(msgs.len(), 1, "整段历史换成一条 checkpoint");
    let text = user_text(&msgs[0]);
    let recent = text
        .split("<recent-context>\n")
        .nth(1)
        .expect("checkpoint 里有保留区")
        .split("\n</recent-context>")
        .next()
        .expect("保留区有收尾");
    assert!(
        recent.starts_with("[User]: "),
        "切点落在 user 消息边界（不把一次往返劈开）：{recent}"
    );
    assert!(recent.contains("第5轮问题"), "保留区是最新那几轮：{recent}");
    assert!(
        !recent.contains("第0轮问题"),
        "最老的那几轮进了摘要、不在保留区：{recent}"
    );
}

/// **AC-7**：三种触发（auto / manual / overflow），含「自动压缩关着时 `/压缩`
/// 仍可用」。
#[tokio::test]
async fn the_three_triggers_behave_like_opencode() {
    // ① auto：到线才压，不到线不压
    let s1 = Script::new(vec![Reply::Text(valid_summary())]);
    let (cp1, _r) = compactor("ac7-auto", Arc::clone(&s1), true, 50);
    let mut a = session_with_usage(150_000, 4, 200);
    assert!(
        matches!(
            cp1.run(&mut a, "ac7a", &policy(200_000, 50), &hooks())
                .await,
            crate::compact::Outcome::Compacted { .. }
        ),
        "auto：到线 ⇒ 压"
    );
    let mut b = session_with_usage(10_000, 4, 200);
    assert!(
        matches!(
            cp1.run(&mut b, "ac7b", &policy(200_000, 50), &hooks())
                .await,
            crate::compact::Outcome::Skipped(_)
        ),
        "auto：不到线 ⇒ 不压"
    );
    // ② 自动压缩**关着**：自动那一路不压，手动那一路照样压
    let s2 = Script::new(vec![Reply::Text(valid_summary())]);
    let (cp2, _r) = compactor("ac7-manual", Arc::clone(&s2), false, 50);
    let mut c = session_with_usage(150_000, 4, 200);
    assert!(
        matches!(
            cp2.run(&mut c, "ac7c", &policy(200_000, 50), &hooks())
                .await,
            crate::compact::Outcome::Skipped(_)
        ),
        "开关关着 ⇒ 自动那一路不压"
    );
    let mut d = session_with_usage(150_000, 4, 200);
    assert!(
        matches!(
            cp2.run_manual(&mut d, "ac7d", &policy(200_000, 50), &hooks())
                .await,
            crate::compact::Outcome::Compacted { .. }
        ),
        "开关关着 ⇒ 手动 `/压缩` 照样能压"
    );
    // ③ overflow：供应商拒收之后压**一次**
    let s3 = Script::new(vec![Reply::Text(valid_summary())]);
    let (cp3, _r) = compactor("ac7-overflow", Arc::clone(&s3), true, 50);
    let mut e = session_with_usage(150_000, 4, 200);
    assert!(
        matches!(
            cp3.run_overflow(&mut e, "ac7e", &policy(200_000, 50), &hooks())
                .await,
            crate::compact::Outcome::Compacted { .. }
        ),
        "overflow：压一次"
    );
    assert_eq!(s3.calls(), 1, "overflow 那一趟只发一次摘要请求");
}
