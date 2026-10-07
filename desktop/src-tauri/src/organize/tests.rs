//! organize 单测：影子编排全链路——记忆管理器成功 / 垃圾输出降级规则切分 /
//! 增量触发 / 反弹熔断 / 崩溃重投（会话在盘）/ 会话已删放弃。设置经临时
//! 目录 settings.json 直写（serde 缺省补齐其余字段，绕开 settings_set
//! 校验以便用极小触发阈值）。

use std::collections::VecDeque;
use std::sync::{Arc, Mutex as StdMutex};

use async_trait::async_trait;
use memory_tree::{MemoryManagerClient, MemoryManagerUsage, OrganizeReason, TreeStore};
use session_store::{Session, SessionEvent};

use super::Organizer;
use crate::settings::SettingsStore;

/// 脚本化假记忆管理器：按序返回原始回复（Ok 文本 / Err 借口）。
struct FakeClient {
    responses: StdMutex<VecDeque<Result<String, String>>>,
}

impl FakeClient {
    fn new(responses: Vec<Result<String, String>>) -> Arc<Self> {
        Arc::new(Self {
            responses: StdMutex::new(responses.into()),
        })
    }
}

#[async_trait]
impl MemoryManagerClient for FakeClient {
    async fn complete(
        &self,
        _system: &str,
        _user: &str,
        _model: Option<&str>,
    ) -> Result<(String, MemoryManagerUsage), String> {
        self.responses
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| "没有脚本回复".to_string())?
            .map(|raw| {
                (
                    raw,
                    MemoryManagerUsage {
                        input: 100,
                        output: 40,
                    },
                )
            })
    }
}

