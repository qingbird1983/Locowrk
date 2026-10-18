//! 真源读面用例（S1 刀②b 立簇）：管理页列举、全局层召回与项目索引**只读 md**，
//! 一条 SQL 都不发。刀②c-1 之后这里没有"另一档"可对照了——兜底档连同
//! `MemoryStore::index_rows` 一起拆掉，本簇的每条用例同时也是一条唯一通路。
//!
//! 为什么单独一簇而不是往 `legacy.rs` 后面接几条：那个文件已经 350 行，再加就撞
//! 400 闸；更要紧的是它管的是"既有形状一字未改"，而这一簇管的是**语义正在反转**
//! 的那件事——两种证据混在一个文件里，以后谁也说不清哪条是等价值、哪条是新行为。

use super::*;
use crate::memory::recall::{inject_loggable, RecallOutcome};
use crate::memory::MemoryRow;
use crate::recall_log::RecallLog;

/// 键位只存在于库里、md 里没有 ⇒ 它**不许出现**在列表里。
///
/// 这条才是"读源真的换了"的正证。只测"md 里有的能读到"永远证不出这一点：
/// 双写时代两边各有一行，读谁都一样，测试全绿而实现还在读库。
///
/// 造"库里有一行"的手法在刀②c-1 换了一次：原先是"把开关关掉再写"（那一档恰好只落库），
/// 现在开关管不到 store 了 ⇒ 改成**写完之后只删文件**，影子那行留着。新手法其实更接近
/// 存量真实形状（2026-10-18 探针：库里有 tags、文件里没有），而且不需要一个已经被取消的
/// 概念才能构造。`export_rows()` 那半句前提高证不能省——少了它，这个"空列表"可能只是
/// 因为整条通路压根没数据（空集里的真话）。
#[test]
fn listing_reads_the_md_layer_not_the_db() {
    let dir = temp_dir("list-src");
    let store = wired(&dir);
    store.write("", "only.in.db", "x", "", "explicit").unwrap();
    std::fs::remove_file(dir.join("memory").join("global").join("only.in.db.md")).unwrap();

    assert_eq!(
        store.export_rows().unwrap().len(),
        1,
        "前提：影子里那一行确实在（本例只删了真源文件）"
    );
    let keys: Vec<String> = store
        .list("", 200)
        .unwrap()
        .into_iter()
        .map(|r| r.key)
        .collect();
    assert!(
        keys.is_empty(),
        "md 层没有这条 ⇒ 列表不许从库里把它捞回来：{keys:?}"
    );
    // 反向对照：同一个 store、同一份磁盘，把文件写回去就看得见 ⇒ 上面那个"看不见"
    // 只可能来自读源，不来自夹具坏了。
    store.write("", "only.in.db", "x", "", "explicit").unwrap();
    assert_eq!(
        store.list("", 200).unwrap().len(),
        1,
        "对照：真源在场就读得到"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// AC-S1-2 的原话证据：**把库文件改名**，管理页照常出数、字段一个不缺。
/// 实现里若还剩一处 `open_conn`，这条会以 `Err` 的形式红，而不是悄悄少几行。
#[test]
fn listing_survives_the_db_being_renamed_away() {
    let dir = temp_dir("list-nodb");
    let store = wired(&dir);
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
    let store = wired(&dir);
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
    let store = wired(&dir);
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

/// **②b-2 的硬证据（AC-S1-2 那一半）**：把库改名之后，注入通路照常从 md 出全局层，
/// 而项目层那条依然不注入（v2.136 裁定的"项目侧全提醒制"不许因为换读源而走样）。
/// 实现里若还剩一处 `open_conn`，这条以 `Err` 红、不是少几行。
#[test]
fn recall_takes_the_global_layer_from_md_with_the_db_gone() {
    let dir = temp_dir("recall-nodb");
    let store = wired(&dir);
    let ws = keysid();
    store
        .write("", "pref.lang", "中文", "语言", "explicit")
        .unwrap();
    store
        .write("", "pref.style", "简短", "文风", "explicit")
        .unwrap();
    store
        .write(&ws, "proj.only", "不该自动注入", "", "explicit")
        .unwrap();
    std::fs::rename(dir.join("memory.sqlite3"), dir.join("memory.sqlite3.gone")).unwrap();

    let rows = store.recall_quiet(None, 10);
    let keys: Vec<&str> = rows.iter().map(|r| r.key.as_str()).collect();
    assert_eq!(
        keys,
        vec!["pref.lang", "pref.style"],
        "只取全局层、且取的是 md（同秒时按 key 升序 = SQL 那条 ORDER BY 同形）：{keys:?}"
    );
    assert_eq!(
        rows[0].tags, "语言",
        "tags 走的是同一份 From 装配（不是再抄一遍字段）"
    );
    // limit 在 md 支路同样生效——这条就是"`.take(limit)` 被丢掉"那种改动的红证
    assert_eq!(store.recall_quiet(None, 1).len(), 1, "召回条数照样钳 limit");
    let _ = std::fs::remove_dir_all(&dir);
}

/// 埋点判据的两档（纯函数，不需要接 `RecallLog` 就有可观测面）。
/// 这一档要是漂成"总是记"，`memory_inject_log` 的使用率会系统性偏高而**没有任何
/// 一条行为测试会红**——所以它必须在这儿钉住。
///
/// 原本是三档：完整 / partial / `Err`（源不可用）。刀②c-1 起 `RecallOutcome` 不再是
/// `Result`，`Err` 那半**没有对应物可测**了：文件制里"整个源不可用"表现为"一层都扫不动"，
/// 而层级故障现在也进 `unreadable` 账（`memlayout::layers` 的第二返回值）⇒ 它就是
/// `partial = true`。一档盖住两种故障，不是少测了一档。
#[test]
fn inject_loggable_requires_a_complete_reading() {
    let row = || MemoryRow {
        key: "k".into(),
        value: "v".into(),
        tags: String::new(),
        source: "explicit".into(),
        workspace: String::new(),
        updated_at: 1,
    };
    let outcome = |partial: bool| RecallOutcome {
        rows: vec![row()],
        n_candidates: 3,
        partial,
    };
    assert!(inject_loggable(&outcome(false)), "完整读数才记");
    assert!(
        !inject_loggable(&outcome(true)),
        "有扫不动的部分（条目或整层）⇒ 不记：候选数偏低就是指标造假"
    );
}

/// 有一条扫不动时**注入照常交付其余**（"注入不拦聊天"那条裁定优先于记账洁癖）。
/// 本条只管交付；"那一轮不写埋点"由下面那条接 `RecallLog` 的用乐视，判据本身
/// 再由 `inject_loggable_requires_a_complete_reading` 单独钉。
#[test]
fn recall_still_delivers_the_readable_ones_when_a_fact_is_unreadable() {
    let dir = temp_dir("recall-partial");
    let store = wired(&dir);
    let ws = keysid();
    store
        .write("", "pref.ok", "读得到", "", "explicit")
        .unwrap();
    // 一个"事实文件"其实是目录 ⇒ 文件级 read 失败 ⇒ 扫描带 partial
    std::fs::create_dir_all(
        dir.join("memory")
            .join("projects")
            .join(plugin_api::memlayout::project_dir_name(&ws))
            .join("facts")
            .join("broken.md"),
    )
    .unwrap();

    let rows = store.recall_quiet(None, 10);
    assert_eq!(rows.len(), 1, "扫不动一条不许连累其余：{rows:?}");
    assert_eq!(rows[0].key, "pref.ok");
    let _ = std::fs::remove_dir_all(&dir);
}

/// 有扫不动的条目时**不写埋点行**。观测量落成文件行数（进程内计数换个进程就没了），
/// 所以这里读的是 `memory_inject_log` 的真实计数。
///
/// 上一条纯函数证的是判据本身，这一条证的是**判据真的接到了召回通路上**：少了它，
/// `partial: scan.unreadable > 0` 漂成恒 false 也不会有任何测试报红（这条红过一次就是
/// 这么发现的——第一版两轮都用 `None` 会话跑，而 `ux_mil_inject` 的唯一键是
/// `(session_key, scope_id)`：第二行被 `INSERT OR IGNORE` 吃掉，于是"仍然只有一行"
/// 在两种实现下都成立 = 零证明力）。所以这里两轮各用一个真实会话 id，并配一组
/// **正对照**：同样的两轮、没有扫不动条目时必须记到第二行，否则上面那个"= 1"
/// 也可能只是因为埋点通路整条没接上。
#[test]
fn an_incomplete_scan_writes_no_inject_row() {
    let dir = temp_dir("recall-partial-log");
    let log = RecallLog::at(dir.join("usage.sqlite3"));
    let store = wired(&dir).with_recall_log(log.clone());
    store
        .write("", "pref.ok", "读得到", "", "explicit")
        .unwrap();

    store.recall_quiet(Some("ws-session"), 10);
    assert_eq!(
        log.stats().unwrap().injections,
        1,
        "前提一：完整读数确实记了一行"
    );

    // 全局层里塞一个"文件"其实是目录 ⇒ 扫描带 partial
    std::fs::create_dir_all(dir.join("memory").join("global").join("broken.md")).unwrap();
    let rows = store.recall_quiet(Some("ws-variant-session"), 10);
    assert_eq!(rows.len(), 1, "注入不拦聊天：照样交付读得到的那些");
    assert_eq!(
        log.stats().unwrap().injections,
        1,
        "partial 那一轮不许记第二行（宁缺毋伪接到通路上）"
    );

    // 正对照：同两轮、无扫不动条目 ⇒ 必须记两行
    let dir2 = temp_dir("recall-full-log");
    let log2 = RecallLog::at(dir2.join("usage.sqlite3"));
    let store2 = wired(&dir2).with_recall_log(log2.clone());
    store2
        .write("", "pref.ok", "读得到", "", "explicit")
        .unwrap();
    store2.recall_quiet(Some("ws-session"), 10);
    store2.recall_quiet(Some("ws-variant-session"), 10);
    assert_eq!(
        log2.stats().unwrap().injections,
        2,
        "正对照：完整两轮就该记两行（否则上面那个 1 是空集里的真话）"
    );
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&dir2);
}
/// **②b-3 的正证**：这个键位的 `MEMORY.md` 是空壳（只有标题）而库里还留着行 ⇒
/// 注入通路必须**如实缺席**。这一条测的就是"两档关系反过来"那个动作本身——
/// 上一版这里会拿到库里那一行，而这正是"库里那份已不再是真源却还在被读"的形状。
/// 对照同批给：把同一行写进 md ⇒ 立刻出块，否则那个 `None` 可能只是因为整条通路没接上。
#[test]
fn a_shell_index_file_means_the_section_is_absent_even_if_the_db_has_rows() {
    let dir = temp_dir("index-shell");
    let store = wired(&dir);
    let ws = keysid();
    {
        // 绕开双写直插一行：模拟"库里有货、人类层是空壳"的存量形状
        // （`temp_dir` 只算路径不建目录 ⇒ 开库前得先建，`legacy.rs` 同款）
        std::fs::create_dir_all(&dir).unwrap();
        let conn = rusqlite::Connection::open(dir.join("memory.sqlite3")).unwrap();
        conn.execute_batch(crate::memory::CREATE_TABLE).unwrap();
        conn.execute(
            "INSERT INTO memories (workspace, key, value, tags, updated_at, source) \
             VALUES (?1, 'k.from.db', 'v', '标签', 1700000000, 'explicit')",
            [ws.as_str()],
        )
        .unwrap();
    }
    let home = dir
        .join("memory")
        .join("projects")
        .join(plugin_api::memlayout::project_dir_name(&ws));
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(home.join("MEMORY.md"), "# 记忆索引\n\n").unwrap();

    assert_eq!(
        store.project_index(Some("ws-session")),
        None,
        "md 空壳 ⇒ 整节缺席，不许退到库里现合成"
    );
    std::fs::write(
        home.join("MEMORY.md"),
        "# 记忆索引\n\n- [k.from.db](k.from.db.md) — 标签\n",
    )
    .unwrap();
    let out = store
        .project_index(Some("ws-session"))
        .expect("md 里有指针行就该出块");
    assert!(out.contains("k.from.db"), "指针行原样进块：{out}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// 整层扫不动（`projects/` 那个位置被一个普通文件冒充）：全局层照常交付，而那一轮
/// **不写埋点行**。
///
/// 它与 `an_incomplete_scan_writes_no_inject_row` 分属**两档故障**：那一条由
/// `read_to_string` 失败进账（文件级），这一条由 `layers()` 的第二个返回值进账
/// （层级）。只留一条的话，撤掉任一档的计数都不会红——而层级那一档在真源 = 文件之后
/// 是最要命的（整层读不动时用户看到的是"我好像从没记过东西"）。
#[test]
fn a_blocked_layer_also_suppresses_the_inject_row() {
    let dir = temp_dir("recall-blocked-layer");
    let log = RecallLog::at(dir.join("usage.sqlite3"));
    let store = wired(&dir).with_recall_log(log.clone());
    store
        .write("", "pref.ok", "读得到", "", "explicit")
        .unwrap();

    store.recall_quiet(Some("ws-session"), 10);
    assert_eq!(
        log.stats().unwrap().injections,
        1,
        "前提：完整那一轮记了一行"
    );

    std::fs::write(dir.join("memory").join("projects"), "not a directory").unwrap();
    let rows = store.recall_quiet(Some("ws-variant-session"), 10);
    assert_eq!(rows.len(), 1, "注入不拦聊天：全局层那条照常交付：{rows:?}");
    assert_eq!(
        log.stats().unwrap().injections,
        1,
        "整层扫不动那一轮同样不许记第二行"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
