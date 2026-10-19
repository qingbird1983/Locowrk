//! 项目记忆**索引注入**（Q2 / MR §4.1）的宿主侧判据：整节缺席的条件、指针行原样透传、
//! 陈旧标注现算、以及"影子坏了读面照常"。自 `legacy.rs` 按面拆出（S1 刀②c-1，
//! 用例本体逐字未改）。
//!
//! 为什么单独一簇：`legacy.rs` 加完刀②c-1 那三条新用例涨到 481 行越过 400 纪律，
//! 而三问自检第①问答"否"——它同时管着 CRUD 往返、键位归一、召回降级**与索引注入**
//! 四件事，索引注入这条线以后还会随 S3（注入面换形状）单独长。夹具（`wired` /
//! `keysid` / `test_resolver`）留在本目录 `mod.rs` 一份，两簇共用。
//!
//! ⚠ 簇标题原先写的是"两档数据源的优先关系"——那两档（md 优先、库里现合成兜底）
//! 在刀②b-3 反了方向、在刀②c-1 只剩一档。这里的判据全部指向**真源 md**：
//! 库里那份是影子，它坏没坏都不改变注入面看到的东西（最后那条用例钉的就是这一句）。

use super::*;

#[test]
fn project_index_uses_the_md_projection_when_it_has_pointer_lines() {
    let dir = temp_dir("idx-md-first");
    let store = wired(&dir);
    let ws = keysid();
    // 照生产路径造 md：宿主 write 会双写 facts/ 并刷新该键位的 MEMORY.md
    store
        .write(&ws, "build.cmd", "pnpm tauri dev", "构建", "explicit")
        .unwrap();
    let out = store.project_index(Some("ws-session")).expect("项目层有货");
    assert!(
        out.contains("- [build.cmd](build.cmd.md)"),
        "md 投影档（指针行原样搬）: {out}"
    );
    assert!(
        out.contains("构建"),
        "hook 来自 frontmatter description: {out}"
    );
    assert!(!out.contains("pnpm tauri dev"), "正文一个字都不进: {out}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// 把 `g{n}p{n}r{n}` 拆回三档。用例要分别断言"**只有该动的那一档**动了"，
/// 只比整串不同的话，"改全局顺带把项目档也刷了"这种过度失效看不出来——
/// 而那正是把 mtime 接进缓存键最可能砸前缀的方式。
fn split_stamp(s: &str) -> (i64, i64, i64) {
    let (g, tail) = s
        .trim_start_matches('g')
        .split_once('p')
        .expect("指纹形状 g/p/r");
    let (p, r) = tail.split_once('r').expect("指纹形状 p/r");
    (
        g.parse().expect("g 档是整数"),
        p.parse().expect("p 档是整数"),
        r.parse().expect("r 档是整数"),
    )
}

/// S3 刀②：预取指纹 = 注入面三份输入的 mtime（全局索引 / 项目索引 / 交接正文）。
/// 四条断语各挡一种假绿：
/// ① 什么都没动 ⇒ 逐字节相同（没有这一条，"改了会生效"那条用例可以靠"每轮都重算"绿）；
/// ② 写一条全局事实 ⇒ 只有 `g` 档变；
/// ③ 写一条项目事实 ⇒ 只有 `p` 档变；
/// ④ 改交接正文 ⇒ 只有 `r` 档变（这一档**不在数据根下**，走的是 `repo_root`，
///    前三档都对时它最容易漂成"恒 0"——所以必须给一个真实存在的仓库目录）。
#[test]
fn the_index_stamp_follows_each_injected_input_and_only_that_one() {
    let dir = temp_dir("stamp-inputs");
    let repo = dir.join("repo");
    std::fs::create_dir_all(repo.join(crate::repo_context::DIR)).unwrap();
    let repo_for_resolver = repo.clone();
    let resolver: crate::workspace::WorkspaceResolver =
        Arc::new(move |_| Some(repo_for_resolver.display().to_string()));
    let store = MemoryStore::new(&dir, resolver);
    let ws = store.scope_of_session(Some("any"));

    let base = split_stamp(&store.index_stamp(Some("any")));
    assert_eq!(
        base,
        split_stamp(&store.index_stamp(Some("any"))),
        "① 没动就逐字节相同"
    );

    store.write("", "g1", "全局一条", "", "explicit").unwrap();
    let after_global = split_stamp(&store.index_stamp(Some("any")));
    assert_ne!(after_global.0, base.0, "② 全局索引被刷新 ⇒ g 档要变");
    assert_eq!(after_global.1, base.1, "改全局不许惊动项目档");
    assert_eq!(after_global.2, base.2, "改全局不许惊动交接档");

    store.write(&ws, "p1", "项目一条", "", "explicit").unwrap();
    let after_project = split_stamp(&store.index_stamp(Some("any")));
    assert_ne!(
        after_project.1, after_global.1,
        "③ 项目索引被刷新 ⇒ p 档要变"
    );
    assert_eq!(after_project.0, after_global.0, "改项目不许惊动全局档");

    let handoff = repo.join(crate::repo_context::DIR).join("MEMORY.md");
    std::fs::write(&handoff, "# 现状\n\n- 改过一次\n").unwrap();
    // 显式拨一个远未来的 mtime：同一毫秒内连写两次的话，`set_modified` 之前那一版
    // 会让这一档"该变而没变"，用例就红得让人以为是实现错了。
    let f = std::fs::File::options().write(true).open(&handoff).unwrap();
    f.set_modified(
        std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_900_000_000),
    )
    .unwrap();
    let after_handoff = split_stamp(&store.index_stamp(Some("any")));
    assert_ne!(after_handoff.2, after_global.2, "④ 交接正文被改 ⇒ r 档要变");
    assert_eq!(after_handoff.0, after_global.0, "改交接不许惊动全局档");
    assert_eq!(after_handoff.1, after_project.1, "改交接不许惊动项目档");
    let _ = std::fs::remove_dir_all(&dir);
}

/// 未归属会话：`p` / `r` 两档恒 0（没有"本项目"可言），但 `g` 档照旧参与——
/// 这一条挡住"顺手把整个指纹在开头早退"那种写法（那样全局索引改了也不会重算）。
#[test]
fn an_unattributed_session_still_carries_the_global_stamp() {
    let dir = temp_dir("stamp-unattributed");
    let store = wired(&dir);
    let before = split_stamp(&store.index_stamp(None));
    assert_eq!(before.1, 0, "无会话身份没有项目档");
    assert_eq!(before.2, 0, "无会话身份没有交接档");
    store.write("", "g1", "v", "", "explicit").unwrap();
    assert_ne!(
        split_stamp(&store.index_stamp(None)).0,
        before.0,
        "全局档必须仍然参与指纹: {:?}",
        store.index_stamp(None)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn project_index_is_absent_for_unattributed_and_empty_projects() {
    let dir = temp_dir("idx-absent");
    let store = wired(&dir);
    // 未归属会话：「本项目」不成立 ⇒ 整节缺席（不是空节）
    store.write("", "g1", "v", "", "explicit").unwrap();
    assert!(store.project_index(Some("other")).is_none(), "未归属不注入");
    assert!(store.project_index(None).is_none(), "无会话身份不注入");
    // 归属会话但项目层空 ⇒ 同样缺席，零开销
    let empty = MemoryStore::new(&dir.join("nothing"), test_resolver());
    assert!(empty.project_index(Some("ws-session")).is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn project_index_is_the_same_keyside_the_injection_cache_uses() {
    // 接线判据：索引取行的键位必须与预取缓存键里那半 (`scope_id`) **同源同串**，
    // 否则会话改归属后缓存键变了而取行仍用旧串 = 注入到过期项目的索引。
    let dir = temp_dir("idx-keyside");
    let store = wired(&dir);
    let ws = keysid();
    store.write(&ws, "k.one", "v", "t", "explicit").unwrap();
    assert_eq!(
        store.scope_id(Some("ws-session")),
        ws,
        "scope_id 与查行键位必须同串"
    );
    let out = store.project_index(Some("ws-session")).unwrap();
    // 指针行的形状由写侧决定（`refresh_index` 写的是 markdown 链接），这里跟着读侧
    // 迁到 md 档订正字面形——断言仍然要求"那一行原样在场"，语义没放宽。
    assert!(out.contains("- [k.one](k.one.md) — t"), "{out}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// 陈旧标注（稿 §4.1 那句 `（N days old）`）必须由**注入侧现算**并贴到指针行尾。
/// 这一维在 md 档很容易丢：文件里那行只有 `- [stem](stem.md) — hook`，`updated_at`
/// 不在文本里；刀②b-3 换读源时就静默丢过一次（当时只有已退役的 `from_rows` 在算它），
/// 所以这条用例是补回来的红证。它必须同时配一条**负对照**（新写的那条不许带标注），
/// 否则实现漂成"每行都硬贴一句"也不会红。
#[test]
fn an_aged_index_line_carries_its_staleness_to_the_model() {
    let dir = temp_dir("idx-stale");
    let store = wired(&dir);
    let ws = keysid();
    store
        .write(&ws, "fresh.one", "刚写的", "新", "explicit")
        .unwrap();
    // 一条"很久以前"的事实：直接改文件的 metadata.updated（生产里由 render 落这个字段）
    store
        .write(&ws, "old.one", "很久以前", "旧", "explicit")
        .unwrap();
    let path = dir
        .join("memory")
        .join("projects")
        .join(plugin_api::memlayout::project_dir_name(&ws))
        .join("facts")
        .join("old.one.md");
    let text = std::fs::read_to_string(&path).unwrap();
    let long_ago = crate::timeutil::now_secs() - 30 * 86_400;
    let patched: String = text
        .lines()
        .map(|l| {
            if l.starts_with("  updated: ") {
                format!("  updated: {long_ago}")
            } else {
                l.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(&path, patched).unwrap();

    let out = store.project_index(Some("ws-session")).expect("项目层有货");
    assert!(
        out.contains("[old.one](old.one.md)") && out.contains("days old"),
        "陈旧那条必须带上新鲜度：{out}"
    );
    let line = out
        .lines()
        .find(|l| l.contains("fresh.one"))
        .expect("新那条也在索引里");
    assert!(
        !line.contains("days old"),
        "负对照：刚写的不许标陈旧（漂成'每行硬贴'那种实现靠这半条抓）：{line}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// **影子坏了 = 读面一个字节都不变**（刀②c-1 之后这一句才是这一族的正解）。
///
/// 它取代了原来那条"库不可用 ⇒ 索引整节缺席 + 一次告警"：那一档的因果链是
/// "索引的兜底数据源是库"，而兜底档已经连 `index_rows` 一起删了。现在把
/// `memory.sqlite3` 写成非数据库字节，四条读面照常出数，写路径也照常成功——
/// 刀③b-2 之后这半档的成因从"影子失败只 warn"变成**写路径根本不开这张表**
/// （`crud.rs` 一条 SQL 都不发），断言一字未改仍然绿，这就是它的新读法。
#[test]
fn a_broken_shadow_db_changes_nothing_on_the_read_or_write_path() {
    let dir = temp_dir("idx-unavailable");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("memory.sqlite3"), b"not a database").unwrap();
    let store = MemoryStore::new(&dir, test_resolver());
    let ws = keysid();
    store
        .write(&ws, "k.one", "v", "t", "explicit")
        .expect("真源写得动，影子坏了不算失败");
    store.write("", "g.one", "gg", "", "explicit").unwrap();

    assert_eq!(store.list("", 200).unwrap().len(), 2, "列举照常");
    assert_eq!(store.recall(None, 5).len(), 1, "全局层召回照常");
    let out = store.project_index(Some("ws-session")).expect("索引照常");
    assert!(out.contains("k.one"), "{out}");
    let _ = std::fs::remove_dir_all(&dir);
}
