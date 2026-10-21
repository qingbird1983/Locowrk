//! **接线面**：压缩的**界面反馈**与**摘要预算**（specs/12 刀② · AC-5 / AC-3 界面半 / AC-7）。
//!
//! 形状照 `compact_wiring.rs`（同一种"驱动 `run_turn` 本体、读事件收集器"的取证
//! 载体），但**各抓各的一层**：
//! ① 压缩**开始**与**没成**各推一条相位事件（改之前这一层只有 `tracing::debug!`
//!    ⇒ 界面上零反馈，用户报告的"卡在思考中"就是这么来的）；
//! ② 熔断停手推「已暂停」相位，且那一轮**不**推「正在压缩」（判说不用压时压根
//!    没有压缩发生，界面不许冒出没发生过的「正在压缩」）；
//! ③ 摘要请求带的那枚预算 = 设置里的那一档，且**下一轮就生效**（不重启）。
//!
//! 为什么另起一个文件而不是并进 `compact_wiring.rs`：那一簇抓的是"压成之后
//! 落盘与投影同形"，这一簇抓的是"压缩**过程**的可见性与预算"——两簇的夹具
//! 相同（`loaded_session` / `threshold_at` / `seed_log`，已上移 `tests/mod.rs`），
//! 断言对象不同。文件超 400 行时"拆"是正解，不是"把行数摊到别处"。

use super::*;

use crate::events::AgentEvent;
use crate::llm::{SummaryClient, SummaryRequest, SummaryUsage};
use crate::runner::{run_turn, UsagePolicy};
use async_trait::async_trait;
use session_store::Session;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

/// 一个"摘要模型没答上"的客户端：如实报错 ⇒ 压缩层记一次失败并给出
/// `WHY_CALL`（界面那半要说的"没压成：摘要模型没答上"就是从这枚判据名来的）。
///
/// 为什么报错文本**不**带任何关键词：判别"超窗 / 预算耗尽"靠的是
/// `llm::retry` 那两份判据（`is_overflow_message` / `is_budget_exhausted`），
/// 压缩层不自备关键词表。这条错误串刻意普通 ⇒ 落进 `WHY_CALL` 那一档。
#[derive(Default)]
struct Failing {
    calls: AtomicUsize,
}

#[async_trait]
impl SummaryClient for Failing {
    async fn complete(&self, _req: &SummaryRequest<'_>) -> Result<(String, SummaryUsage), String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err("摘要模型没答上".into())
    }
}

/// 装一个"摘要必失败"的自动压缩器（返回熔断表供断言）。
fn failing_compactor(
    root: &std::path::Path,
) -> (
    Arc<crate::compact::deep::Compactor>,
    Arc<crate::compact::Failures>,
    Arc<Failing>,
) {
    std::fs::write(
        root.join("settings.json"),
        serde_json::json!({"context": {"thresholdTokens": 100_000}}).to_string(),
    )
    .unwrap();
    let settings = Arc::new(crate::settings::SettingsStore::open(root).unwrap());
    let failures = Arc::new(crate::compact::Failures::default());
    let cli = Arc::new(Failing::default());
    (
        Arc::new(crate::compact::deep::Compactor::new(
            settings,
            Arc::clone(&cli) as Arc<dyn SummaryClient>,
            Arc::clone(&failures),
            Arc::new(crate::compact::Overflow::default()),
            None,
        )),
        failures,
        cli,
    )
}

/// 相位事件的读数（`(相位, 判据名)` 序列）——界面那半的输入。
fn compact_phases(collector: &Collector) -> Vec<(String, Option<String>)> {
    collector
        .events()
        .iter()
        .filter_map(|e| match e {
            AgentEvent::CompactStatus { phase, reason, .. } => {
                Some((phase.clone(), reason.clone()))
            }
            _ => None,
        })
        .collect()
}

