//! **接线面**压缩用例（S2 刀④；S4 specs/11 AC-1 起补齐顺序断言）：驱动 `run_turn`
//! 本体，证「runner 的 step 边界那句压缩调用真的在跑、一轮只跑一次、而且落到了磁盘上；
//! 压缩事件排在本步产出之前 = 模型确实停着等它」。
//!
//! 为什么这一簇必须存在：`compact/tests_deep.rs` 那十一条直接调 `Compactor::run`，
//! 全绿也只证明那个函数自己对——把 `runner/mod.rs` 里那三行删掉，它们一条都不会红。
//! 这正是队列 §6 第 10 条点名的形状（跨文件的关系必须有跨文件判据）。另半边
//! （不给 compactor 就一行都不写）也在这簇里，因为"默认什么都不做"是这一层能不能
//! 被关掉的那条证据。

use super::*;

use crate::events::AgentEvent;
use crate::llm::{SummaryClient, SummaryRequest, SummaryUsage};
use crate::runner::{run_turn, UsagePolicy};
use async_trait::async_trait;
use session_store::{self, Message, Session, SessionEvent};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex as StdMutex};

/// 数得出趟数、并对送出去的请求当场挑刺的摘要客户端。
#[derive(Default)]
struct Counting {
    calls: AtomicUsize,
    /// 每次调用的 `(system, user)` 都留下来，供用例断言"只有被压掉那半段进摘要请求"
    seen: StdMutex<Vec<(String, String)>>,
}

