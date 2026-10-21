//! **接线面**压缩用例（specs/13 刀③）：驱动 `run_turn` 本体，证「runner 的 step
//! 边界那句压缩调用真的在跑、一轮只跑一次、而且落到了磁盘上；压成的那条
//! checkpoint 重放得出来」。
//!
//! 为什么这一簇必须存在：`compact/**` 那几簇直接调 `Compactor::run`，全绿也只
//! 证明那个函数自己对——把 `runner/mod.rs` 里那一句 `cp.run(` 删掉，它们一条都
//! 不会红。这正是队列 §6 第 10 条点名的形状（跨文件的关系必须有跨文件判据）。
//! 另半边（不给 compactor 就一行都不写）也在这簇里，因为"默认什么都不做"是
//! 这一层能不能被关掉的那条证据。

use super::*;

use crate::compact::deep::{CompactionLlm, Compactor};
use crate::runner::{run_turn, UsagePolicy};
use kernel_agent::{LlmError, LlmRequest, LlmResponse, LlmUsage};
use session_store::Message;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

/// 一份**合格**的摘要（带模板标题行 ⇒ 输出校验通过；上游同款：任一标题行即过）。
const SUMMARY: &str =
    "## Objective\n- 把上下文压缩按 OpenCode V2 重写\n\n## Work State\n### Completed\n- 压缩面已重写";

/// 压缩发送口的替身：**数趟数**，并回一份合格摘要。
///
/// 为什么要有它：压缩请求的真实出口是网络，而这一簇要钉的是"接线面真的跑了"。
/// 替身只管回话，不碰判定——判定在 `compact/**` 那几簇里各自钉死。
struct FakeLlm {
    calls: AtomicUsize,
}

impl FakeLlm {
    fn answering() -> Arc<Self> {
        Arc::new(Self {
            calls: AtomicUsize::new(0),
        })
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl CompactionLlm for FakeLlm {
    async fn complete(
        &self,
        _req: LlmRequest,
        _budget: Duration,
        _on_delta: &(dyn for<'a> Fn(&'a str) -> bool + Send + Sync),
    ) -> Result<LlmResponse, LlmError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(LlmResponse {
            content: SUMMARY.to_string(),
            tool_calls: Vec::new(),
            usage: Some(LlmUsage {
                input: 111,
                output: 222,
                cache_read: 0,
                cache_write: 0,
                reasoning: 0,
                total: 333,
            }),
        })
    }
}

/// 装一台压缩器（设置 + 内核 + 发送口替身）。
fn compactor_with(root: &Path, llm: Arc<dyn CompactionLlm>) -> Arc<Compactor> {
    std::fs::write(
        root.join("settings.json"),
        serde_json::json!({"context": {"compactEnabled": true, "autoCompactPct": 50}}).to_string(),
    )
    .unwrap();
    let settings = Arc::new(crate::settings::SettingsStore::open(root).unwrap());
    Arc::new(Compactor::new(
        settings,
        Arc::new(kernel_with_tools()),
        llm,
        Arc::new(crate::compact::Failures::default()),
        Arc::new(crate::compact::Overflow::default()),
        None,
    ))
}

/// 一份"到线了"的会话：六轮对话（每轮够大 ⇒ 保留区切得进中段）+ 一条用量上报。
///
/// 为什么每轮要够大：保留区按 **15000 token** 切（上游写死），整段历史装得下时
/// 上游只留**最新一次往返**——那样"保留区"里永远只有一句话，钉不住"指针 + 读时
/// 重建"这条语义。每轮 ~3000 token ⇒ 切点落在中段 ⇒ 保留区横跨好几轮。
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

/// 门槛策略：窗口 1000K + 用户 50% ⇒ 门槛 500K；那 900K 的上报稳在线上。
fn usage_policy() -> UsagePolicy {
    UsagePolicy::assemble(50, Some(1_000_000), None, 0, 0)
}

/// checkpoint 里 `<recent-context>` 那一段的文本（读时重建出来的那份）。
fn recent_of(text: &str) -> &str {
    text.split("<recent-context>\n")
        .nth(1)
        .expect("checkpoint 里有保留区")
        .split("\n</recent-context>")
        .next()
        .expect("保留区有收尾")
}