/// AC-5 接线面（specs/12 刀②）：自动压缩**开始**与**没成**各推一条相位事件。
///
/// 改之前这一层只有 `tracing::debug!` ⇒ 界面上零反馈。用户报告的"卡在思考中"
/// 就是这么来的：那一轮在等摘要，而用户什么也看不见（连"正在压"都没有）。
///
/// 顺序也要钉住：「开始」必须在「没成」**之前**（那是同一趟压缩的两个时刻，
/// 顺序反了界面会先显示失败再显示开始）。
#[tokio::test]
async fn the_auto_compaction_reports_its_start_and_its_failure_to_the_ui() {
    let root = crate::testutil::work_dir("s12-status-wiring");
    let (cp, failures, cli) = failing_compactor(&root);
    let log = root.join("sessions");
    let mut session = loaded_session();
    seed_log(&log, "s-status", &session);
    let line = threshold_at(&session);
    let collector = Collector::default();
    let steps = run_turn(
        &kernel_with_tools(),
        &mut session,
        "s-status",
        "继续".into(),
        Vec::new(),
        Some(UsagePolicy {
            threshold_tokens: line,
            window_tokens: 1_000_000,
            tools_est_tokens: 0,
            refused_tokens: 0,
        }),
        &collector,
        None,
        Some(&log),
        crate::runner::DEFAULT_MAX_STEPS,
        None,
        None,
        Some(&cp),
    )
    .await
    .expect("turn ok");
    assert!(steps >= 1, "轮次照常收束（压缩失败不许杀死主任务）");
    let phases = compact_phases(&collector);
    assert_eq!(
        phases,
        vec![
            (crate::events::compact_phase::RUNNING.to_string(), None,),
            (
                crate::events::compact_phase::FAILED.to_string(),
                Some(crate::compact::deep::WHY_CALL.to_string()),
            ),
        ],
        "自动压缩要把「开始」与「没成」都推给界面（实测 {phases:?}）"
    );
    assert_eq!(
        cli.calls.load(Ordering::SeqCst),
        1,
        "这一趟只发了一条摘要请求"
    );
    assert_eq!(failures.of("s-status"), 1, "失败要记一次（推进熔断的输入）");
}

/// AC-3 界面那半（specs/12 刀②）：连续 3 次失败后，第 4 次到线时推「已暂停」。
///
/// 这一条把"熔断"从**判据名**那一族里拎出来单测：熔断在判据名表里属于"手动
/// 让开"的一族（手动 `/压缩` 压根不熔断），所以它走**相位** `paused`。
///
/// 顺带钉住两件事：① 第 4 次**不再发摘要请求**（熔断的意义就在这儿）；
/// ② 第 4 次**不发「正在压缩」**——判说"不用压"时压根没有压缩发生，界面不许
/// 冒出没发生过的「正在压缩」。
#[tokio::test]
async fn the_breaker_pauses_the_auto_compaction_in_the_ui() {
    let root = crate::testutil::work_dir("s12-paused-wiring");
    let (cp, failures, cli) = failing_compactor(&root);
    let log = root.join("sessions");
    let mut session = loaded_session();
    seed_log(&log, "s-paused", &session);
    let line = threshold_at(&session);
    let collector = Collector::default();
    // 连跑四轮（每轮一步 ⇒ 每轮一次压缩尝试；DemoLlm 不发工具调用，轮次一步收敛）
    for _ in 0..4 {
        run_turn(
            &kernel_with_tools(),
            &mut session,
            "s-paused",
            "继续".into(),
            Vec::new(),
            Some(UsagePolicy {
                threshold_tokens: line,
                window_tokens: 1_000_000,
                tools_est_tokens: 0,
                refused_tokens: 0,
            }),
            &collector,
            None,
            Some(&log),
            crate::runner::DEFAULT_MAX_STEPS,
            None,
            None,
            Some(&cp),
        )
        .await
        .expect("turn ok");
    }
    let phases = compact_phases(&collector);
    assert_eq!(
        phases,
        vec![
            (crate::events::compact_phase::RUNNING.to_string(), None),
            (
                crate::events::compact_phase::FAILED.to_string(),
                Some(crate::compact::deep::WHY_CALL.to_string()),
            ),
            (crate::events::compact_phase::RUNNING.to_string(), None),
            (
                crate::events::compact_phase::FAILED.to_string(),
                Some(crate::compact::deep::WHY_CALL.to_string()),
            ),
            (crate::events::compact_phase::RUNNING.to_string(), None),
            (
                crate::events::compact_phase::FAILED.to_string(),
                Some(crate::compact::deep::WHY_CALL.to_string()),
            ),
            (crate::events::compact_phase::PAUSED.to_string(), None),
        ],
        "三次失败之后要推「已暂停」，且那一轮**不**推「正在压缩」（实测 {phases:?}）"
    );
    assert_eq!(
        cli.calls.load(Ordering::SeqCst),
        3,
        "熔断之后不许再发摘要请求（第 4 次那次压根没发）"
    );
    assert_eq!(failures.of("s-paused"), 3, "失败计数 = 熔断线那三格");
}

