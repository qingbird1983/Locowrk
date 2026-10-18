//! 刀②b 的**读源**用例（管理页列举）：人类层可用时读文件，一条 SQL 都不发。
//!
//! 为什么单独一簇而不是往 `legacy.rs` 后面接三条：那个文件已经 350 行，再加就撞
//! 400 闸；更要紧的是它管的是"既有形状一字未改"，而这一簇管的是**语义正在反转**
//! 的那件事——两种证据混在一个文件里，以后谁也说不清哪条是等价值、哪条是新行为。

use super::*;

/// 键位只存在于库里、md 里没有 ⇒ 人类层开着时它**不许出现**在列表里。
///
/// 这条才是"读源真的换了"的正证。只测"md 里有的能读到"永远证不出这一点：
/// 双写时代两边各有一行，读谁都一样，测试全绿而实现还在读库。
#[test]
fn listing_reads_the_md_layer_not_the_db() {
    let dir = temp_dir("list-src");
    // 开关关着写一条：sqlite 落行、md 不落 —— 这就是存量库的真实形状。
    let off = wired(&dir, false);
    off.write("", "only.in.db", "x", "", "explicit").unwrap();
    let on = wired(&dir, true);
    let keys: Vec<String> = on
        .list("", 200)
        .unwrap()
        .into_iter()
        .map(|r| r.key)
        .collect();
    assert!(
        keys.is_empty(),
        "md 层没有这条 ⇒ 列表不许从库里把它捞回来：{keys:?}"
    );
    // 同一份磁盘数据、换个开关就看得见 ⇒ 两档的差别只可能来自读源，不来自夹具。
    let fallback: Vec<String> = off
        .list("", 200)
        .unwrap()
        .into_iter()
        .map(|r| r.key)
        .collect();
    assert_eq!(
        fallback,
        vec!["only.in.db".to_string()],
        "sqlite 兜底档仍在（那是刀②c 要取消的暂时状态，本条断言就是它的墓碑）"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// AC-S1-2 的原话证据：**把库文件改名**，管理页照常出数、字段一个不缺。
/// 实现里若还剩一处 `open_conn`，这条会以 `Err` 的形式红，而不是悄悄少几行。
#[test]
fn listing_survives_the_db_being_renamed_away() {
    let dir = temp_dir("list-nodb");
    let store = wired(&dir, true);
    let ws = keysid();
    store
        .write(&ws, "db.choice", "postgres", "db", "organized")
        .unwrap();
    store
        .write("", "pref.pnpm", "always pnpm", "偏好", "explicit")
        .unwrap();
    std::fs::rename(dir.join("memory.sqlite3"), dir.join("memory.sqlite3.gone")).unwrap();

    let rows = store.list("", 200).unwrap();
    assert_eq!(rows.len(), 2, "库不在场，两条照样出：{rows:?}");
    let g = rows
        .iter()
        .find(|r| r.key == "pref.pnpm")
        .expect("全局层那条");
    assert_eq!(g.value, "always pnpm");
    assert_eq!(
        g.tags, "偏好",
        "tags 从 frontmatter 那行读回（一等字段贯通到管理页）"
    );
    assert_eq!(g.source, "explicit");
    assert_eq!(g.workspace, "", "全局层键位 = ''");
    let p = rows
        .iter()
        .find(|r| r.key == "db.choice")
        .expect("项目层那条");
    assert_eq!(
        p.workspace, ws,
        "md 读回的 workspace 与 sqlite 键位同形（两侧都是归一串）"
    );
    assert_eq!(p.source, "organized", "source 是读回来的，不是默认值蒙的");
    assert!(p.updated_at > 0, "updated_at 从 metadata.updated 读回");
    let _ = std::fs::remove_dir_all(&dir);
}

/// 条数上限在**md 分支同样生效**。既有那条 `limit_is_clamped` 走的是未接线的 SQL
/// 通路，钳不住这条新支路 ⇒ `LIST_MAX` 的语义会在读源翻转时静默丢掉（表现为
/// 管理页一次拉出几千行）。
#[test]
fn listing_clamps_limit_on_the_md_branch() {
    let dir = temp_dir("list-limit");
    let store = wired(&dir, true);
    for key in ["k.a", "k.b", "k.c"] {
        store.write("", key, "v", "", "explicit").unwrap();
    }
    assert_eq!(store.list("", 0).unwrap().len(), 1, "0 收敛到下限 1");
    assert_eq!(
        store.list("", 2).unwrap().len(),
        2,
        "limit 在 md 支路照样钳"
    );
    assert_eq!(
        store.list("", 999).unwrap().len(),
        3,
        "钳到上限，但不许丢真货"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// 过滤语义必须与 SQL 通路逐字对齐：key / value / tags 任一子串命中，
/// **ASCII 大小写不敏感**（SQLite `LIKE` 的默认档），通配符按字面量。
/// 折叠大小写这半要是漂了，表现是"搜 pnpm 搜不到写着 PNPM 的那条"——没人会
/// 立刻报上来，所以它在测试里必须是断言而不是注释。
#[test]
fn listing_filter_matches_tags_and_folds_case_like_sql_like() {
    let dir = temp_dir("list-filter");
    let store = wired(&dir, true);
    store
        .write(
            "",
            "build.cmd",
            "Use PNPM, never npm",
            "构建, cli",
            "explicit",
        )
        .unwrap();
    let n = |q: &str| {
        store
            .list(q, 200)
            .unwrap()
            .into_iter()
            .filter(|r| r.key == "build.cmd")
            .count()
    };
    assert_eq!(n("构建"), 1, "tags 命中");
    assert_eq!(n("pnpm"), 1, "value 命中：needle 小写、原文大写 ⇒ 必须折叠");
    assert_eq!(n("BUILD"), 1, "key 命中：needle 大写、原文小写 ⇒ 必须折叠");
    assert_eq!(n("npm%"), 0, "通配符按字面量（SQL 侧 ESCAPE 同义）");
    assert_eq!(n("nope"), 0, "不命中就是零条，不许回落到全量");
    let _ = std::fs::remove_dir_all(&dir);
}