fn temp_root(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("orgz-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// 直写 settings.json（camelCase；其余字段 serde default 补齐）。
fn settings_with(json: serde_json::Value, root: &std::path::Path) -> Arc<SettingsStore> {
    std::fs::write(root.join("settings.json"), json.to_string()).unwrap();
    Arc::new(SettingsStore::open(root).unwrap())
}

fn resolver() -> crate::workspace::WorkspaceResolver {
    Arc::new(|_sid: &str| Some(r"F:\ws\demo".to_string()))
}

/// 六事件会话：三轮问答（3 User + 3 Assistant）。
fn session() -> Session {
    let mut s = Session::new();
    for i in 0..3 {
        s.append(SessionEvent::UserMessage {
            content: format!("第{i}轮问题"),
        });
        s.append(SessionEvent::AssistantMessage {
            content: format!("第{i}轮回答"),
            tool_calls: vec![],
        });
    }
    s
}

/// 合法铺满 6 项材料的记忆管理器输出（两章节）。
fn good_reply() -> String {
    r#"[{"title":"话题一","summary":"摘要一。","event_range":[0,1]},{"title":"话题二","summary":"摘要二。","event_range":[2,5]}]"#.into()
}

fn organizer(
    root: &std::path::Path,
    settings: Arc<SettingsStore>,
    client: Arc<dyn MemoryManagerClient>,
) -> Arc<Organizer> {
    Arc::new(Organizer::new(
        settings,
        root.join("sessions"),
        client,
        resolver(),
        root.to_path_buf(),
        Arc::new(crate::organizer_mode::OrganizerModeStore::new(
            root.join("sessions"),
            crate::organizer_mode::OrganizerMode::Legacy,
        )),
        std::sync::Arc::new(|_, _| {}), // 哑通知（不触 emit）
    ))
}

fn tree_at(
    root: &std::path::Path,
    _settings: &SettingsStore,
    session_id: &str,
) -> memory_tree::MemoryTree {
    TreeStore::new(root)
        .load(r"F:\ws\demo", session_id)
        .unwrap_or_else(|| memory_tree::MemoryTree::new(r"F:\ws\demo", session_id))
}

#[tokio::test]
async fn manual_organize_records_recorder_bundles() {
    let root = temp_root("manual");
    let settings = settings_with(serde_json::json!({}), &root);
    let org = organizer(&root, settings, FakeClient::new(vec![Ok(good_reply())]));
    let session = session();
    let task = org.plan_manual(&session, "s-manual").expect("发起成功");
    assert_eq!(task.reason, OrganizeReason::Manual);
    org.clone().run_task(task).await;
    let tree = tree_at(&root, org.settings.as_ref(), "s-manual");
    assert_eq!(tree.chapters.len(), 2);
    assert_eq!(tree.chaptered_until, 6);
    assert!(tree.pending.is_none());
    assert_eq!(tree.history[0].mode, "recorder");
    assert_eq!(tree.history[0].status, "ok");
    assert_eq!(tree.history[0].manager_input, 100);
    assert_eq!(tree.rebound.users_at_last, 3);
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn garbage_recorder_output_degrades_to_rule_split() {
    let root = temp_root("degrade");
    let settings = settings_with(serde_json::json!({}), &root);
    // 记忆管理器连续回垃圾（超重试次数），随后管道降级规则切分
    let junk = vec![Ok("我不会".into()); 4];
    let org = organizer(&root, settings, FakeClient::new(junk));
    let session = session();
    let task = org.plan_manual(&session, "s-rule").expect("发起成功");
    org.clone().run_task(task).await;
    let tree = tree_at(&root, org.settings.as_ref(), "s-rule");
    assert_eq!(tree.history[0].mode, "rule");
    assert_eq!(tree.history[0].status, "degraded");
    assert_eq!(tree.chaptered_until, 6); // 游标照样推进（尾巴已章节）
                                         // 规则切分铺满全部材料
    let covered: usize = tree
        .chapters
        .values()
        .map(|b| b.event_range.1 - b.event_range.0 + 1)
        .sum();
    assert_eq!(covered, 6);
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn incremental_trigger_then_rebound_pauses_auto() {
    let root = temp_root("rebound2");
    let settings = settings_with(
        serde_json::json!({
            "memory": {"organizePacing": "incremental", "organizeChunkTokens": 1},
            "developer": {"organizeReboundK": 1, "organizeReboundM": 1}
        }),
        &root,
    );
    let client = FakeClient::new(vec![Ok(good_reply()), Ok(good_reply())]);
    let org = organizer(&root, Arc::clone(&settings), client);

    // 第 1 轮收束：触发 → 整理（基线 users=3 落账）
    let first = session();
    let task = org.plan_auto(&first, "s-rb").expect("增量档应触发");
    org.clone().run_task(task).await;

    // 第 2 轮收束（k=1 轮内再触发，m=1 → 第一次反弹即熔断）：不 spawn
    let mut session2 = session();
    session2.append(SessionEvent::UserMessage {
        content: "第3轮问题".into(),
    });
    session2.append(SessionEvent::AssistantMessage {
        content: "第3轮回答".into(),
        tool_calls: vec![],
    });
    assert!(
        org.plan_auto(&session2, "s-rb").is_none(),
        "熔断后自动整理应跳过"
    );
    let tree = tree_at(&root, org.settings.as_ref(), "s-rb");
    assert!(tree.paused_auto);

    // 手动 /整理 不受限且复位
    let task = org
        .plan_manual(&session2, "s-rb")
        .expect("手动不受熔断限制");
    org.clone().run_task(task).await;
    let tree = tree_at(&root, org.settings.as_ref(), "s-rb");
    assert!(!tree.paused_auto);
    assert_eq!(tree.history.last().unwrap().reason, "manual");
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn batch_without_window_never_auto_fires() {
    let root = temp_root("nowin");
    let settings = settings_with(serde_json::json!({}), &root); // 缺省 batch + 窗口未知
    let org = organizer(&root, settings, FakeClient::new(vec![]));
    let session = session();
    assert!(org.plan_auto(&session, "s-nowin").is_none());
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn crash_recovery_redispatches_pending_task() {
    let root = temp_root("recover");
    let settings = settings_with(serde_json::json!({}), &root);
    // 会话落盘（模拟上次运行崩在整理中途：投影带 pending）
    let sessions_dir = root.join("sessions");
    std::fs::create_dir_all(&sessions_dir).unwrap();
    let session = session();
    crate::session_log::append_slice(&sessions_dir, "s-crash", &session, 0);
    let store = TreeStore::new(&root);
    let mut tree = memory_tree::MemoryTree::new(r"F:\ws\demo", "s-crash");
    tree.insert_placeholder(0, 5, 1).unwrap();
    store.save(&tree).unwrap();

    let org = organizer(&root, settings, FakeClient::new(vec![Ok(good_reply())]));
    let task = org.plan_recovery("s-crash").expect("在途任务应重投");
    assert_eq!(task.reason, OrganizeReason::Recovery);
    org.clone().run_task(task).await;
    let tree = tree_at(&root, org.settings.as_ref(), "s-crash");
    assert!(tree.pending.is_none());
    assert_eq!(tree.chaptered_until, 6);
    assert_eq!(tree.history[0].reason, "recovery");
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn recovery_gives_up_when_session_file_deleted() {
    let root = temp_root("gone");
    let settings = settings_with(serde_json::json!({}), &root);
    let store = TreeStore::new(&root);
    let mut tree = memory_tree::MemoryTree::new(r"F:\ws\demo", "s-gone");
    tree.insert_placeholder(0, 5, 1).unwrap();
    store.save(&tree).unwrap();

    let org = organizer(&root, settings, FakeClient::new(vec![]));
    assert!(org.plan_recovery("s-gone").is_none());
    let tree = tree_at(&root, org.settings.as_ref(), "s-gone");
    assert!(tree.pending.is_none()); // 占位撤除
    assert_eq!(tree.history[0].status, "aborted");
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn view_bundles_and_original_read_from_disk() {
    let root = temp_root("view");
    let settings = settings_with(serde_json::json!({}), &root);
    let org = organizer(&root, settings, FakeClient::new(vec![Ok(good_reply())]));
    assert!(org.tree_sessions().is_empty(), "空数据根 = 空清单");

    // 会话落盘 + 手动整理（M3 视图数据源 = 磁盘，与内存会话无关）
    let sessions_dir = root.join("sessions");
    std::fs::create_dir_all(&sessions_dir).unwrap();
    let session = session();
    crate::session_log::append_slice(&sessions_dir, "s-view", &session, 0);
    let disk_path = session_store::session_file(&sessions_dir, "s-view").unwrap();
    let (disk, _) = session_store::load_session(&disk_path).unwrap();
    let task = org.plan_manual(&disk, "s-view").expect("发起成功");
    org.clone().run_task(task).await;

    // 清单：一树两章节，mtime 有值
    let list = org.tree_sessions();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].session_id, "s-view");
    assert_eq!(list[0].chapter_count, 2);
    assert!(list[0].updated_ms > 0);
    assert!(!list[0].pending);

    // 章节清单：数值序 b1、b2（BTreeMap 字典序会把 b10 排到 b2 前）
    let view = org.tree_chapters("s-view", r"F:\ws\demo");
    assert!(view.exists);
    assert_eq!(view.chapters.len(), 2);
    // 占位章节消耗 b1，记忆管理器真章节从 b2 起
    assert_eq!(view.chapters[0].chapter_id, "b2");
    assert_eq!(view.chapters[0].title, "话题一");
    assert_eq!(view.chapters[0].source, "recorder");
    assert!(!view.chapters[0].pending);
    assert_eq!(view.chapters[0].event_from, 0);
    assert_eq!(view.chapters[0].event_to, 1);
    assert!(
        !org.tree_chapters("s-none", r"F:\ws\demo").exists,
        "缺树 = 空态"
    );

    // 原文切片与记忆管理器材料同形（章节 b1 = 事件 0..=1 一问一答）
    let orig = org
        .tree_original("s-view", r"F:\ws\demo", "b2")
        .expect("原文可读");
    assert_eq!(orig.len(), 2);
    assert_eq!(orig[0].text, "第0轮问题");
    assert_eq!(orig[0].kind, memory_tree::MaterialKind::User);
    assert_eq!(orig[1].kind, memory_tree::MaterialKind::Assistant);
    assert!(org.tree_original("s-view", r"F:\ws\demo", "b9").is_err());
    assert!(org.tree_original("s-none", r"F:\ws\demo", "b1").is_err());
    let _ = std::fs::remove_dir_all(&root);
}

// ---- M4 按会话切流：模式 B 溢出整理（不记反弹 / 熔断不受限）+ 自动收起 ----

use crate::organizer_mode::OrganizerMode as Mode;

/// 树模式编排器（organizer() 辅助硬编码 Legacy，溢出/收起要 Tree 档）。
fn tree_organizer(
    root: &std::path::Path,
    settings: Arc<SettingsStore>,
    client: Arc<dyn MemoryManagerClient>,
) -> Arc<Organizer> {
    Arc::new(Organizer::new(
        settings,
        root.join("sessions"),
        client,
        resolver(),
        root.to_path_buf(),
        Arc::new(crate::organizer_mode::OrganizerModeStore::new(
            root.join("sessions"),
            Mode::Tree,
        )),
        std::sync::Arc::new(|_, _| {}),
    ))
}

/// 十二轮会话（溢出窗口的孤儿轮素材）。
fn long_session() -> Session {
    let mut s = Session::new();
    for i in 0..12 {
        s.append(SessionEvent::UserMessage {
            content: format!("第{i}轮问题"),
        });
        s.append(SessionEvent::AssistantMessage {
            content: format!("第{i}轮回答"),
            tool_calls: vec![],
        });
    }
    s
}

#[tokio::test]
async fn window_overflow_organizes_orphans_without_rebound() {
    let root = temp_root("overflow");
    let settings = settings_with(
        serde_json::json!({
            "memory": {"tailStrategy": "window", "tailWindowTurns": 1},
            "developer": {"organizeReboundK": 1, "organizeReboundM": 1}
        }),
        &root,
    );
    let client = FakeClient::new(vec![Ok(good_reply()), Ok(good_reply())]);
    let org = tree_organizer(&root, Arc::clone(&settings), client);

    // 轮 1：chaptered_until=0 < 窗口起点（22）→ 溢出整理只归孤儿 [0..22)
    let s = long_session();
    let task = org.plan_auto(&s, "s-ov").expect("孤儿轮在场应触发溢出整理");
    assert_eq!(task.reason, memory_tree::OrganizeReason::WindowOverflow);
    assert_eq!(task.items.first().unwrap().event, 0);
    assert_eq!(
        task.items.last().unwrap().event,
        21,
        "只归窗口滑出的部分，最后一轮留尾巴"
    );
    org.clone().run_task(task).await;
    let tree = tree_at(&root, org.settings.as_ref(), "s-ov");
    assert_eq!(tree.chaptered_until, 22, "游标推进到窗口起点");
    assert_eq!(tree.rebound.users_at_last, 0, "溢出整理不碰反弹基线");
    assert!(!tree.paused_auto);

    // 轮 2：窗口再滑一行——但本轮只有既有 24 事件（无新轮），窗口起点
    // 仍在 22 = 游标处，不触发
    assert!(org.plan_auto(&s, "s-ov").is_none(), "无新孤儿不触发");

    // 加一轮：窗口起点滑到 24 > 游标 22 → 再溢出（连续第 2 次也不熔断——
    // k/m 只惩罚阈值/增量档的烧钱反弹）
    let mut s2 = s;
    s2.append(SessionEvent::UserMessage {
        content: "第12轮问题".into(),
    });
    s2.append(SessionEvent::AssistantMessage {
        content: "第12轮回答".into(),
        tool_calls: vec![],
    });
    let task = org.plan_auto(&s2, "s-ov").expect("新孤儿再触发");
    assert_eq!(task.reason, memory_tree::OrganizeReason::WindowOverflow);
    org.clone().run_task(task).await;
    let tree = tree_at(&root, org.settings.as_ref(), "s-ov");
    assert_eq!(tree.chaptered_until, 24);
    assert!(!tree.paused_auto, "溢出整理永不触发熔断");
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn overflow_respects_pending_and_legacy_mode() {
    let root = temp_root("ovgates");
    let settings = settings_with(
        serde_json::json!({"memory": {"tailStrategy": "window", "tailWindowTurns": 1}}),
        &root,
    );
    let client = FakeClient::new(vec![]);
    let org = tree_organizer(&root, Arc::clone(&settings), client);
    let s = long_session();

    // 在途任务占位：溢出让位（单写者）
    let mut tree = memory_tree::MemoryTree::new(r"F:\ws\demo", "s-ov2");
    tree.insert_placeholder(0, 5, 1).unwrap();
    memory_tree::TreeStore::new(&root).save(&tree).unwrap();
    assert!(org.plan_auto(&s, "s-ov2").is_none(), "在途任务期间不触发");
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn auto_collapse_relieves_oldest_expanded() {
    let root = temp_root("autocollapse");
    // thresholdPct 90：把「自动整理」触发阈值抬到本测锚点之上——本测
    // 只验收起，若 600/1000 也过整理线，after_turn 会经 tauri::async_
    // runtime（独立全局 runtime，不受测试单线程 tokio 约束）派生整理
    // 任务，FakeClient 空队列秒失败走规则降级、重建章节抹掉 collapsed_
    // at 回写，与本测读盘竞速（真实 flaky 根因，2026-10-06 定位）。
    let settings = settings_with(
        serde_json::json!({
            "context": {"windowTokens": 1000, "thresholdPct": 90},
            "memory": {"collapseEnabled": true, "collapseThresholdPct": 50}
        }),
        &root,
    );
    let client = FakeClient::new(vec![]);
    let org = tree_organizer(&root, Arc::clone(&settings), client);

    // 树 + 两个已展开章节（b2 更早展开）
    let mut tree = memory_tree::MemoryTree::new(r"F:\ws\demo", "s-ac");
    let d1 = vec![memory_tree::sanitize_draft("章节一", "摘要一", (0, 1)).unwrap()];
    let chapters = tree.build_chapters(&d1, "recorder", 1);
    tree.apply_organized(
        chapters,
        1,
        memory_tree::OrganizeRecord {
            ts: 1,
            reason: "manual".into(),
            mode: "recorder".into(),
            status: "ok".into(),
            from: 0,
            to: 1,
            chapters: 1,
            manager_input: 0,
            manager_output: 0,
            duration_ms: 1,
        },
    );
    tree.mark_expanded("b1", 100);
    memory_tree::TreeStore::new(&root).save(&tree).unwrap();

    // 占用未达阈值：不收
    let mut session = long_session();
    session.events().iter().for_each(|_| {});
    org.after_turn(&session, "s-ac");
    let t = tree_at(&root, org.settings.as_ref(), "s-ac");
    assert!(t.chapters["b1"].collapsed_at.is_none(), "占用低不收");

    // 用量锚点来自最后一条 LlmUsage（600 / 1000 = 60% ≥ 50%）→ 收起 b1
    session.append(SessionEvent::LlmUsage {
        input: 600,
        output: 10,
        cache_read: 0,
        cache_write: 0,
        reasoning: 0,
        total: 610,
        duration_ms: 1,
    });
    org.after_turn(&session, "s-ac");
    let t = tree_at(&root, org.settings.as_ref(), "s-ac");
    assert!(
        t.chapters["b1"].collapsed_at.is_some(),
        "过阈值收起最早展开"
    );
    assert_eq!(t.oldest_expanded(), None);

    // legacy 会话零参与
    let legacy_org = organizer(&root, Arc::clone(&settings), FakeClient::new(vec![]));
    legacy_org.after_turn(&session, "s-legacy");
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn edit_chapter_updates_projection_and_marks_manual() {
    let root = temp_root("edit");
    let settings = settings_with(serde_json::json!({}), &root);
    let org = organizer(&root, settings, FakeClient::new(vec![Ok(good_reply())]));
    let session = session();
    let task = org.plan_manual(&session, "s-edit").expect("发起成功");
    org.clone().run_task(task).await;

    // 编辑标题+摘要 → 投影更新 + 手动标记 + tree.md 同步（人读层在盘）
    org.tree_edit_chapter(
        r"F:\ws\demo",
        "s-edit",
        "b2",
        Some("  人改标题  "),
        Some("人改摘要"),
    )
    .unwrap();
    let tree = tree_at(&root, org.settings.as_ref(), "s-edit");
    let b2 = tree.chapters.get("b2").unwrap();
    assert_eq!(b2.title, "人改标题");
    assert_eq!(b2.summary, "人改摘要");
    assert!(b2.title_edited_by_user);
    assert!(b2.summary_edited_by_user);
    let ws_dir = plugin_api::memlayout::project_dir_name(r"F:\ws\demo");
    assert!(root
        .join("memory/projects")
        .join(ws_dir)
        .join("tree.md")
        .exists(),
        "编辑后 tree.md 已重建");

    // 校验拒绝：超上限标题 / 空+空 / 占位章节
    assert!(org
        .tree_edit_chapter(r"F:\ws\demo", "s-edit", "b2", Some(&"标".repeat(41)), None)
        .is_err());
    assert!(org
        .tree_edit_chapter(r"F:\ws\demo", "s-edit", "b2", Some("  "), Some(" "))
        .is_err());
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn delete_chapter_removes_projection_keeps_cursor() {
    let root = temp_root("delete");
    let settings = settings_with(serde_json::json!({}), &root);
    let org = organizer(&root, settings, FakeClient::new(vec![Ok(good_reply())]));
    let session = session();
    let task = org.plan_manual(&session, "s-del").expect("发起成功");
    org.clone().run_task(task).await;

    org.tree_delete_chapter(r"F:\ws\demo", "s-del", "b2").unwrap();
    let tree = tree_at(&root, org.settings.as_ref(), "s-del");
    assert_eq!(tree.chapters.len(), 1, "b2 已摘除");
    assert!(tree.chapters.contains_key("b3"));
    assert_eq!(tree.chaptered_until, 6, "游标不动：被删区间停留已整理侧");
    // 再删不存在章节 = Err
    assert!(org
        .tree_delete_chapter(r"F:\ws\demo", "s-del", "b2")
        .is_err());
    let _ = std::fs::remove_dir_all(&root);
}
