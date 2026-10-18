//! 真源读面用例（S1 刀②b 立簇）：管理页列举、全局层召回与项目索引**只读 md**，
//! 一条 SQL 都不发。刀②c-1 之后这里没有"另一档"可对照了——兜底档连同
//! `MemoryStore::index_rows` 一起拆掉，本簇的每条用例同时也是一条唯一通路。
//!
//! 为什么单独一簇而不是往 `legacy.rs` 后面接几条：那个文件已经 350 行，再加就撞
//! 400 闸；更要紧的是它管的是"既有形状一字未改"，而这一簇管的是**语义正在反转**
//! 的那件事——两种证据混在一个文件里，以后谁也说不清哪条是等价值、哪条是新行为。
//!
//! 刀③b-2 按同一把尺子又拆了一次：本簇从此**只管读源**（列举 / 召回 / 项目索引只读 md），
//! "降级时照常交付 + 埋点宁缺毋伪"那一族搬去 `inject_row.rs`——用例本体逐字搬、断言一字
//! 未改。会撞到 400 行闸正是因为这两件事各自都还在动，抬基线与压注释都算作弊。

use super::*;

/// 键位只存在于库里、md 里没有 ⇒ 它**不许出现**在列表里。
///
/// 这条才是"读源真的换了"的正证。只测"md 里有的能读到"永远证不出这一点：
/// 双写时代两边各有一行，读谁都一样，测试全绿而实现还在读库。
///
/// 造"库里有一行"的手法换过两次：刀②c-1 之前是"把开关关掉再写"（那一档恰好只落库），
/// 开关管不到 store 之后改成**写完之后只删文件**（影子那行留着）；刀③b-2 摘掉四条
/// `shadow_*` 之后连这一招也不成立了（`write` 不再碰库），现在用生产里唯一还写内容列的
/// 入口 `reconcile_upsert` 造那一行——手法跟着**真源形状**走，不跟着方便走。新手法其实
/// 更接近存量真实形状（2026-10-18 探针：库里有 tags、文件里没有）。
/// `export_rows()` 那半句前提高证不能省——少了它，这个"空列表"可能只是因为整条通路压根
/// 没数据（空集里的真话）。
#[test]
fn listing_reads_the_md_layer_not_the_db() {
    let dir = temp_dir("list-src");
    let store = wired(&dir);
    store.write("", "only.in.db", "x", "", "explicit").unwrap();
    store
        .reconcile_upsert("", "only.in.db", "x", "", "explicit", 1)
        .unwrap();
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
    // 库要先**真的存在一次**才谈得上"改名之后不在场"：刀③b-2 之后写路径不开库，
    // 不补这一步的 rename 会直接 NotFound  panic（那是夹具坏了，不是判据）。
    // 用 `reconcile_upsert` 建这层，因为它就是生产里建库+写行的那个入口。
    store
        .reconcile_upsert("", "pref.pnpm", "always pnpm", "偏好", "explicit", 2)
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

/// **②b-2 的硬证据（AC-S1-2 那一半）**：影子库是坏字节时，注入通路照常从 md 出全局层，
/// 而项目层那条依然不注入（v2.136 裁定的"项目侧全提醒制"不许因为换读源而走样）。
/// 实现里若还剩一处开库读它，这条以 `Err` 红、不是少几行。
#[test]
fn recall_takes_the_global_layer_from_md_with_the_db_gone() {
    let dir = temp_dir("recall-nodb");
    let store = wired(&dir);
    let ws = keysid();
    // 三条的 `updated_at` **钉死成同一个值**：这条断言判的是"同秒时按 key 升序"，
    // 而 `store.write` 取的是墙钟 ⇒ 两次写入跨过秒边界时"同秒"这个前提就没了，
    // 一条讲排序口径的用例会每隔若干次全量跑假红一次（实测抓到一回，报的是
    // `[pref.style, pref.lang]`）。改走 `memlayout::write_fact` 既能钉住这一维，
    // 落的又仍然是生产渲染与生产寻址——不是把断言放宽。
    for (workspace, key, value, tags) in [
        ("", "pref.lang", "中文", "语言"),
        ("", "pref.style", "简短", "文风"),
        (ws.as_str(), "proj.only", "不该自动注入", ""),
    ] {
        plugin_api::memlayout::write_fact(
            store.md_root(),
            &plugin_api::memlayout::FactEntry {
                workspace,
                key,
                value,
                tags,
                source: "explicit",
                updated_at: 1_700_000_000,
                ty: "",
            },
        )
        .unwrap();
    }
    // 库这一侧现在**根本没被写过**（上面三条走的是真源通路，不碰 sqlite），所以夹具
    // 直接在那条路径上放一份"不是数据库"的字节：实现里若还剩一处开库读它，拿到的会是
    // `Err` 而不是"少几行"。这比原先那步 rename 更硬——rename 之后 open 会静悄悄
    // 新建一个空库，那条用例的红色就只剩"少行"一种形态了。
    std::fs::write(dir.join("memory.sqlite3"), b"not a database at all").unwrap();

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
