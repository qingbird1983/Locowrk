//! 尾巴材料与游标守恒（审计队列 **Q3b** · v2.187）。
//!
//! 本簇五条用例守的是同一个缺陷的两半，缺任何一半都算没修干净（用甲/乙编号，避免与
//! 下面各条用例的 ①–⑤ 标签撞号）：
//! **（甲）** `build_items` 必须收**尾巴切片**而不是全量事件——否则每次整理重喂整段历史，
//!   并且 `event` 被记成「游标 + 下标」的假绝对位；
//! **（乙）** 越界游标必须**当场钳回并落盘**——`apply_organized` 用 `max(to + 1)` 单调推进，
//!   历史损坏值永不自愈；只给切片加 `min()` 防越界而不落盘，缺陷就从"重喂全段"变成
//!   "该会话永久不再整理"，是更坏的一种。
//!
//! （甲）的直接后果还有一条写在根侧：`parse_drafts` 原来拿模型给的 `event_range` 直接下标
//! 取 `items[b]` ⇒ 材料变少之后越界是**常态**而非罕见，panic 被既有测试当场撞出来
//! （判据 = `manager::tests::parse_drafts_rejects_an_event_range_past_the_material`）。

use super::*;

/// 覆盖两项材料的合法回复（第二次整理的尾巴正好两项）。
fn two_item_reply() -> String {
    r#"[{"title":"话题三","summary":"摘要三。","event_range":[0,1]}]"#.to_string()
}

/// 追加第四轮问答（会话从 6 条事件变 8 条）。
fn with_extra_turn() -> Session {
    let mut s = session();
    s.append(SessionEvent::UserMessage {
        content: "第四轮问题".into(),
        parts: Vec::new(),
    });
    s.append(SessionEvent::AssistantMessage {
        content: "第四轮回答".into(),
        tool_calls: vec![],
    });
    s
}

/// ① 第二次整理的材料 = 新增那一轮，且 `event` 是**真实日志位**。
#[tokio::test]
async fn second_organize_gets_only_the_new_events_and_their_real_log_positions() {
    let root = temp_root("tail-slice");
    let settings = settings_with(serde_json::json!({}), &root);
    let org = organizer(
        &root,
        settings,
        FakeClient::new(vec![Ok(good_reply()), Ok(two_item_reply())]),
    );
    let task = org.plan_manual(&session(), "s-tail").expect("第一次发起");
    assert_eq!(task.items.len(), 6, "第一次（游标 0）材料就是全量");
    org.clone().run_task(task).await;
    assert_eq!(
        tree_at(&root, org.settings.as_ref(), "s-tail").chaptered_until,
        6,
        "第一次收束后游标 = 事件数"
    );

    let s2 = with_extra_turn();
    let task = org.plan_manual(&s2, "s-tail").expect("第二次发起");
    assert_eq!(task.items.len(), 2, "第二次材料必须只剩新增那一轮");
    assert_eq!(task.items[0].event, 6, "首项绝对位 = 日志第 7 条");
    assert_eq!(task.items[1].event, 7);
    org.clone().run_task(task).await;

    let tree = tree_at(&root, org.settings.as_ref(), "s-tail");
    assert_eq!(
        tree.chaptered_until,
        s2.events().len(),
        "游标等于事件数，不再跳出日志末尾（修复前这里是 14）"
    );
    assert_eq!(tree.chapters.len(), 3, "两次各归各的章节，不重复铺同一段");
    let _ = std::fs::remove_dir_all(&root);
}

/// ② 损坏投影（越界游标）必须**钳回 + 落盘**，并且会话之后还能重新整理。
///
/// 只加 `min()` 而不落盘的话：游标 99 每次都被顶到当前事件数 ⇒ 新事件永远落在游标
/// 之下 ⇒ 尾巴恒空 ⇒ 该会话永久静默（比原来的"重喂全段"更坏，因为它悄无声息）。
#[tokio::test]
async fn a_cursor_past_the_end_is_clamped_persisted_and_the_session_recovers() {
    let root = temp_root("cursor-clamp");
    let settings = settings_with(serde_json::json!({}), &root);
    let org = organizer(&root, settings, FakeClient::new(vec![Ok(two_item_reply())]));
    // 手造 Q3b 修复前的产物形态：整理过一次之后游标被推到越界值
    let mut broken = memory_tree::MemoryTree::new(r"F:\ws\demo", "s-clamp");
    broken.chaptered_until = 99;
    TreeStore::new(&root).save(&broken).unwrap();

    // `TaskPayload` 不带 `Debug` ⇒ `unwrap_err()`（它要求 Ok 侧也可 Debug）用不了；
    // 不为一条测试去给生产类型加派生，本地 match 一样说得清。
    match org.plan_manual(&session(), "s-clamp") {
        Err(e) => assert!(e.contains("没有可整理的尾巴"), "越界那截账作废: {e}"),
        Ok(_) => panic!("游标钳回之后不该再有尾巴"),
    }
    assert_eq!(
        tree_at(&root, org.settings.as_ref(), "s-clamp").chaptered_until,
        6,
        "钳回之后必须已经落盘（否则下次决策还要从头钳，且新事件永远进不了尾巴）"
    );

    // 新事件到来 ⇒ 正常进入尾巴：钳制是把账作废，不是把会话判死刑
    let s2 = with_extra_turn();
    let task = org.plan_manual(&s2, "s-clamp").expect("新事件要能整理");
    assert_eq!(task.items.len(), 2, "钳回之后尾巴照常");
    assert_eq!(task.items[0].event, 6);
    let _ = std::fs::remove_dir_all(&root);
}

