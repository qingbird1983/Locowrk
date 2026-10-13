//! 轮内水位泄压（队列 Q8b）。
//!
//! 全部用例是**同步 `#[test]`**：`release_pressure` 不调记忆管理器，所以夹具
//! 里的 `FakeClient` 一律给空脚本（`vec![]`）——只要实现里冒出一句模型调用，
//! 这些用例就会因为"没有脚本回复"或异步运行时缺失而红，而不是安静地绿。
//! 这一条本身就是「泄压 = 机械切、不等模型」的判据形状（变异 M2 打这一刀）。

use session_store::TurnId;

// 类型由 `organize::pressure` 直接引用来命名，**不走 `organize::mod.rs` 的再导出**：
// `organize` 在 lib.rs 里是私有模块，簇内 `pub use pressure::Release;` 既出不了 crate、
// 又没有 lib 侧的命名点 ⇒ clippy 的 `unused_imports` 把它判成一条没用的导入而 BLOCK
// （本批 clippy 首跑就红在这里）。测试是 organize 的后代，直连簇路径就够，不需要门面。
use crate::organize::pressure::Release;

use super::*;

/// 历史五轮 + 一次 provider 用量上报（anchor = 9,500）+ 本轮进场两事件。
/// 返回值第二项 = 本轮开始时的日志长度（泄压切口的上界）。
fn history_then_turn() -> (Session, usize) {
    let mut s = Session::new();
    for i in 0..5 {
        s.append(SessionEvent::UserMessage {
            content: format!("历史第{i}轮问题"),
            parts: Vec::new(),
        });
        s.append(SessionEvent::AssistantMessage {
            content: format!("历史第{i}轮回答"),
            tool_calls: vec![],
        });
    }
    s.append(SessionEvent::LlmUsage {
        input: 9_500,
        output: 10,
        cache_read: 0,
        cache_write: 0,
        reasoning: 0,
        total: 9_510,
        duration_ms: 1,
    });
    let turn_start = s.len();
    s.append(SessionEvent::TurnStart {
        turn_id: TurnId(6),
        ts: 1,
    });
    s.append(SessionEvent::UserMessage {
        content: "本轮新问题".into(),
        parts: Vec::new(),
    });
    (s, turn_start)
}

/// 窗口 10,000 / 水位缺省 90% ⇒ 动手线 = 9,000；anchor 9,500 已过线，
/// 整理阈值 1,000 也已过线（两个条件同时成立才是泄压）。
fn window() -> u64 {
    10_000
}

fn settings_threshold(root: &std::path::Path, tokens: u64) -> Arc<SettingsStore> {
    settings_with(
        serde_json::json!({"context": {"thresholdTokens": tokens}}),
        root,
    )
}

#[test]
fn release_at_watermark_folds_only_the_history_before_this_turn() {
    let root = temp_root("pressure-cut");
    let settings = settings_threshold(&root, 1_000);
    let org = organizer(&root, Arc::clone(&settings), FakeClient::new(vec![]));
    let (session, turn_start) = history_then_turn();

    let out = org.release_pressure(&session, "s-cut", window(), 0, turn_start);
    assert!(
        matches!(
            out,
            Release::Cut {
                chapters: 1,
                until: 10
            }
        ),
        "应切 1 章、游标到本轮之前的最后一条材料事件 +1：{out:?}"
    );

    let tree = tree_at(&root, org.settings.as_ref(), "s-cut");
    assert_eq!(tree.chaptered_until, 10, "游标推进到 9+1，不是 11");
    assert_eq!(tree.chapter_count(), 1);
    let b = &tree.chapters["b1"];
    assert_eq!(b.event_range, (0, 9), "章节区间 = 本轮之前的历史");
    assert_eq!(b.source, "rule", "泄压产物是机械切分，不是模型总结");
    assert!(!b.pending);
    for chapter in tree.chapters.values() {
        assert!(
            chapter.event_range.1 < turn_start,
            "本轮的起点事件 {} 不许被章节覆盖（实际到 {}）",
            turn_start,
            chapter.event_range.1
        );
    }
    assert!(tree.pending.is_none());
    let last = tree.history.last().expect("泄压必须留一条台账");
    assert_eq!(last.reason, "pressure_release");
    assert_eq!((last.mode.as_str(), last.status.as_str()), ("rule", "ok"));
    assert_eq!((last.from, last.to, last.chapters), (0, 9, 1));
    assert_eq!(
        tree.rebound.users_at_last, 0,
        "泄压不记反弹——它不是「整理后立刻再超」的烧钱信号（与溢出整理同族）"
    );
    std::fs::remove_dir_all(&root).ok();
}

