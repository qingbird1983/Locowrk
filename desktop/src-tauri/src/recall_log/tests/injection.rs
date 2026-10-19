//! 注入档埋点判据（父簇 `mod.rs` 只放夹具，用例在本簇）。

use kernel_agent::MemorySource;

use crate::memory::MemoryStore;
use crate::recall_log::RecallLog;

use super::{rows, shape, store_with_log, temp_dir, test_resolver};

// ---------------------------------------------------------------- 三档埋点（§2.4）

#[test]
fn nonempty_injection_lands_one_row_with_keys_and_candidates() {
    let (store, log) = store_with_log(
        "nonempty",
        &[
            ("", "user.lang", "中文"),
            ("", "user.tone", "直给"),
            ("F:/ws/a", "project.rule", "别动 lockfile"),
        ],
    );
    let got = store.recall(Some("ws-session"), 10);
    assert_eq!(got.len(), 2, "现役只注入全局层：两条全局、项目层不进");

    let r = rows(&log);
    assert_eq!(r.len(), 1, "一次 recall 一行");
    assert_eq!(r[0].mode, "inject");
    assert_eq!(r[0].session_key, "ws-session");
    assert_eq!(r[0].n_candidates, 2, "候选只数全局层，与查询同口径");
    assert_eq!(r[0].returned, 2);
    let mut keys = r[0].keys.clone();
    keys.sort();
    assert_eq!(keys, vec!["user.lang".to_string(), "user.tone".to_string()]);
    assert!(
        !r[0].scope_id.is_empty(),
        "归属会话必须带键位（去重键的一半）"
    );
}

#[test]
fn empty_store_injection_still_lands_a_row() {
    // L0 §2.4 的「库为空」档：**当前真实现场就是这一档**（2026-10-12 实测 memories
    // 表 0 行、四个项目 facts/ 全空）。它必须留下一行 returned=0，否则空块率无从算起。
    let (store, log) = store_with_log("emptystore", &[]);
    assert!(store.recall(Some("ws-session"), 10).is_empty());
    let r = rows(&log);
    assert_eq!(r.len(), 1);
    assert_eq!((r[0].n_candidates, r[0].returned), (0, 0));
    assert!(r[0].keys.is_empty(), "空交付的 keys 是空表不是 [\"\"]");
    let st = log.stats().unwrap();
    assert_eq!((st.injections, st.empty_injections), (1, 1));
    assert_eq!(st.store_empty_ratio, Some(1.0));
}

#[test]
fn project_only_store_reads_as_empty_injection() {
    // 「注入为空」的第二种成因：库里有货但都在项目层（现役不自动注入）。
    // 这一档与「库真的空」在指标上**故意同值**——空块率量的是「进了上下文的有没有」，
    // 不是「库里有没有」。写成断言是为了防止后来人「顺手」把 n_candidates 改成全表计数。
    let (store, log) = store_with_log("projonly", &[("F:/ws/a", "project.rule", "别动 lockfile")]);
    assert!(store.recall(Some("ws-session"), 10).is_empty());
    let r = rows(&log);
    assert_eq!((r[0].n_candidates, r[0].returned), (0, 0));
}

// ------------------------------------------------------ 去重口径（§2.4 重新计数）

#[test]
fn same_session_and_scope_dedups_to_one_row() {
    // 无 session_id 的调用每 step 都走到 recall（`memory.rs` 原注释「每 step 都 recall，
    // 别刷日志」就是这个场景）。不去重 = 日志按 step 膨胀、指标被重复行主导。
    let (store, log) = store_with_log("dedup", &[("", "user.lang", "中文")]);
    for _ in 0..5 {
        store.recall(None, 10);
    }
    let r = rows(&log);
    assert_eq!(r.len(), 1, "同一键位反复召回只留第一条");
    assert_eq!(
        r[0].session_key, "",
        "无会话用空串哨兵（NULL 进不了部分唯一索引）"
    );
}

#[test]
fn scope_change_recounts_as_a_new_row() {
    // L0 §2.4 明写的验收：「scope_id 变更时重新计数」。两个会话归属不同项目 = 键位不同
    // = 各留一条；否则「换了项目之后的召回」会被去重吞掉。
    let (store, log) = store_with_log("scopechange", &[("", "user.lang", "中文")]);
    store.recall(Some("ws-session"), 10);
    store.recall(Some("other-session"), 10);
    let r = rows(&log);
    assert_eq!(r.len(), 2, "键位变了必须新记一条");
    assert_ne!(r[0].scope_id, r[1].scope_id);
    assert_eq!(r[0].session_key, "other-session", "recent 是倒序");
}

#[test]
fn same_scope_different_session_still_dedups_per_session() {
    // 去重键含 session_key（不只按 scope）：同一项目下多个会话是**不同**的观测现场，
    // 压成一条会低估空块率。
    let (store, log) = store_with_log("persession", &[("", "user.lang", "中文")]);
    store.recall(Some("anon-1"), 10);
    store.recall(Some("anon-2"), 10);
    assert_eq!(rows(&log).len(), 2);
}

// ------------------------------------------------------ 零行为变更 / 观测不反噬

