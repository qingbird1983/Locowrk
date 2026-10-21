//! **接线面**：压缩的**界面反馈**与**摘要预算**（specs/13 刀③ · C2 / C3）。
//!
//! 形状照 `compact_wiring.rs`（同一种"驱动 `run_turn` 本体、读事件收集器"的取证
//! 载体），但**各抓各的一层**：
//! ① 压缩**开始**与**没成**各推一条相位事件（判据名进 `reason`，前端按名查文案）；
//! ② 熔断停手推「已暂停」，且那一轮**不**推「正在压缩」（判说不用压时压根没有
//!    压缩发生，界面不许冒出没发生过的「正在压缩」）；
//! ③ 摘要请求带的那枚预算 = 设置里的那一档，且**下一轮就生效**（不重启）。
//!
//! 为什么另起一个文件而不是并进 `compact_wiring.rs`：那一簇抓的是"压成之后
//! 落盘与投影同形"，这一簇抓的是"压缩**过程**的可见性与预算"——两簇的夹具
//! 不同（这一簇要能喂"失败"与"预算读数"两种结局），断言对象也不同。

use super::*;

use crate::compact::deep::{CompactionLlm, Compactor};
use crate::events::AgentEvent;
use crate::runner::{run_turn, UsagePolicy};
use kernel_agent::{LlmError, LlmRequest, LlmResponse};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

/// 一个"摘要模型没答上"的发送口：如实报错 ⇒ 压缩层把它归到 `provider-error`
/// （界面那半要说的"没压成"就是从这枚判据名来的）。
///
/// 为什么报错文本**不**带任何关键词：判别"超窗 / 预算耗尽"靠的是模型接入面那
/// 两份判据（`llm::is_overflow_message` / `llm::is_budget_exhausted`），压缩层
/// 不自备关键词表。这条错误串刻意普通 ⇒ 落进 `provider-error` 那一档。
struct Failing {
    calls: AtomicUsize,
}

impl Failing {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            calls: AtomicUsize::new(0),
        })
    }
}

#[async_trait]
impl CompactionLlm for Failing {
    async fn complete(
        &self,
        _req: LlmRequest,
        _budget: Duration,
        _on_delta: &(dyn for<'a> Fn(&'a str) -> bool + Send + Sync),
    ) -> Result<LlmResponse, LlmError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(LlmError::Backend("摘要模型没答上".into()))
    }
}

/// 记录每次请求带的那枚**总时长预算**的发送口。
struct BudgetProbe {
    budgets: StdMutex<Vec<Duration>>,
}

#[async_trait]
impl CompactionLlm for BudgetProbe {
    async fn complete(
        &self,
        _req: LlmRequest,
        budget: Duration,
        _on_delta: &(dyn for<'a> Fn(&'a str) -> bool + Send + Sync),
    ) -> Result<LlmResponse, LlmError> {
        self.budgets.lock().unwrap().push(budget);
        // 如实失败：压缩不落账 ⇒ 占用读数不回落 ⇒ 下一轮照样到线、照样再压一次
        // （压成的话第二轮就不压了，测不到"下一轮生效"）。
        Err(LlmError::Backend("摘要模型没答上".into()))
    }
}

