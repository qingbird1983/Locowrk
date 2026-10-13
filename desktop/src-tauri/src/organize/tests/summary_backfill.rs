//! 摘要补写通路的**行为边界**用例（队列 Q8c）。
//!
//! 债一律由**生产通路**造出来（`release_pressure` 泄压 / `run_task` 降级支），
//! 不手搭章节字面量：手搭造不出 `summary_pending` 的真实来源，本批要证的
//! 恰好是「谁欠债、谁来还」。异步用例走 `#[tokio::test]` + 直接驱动
//! `backfill_summaries`（`schedule_summary_backfill` 是 spawn 门面，它的**接线
//! 次序**由形状判据 `tmp/q8c-shapes.mjs` 钉，不在这里做成竞态用例——批 Q6
//! 立的口径：能做成确定性的就不要做成竞态）。
//!
//! 本簇只留通路行为的四条：注入目录逐字节不变（前缀缓存的字节前提）/ 0 档零改写 /
//! 失败落尝试次数并停在上限 / 会话日志缺席诚实放弃。「谁欠债」那三条在 `summary_debt.rs`
//! （v2.194 拆簇，理由 = 三问自检第③问：两拨修改动机不同）。

use kernel_agent::TreeContextSource;

use crate::organize::TreeAssembler;

use super::*;

/// **本批最值钱的一条**：注入侧看到的目录字节补写前后逐字节相同。
/// 两个事实叠起来才成立，缺一都会打断前缀缓存：
/// ① 目录行只由 `chapter_id` + `title` 拼成（`directory_text`）；
/// ② 项目级进展行在**同一会话内钉死**（Q2 的 `ProgressPin`——它读投影 mtime，
///   补写落盘会把 mtime 推新，靠钉死才不会把"last organize"改写）。
/// 所以这里必须复用同一个 `TreeAssembler` 实例：每次新建一个就等于绕过了②。
#[tokio::test]
async fn backfill_keeps_the_injected_directory_byte_identical() {
    let root = temp_root("backfill-cache");
    let settings = settings_with(
        serde_json::json!({"context": {"thresholdTokens": 1_000}}),
        &root,
    );
    let org = organizer(
        &root,
        Arc::clone(&settings),
        FakeClient::new(vec![Ok(GOOD_SUMMARY_REPLY.into())]),
    );
    let (session, turn_start) = persisted_turn(&root, "s-bfc");
    let assembler = TreeAssembler::new(
        Arc::clone(&settings),
        resolver(),
        memory_tree::TreeStore::new(&root),
        root.join("sessions"),
    );

    // 先泄压造出一章，**再**首次组装：目录里必须有 `- b1:` 那行，项目级进展行
    // 也在这一刻被钉死（真机 = 会话首请求）。⚠ 这条用例第一次跑就红了，红的
    // 是我的顺序：原先把首次组装排在泄压之前，那时树里还没有章节，`before` 是
    // 空串——「补写前后字节相同」被证成了一句空话（空串等于空串）。判据要打在
    // "目录里真有内容"的状态上，所以这里同时断言 `before` 非空。
    assert_eq!(release_once(&org, &session, "s-bfc", turn_start), 1);
    let before = assembler
        .plan("s-bfc")
        .await
        .expect("树会话必有计划")
        .directory;
    assert!(before.contains("- b1:"), "补写前目录就该有这一章：{before}");

    org.backfill_summaries(r"F:\ws\demo", "s-bfc").await;
    let after = assembler
        .plan("s-bfc")
        .await
        .expect("树会话必有计划")
        .directory;
    assert_eq!(before, after, "补写不得改变注入字节（前缀缓存的命中前提）");

    // 磁盘上摘要确实换了——"字节没变"不是因为"什么都没发生"
    let tree = tree_at(&root, org.settings.as_ref(), "s-bfc");
    assert!(tree.chapters["b1"].summary.contains("共享 runtime"));
    assert!(after.contains("- b1:"), "目录里就该只有编号与标题：{after}");
    assert!(
        !after.contains("（规则切分）") && !after.contains("共享 runtime"),
        "目录段不许包含摘要正文（含了才真的会因补写而变字节）"
    );
    std::fs::remove_dir_all(&root).ok();
}

