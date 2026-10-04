//! 稳定期纪律单测（M1，KV v2 §6.4）：结算后 N 个用户轮内不触发改前缀
//! 操作，越过 N 轮放行下一次结算。独立成文件守 400 行纪律。

use std::sync::{Arc, RwLock};

use cordis_core::{Context, WaterfallDecision, WaterfallListener};
use serde_json::json;
use session_store::Message;

use crate::middleware::compact::{CompactConfig, CompactStrategy, CtxCompact};
use crate::waterfall::{RequestPayload, points};

/// 与主套件同款的交替长消息流（每条约 100 token）。
fn long_transcript(turns: usize) -> Vec<Message> {
    let filler = "x".repeat(400);
    let mut messages = Vec::new();
    for i in 0..turns {
        messages.push(Message::User {
            content: format!("turn {i}: {filler}"),
        });
        messages.push(Message::Assistant {
            content: format!("reply {i}: {filler}"),
            tool_calls: vec![],
        });
    }
    messages
}

// ---- M1 稳定期纪律（KV v2 §6.4）----

/// 结算后 N 轮内不再触发：结算当轮（diff 0）与之后 N 个用户轮被抑制，
/// 第 N+1 轮放行下一次结算。payload 模拟真实流的重建方式 = 上一次结算
/// 后的消息视图 + 新追加的用户轮。
#[tokio::test]
async fn stability_period_suppresses_settlements_after_compaction() {
    let ctx = Arc::new(Context::new());
    let cfg = Arc::new(RwLock::new(CompactConfig {
        threshold_tokens: 400,
        strategy: CompactStrategy::SlidingWindow, // 本地裁剪，不需要 LLM
        stability_rounds: 2,
    }));
    let compact = CtxCompact::new(ctx.clone(), cfg);

    async fn fire(compact: &CtxCompact, messages: Vec<Message>) -> RequestPayload {
        let payload = serde_json::to_value(RequestPayload {
            messages,
            model: None,
            params: json!({}),
            tools: vec![],
            compacted: None,
            system: None,
            session_id: Some("s1".into()),
        })
        .unwrap();
        match compact.handle(points::REQUEST, &payload).await {
            WaterfallDecision::Replace(v) => serde_json::from_value(v).unwrap(),
            WaterfallDecision::Continue => serde_json::from_value(payload).unwrap(),
            _ => panic!("unexpected decision"),
        }
    }

    // 用户轮 1（10 条长消息 > 阈值）→ 结算一次
    let r1 = fire(&compact, long_transcript(10)).await;
    let Some(c1) = &r1.compacted else {
        panic!("超阈值必须结算");
    };
    assert_eq!(c1.strategy, "sliding_window");
    // 结算视图 = r1.messages（真实流下一次请求从此重建）
    let settled_view = r1.messages.clone();
    let users_of = |ms: &[Message]| {
        ms.iter()
            .filter(|m| matches!(m, Message::User { .. }))
            .count()
    };
    let settled_users = users_of(&settled_view);

    let new_user = |n: usize| Message::User {
        content: format!("fresh turn {n}: {}", "x".repeat(400)),
    };
    let mut grown = settled_view.clone();
    let mut fresh = 0usize;

    // 结算当轮的后续 step（无新用户轮）→ 抑制
    let r_same = fire(&compact, settled_view.clone()).await;
    assert!(r_same.compacted.is_none(), "结算当轮不二次结算");

    // 之后 2 个用户轮 = 稳定期 → 抑制
    for _ in 0..2 {
        fresh += 1;
        grown.push(new_user(fresh));
        let r = fire(&compact, grown.clone()).await;
        assert!(r.compacted.is_none(), "稳定期内（第 {fresh} 轮）必须抑制");
    }

    // 第 3 个用户轮（diff = 3 > N=2）→ 放行下一次结算
    fresh += 1;
    grown.push(new_user(fresh));
    let r_next = fire(&compact, grown).await;
    assert!(r_next.compacted.is_some(), "稳定期结束后放行下一次结算");
    assert!(users_of(&settled_view) == settled_users);
}