/// 装一台"摘要必失败"的自动压缩器（返回熔断表供断言）。
fn failing_compactor(
    root: &Path,
    llm: Arc<Failing>,
    failures: Arc<crate::compact::Failures>,
) -> Arc<Compactor> {
    std::fs::write(
        root.join("settings.json"),
        serde_json::json!({"context": {"compactEnabled": true, "autoCompactPct": 50}}).to_string(),
    )
    .unwrap();
    let settings = Arc::new(crate::settings::SettingsStore::open(root).unwrap());
    Arc::new(Compactor::new(
        settings,
        Arc::new(kernel_with_tools()),
        llm as Arc<dyn CompactionLlm>,
        failures,
        Arc::new(crate::compact::Overflow::default()),
        None,
    ))
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

/// 门槛策略：窗口 1000K + 用户 50% ⇒ 门槛 500K。
fn usage_policy() -> UsagePolicy {
    UsagePolicy::assemble(50, Some(1_000_000), None, 0, 0)
}

/// 一份"到线了"的会话（用量上报把占用顶到线上，压缩才有理由起步）。
fn busy_session() -> Session {
    let mut s = Session::new();
    for i in 0..6 {
        s.append(SessionEvent::UserMessage {
            content: format!("第{i}轮问题：") + &"细节".repeat(2500),
            parts: Vec::new(),
        });
        s.append(SessionEvent::AssistantMessage {
            content: format!("第{i}轮答复"),
            tool_calls: vec![],
        });
    }
    s.append(SessionEvent::LlmUsage {
        input: 900_000,
        output: 10,
        cache_read: 0,
        cache_write: 0,
        reasoning: 0,
        total: 900_010,
        duration_ms: 1,
    });
    s
}

/// **C2**：自动压缩**开始**与**没成**各推一条相位事件。
///
/// 顺序也要钉住：「开始」必须在「没成」**之前**（那是同一趟压缩的两个时刻，
/// 顺序反了界面会先显示失败再显示开始）。
///
/// ⚠ auto 这一路的失败 = **整步失败**（specs/13 · AC-8）：压不动还继续把整段
/// 历史发给模型 = 再被拒一次，用户看到的是"模型报错"而不是"压不动"。所以这一轮
/// 以 `Err` 收束——但相位事件已经推过了，界面照样看得见"没成"。
#[tokio::test]
async fn the_auto_compaction_reports_its_start_and_its_failure_to_the_ui() {
    let root = crate::testutil::work_dir("s13-status-wiring");
    let llm = Failing::new();
    let failures = Arc::new(crate::compact::Failures::default());
    let cp = failing_compactor(&root, Arc::clone(&llm), Arc::clone(&failures));
    let log = root.join("sessions");
    let mut session = busy_session();
    seed_log(&log, "s-status", &session);
    let collector = Collector::default();
    let err = run_turn(
        &kernel_with_tools(),
        &mut session,
        "s-status",
        "继续".into(),
        Vec::new(),
        Some(usage_policy()),
        &collector,
        None,
        Some(&log),
        crate::runner::DEFAULT_MAX_STEPS,
        None,
        None,
        Some(&cp),
    )
    .await
    .expect_err("auto 压缩失败 ⇒ 整步失败");
    assert!(
        err.contains("provider-error"),
        "失败要报判据名（前端按名查文案）：{err}"
    );
    let phases = compact_phases(&collector);
    assert_eq!(
        phases,
        vec![
            (crate::events::compact_phase::RUNNING.to_string(), None,),
            (
                crate::events::compact_phase::FAILED.to_string(),
                Some("provider-error".to_string()),
            ),
        ],
        "自动压缩要把「开始」与「没成」都推给界面（实测 {phases:?}）"
    );
    assert_eq!(
        llm.calls.load(Ordering::SeqCst),
        1,
        "这一趟只发了一条摘要请求"
    );
    assert_eq!(failures.of("s-status"), 1, "失败要记一次（推进熔断的输入）");
}

/// **C2 的一半**：连续 3 次失败后，第 4 次到线时推「已暂停」。
///
/// 这一条把"熔断"从**判据名**那一族里拎出来单测：熔断不在判据名表里（它走
/// **相位** `paused`），所以界面上是「已暂停」而不是「没压成：xxx」。
///
/// 顺带钉住两件事：① 第 4 次**不再发摘要请求**（熔断的意义就在这儿）；
/// ② 第 4 次**不发「正在压缩」**——判说"不用压"时压根没有压缩发生，界面不许
/// 冒出没发生过的「正在压缩」。
#[tokio::test]
async fn the_breaker_pauses_the_auto_compaction_in_the_ui() {
    let root = crate::testutil::work_dir("s13-paused-wiring");
    let llm = Failing::new();
    let failures = Arc::new(crate::compact::Failures::default());
    let cp = failing_compactor(&root, Arc::clone(&llm), Arc::clone(&failures));
    let log = root.join("sessions");
    let mut session = busy_session();
    seed_log(&log, "s-paused", &session);
    let collector = Collector::default();
    // 连跑四轮（每轮一步 ⇒ 每轮一次压缩尝试；压缩失败即整步失败 ⇒ 轮次收束）
    for _ in 0..4 {
        let _ = run_turn(
            &kernel_with_tools(),
            &mut session,
            "s-paused",
            "继续".into(),
            Vec::new(),
            Some(usage_policy()),
            &collector,
            None,
            Some(&log),
            crate::runner::DEFAULT_MAX_STEPS,
            None,
            None,
            Some(&cp),
        )
        .await;
    }
    let phases = compact_phases(&collector);
    assert_eq!(
        phases,
        vec![
            (crate::events::compact_phase::RUNNING.to_string(), None),
            (
                crate::events::compact_phase::FAILED.to_string(),
                Some("provider-error".to_string()),
            ),
            (crate::events::compact_phase::RUNNING.to_string(), None),
            (
                crate::events::compact_phase::FAILED.to_string(),
                Some("provider-error".to_string()),
            ),
            (crate::events::compact_phase::RUNNING.to_string(), None),
            (
                crate::events::compact_phase::FAILED.to_string(),
                Some("provider-error".to_string()),
            ),
            (crate::events::compact_phase::PAUSED.to_string(), None),
        ],
        "三次失败之后要推「已暂停」，且那一轮**不**推「正在压缩」（实测 {phases:?}）"
    );
    assert_eq!(
        llm.calls.load(Ordering::SeqCst),
        3,
        "熔断之后不许再发摘要请求（第 4 次那次压根没发）"
    );
    assert_eq!(failures.of("s-paused"), 3, "失败计数 = 熔断线那三格");
}

/// **C1 那一格**：摘要请求带的那枚预算 = 设置里的那一档，且**下一轮就生效**
/// （不重启、不重开应用）。
///
/// 为什么这条住在**接线面**而不是压缩层单元测试：设置项改完到下一次压缩读到
/// 新值，中间隔着"压缩层每趟重新取一次活配置"这一跳——单元测试改不了设置。
///
/// 那一枚预算是**总时长**（整趟共享，含全部重试与那一次提醒重发），传下去的是
/// "还剩多久"⇒ 实测值略小于设定值（差的是已经花掉的那点）。
struct BudgetCase {
    settings: Arc<crate::settings::SettingsStore>,
    cp: Arc<Compactor>,
    probe: Arc<BudgetProbe>,
}

fn budget_case(root: &Path) -> BudgetCase {
    std::fs::write(
        root.join("settings.json"),
        serde_json::json!({
            "context": { "compactEnabled": true, "autoCompactPct": 50, "summaryBudgetMs": 30_000 }
        })
        .to_string(),
    )
    .unwrap();
    let settings = Arc::new(crate::settings::SettingsStore::open(root).unwrap());
    let probe = Arc::new(BudgetProbe {
        budgets: StdMutex::new(Vec::new()),
    });
    let cp = Arc::new(Compactor::new(
        Arc::clone(&settings),
        Arc::new(kernel_with_tools()),
        Arc::clone(&probe) as Arc<dyn CompactionLlm>,
        Arc::new(crate::compact::Failures::default()),
        Arc::new(crate::compact::Overflow::default()),
        None,
    ));
    BudgetCase {
        settings,
        cp,
        probe,
    }
}

/// 那一枚预算 = 设置里的那一档（容差一秒：传下去的是"还剩多久"）。
fn assert_budget(budgets: &[Duration], ms: u64) {
    let got = budgets.last().expect("有预算记录").as_millis() as u64;
    assert!(
        got <= ms && got + 1_000 >= ms,
        "预算 = 设置里那一档（{ms}ms），实测 {got}ms"
    );
}

#[tokio::test]
async fn the_summary_budget_takes_effect_on_the_next_round() {
    let root = crate::testutil::work_dir("s13-budget-wiring");
    let case = budget_case(&root);
    let log = root.join("sessions");
    let mut session = busy_session();
    seed_log(&log, "s-budget", &session);
    let collector = Collector::default();
    // 压缩失败 ⇒ 整步失败 ⇒ 轮以 Err 收束；但那一趟的请求已经发出去了（预算读数
    // 就在里面），这一轮照样算数。
    let _ = run_turn(
        &kernel_with_tools(),
        &mut session,
        "s-budget",
        "继续".into(),
        Vec::new(),
        Some(usage_policy()),
        &collector,
        None,
        Some(&log),
        crate::runner::DEFAULT_MAX_STEPS,
        None,
        None,
        Some(&case.cp),
    )
    .await;
    let budgets = case.probe.budgets.lock().unwrap().clone();
    assert_eq!(budgets.len(), 1, "第一轮发了一条压缩请求");
    assert_budget(&budgets, 30_000);
    // 改设置（走真实写入面：校验 + 落盘 + 换内存态）⇒ 下一轮就该带上新值。
    case.settings
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
    let _ = run_turn(
        &kernel_with_tools(),
        &mut session,
        "s-budget",
        "继续".into(),
        Vec::new(),
        Some(usage_policy()),
        &collector,
        None,
        Some(&log),
        crate::runner::DEFAULT_MAX_STEPS,
        None,
        None,
        Some(&case.cp),
    )
    .await;
    let budgets = case.probe.budgets.lock().unwrap().clone();
    assert_eq!(budgets.len(), 2, "第二轮又发了一条压缩请求");
    assert_budget(&budgets, 300_000);
}