/// **0 = 关掉补写**（可配置原则② + 队列 §6 第 12 条）：投影字节与关掉之前
/// 逐字节相同，连一次改写都没有（判据形状 = 字节不变，而不是"没报错"）。
#[tokio::test]
async fn backfill_off_writes_nothing_at_all() {
    let root = temp_root("backfill-off");
    let settings = settings_with(
        serde_json::json!({
            "context": {"thresholdTokens": 1_000},
            "developer": {"organizeSummaryChapters": 0}
        }),
        &root,
    );
    let org = organizer(
        &root,
        Arc::clone(&settings),
        FakeClient::new(vec![Ok(GOOD_SUMMARY_REPLY.into())]),
    );
    let (session, turn_start) = persisted_turn(&root, "s-bfo");
    release_once(&org, &session, "s-bfo", turn_start);

    // 投影的序列化形状前后必须完全一致（不猜 `memory-tree/<scope>/<id>.json` 的
    // 键位编码：语义零变化由这份字符串证，"零调用"由下面那条空脚本夹具证）
    let dump =
        |sid: &str| serde_json::to_string(&tree_at(&root, org.settings.as_ref(), sid)).unwrap();
    let before = dump("s-bfo");
    assert!(before.contains("（规则切分）"), "关着之前那章仍是机械摘要");
    org.backfill_summaries(r"F:\ws\demo", "s-bfo").await;
    assert_eq!(dump("s-bfo"), before, "关掉的那条通路必须一个字节都不写");

    // 换一把空脚本再走一次：真去调模型就会拿到「没有脚本回复」的 Err 并留下
    // 尝试计数，字节仍然不变 = 这条通路根本没被走到（不是走了又白跑一趟）
    let org2 = organizer(&root, Arc::clone(&settings), FakeClient::new(vec![]));
    org2.backfill_summaries(r"F:\ws\demo", "s-bfo").await;
    assert_eq!(dump("s-bfo"), before, "0 档连一次模型调用都不该发起");
    std::fs::remove_dir_all(&root).ok();
}

/// 失败要落账：模型给的是"能对上的空数组"⇒ 这批章节计入尝试次数，
/// 到上限后**不再入选**（否则坏章每轮重烧一次，无界）。
#[tokio::test]
async fn failed_backfill_accounts_attempts_and_stops_at_the_cap() {
    let root = temp_root("backfill-fail");
    let settings = settings_with(
        serde_json::json!({
            "context": {"thresholdTokens": 1_000},
            "developer": {"organizeSummaryChapters": 6, "organizeSummaryAttempts": 2, "recorderMaxRetries": 0}
        }),
        &root,
    );
    // 三条脚本全是空数组：前两次各烧一次调用并记账，第三次通路自己就不成立了
    let org = organizer(
        &root,
        Arc::clone(&settings),
        FakeClient::new(vec![Ok("[]".into()), Ok("[]".into())]),
    );
    let (session, turn_start) = persisted_turn(&root, "s-bff");
    release_once(&org, &session, "s-bff", turn_start);

    org.backfill_summaries(r"F:\ws\demo", "s-bff").await;
    let tree = tree_at(&root, org.settings.as_ref(), "s-bff");
    assert_eq!(tree.chapters["b1"].summary_attempts, 1, "失败计入尝试次数");
    assert!(
        tree.chapters["b1"].summary_pending,
        "失败不清债：磁盘仍是机械摘要"
    );
    assert!(tree.chapters["b1"].summary.starts_with("（规则切分）"));

    org.backfill_summaries(r"F:\ws\demo", "s-bff").await;
    assert_eq!(
        tree_at(&root, org.settings.as_ref(), "s-bff").chapters["b1"].summary_attempts,
        2
    );

    // 第 3 次：候选池已空 ⇒ 脚本里那条"没被消费"的回复不会被取走，磁盘字节不变
    let before = serde_json::to_string(&tree_at(&root, org.settings.as_ref(), "s-bff")).unwrap();
    let org2 = organizer(
        &root,
        Arc::clone(&settings),
        FakeClient::new(vec![]), // 空脚本：真去调模型就会拿到 Err 而走失败记账，字节不可能不变
    );
    org2.backfill_summaries(r"F:\ws\demo", "s-bff").await;
    let after = serde_json::to_string(&tree_at(&root, org2.settings.as_ref(), "s-bff")).unwrap();
    assert_eq!(before, after, "到上限后这批章节不再被挑，也不被改写");
    std::fs::remove_dir_all(&root).ok();
}

/// 会话日志已删 = 诚实放弃：不烧模型、不动投影（材料取不到就什么都不做，
/// 与补跑/崩溃恢复同一口径）。
#[tokio::test]
async fn backfill_gives_up_when_the_session_log_is_gone() {
    let root = temp_root("backfill-nolog");
    let settings = settings_with(
        serde_json::json!({"context": {"thresholdTokens": 1_000}}),
        &root,
    );
    let org = organizer(
        &root,
        Arc::clone(&settings),
        FakeClient::new(vec![Ok(GOOD_SUMMARY_REPLY.into())]),
    );
    let (session, turn_start) = persisted_turn(&root, "s-bfn");
    release_once(&org, &session, "s-bfn", turn_start);
    std::fs::remove_file(session_store::session_file(&root.join("sessions"), "s-bfn").unwrap())
        .unwrap();

    let before = tree_at(&root, org.settings.as_ref(), "s-bfn");
    org.backfill_summaries(r"F:\ws\demo", "s-bfn").await;
    let after = tree_at(&root, org.settings.as_ref(), "s-bfn");
    assert_eq!(after.chapters["b1"].summary, before.chapters["b1"].summary);
    assert!(
        after.chapters["b1"].summary_pending,
        "取不到材料就不该把债判成已还"
    );
    assert_eq!(
        after.chapters["b1"].summary_attempts, 0,
        "日志缺席不算一次失败尝试"
    );
    std::fs::remove_dir_all(&root).ok();
}
