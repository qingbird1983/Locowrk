//! **接线面**泄压用例（队列 Q8b）：驱动 `run_turn` 本体，证的是「step 边界那句
//! 调用真的在跑」——`organize/tests/pressure.rs` 那八条直接调 `release_pressure`，
//! 全绿也只证明函数自己对；把调用从 runner 里删掉，那八条一条都不会红。
//! 这正是队列 §6 第 10 条点名的形状（跨文件的关系必须有跨文件判据），
//! 所以这里既有**功能面**这一条（跑到终态看磁盘投影），也有 `tmp/q8b-shapes.mjs`
//! 的静态形状判据（调用点必须落在 `kernel.step_as` **之前**）。

use super::*;

use crate::runner::{run_turn, UsagePolicy};
use async_trait::async_trait;
use memory_tree::{MemoryManagerClient, MemoryManagerUsage};
use session_store::{Session, SessionEvent};
use std::sync::{Arc, Mutex as StdMutex};

/// 记录型通知口：泄压的告知必须打到这条通路才算"用户看得见"。
type Captured = Arc<StdMutex<Vec<(String, serde_json::Value)>>>;

/// 故意**不给任何脚本回复**的记忆管理器：泄压不该调模型。真调了就是
/// `Err("不该被调用")` ⇒ 章节 `source` 会是 "rule" 但台账里那条 organize
/// 记录的出现路径不同——本测断言的是"没有 recorder 产出"，见下方 assert。
struct NoCallClient;

#[async_trait]
impl MemoryManagerClient for NoCallClient {
    async fn complete(
        &self,
        _system: &str,
        _user: &str,
        _model: Option<&str>,
    ) -> Result<(String, MemoryManagerUsage), String> {
        Err("轮内泄压不许调记忆管理器".to_string())
    }
}

fn resolver() -> crate::workspace::WorkspaceResolver {
    Arc::new(|_sid: &str| Some(r"F:\ws\demo".to_string()))
}