#[test]
fn recall_rows_are_identical_with_and_without_instrumentation() {
    // **本批的核心等价判据**：接了观测句柄的 store 与没接的，召回结果必须逐字段相等
    // （含顺序与 updated_at）。只有「零行为变更」成立，L0 才配当 Q2/Q3/Q9 的基线。
    const SEED: [(&str, &str, &str); 3] = [
        ("", "user.lang", "中文"),
        ("", "user.tone", "直给"),
        ("", "user.stack", "Rust"),
    ];
    let plain = MemoryStore::new(&temp_dir("plain"), test_resolver());
    let (store, log) = store_with_log("instr", &[]);
    // ⚠ 时间戳必须**钉死**，不能用常规 `write`（它内部取 `now_secs()`）：两侧各写
    // 三行，只要跨过一秒边界，同一条就在两侧带上不同的 `updated_at`，排序随之不同
    // ⇒ 这条等价判据会红在时钟上而不是行为上（2026-10-12 首次撞上：单跑绿、
    // 全量跑红）。
    //
    // ⚠ 造数据从 `reconcile_upsert` 换成 **`memlayout::write_fact`**（S1 刀②c-1 的必然后果）：
    // 前者只写影子不写真源，读源换到 md 之后它造的三行一条也读不到 ⇒ 这条判据会红在
    // 最后那句"阳性对照：不是空对空"上。**那次红是判据在正常工作**：它拒绝让
    // "两侧都召回 0 行" 冒充 "召回逐字段相等"。`write_fact` 是既能指定 `at`、又走
    // 生产渲染（frontmatter 落 `metadata.updated`）的那个入口。
    const BASE: i64 = 1_700_000_000;
    for (i, (ws, key, value)) in SEED.iter().enumerate() {
        let at = BASE + i as i64;
        for store in [&plain, &store] {
            plugin_api::memlayout::write_fact(
                store.md_root(),
                &plugin_api::memlayout::FactEntry {
                    workspace: ws,
                    key,
                    value,
                    tags: "t",
                    source: "explicit",
                    updated_at: at,
                    // 等价判据要比的是"召回出来的行逐字段相同"，分类这一维两边都给未分类
                    // ⇒ 它不参与这条判据（带 type 的往返在 `memlayout/tests/kind.rs`）。
                    ty: "",
                },
            )
            .unwrap();
        }
    }

    let a = shape(&plain.recall(Some("ws-session"), 10));
    let b = shape(&store.recall(Some("ws-session"), 10));
    assert_eq!(a, b, "接线前后召回逐字段相等（含排序）");
    assert_eq!(a.len(), 3, "阳性对照：三行都进比较，不是空对空");
    assert_eq!(
        rows(&log).len(),
        1,
        "且确实走了埋点（不是两边都没记所以相等）"
    );
}

#[test]
fn unwired_store_recalls_normally_and_builds_no_database() {
    // 未接线（单测现场、降级启动）= 召回照常，且**不许自己造出观测库**——
    // 造了就等于观测面在改写现场。
    let dir = temp_dir("unwired");
    let store = MemoryStore::new(&dir, test_resolver());
    store.write("", "a", "1", "", "explicit").unwrap();
    assert_eq!(store.recall(Some("ws-session"), 10).len(), 1);
    assert!(
        !dir.join("usage.sqlite3").exists(),
        "未接线不许建 usage.sqlite3"
    );
}

/// **影子坏了不再等于"源不可用"**（S1 刀②c-1 之后这一句才是这一族的正解）。
///
/// 原先这条叫 `unavailable_memory_store_does_not_write_a_row`，内容是"打不开库 ⇒ 一行
/// 都不记"（`n_candidates` 取不到，记 0 就是把"库不可用"冒充成"库是空的"）。读源换成
/// 真源之后，`memory.sqlite3` 坏了根本问不到召回什么事：真源读得动、读数完整 ⇒
/// **该记就记**。那条"宁缺毋伪"的纪律没有消失，它转到了 `partial` 那一维上，红证在
/// `memory/tests/md_read.rs` 的 `an_incomplete_scan_writes_no_inject_row`（文件级）与
/// `a_blocked_layer_also_suppresses_the_inject_row`（层级）。
///
/// 这条留在本簇的理由：它盯的是"**观测面不许因为一份没人读的副本坏掉而改写现场**"——
/// 写路径不能被影子拖倒，也不能因为影子失败就不记本可以记的账。
#[test]
fn a_broken_shadow_db_still_recalls_and_still_logs() {
    let dir = temp_dir("unavailable");
    std::fs::write(dir.join("memory.sqlite3"), b"not a database").unwrap();
    let log = RecallLog::at(dir.join("usage.sqlite3"));
    let store = MemoryStore::new(&dir, test_resolver()).with_recall_log(log.clone());
    store
        .write("", "pref.lang", "中文", "", "explicit")
        .expect("真源写得动：影子打不开不算失败");

    assert_eq!(store.recall(Some("ws-session"), 10).len(), 1);
    assert_eq!(
        log.stats().unwrap().injections,
        1,
        "读数完整就该记这一行（影子坏了不是不记的理由）"
    );
}

#[test]
fn schema_is_idempotent_across_reopen() {
    // 每次 open 都重跑 DDL（生产就是这样）：反复开库既不报错也不丢已有观测。
    // 若把 DDL 写成 DROP+CREATE，第二条断言就会红。
    let path = temp_dir("reopen").join("usage.sqlite3");
    let log = RecallLog::at(&path);
    log.note_inject(
        "s",
        "",
        1,
        1,
        &["a".to_string()],
        std::time::Duration::from_micros(7),
    )
    .unwrap();
    let log2 = RecallLog::at(&path);
    let log3 = RecallLog::at(&path);
    assert_eq!(rows(&log2).len(), 1, "第二次 open 重跑 DDL：旧观测还在");
    assert_eq!(rows(&log3).len(), 1, "第三次同上 = 幂等而非重建");
    assert_eq!(
        rows(&log)[0].mode,
        "inject",
        "阳性对照：写入档就是 'inject'，Q1b 只往 mode 加值不迁移表"
    );
}