impl Counting {
    fn count(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl SummaryClient for Counting {
    async fn complete(&self, req: &SummaryRequest<'_>) -> Result<(String, SummaryUsage), String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.seen
            .lock()
            .unwrap()
            .push((req.system.to_string(), req.user.to_string()));
        // 当场挑刺：接线面上也要证"送出去的就是装配好的那份骨架"，而不是一个空串
        assert!(
            req.system.contains("只用文本回答"),
            "摘要请求的 system 要带上不许调工具那一节"
        );
        assert!(!req.user.is_empty(), "摘要请求不许喂空");
        Ok((
            crate::compact::sample_summary(),
            SummaryUsage {
                input: 10,
                output: 5,
            },
        ))
    }
}

/// 六轮历史 + 一次远超阈值的用量上报（占用顶到线上，压缩才有理由起步）。
///
/// 每轮的分量要够大：目标位 = 阈值 × 50% 再减掉摘要自己要腾的 1400 token
/// （`compact::summary_reserve`），夹具小过这个数就会塌成"只剩最后一条"，
/// 那时候"近期原文要留着"这一档根本无从观察。
fn loaded_session() -> Session {
    let mut s = Session::new();
    for i in 0..6 {
        s.append(SessionEvent::UserMessage {
            content: format!("第{i}轮问题：") + &"细节".repeat(600),
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

/// 阈值取"前史自身的估算量" ⇒ 占用恰好站在线上当场到线（取数纪律：不钉魔法数）。
fn threshold_at(session: &Session) -> u64 {
    kernel_agent::estimate_tokens(&session.derive_messages())
}

fn compactor_with(root: &std::path::Path) -> (Arc<crate::compact::deep::Compactor>, Arc<Counting>) {
    std::fs::write(
        root.join("settings.json"),
        serde_json::json!({"context": {"thresholdTokens": 100_000}}).to_string(),
    )
    .unwrap();
    let settings = Arc::new(crate::settings::SettingsStore::open(root).unwrap());
    let cli = Arc::new(Counting::default());
    (
        Arc::new(crate::compact::deep::Compactor::new(
            settings,
            Arc::clone(&cli) as Arc<dyn SummaryClient>,
            Arc::new(crate::compact::Failures::default()),
            Arc::new(crate::compact::Overflow::default()),
            // 停止标记（specs/12 刀①）：None = 没有停止面（这一簇不测停止）。
            None,
        )),
        cli,
    )
}

fn compacted_count(session: &Session) -> usize {
    session
        .events()
        .iter()
        .filter(|e| matches!(e, SessionEvent::ContextCompacted { .. }))
        .count()
}

/// 先把内存会话的**前史**写进磁盘。
///
/// `run_turn` 只追加本轮新增段（磁盘是日志的镜像、与投影共用游标），而夹具里的会话
/// 是内存里造的 ⇒ 不落这一次，磁盘上就没有前史。而折叠是**按投影里有多少条消息**算
/// 的（`kept_messages` 与投影条数取小），缺前史的重放会得出另一个窗口——那是夹具的
/// 缺陷，不是压缩层的性质，所以先把地基铺好再谈取证。
fn seed_log(log: &std::path::Path, id: &str, session: &Session) {
    crate::session_log::append_slice(log, id, session, 0);
}

/// 正半边：跑到终态，内存与磁盘都要有那条 `ContextCompacted`，而且压后重放的投影
/// 头一条就是那条共享构造函数造的摘要消息（= 下一个 step 发给模型的那一份）。
/// 顺序断言（specs/11 AC-1）：压缩事件排在触发轮的 `LlmUsage` 之后、本步任何
/// `AssistantMessage` 之前——`run_turn` 是 `await cp.run` 后才进 `kernel.step_as`，
/// 模型没有并发开工；`DemoLlm` 不报用量，所以"下一步的 LlmUsage"在这条夹具里
/// 由"本步产出"等价承重（日志只追加，先落的事件就是先发生的）。
#[tokio::test]
async fn auto_compaction_runs_once_at_the_line_and_the_turn_continues() {
    let root = crate::testutil::work_dir("s2-compact-wiring");
    let (cp, cli) = compactor_with(&root);
    let log = root.join("sessions");
    let mut session = loaded_session();
    seed_log(&log, "s-cp", &session);
    let line = threshold_at(&session);
    let collector = Collector::default();
    let steps = run_turn(
        &kernel_with_tools(),
        &mut session,
        "s-cp",
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
    assert!(steps >= 1, "本轮照常跑完：{steps}");
    assert!(cli.count() >= 1, "step 边界那句调用必须真的跑过");
    // AC-9 接线面：压成的那条要**实时**推到 sink，且随后那条 context_usage 的
    // 读数要回落。只落内存/磁盘不够——界面「已压缩」系统条与浮层读数都由这两条
    // 事件长出来；自动路径漏投影时，用户得切会话重放才看得见（真窗口抓到过）。
    let pushed = collector.events();
    let names: Vec<&str> = pushed.iter().map(kind).collect();
    assert!(
        names.contains(&"context_compacted"),
        "自动压缩成功后要立刻推 context_compacted 给界面，实测事件序 {names:?}"
    );
    let last_used = pushed
        .iter()
        .rev()
        .find_map(|e| match e {
            AgentEvent::ContextUsage { used, .. } => Some(*used),
            _ => None,
        })
        .expect("每步后都有一条 context_usage");
    assert!(
        last_used < 900_000,
        "触发读数 900000 压缩后必须回落（浮层看到的 used），实测 {last_used}"
    );
    assert_eq!(
        cli.count(),
        1,
        "一轮只该花一条摘要往返：压完后占用读数换吃压后投影，不许每个 step 再压一次"
    );
    let in_memory = compacted_count(&session);
    assert_eq!(in_memory, 1, "一轮只许压一次（AC-1 的 once）");
    // 顺序（AC-1）：触发轮那份 LlmUsage 在压缩事件之前；本步的答复在它之后。
    // 日志只追加 ⇒ 位置就是先后；两条一起才说明"模型停着等压缩、压完才继续"。
    let events = session.events();
    let compact_at = events
        .iter()
        .position(|e| matches!(e, SessionEvent::ContextCompacted { .. }))
        .expect("内存会话里要有一条压缩事件");
    let trigger_at = events[..compact_at]
        .iter()
        .rposition(|e| matches!(e, SessionEvent::LlmUsage { .. }))
        .expect("触发轮的 LlmUsage 应在压缩事件之前");
    assert!(trigger_at < compact_at, "压缩必须晚于触发读数");
    let step_output_at = events
        .iter()
        .enumerate()
        .skip(compact_at + 1)
        .find(|(_, e)| matches!(e, SessionEvent::AssistantMessage { .. }))
        .map(|(i, _)| i)
        .expect("压缩后本步要产出一条答复");
    assert!(
        compact_at < step_output_at,
        "压缩事件必须早于本步产出 = 模型停着等它（compact_at {compact_at} / output_at {step_output_at}）"
    );
    // 磁盘镜像同数——不然是"只在内存里压了"，切会话/重启后重放看不见这一笔
    let path = session_store::session_file(&log, "s-cp").expect("会话日志路径可算");
    let (restored, _) = session_store::load_session(&path).expect("磁盘上有这份日志");
    assert_eq!(
        compacted_count(&restored),
        in_memory,
        "落盘的压缩事件必须与内存同数"
    );
    let msgs = restored.derive_messages();
    assert!(
        matches!(&msgs[0], Message::User { content, .. }
            if content.starts_with(session_store::SUMMARIZED_MARKER)),
        "重放出来的窗口头一条该是那条摘要消息，实测 {:?}",
        msgs.first()
    );
    // 最新那句用户问题仍在窗口里（压的是较旧那一段，不是整段历史）
    assert!(
        restored
            .derive_messages()
            .iter()
            .any(|m| matches!(m, Message::User { content, .. } if content.contains("第5轮问题"))),
        "近期原文要留在窗口里"
    );
}

/// 反半边：**不给 compactor 就一行都不写**（headless / 测试路径 = 与改前逐字节同形）。
/// 这一条与上面那条一起才把"这一层是可以被关掉的"钉住：只测给进去那一半，
/// 把默认值改成"永远压"也不会红。
#[tokio::test]
async fn nothing_compacts_without_the_compactor() {
    let root = crate::testutil::work_dir("s2-compact-absent");
    let log = root.join("sessions");
    let mut session = loaded_session();
    seed_log(&log, "s-none", &session);
    let line = threshold_at(&session);
    let collector = Collector::default();
    let steps = run_turn(
        &kernel_with_tools(),
        &mut session,
        "s-none",
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
        None,
    )
    .await
    .expect("turn ok");
    assert!(steps >= 1);
    assert_eq!(
        compacted_count(&session),
        0,
        "没给 compactor 就不许有压缩事件"
    );
    let path = session_store::session_file(&log, "s-none").expect("会话日志路径可算");
    let (restored, _) = session_store::load_session(&path).expect("磁盘上有这份日志");
    assert_eq!(compacted_count(&restored), 0, "磁盘上也不许有");
    assert_eq!(
        restored.derive_messages().len(),
        session.derive_messages().len(),
        "反证的另一半：投影长度与改前同形"
    );
}

// ---------- specs/12 刀①：停止命中压缩 ⇒ 整轮以 Interrupted 收束 ----------

/// 一个"被用户叫停"的摘要客户端：在**被调用时**把停止标记置位（= 用户在
/// 压缩进行时点了「停止生成」），然后如实报错。
///
/// 不起线程、不等真实时钟：要验的是 `run_turn` 收到「已停止」之后**怎么收束**，
/// 不是时序。停止标记由 `run_turn` 自己登记（`stop.guard`），所以这里置位
/// 一定命中。
struct Stoppable {
    stop: Arc<crate::stop::StopFlags>,
}

#[async_trait]
impl SummaryClient for Stoppable {
    async fn complete(&self, req: &SummaryRequest<'_>) -> Result<(String, SummaryUsage), String> {
        self.stop.request(req.session_id.unwrap_or_default());
        Err("已停止".into())
    }
}

/// AC-4 接线面（specs/12 刀①）：停止信号在压缩里命中 ⇒ 整轮以 **Interrupted**
/// 收束（不是 Completed、不是失败），且**不计入熔断失败数**。
///
/// 这一条是"用户按了停止也没用"的正面反面：改之前压缩里的摘要调用压根查不到
/// 停止标记（hooks 写死 None），用户按一万次停止也没用，界面永远转。
#[tokio::test]
async fn a_stop_during_compaction_ends_the_turn_as_interrupted() {
    let root = crate::testutil::work_dir("s12-stop-wiring");
    std::fs::write(
        root.join("settings.json"),
        serde_json::json!({"context": {"thresholdTokens": 100_000}}).to_string(),
    )
    .unwrap();
    let settings = Arc::new(crate::settings::SettingsStore::open(&root).unwrap());
    let stop_flags = Arc::new(crate::stop::StopFlags::default());
    let failures = Arc::new(crate::compact::Failures::default());
    let cli = Arc::new(Stoppable {
        stop: Arc::clone(&stop_flags),
    });
    let cp = Arc::new(crate::compact::deep::Compactor::new(
        settings,
        Arc::clone(&cli) as Arc<dyn SummaryClient>,
        Arc::clone(&failures),
        Arc::new(crate::compact::Overflow::default()),
        Some(Arc::clone(&stop_flags)),
    ));
    let log = root.join("sessions");
    let mut session = loaded_session();
    seed_log(&log, "s-stop", &session);
    let line = threshold_at(&session);
    let collector = Collector::default();
    let steps = run_turn(
        &kernel_with_tools(),
        &mut session,
        "s-stop",
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
        Some(&stop_flags),
        None,
        Some(&cp),
    )
    .await
    .expect("turn ok");
    assert!(steps >= 1, "轮次照常收束");
    assert!(
        session.events().iter().any(|e| matches!(
            e,
            SessionEvent::TurnEnd {
                outcome: session_store::TurnOutcome::Interrupted,
                ..
            }
        )),
        "停止命中的那一轮必须以 Interrupted 收束（不是 Completed）"
    );
    assert_eq!(failures.of("s-stop"), 0, "用户主动停不是失败：熔断账不许动");
}