/// 正半边：跑到终态，内存与磁盘都要有那条 `ContextCompacted`，而且压后重放的
/// 窗口头一条就是那条 checkpoint（= 下一个 step 发给模型的那一份）。
///
/// 顺序断言：压缩事件排在**本步产出之前**——`run_turn` 是 `await cp.run` 后才进
/// `kernel.step_as`，模型没有并发开工；日志只追加 ⇒ 位置就是先后。
#[tokio::test]
async fn auto_compaction_runs_once_at_the_line_and_the_turn_continues() {
    let root = crate::testutil::work_dir("s13-compact-wiring");
    let llm = FakeLlm::answering();
    let cp = compactor_with(&root, Arc::clone(&llm) as Arc<dyn CompactionLlm>);
    let log = root.join("sessions");
    let mut session = busy_session();
    seed_log(&log, "s-cp", &session);
    let collector = Collector::default();
    let steps = run_turn(
        &kernel_with_tools(),
        &mut session,
        "s-cp",
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
    .expect("turn ok");
    assert!(steps >= 1, "本轮照常跑完：{steps}");
    assert_eq!(llm.calls(), 1, "一轮只该花一条摘要往返");
    // 压成的那条要**实时**推到 sink：界面「已压缩」系统条与浮层读数都由这条
    // 事件长出来；只落内存/磁盘不够——自动路径漏投影时，用户得切会话重放才看得见。
    let pushed = collector.events();
    let names: Vec<&str> = pushed.iter().map(kind).collect();
    assert!(
        names.contains(&"context_compacted"),
        "自动压缩成功后要立刻推 context_compacted 给界面，实测事件序 {names:?}"
    );
    // 压后读数回落（浮层看到的 used）：压缩那一趟的用量排在 `ContextCompacted`
    // **之前** ⇒ 占用分子认成 stale ⇒ 重新走全量估算（锚点不被压缩请求顶住）。
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
    let in_memory = compacted_count(&session);
    assert_eq!(in_memory, 1, "一轮只许压一次（AC-1 的 once）");
    // 顺序：压缩事件必须早于本步产出 = 模型停着等它。
    let events = session.events();
    let compact_at = events
        .iter()
        .position(|e| matches!(e, SessionEvent::ContextCompacted { .. }))
        .expect("内存会话里要有一条压缩事件");
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
    // 磁盘镜像同数——不然是"只在内存里压了"，切会话/重启后重放看不见这一笔。
    let path = session_store::session_file(&log, "s-cp").expect("会话日志路径可算");
    let (restored, _) = session_store::load_session(&path).expect("磁盘上有这份日志");
    assert_eq!(
        compacted_count(&restored),
        in_memory,
        "落盘的压缩事件必须与内存同数"
    );
    // 重放出来的窗口头一条 = 那条 checkpoint（上游逐字形状，AC-5 另有专测）。
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
        content.contains(SUMMARY),
        "摘要正文逐字进 checkpoint：{content}"
    );
    // 保留区**读时重建**：指针切出来的中段横跨好几轮，最老的那几轮不在里面。
    let recent = recent_of(content);
    assert!(
        recent.contains("第5轮问题"),
        "近期原文要留在窗口里：{recent}"
    );
    assert!(
        !recent.contains("第0轮问题"),
        "最老的那几轮进了摘要、不在保留区：{recent}"
    );
}