#[tokio::test]
async fn run_turn_releases_pressure_before_the_next_step() {
    let root = crate::testutil::work_dir("q8b-wiring");
    std::fs::write(
        root.join("settings.json"),
        serde_json::json!({"context": {"thresholdTokens": 1_000}}).to_string(),
    )
    .unwrap();
    let settings = Arc::new(crate::settings::SettingsStore::open(&root).unwrap());
    let captured: Captured = Arc::new(StdMutex::new(Vec::new()));
    let sink_box = Arc::clone(&captured);
    let notifier: crate::organize::OrganizeNotifier = Arc::new(move |name, payload| {
        sink_box.lock().unwrap().push((name.to_string(), payload));
    });
    let org = Arc::new(crate::organize::Organizer::new(
        Arc::clone(&settings),
        root.join("sessions"),
        Arc::new(NoCallClient),
        resolver(),
        root.clone(),
        Arc::new(crate::model_override::ModelOverrideStore::new(
            root.join("sessions"),
        )),
        Arc::new(crate::llm::catalog::CatalogStore::new(
            crate::testutil::unique_dir("q8b-catalog-absent"),
        )),
        notifier,
        None,
        Arc::new(crate::memory::MemoryStore::new(&root, resolver())),
    ));

    // 会话现状：历史五轮 + 一次 provider 用量上报（anchor 9,500 / 窗口 10,000
    // = 95% ≥ 缺省水位 90%）。本轮从这之后开始 ⇒ 泄压的刀口上界。
    let mut session = Session::new();
    for i in 0..5 {
        session.append(SessionEvent::UserMessage {
            content: format!("历史第{i}轮问题"),
            parts: Vec::new(),
        });
        session.append(SessionEvent::AssistantMessage {
            content: format!("历史第{i}轮回答"),
            tool_calls: vec![],
        });
    }
    session.append(SessionEvent::LlmUsage {
        input: 9_500,
        output: 10,
        cache_read: 0,
        cache_write: 0,
        reasoning: 0,
        total: 9_510,
        duration_ms: 1,
    });
    let turn_start = session.len();

    let collector = Collector::default();
    let steps = run_turn(
        &kernel_with_tools(),
        &mut session,
        "s-wired",
        "echo 继续".into(),
        Vec::new(),
        Some(UsagePolicy {
            threshold_tokens: 1_000,
            window_tokens: 10_000,
            tools_est_tokens: 0,
        }),
        &collector,
        None,
        None,
        crate::runner::DEFAULT_MAX_STEPS,
        None,
        None,
        None,
        Some(&org),
    )
    .await
    .expect("turn ok");
    assert!(steps >= 1, "本轮照常跑完：{steps}");

    let tree = memory_tree::TreeStore::new(&root)
        .load(r"F:\ws\demo", "s-wired")
        .unwrap_or_else(|| panic!("轮内泄压必须留下投影（root={root:?}）"));
    assert!(
        tree.chapter_count() >= 1,
        "第一个 step 之前就该切出一章，实测 {}",
        tree.chapter_count()
    );
    for b in tree.chapters.values() {
        assert_eq!(b.source, "rule", "泄压是机械切分，不是模型总结");
        assert!(
            b.event_range.1 < turn_start,
            "本轮的事件（起点 {turn_start}）不许被归进章节，实测章节到 {}",
            b.event_range.1
        );
    }
    let last = tree.history.last().expect("泄压要留台账");
    assert_eq!(
        (
            last.reason.as_str(),
            last.mode.as_str(),
            last.status.as_str()
        ),
        ("pressure_release", "rule", "ok"),
        "台账的 reason 必须与 `OrganizeReason::PressureRelease::as_str()` 同一个串"
    );

    // 告知面：一次 `lw://organize-done`，载荷 reason 与前端分流的那个字面量同源
    let hits = captured.lock().unwrap().clone();
    assert_eq!(hits.len(), 1, "一轮一次泄压就该只推一条，实测 {hits:?}");
    assert_eq!(hits[0].0, "lw://organize-done");
    assert_eq!(hits[0].1["reason"], "pressure_release");
    assert_eq!(hits[0].1["mode"], "rule");
    assert_eq!(hits[0].1["status"], "ok");
    assert!(hits[0].1["sessionId"].is_number() || hits[0].1["sessionId"].is_string());
    assert!(hits[0].1.get("chapters").is_some(), "载荷要带章节数");

    // 反证另一半：**不给 organizer 就没有任何泄压**
    let root2 = crate::testutil::work_dir("q8b-wiring-off");
    std::fs::write(
        root2.join("settings.json"),
        serde_json::json!({"context": {"thresholdTokens": 1_000}}).to_string(),
    )
    .unwrap();
    let settings2 = Arc::new(crate::settings::SettingsStore::open(&root2).unwrap());
    let org2 = Arc::new(crate::organize::Organizer::new(
        Arc::clone(&settings2),
        root2.join("sessions"),
        Arc::new(NoCallClient),
        resolver(),
        root2.clone(),
        Arc::new(crate::model_override::ModelOverrideStore::new(
            root2.join("sessions"),
        )),
        Arc::new(crate::llm::catalog::CatalogStore::new(
            crate::testutil::unique_dir("q8b-catalog-absent2"),
        )),
        Arc::new(|_, _| {}),
        None,
        Arc::new(crate::memory::MemoryStore::new(&root2, resolver())),
    ));
    let mut session2 = Session::new();
    for i in 0..5 {
        session2.append(SessionEvent::UserMessage {
            content: format!("历史第{i}轮问题"),
            parts: Vec::new(),
        });
        session2.append(SessionEvent::AssistantMessage {
            content: format!("历史第{i}轮回答"),
            tool_calls: vec![],
        });
    }
    session2.append(SessionEvent::LlmUsage {
        input: 9_500,
        output: 10,
        cache_read: 0,
        cache_write: 0,
        reasoning: 0,
        total: 9_510,
        duration_ms: 1,
    });
    let collector2 = Collector::default();
    let _ = run_turn(
        &kernel_with_tools(),
        &mut session2,
        "s-unwired",
        "echo 继续".into(),
        Vec::new(),
        Some(UsagePolicy {
            threshold_tokens: 1_000,
            window_tokens: 10_000,
            tools_est_tokens: 0,
        }),
        &collector2,
        None,
        None,
        crate::runner::DEFAULT_MAX_STEPS,
        None,
        None,
        None,
        None,
    )
    .await
    .expect("turn ok");
    assert!(
        !root2.join("memory-tree").exists(),
        "organizer 缺席（headless / 测试路径）时必须一行都不写 = 与改前逐字节同形"
    );
    let _ = org2;
    std::fs::remove_dir_all(&root).ok();
    std::fs::remove_dir_all(&root2).ok();
}