/// **0 = 关掉轮内泄压**（可配置原则②）。这一条要证的不是"没报错"，而是
/// "行为与本批之前逐字节相同"：判定命中、水位也命中，但上限为 0 ⇒ 连投影
/// 目录都不该被创建（泄压通路一次写盘都不做）。
#[test]
fn release_cap_zero_writes_nothing_at_all() {
    let root = temp_root("pressure-off");
    let settings = settings_with(
        serde_json::json!({
            "context": {"thresholdTokens": 1_000},
            "developer": {"organizeMaxRelease": 0}
        }),
        &root,
    );
    let org = organizer(&root, Arc::clone(&settings), FakeClient::new(vec![]));
    let (session, turn_start) = history_then_turn();

    let out = org.release_pressure(&session, "s-off", window(), 0, turn_start);
    assert_eq!(out, Release::Skip("off-or-cap"));
    assert!(
        !root.join("memory-tree").exists(),
        "关掉的那条通路必须一个字节都不写（投影目录都不该出现）"
    );
    std::fs::remove_dir_all(&root).ok();
}

/// 上限数的是**泄压次数**不是章节数：第二次进来（released=1）而缺省上限 2 时
/// 仍应工作，released=2 时收手。
#[test]
fn release_cap_counts_releases_not_chapters() {
    let root = temp_root("pressure-cap");
    let settings = settings_threshold(&root, 1_000);
    let org = organizer(&root, Arc::clone(&settings), FakeClient::new(vec![]));
    let (session, turn_start) = history_then_turn();

    assert_eq!(
        org.release_pressure(&session, "s-cap", window(), 1, turn_start)
            .chapters(),
        1,
        "第 2 次仍在缺省上限（2）之内"
    );
    assert_eq!(
        org.release_pressure(&session, "s-cap2", window(), 2, turn_start),
        Release::Skip("off-or-cap"),
        "到达上限即收手（上限是硬闸，不是建议）"
    );
    std::fs::remove_dir_all(&root).ok();
}

/// 该整理 ≠ 该泄压：水位未到就一律不动，且**不写盘**（每个 step 都会问一次，
/// 没到线却写盘 = 每步一次投影往返）。
#[test]
fn release_waits_for_the_watermark_even_when_organizing_is_due() {
    let root = temp_root("pressure-watermark");
    let settings = settings_threshold(&root, 1_000);
    let org = organizer(&root, Arc::clone(&settings), FakeClient::new(vec![]));
    let (session, turn_start) = history_then_turn();

    // anchor 9,500 / 窗口 20,000 = 47.5% < 90% ⇒ 整理判定命中、水位不命中
    let out = org.release_pressure(&session, "s-wm", 20_000, 0, turn_start);
    assert_eq!(out, Release::Skip("below-watermark"));
    assert!(
        !root.join("memory-tree").exists(),
        "未到水位的检查必须是纯读：不创建投影"
    );
    std::fs::remove_dir_all(&root).ok();
}

/// 窗口未知 = 不猜口径（与 `auto_collapse` 的 `window == 0` 同形）。
#[test]
fn release_needs_a_known_window() {
    let root = temp_root("pressure-window");
    let settings = settings_threshold(&root, 1_000);
    let org = organizer(&root, Arc::clone(&settings), FakeClient::new(vec![]));
    let (session, turn_start) = history_then_turn();

    assert_eq!(
        org.release_pressure(&session, "s-win", 0, 0, turn_start),
        Release::Skip("window-unknown")
    );
    std::fs::remove_dir_all(&root).ok();
}

