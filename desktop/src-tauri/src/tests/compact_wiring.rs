//! **接线面**压缩用例（S2 刀④）：驱动 `run_turn` 本体，证「runner 的 step 边界那句
//! 压缩调用真的在跑、而且落到了磁盘上」。
//!
//! 为什么这一簇必须存在：`compact/tests_deep.rs` 那十一条直接调 `Compactor::run`，
//! 全绿也只证明那个函数自己对——把 `runner/mod.rs` 里那三行删掉，它们一条都不会红。
//! 这正是队列 §6 第 10 条点名的形状（跨文件的关系必须有跨文件判据），模板 =
//! `tests/pressure_wiring.rs`。另半边（不给 compactor 就一行都不写）也在这簇里，
//! 因为"默认什么都不做"是这一层能不能被关掉的那条证据。

use super::*;

use crate::runner::{run_turn, UsagePolicy};
use async_trait::async_trait;
use memory_tree::{MemoryManagerClient, MemoryManagerUsage};
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
impl MemoryManagerClient for Counting {
    async fn complete(
        &self,
        system: &str,
        user: &str,
        _model: Option<&str>,
    ) -> Result<(String, MemoryManagerUsage), String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.seen
            .lock()
            .unwrap()
            .push((system.to_string(), user.to_string()));
        // 当场挑刺：接线面上也要证"送出去的就是装配好的那份骨架"，而不是一个空串
        assert!(
            system.contains("只用文本回答"),
            "摘要请求的 system 要带上不许调工具那一节"
        );
        assert!(!user.is_empty(), "摘要请求不许喂空");
        Ok((
            crate::compact::sample_summary(),
            MemoryManagerUsage {
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
            Arc::clone(&cli) as Arc<dyn MemoryManagerClient>,
            Arc::new(crate::compact::Failures::default()),
            Arc::new(crate::compact::Overflow::default()),
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
#[tokio::test]
async fn run_turn_compacts_at_the_step_boundary_and_mirrors_it_to_disk() {
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
        None,
        Some(&cp),
    )
    .await
    .expect("turn ok");
    assert!(steps >= 1, "本轮照常跑完：{steps}");
    assert!(cli.count() >= 1, "step 边界那句调用必须真的跑过");
    assert_eq!(
        cli.count(),
        1,
        "一轮只该花一条摘要往返：压完后占用读数换吃压后投影，不许每个 step 再压一次"
    );
    let in_memory = compacted_count(&session);
    assert!(in_memory >= 1, "内存会话里要有一条压缩事件");
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
