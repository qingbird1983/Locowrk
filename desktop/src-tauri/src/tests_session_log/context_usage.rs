//! 会话上下文用量快照（SessionEvent::ContextUsage）集成两测，自
//! tests_session_log.rs 拆出守 400 行纪律（compact/tests 先例）。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use session_store::{Session, SessionEvent};

use crate::events::AgentEvent;
use crate::runner::run_turn;
use crate::session_log;
use crate::tests::{kernel_with_tools, Collector};

use super::tmp_dir;

/// v2.62 §3.12 上下文统计持久化：run_turn（带 UsagePolicy + log）把
/// `SessionEvent::ContextUsage` 落日志（轮次前 + 每 step 后各一条），
/// 重启恢复后全量投影含同形事件且逐字段透传——切会话回来用量环恢复
/// （此前快照只推实时通道，重放面没有该事件，切回来统计即消失）。
#[tokio::test]
async fn context_usage_persists_and_replays() {
    let dir = tmp_dir("ctx-usage");
    let kernel = kernel_with_tools();
    let mut session = Session::new();
    let sink = Collector::default();

    run_turn(
        &kernel,
        &mut session,
        "s-ctx",
        "echo 你好".into(),
        Vec::new(),
        Some(crate::runner::UsagePolicy {
            threshold_tokens: 8000,
            window_tokens: 32000,
            tools_est_tokens: 512,
        }),
        &sink,
        None,
        Some(&dir),
        crate::runner::DEFAULT_MAX_STEPS,
        None,
        None,
        None,
        None,
    )
    .await
    .expect("turn ok");

    // 实时面：进度条数据源照常推送
    assert!(sink
        .events()
        .iter()
        .any(|e| matches!(e, AgentEvent::ContextUsage { .. })));

    // 重启恢复面：全量投影含同形快照（轮次前 + 每 step 后 ≥ 2 条）
    let map: Arc<Mutex<HashMap<String, Arc<tokio::sync::Mutex<Session>>>>> =
        Arc::new(Mutex::new(HashMap::new()));
    assert_eq!(session_log::restore_all(&dir, &map), 1);
    let restored = map.lock().unwrap().get("s-ctx").cloned().unwrap();
    let restored = restored.lock().await;
    let projected = crate::runner::project_events(restored.events(), "s-ctx");
    let hits: Vec<&AgentEvent> = projected
        .iter()
        .filter(|e| matches!(e, AgentEvent::ContextUsage { .. }))
        .collect();
    assert!(
        hits.len() >= 2,
        "轮次前 + 每 step 后各一条快照，实际 {} 条",
        hits.len()
    );
    match hits.last() {
        Some(AgentEvent::ContextUsage {
            used,
            threshold,
            window_tokens,
            tools_est_tokens,
            ..
        }) => {
            assert_eq!(*threshold, 8000);
            assert_eq!(*window_tokens, 32000);
            assert_eq!(*tools_est_tokens, 512);
            assert!(*used > 0, "估认真发生");
        }
        other => panic!("末条投影应为 ContextUsage：{:?}", other.is_none()),
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// v2.138 口径统一：会话已有供应商真实上报（LlmUsage）时，ContextUsage
/// 的 `used` 同吃真实 input 锚点，不再字符估算——进度条/用量环与整理
/// 触发（Organizer::anchor）同一把尺，「看到的 = 触发用的」。压力实测
/// 抓的分裂（环显估算 110K、触发判真实 95K < 100K 阈值不整理）锁此处。
/// 无上报回落估算由上一测覆盖（demo 内核不报用量，used > 0 即估算路径）。
#[tokio::test]
async fn context_usage_prefers_reported_input() {
    let dir = tmp_dir("ctx-usage-real");
    let kernel = kernel_with_tools();
    let mut session = Session::new();
    // 预置一条真实上报（字段形状复制自供应商轮次；数值取压力实测最后
    // 一步：真实 95,483 vs 估算 112,606 的那轮）。demo 内核不再产生新
    // 上报 → 轮次前 + 每 step 后的每条快照都必须吃这个锚点。
    session.append(SessionEvent::LlmUsage {
        input: 95483,
        output: 546,
        cache_read: 93312,
        cache_write: 0,
        reasoning: 47,
        total: 96029,
        duration_ms: 8487,
    });
    let sink = Collector::default();

    run_turn(
        &kernel,
        &mut session,
        "s-ctx-real",
        "echo 你好".into(),
        Vec::new(),
        Some(crate::runner::UsagePolicy {
            threshold_tokens: 8000,
            window_tokens: 32000,
            tools_est_tokens: 512,
        }),
        &sink,
        None,
        Some(&dir),
        crate::runner::DEFAULT_MAX_STEPS,
        None,
        None,
        None,
        None,
    )
    .await
    .expect("turn ok");

    let useds: Vec<u64> = session
        .events()
        .iter()
        .filter_map(|e| match e {
            SessionEvent::ContextUsage { used, .. } => Some(*used),
            _ => None,
        })
        .collect();
    assert!(
        useds.len() >= 2,
        "轮次前 + 每 step 后各一条快照，实际 {} 条",
        useds.len()
    );
    assert!(
        useds.iter().all(|&u| u == 95483),
        "used 应全为真实 input 95483（估算回落不得发生），实际 {useds:?}"
    );
    std::fs::remove_dir_all(&dir).ok();
}