/// ③ 开发者观测面板读的也必须是尾巴：它是人判断"这次要喂多大一段"的唯一窗口，
/// 报全量等于**用观测面把缺陷掩护住**（历批「观测不许失真」同族，方向相反的一侧）。
#[tokio::test]
async fn tree_debug_reports_the_tail_not_the_whole_log() {
    let root = temp_root("tail-debug");
    let settings = settings_with(serde_json::json!({}), &root);
    let org = organizer(&root, settings, FakeClient::new(vec![Ok(good_reply())]));
    assert_eq!(
        org.tree_view(&session(), "s-dbg").tail_events,
        6,
        "整理之前尾巴 = 全部事件（游标 0）"
    );
    let task = org.plan_manual(&session(), "s-dbg").expect("发起");
    org.clone().run_task(task).await;
    let d = org.tree_view(&session(), "s-dbg");
    assert_eq!(d.chaptered_until, 6, "游标推进到事件数");
    assert_eq!(
        d.tail_events, 0,
        "整理过的会话在观测面上必须是零尾巴（修复前这里报 6 = 与 plan_* 同样的假读数）"
    );
    assert_eq!(d.tail_tokens_est, 0, "token 读数同源");
    let _ = std::fs::remove_dir_all(&root);
}

/// ④ `tail_material` 的**私有函数契约**（第二道防线的独有点）：`clamp_cursor` 与这里
/// 的 `min()` 对越界游标构成冗余防御——历批教训（Q2 的"两道等价判空任摘其一都不红"）
/// 要求给次要那道单独找一个只有它能守住的点，否则对它做变异等于没验。
///
/// 这个点就是"不 panic"：`clamp_cursor` 只在 `plan_*` 的决策路径上跑，而
/// `tail_material` 也被 `tree_view`（只读诊断面，**不钳游标**）调用 ⇒ 拿一个越界游标
/// 直接调它，越界的兜底只可能来自 `min()`。
#[test]
fn tail_material_clamps_a_cursor_past_the_end_instead_of_panicking() {
    let events = session();
    let all = events.events();
    // 越界输入：base 99 > 事件数
    let (tail, base) = Organizer::tail_material(all, 99);
    assert_eq!(base, all.len(), "起点钳到事件数（不是 99，也不是 0）");
    assert!(tail.is_empty(), "越界给空尾巴而不是 panic、也不是整段");
    // 正常输入不受扰动
    let (tail6, base6) = Organizer::tail_material(all, 4);
    assert_eq!(base6, 4);
    assert_eq!(tail6.len(), all.len() - 4, "游标合法时切的就是尾巴");
    // 阳性对照：游标 0 时尾巴 = 全量（否则上面的"切的就是尾巴"分不清切对了方向）
    let (tail0, base0) = Organizer::tail_material(all, 0);
    assert_eq!((base0, tail0.len()), (0, all.len()));
}

/// ⑤ `should_auto_organize` 收到的 `tail_tokens` / `tail_events` 必须是**尾巴**的量。
///
/// 用增量档钉（它按 `chunk_tokens` 触发、不看会话总预算，所以能把"尾巴读数失实"
/// 单独隔离出来）：修复前每次自动触发都拿到全量对话 ⇒ 整理过的会话仍然"有尾巴"、
/// 反复重整理；修复后尾巴为 0 ⇒ 恒不触发，来一轮才触发一轮。
#[tokio::test]
async fn incremental_auto_trigger_measures_the_tail_not_the_whole_log() {
    let root = temp_root("tail-auto");
    let settings = settings_with(
        serde_json::json!({
            "memory": {"organizePacing": "incremental", "organizeChunkTokens": 1}
        }),
        &root,
    );
    let org = organizer(
        &root,
        settings,
        FakeClient::new(vec![Ok(good_reply()), Ok(two_item_reply())]),
    );
    let task = org.plan_manual(&session(), "s-auto").expect("手动发起");
    org.clone().run_task(task).await;
    assert!(
        org.plan_auto(&session(), "s-auto").is_none(),
        "全部事件都已章节化 ⇒ 没有尾巴，增量档也不该再触发一次整理"
    );

    let s2 = with_extra_turn();
    // （`TaskPayload` 不带 `Debug`，Option::expect 因此用不了——不为一条测试去给
    //  生产类型加派生，改成本地 match，失败信息一样说得清）
    let task = match org.plan_auto(&s2, "s-auto") {
        Some(t) => t,
        None => panic!("新增一轮应当触发增量整理"),
    };
    assert_eq!(
        task.items.len(),
        2,
        "自动触发的材料 = 新增那一轮，不是全量 8 条"
    );
    assert_eq!(task.items[0].event, 6);
    assert_eq!(task.items[1].event, 7);
    let _ = std::fs::remove_dir_all(&root);
}