/// 整理没到阈值时，水位再高也不切（切了也没有"该归档"的依据，且会把目录
/// 变成每轮固定开销）。
#[test]
fn release_does_not_fire_before_the_organizing_threshold() {
    let root = temp_root("pressure-notdue");
    let settings = settings_threshold(&root, 100_000);
    let org = organizer(&root, Arc::clone(&settings), FakeClient::new(vec![]));
    let (session, turn_start) = history_then_turn();

    assert_eq!(
        org.release_pressure(&session, "s-nd", window(), 0, turn_start),
        Release::Skip("not-due")
    );
    assert!(!root.join("memory-tree").exists(), "判定未命中同样不许写盘");
    std::fs::remove_dir_all(&root).ok();
}

/// 在途与熔断两条让位支：泄压也是写路径，单写者纪律（§16）不容两条通路
/// 同时推游标；熔断暂停的是自动整理，泄压若绕过它 = 给暂停中的会话开后门
/// （与 §8.4「补跑不是绕过熔断的后门」同一口径）。
#[test]
fn release_yields_to_pending_and_circuit_break() {
    let root = temp_root("pressure-yield");
    let settings = settings_threshold(&root, 1_000);
    let org = organizer(&root, Arc::clone(&settings), FakeClient::new(vec![]));
    let (session, turn_start) = history_then_turn();

    let mut tree = memory_tree::MemoryTree::new(r"F:\ws\demo", "s-busy");
    tree.insert_placeholder(0, 9, 1).unwrap();
    memory_tree::TreeStore::new(&root).save(&tree).unwrap();
    assert_eq!(
        org.release_pressure(&session, "s-busy", window(), 0, turn_start),
        Release::Skip("in-flight")
    );

    let mut tree = memory_tree::MemoryTree::new(r"F:\ws\demo", "s-paused");
    tree.paused_auto = true;
    memory_tree::TreeStore::new(&root).save(&tree).unwrap();
    assert_eq!(
        org.release_pressure(&session, "s-paused", window(), 0, turn_start),
        Release::Skip("circuit-broken")
    );
    // 让位支不得动磁盘上的投影：两次调用后占位仍在原位
    let after = tree_at(&root, org.settings.as_ref(), "s-busy");
    assert!(after.pending.is_some(), "让位不是撤占位");
    std::fs::remove_dir_all(&root).ok();
}

/// 本轮之前没有历史（游标已顶到本轮起点）时不许空切——这条挡的是
/// "把本轮刚发生的事归成章节"那种越界刀口。
#[test]
fn release_refuses_to_cut_when_nothing_precedes_the_turn() {
    let root = temp_root("pressure-empty");
    let settings = settings_threshold(&root, 1_000);
    let org = organizer(&root, Arc::clone(&settings), FakeClient::new(vec![]));
    let (session, turn_start) = history_then_turn();

    // turn_start_event = 0 ⇒ 本轮之前什么都不存在
    assert_eq!(
        org.release_pressure(&session, "s-empty", window(), 0, 0),
        Release::Skip("nothing-before-this-turn")
    );
    // 越界游标会被当场钳回（Q3b 的自愈），钳完 = 事件数 ⇒ 尾巴为空。
    // ⚠ 期望的理由串是 `not-due` 而不是 `nothing-before-this-turn`：钳完之后
    // `should_auto_organize` 先看到 `tail_events == 0` 就说"不该整理"，刀口那条
    // 支根本轮不到。**这一条第一次跑就红了**，红的是我的期望（照措辞写的），
    // 磁盘上的行为是对的：钳回后既不切也不写第二遍。判据要照代码实际形状而不是
    // 照我想像的顺序（历批「判据照措辞不照磁盘形状」同族，登记进本批判据自纠）。
    let mut tree = memory_tree::MemoryTree::new(r"F:\ws\demo", "s-clamped");
    tree.chaptered_until = 999;
    memory_tree::TreeStore::new(&root).save(&tree).unwrap();
    assert_eq!(
        org.release_pressure(&session, "s-clamped", window(), 0, turn_start),
        Release::Skip("not-due"),
        "钳回后尾巴为空 ⇒ 判定层先拦住（结果一样：不切）"
    );
    let after = tree_at(&root, org.settings.as_ref(), "s-clamped");
    assert_eq!(
        after.chaptered_until,
        session.events().len(),
        "越界游标在泄压通路上也被钳回（与 plan_auto 同形）"
    );
    std::fs::remove_dir_all(&root).ok();
}