/// 反半边：**不给 compactor 就一行都不写**（headless / 测试路径 = 与改前逐字节同形）。
/// 这一条与上面那条一起才把"这一层是可以被关掉的"钉住：只测给进去那一半，
/// 把默认值改成"永远压"也不会红。
#[tokio::test]
async fn nothing_compacts_without_the_compactor() {
    let root = crate::testutil::work_dir("s13-compact-absent");
    let log = root.join("sessions");
    let mut session = busy_session();
    seed_log(&log, "s-none", &session);
    let collector = Collector::default();
    let steps = run_turn(
        &kernel_with_tools(),
        &mut session,
        "s-none",
        "继续".into(),
        Vec::new(),
        Some(usage_policy()),
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

// ---------- specs/13 · 判据 `interrupted`：被叫停 ≠ 压不动 ----------

/// 一个"被用户叫停"的发送口：在**被调用时**把停止标记置位（= 用户在压缩进行时
/// 点了「停止生成」），然后如实报错。
///
/// 不起线程、不等真实时钟：要验的是 `run_turn` 收到「已停止」之后**怎么收束**，
/// 不是时序。停止标记由 `run_turn` 自己登记（`stop.guard`），所以这里置位
/// 一定命中。
struct Stoppable {
    stop: Arc<crate::stop::StopFlags>,
}

#[async_trait]
impl CompactionLlm for Stoppable {
    async fn complete(
        &self,
        _req: LlmRequest,
        _budget: Duration,
        _on_delta: &(dyn for<'a> Fn(&'a str) -> bool + Send + Sync),
    ) -> Result<LlmResponse, LlmError> {
        self.stop.request("s-stop");
        Err(LlmError::Backend("已停止".into()))
    }
}

/// 压缩被叫停 ⇒ 轮以 **Interrupted** 收束（不是 Failed、也不是成功），且
/// **不计入熔断失败数**。
///
/// 这一条是"用户按了停止也没用"的正面反面：改之前压缩里的请求压根查不到停止
/// 标记，用户按一万次停止也没用，界面永远转。现在压动的请求查得到 ⇒ 这一趟
/// 如实收束。判据名照样是 `interrupted`（界面那句话仍要说"被叫停"），但熔断
/// 账不动——停过三次不该让压缩永久停手。
#[tokio::test]
async fn a_stop_during_compaction_ends_the_turn_as_interrupted() {
    let root = crate::testutil::work_dir("s13-stop-wiring");
    std::fs::write(
        root.join("settings.json"),
        serde_json::json!({"context": {"compactEnabled": true, "autoCompactPct": 50}}).to_string(),
    )
    .unwrap();
    let settings = Arc::new(crate::settings::SettingsStore::open(&root).unwrap());
    let stop_flags = Arc::new(crate::stop::StopFlags::default());
    let failures = Arc::new(crate::compact::Failures::default());
    let llm = Arc::new(Stoppable {
        stop: Arc::clone(&stop_flags),
    });
    let cp = Arc::new(Compactor::new(
        settings,
        Arc::new(kernel_with_tools()),
        llm as Arc<dyn CompactionLlm>,
        Arc::clone(&failures),
        Arc::new(crate::compact::Overflow::default()),
        Some(Arc::clone(&stop_flags)),
    ));
    let log = root.join("sessions");
    let mut session = busy_session();
    seed_log(&log, "s-stop", &session);
    let collector = Collector::default();
    let steps = run_turn(
        &kernel_with_tools(),
        &mut session,
        "s-stop",
        "继续".into(),
        Vec::new(),
        Some(usage_policy()),
        &collector,
        None,
        Some(&log),
        crate::runner::DEFAULT_MAX_STEPS,
        Some(&stop_flags),
        None,
        Some(&cp),
    )
    .await
    .expect("被叫停不是失败：轮照常收束");
    assert!(steps >= 1, "轮次照常收束");
    assert!(
        session.events().iter().any(|e| matches!(
            e,
            SessionEvent::TurnEnd {
                outcome: session_store::TurnOutcome::Interrupted,
                ..
            }
        )),
        "停止命中的那一轮必须以 Interrupted 收束（不是 Failed）"
    );
    assert_eq!(failures.of("s-stop"), 0, "用户主动停不是失败：熔断账不许动");
    let reasons: Vec<Option<String>> = collector
        .events()
        .iter()
        .filter_map(|e| match e {
            AgentEvent::CompactStatus { phase, reason, .. }
                if phase == crate::events::compact_phase::FAILED =>
            {
                Some(reason.clone())
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        reasons,
        vec![Some("interrupted".to_string())],
        "判据名照旧是 interrupted（界面那句话仍要说被叫停）"
    );
}