/// AC-7 后端那半（specs/12 刀②）：摘要请求带的那枚预算 = 设置里的那一档，
/// 且**下一轮就生效**（不重启、不重开应用）。
///
/// 为什么这条住在**接线面**而不是压缩层单元测试：设置项改完到下一次压缩读到
/// 新值，中间隔着"压缩层每轮重新取一次活配置"这一跳——单元测试改不了设置。
struct BudgetProbe {
    budgets: StdMutex<Vec<Option<Duration>>>,
}

#[async_trait]
impl SummaryClient for BudgetProbe {
    async fn complete(&self, req: &SummaryRequest<'_>) -> Result<(String, SummaryUsage), String> {
        self.budgets.lock().unwrap().push(req.budget);
        // 如实失败：压缩不落账 ⇒ 占用读数不回落 ⇒ 下一轮照样到线、照样再压一次
        // （压成的话第二轮就不压了，测不到"下一轮生效"）。
        Err("摘要模型没答上".into())
    }
}

#[tokio::test]
async fn the_summary_budget_takes_effect_on_the_next_round() {
    let root = crate::testutil::work_dir("s12-budget-wiring");
    std::fs::write(
        root.join("settings.json"),
        serde_json::json!({"context": {"thresholdTokens": 100_000, "summaryBudgetMs": 30_000}})
            .to_string(),
    )
    .unwrap();
    let settings = Arc::new(crate::settings::SettingsStore::open(&root).unwrap());
    let cli = Arc::new(BudgetProbe {
        budgets: StdMutex::new(Vec::new()),
    });
    let cp = Arc::new(crate::compact::deep::Compactor::new(
        Arc::clone(&settings),
        Arc::clone(&cli) as Arc<dyn SummaryClient>,
        Arc::new(crate::compact::Failures::default()),
        Arc::new(crate::compact::Overflow::default()),
        None,
    ));
    let log = root.join("sessions");
    let mut session = loaded_session();
    seed_log(&log, "s-budget", &session);
    let line = threshold_at(&session);
    let collector = Collector::default();
    turn_with(&mut session, "s-budget", line, &log, &cp, &collector).await;
    assert_eq!(
        cli.budgets.lock().unwrap().as_slice(),
        [Some(Duration::from_millis(30_000))],
        "第一轮要带上设置里那一档（30s）"
    );
    // 改设置（走真实写入面：校验 + 落盘 + 换内存态）⇒ 下一轮就该带上新值。
    settings
        .update(
            None,
            None,
            None,
            None,
            None,
            Some(crate::settings::ContextUpdate {
                summary_budget_ms: Some(300_000),
                ..Default::default()
            }),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .expect("设置更新");
    turn_with(&mut session, "s-budget", line, &log, &cp, &collector).await;
    assert_eq!(
        cli.budgets.lock().unwrap().as_slice(),
        [
            Some(Duration::from_millis(30_000)),
            Some(Duration::from_millis(300_000)),
        ],
        "改完设置**下一轮**就带上新档（不重启）"
    );
}

/// 跑一轮（带着压缩器）——AC-7 那条要连跑两轮看预算换不换，闭包捕获会把它
/// 变成 FnOnce，所以这是一件独立的事。
async fn turn_with(
    session: &mut Session,
    session_id: &str,
    line: u64,
    log: &std::path::Path,
    cp: &Arc<crate::compact::deep::Compactor>,
    collector: &Collector,
) {
    run_turn(
        &kernel_with_tools(),
        session,
        session_id,
        "继续".into(),
        Vec::new(),
        Some(UsagePolicy {
            threshold_tokens: line,
            window_tokens: 1_000_000,
            tools_est_tokens: 0,
            refused_tokens: 0,
        }),
        collector,
        None,
        Some(log),
        crate::runner::DEFAULT_MAX_STEPS,
        None,
        None,
        Some(cp),
    )
    .await
    .expect("turn ok");
}
