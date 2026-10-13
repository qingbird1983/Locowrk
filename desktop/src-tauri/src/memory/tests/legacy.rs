//! 既有形状（v2.136 建簇以来累积，本批自 `memory/tests.rs` 整块搬入，
//! 断言与顺序**一字未改**——它们的原样在场就是「加两列零行为变化」的证据）。

use super::*;

#[test]
fn crud_roundtrip() {
    let dir = temp_dir("crud");
    // 先手工建"旧版"4 列表（模拟老插件建的库），MemoryStore 首次访问走
    // 整表重建（workspace 键位 + source 列）——迁移路径本身被本用例覆盖
    std::fs::create_dir_all(&dir).unwrap();
    {
        let conn = rusqlite::Connection::open(dir.join("memory.sqlite3")).unwrap();
        conn.execute_batch(
            "CREATE TABLE memories (
                 key TEXT PRIMARY KEY, value TEXT NOT NULL,
                 tags TEXT NOT NULL DEFAULT '', updated_at INTEGER NOT NULL);
             INSERT INTO memories VALUES ('a', 'old', 't', 100);
             INSERT INTO memories VALUES ('b', 'bee', '', 200);",
        )
        .unwrap();
    }
    let store = MemoryStore::new(&dir, test_resolver());

    let rows = store.list("", 200).unwrap();
    assert_eq!(rows.len(), 2, "旧库两行迁入未归属键位");
    assert!(rows.iter().all(|r| r.workspace.is_empty()));
    assert_eq!(rows[0].key, "b", "新→旧排序");
    assert_eq!(rows[0].source, "explicit", "旧行迁移补 explicit 来源");
    assert_eq!(rows[1].updated_at, 100);

    // 经 update 语义没有"新增"，编辑不存在的键返回 false
    assert!(!store.update_value("", "ghost", "v1").unwrap());

    // 过滤：子串命中 value，通配符按字面量
    assert_eq!(store.list("bee", 200).unwrap().len(), 1);
    assert_eq!(store.list("%", 200).unwrap().len(), 0);

    // 编辑：只改 value + 时间，key/source 不动
    assert!(store.update_value("", "a", "new").unwrap());
    let rows = store.list("new", 200).unwrap();
    assert_eq!(rows[0].value, "new");
    assert_eq!(rows[0].key, "a");
    assert!(rows[0].updated_at > 100);

    // 删除 + 清空（全按键位）
    assert!(store.delete("", "a").unwrap());
    assert!(!store.delete("", "a").unwrap(), "重复删除 = 不存在");
    assert_eq!(store.clear_all().unwrap(), 1);
    assert!(store.list("", 200).unwrap().is_empty());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn update_validation_rejects_oversize() {
    let dir = temp_dir("validate");
    let store = MemoryStore::new(&dir, test_resolver());
    assert!(store.update_value("", "", "v").is_err());
    assert!(store.update_value("", &"k".repeat(513), "v").is_err());
    assert!(store
        .update_value("", "k", &"v".repeat(256 * 1024 + 1))
        .is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn recall_injects_global_layer_only() {
    let dir = temp_dir("recall-scope");
    let store = MemoryStore::new(&dir, test_resolver());
    // 项目键位与全局层各一条（同 key，(workspace,key) 主键允许并存）
    store
        .write("F:/ws/a", "k", "project-v", "", "explicit")
        .unwrap();
    store.write("", "k", "global-v", "", "explicit").unwrap();

    // v2.136 裁定「项目侧全提醒制」：注入只取全局层——项目层事实不自动
    // 注入（模型 memory_read / memory_search 按需查），与会话归属无关。
    for sid in [Some("ws-session"), Some("other"), None] {
        let got = store.recall(sid, 5);
        assert_eq!(got.len(), 1, "任何会话都只见全局层");
        assert_eq!(got[0].value, "global-v");
        assert_eq!(got[0].scope, "global");
    }

    // 管理面 list 看全量两行（跨键位）——项目层仍在库与 md 人类层
    assert_eq!(store.list("", 200).unwrap().len(), 2);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn scope_id_and_context_line_follow_attribution() {
    let dir = temp_dir("scope-id");
    let store = MemoryStore::new(&dir, test_resolver());
    // 归属会话：键位身份 = 项目根的**归一形**（Q2 / MR §6）；上下文行取同一个串
    // ——会话里不许同时出现两种项目写法。字面形不在此写死：大小写折叠是 Windows
    // 专属语义，钉字面串会得到一条 Linux 绿、Windows 红的假判据（串形归
    // `wskey/tests.rs` 各档管）。
    let key = plugin_api::wskey::normalize("F:/ws/a");
    assert_eq!(store.scope_id(Some("ws-session")), key);
    let line = store.context_line(Some("ws-session")).unwrap();
    assert!(line.contains(&key), "{line}");
    assert!(line.contains("this project"), "{line}");
    // v2.139 `<environment>`：日期/平台两行 + 按需查路径（开场先读退役）
    assert!(line.contains("Today: "), "{line}");
    assert!(line.contains("Platform: "), "{line}");
    assert!(!line.contains("list/read it first"), "开场先读钩子退役");
    // 未归属 / 无会话：空键位 + 无上下文行（不注入）
    assert_eq!(store.scope_id(Some("other")), "");
    assert_eq!(store.scope_id(None), "");
    assert!(store.context_line(Some("other")).is_none());
    assert!(store.context_line(None).is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn two_spellings_of_one_project_share_one_scope_id() {
    // **本刀在宿主这一侧的承重判据**：预取缓存键 = `(session_id, scope_id)`，
    // 注入索引与观测表也按 scope_id 取键位——同一个项目若因写法不同给出两个
    // scope_id，注入面就会把它的记忆读成"另一份"（MR §0 的断点之一）。
    let dir = temp_dir("scope-collapse");
    let store = MemoryStore::new(&dir, test_resolver());
    #[cfg(windows)]
    assert_eq!(
        store.scope_id(Some("ws-session")),
        store.scope_id(Some("ws-variant-session")),
        "斜杠方向/尾分隔符漂移在 Windows 上必须同键位"
    );
    #[cfg(not(windows))]
    assert!(
        !store.scope_id(Some("ws-variant-session")).is_empty(),
        "POSIX 上该串是另一个键位，但不许被归成未归属"
    );
    // 两种写法都**不是**未归属（归一不许把项目串吞成 ""，那会污染全局偏好层）
    assert!(!store.scope_id(Some("ws-session")).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn recall_projects_memory_entries_and_degrades_to_empty() {
    let dir = temp_dir("recall");
    let store = MemoryStore::new(&dir, test_resolver());
    std::fs::create_dir_all(&dir).unwrap();
    // 打不开的目录（用文件路径当目录制造失败）→ 空表不 panic
    let bad = MemoryStore::new(&dir.join("file-not-dir"), test_resolver());
    std::fs::write(dir.join("file-not-dir"), "x").unwrap();
    assert!(bad.recall(None, 5).is_empty());
    {
        let conn = rusqlite::Connection::open(dir.join("memory.sqlite3")).unwrap();
        conn.execute_batch(
            "CREATE TABLE memories (
                 key TEXT PRIMARY KEY, value TEXT NOT NULL,
                 tags TEXT NOT NULL DEFAULT '', updated_at INTEGER NOT NULL,
                 source TEXT NOT NULL DEFAULT 'explicit');
             INSERT INTO memories VALUES ('k', 'v', 'tg', 1, 'explicit');",
        )
        .unwrap();
    }
    let got = store.recall(None, 5);
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].key, "k");
    assert_eq!(got[0].value, "v");
    assert_eq!(got[0].tags, "tg");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn limit_is_clamped() {
    let dir = temp_dir("limit");
    let store = MemoryStore::new(&dir, test_resolver());
    {
        std::fs::create_dir_all(&dir).unwrap();
        let conn = rusqlite::Connection::open(dir.join("memory.sqlite3")).unwrap();
        conn.execute_batch(
            "CREATE TABLE memories (
                 key TEXT PRIMARY KEY, value TEXT NOT NULL,
                 tags TEXT NOT NULL DEFAULT '', updated_at INTEGER NOT NULL,
                 source TEXT NOT NULL DEFAULT 'explicit');
             INSERT INTO memories VALUES ('a', 'v', '', 1, 'explicit'),
                                          ('b', 'v', '', 2, 'explicit');",
        )
        .unwrap();
    }
    assert_eq!(store.list("", 0).unwrap().len(), 1, "0 收敛到下限 1");
    assert_eq!(store.list("", 1).unwrap().len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- F1 人类层双写（《Memory File-ization Design》§3）----

fn wired(dir: &std::path::Path, on: bool) -> MemoryStore {
    let files = Arc::new(crate::memory_files::MemoryFiles::new(dir));
    let flag: Arc<dyn Fn() -> bool + Send + Sync> = Arc::new(move || on);
    MemoryStore::new(dir, test_resolver()).with_files(files, flag)
}

#[test]
fn host_writes_sync_md_human_layer() {
    let dir = temp_dir("md-sync");
    let store = wired(&dir, true);
    // 传**原始串**（v2.186 起 `write()` 在入口归一键位），落点用 `keysid()` 取：
    // sqlite 的 workspace 列与 md 目录名必须是同一个归一串——这正是本批接上的
    // 那条欠账（Q2 留的「宿主 write 未归一 ⇒ 同项目双键位」）。
    let ws = keysid();
    store
        .write("F:/ws/a", "db.choice", "sqlite", "", "explicit")
        .unwrap();
    let md = dir
        .join("memory")
        .join("projects")
        .join(plugin_api::memlayout::project_dir_name(&ws))
        .join("facts")
        .join("db.choice.md");
    assert!(md.exists(), "提取/宿主写路径同步 md");
    // 管理页编辑同步（管理页拿到的是 list 出来的归一串 ⇒ 按归一键位寻址）
    store.update_value(&ws, "db.choice", "postgres").unwrap();
    assert!(std::fs::read_to_string(&md).unwrap().contains("postgres"));
    // 管理页删除同步
    store.delete(&ws, "db.choice").unwrap();
    assert!(!md.exists());
    // 清空同步（全局层）
    store.write("", "pref.x", "y", "", "explicit").unwrap();
    let g = dir.join("memory").join("global").join("pref.x.md");
    assert!(g.exists());
    store.clear_all().unwrap();
    assert!(!g.exists(), "清空逐条删 md");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn files_switch_off_keeps_pure_sqlite() {
    let dir = temp_dir("md-off");
    let store = wired(&dir, false);
    store.write("F:/ws/b", "k", "v", "", "explicit").unwrap();
    assert!(!dir.join("memory").exists(), "总开关关 = 不写 md");
    let rows = store.export_rows().unwrap();
    assert_eq!(rows.len(), 1, "sqlite 照常");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn export_rows_covers_both_scopes() {
    let dir = temp_dir("md-export");
    let store = wired(&dir, true);
    store.write("", "g1", "v", "", "explicit").unwrap();
    store.write(r"F:\ws\a", "p1", "v", "", "extracted").unwrap(); // **反斜杠原始串**：归一必须由 write() 自己做
    let rows = store.export_rows().unwrap();
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().any(|r| r.workspace.is_empty() && r.key == "g1"));
    // 钉的是「库里落的键位 = `scope_key` 的归一形」，不钉具体串形（Windows 上
    // 它是 `f:\ws\a`、POSIX 上仍是原串——写死字面就得到一条跨平台假判据）
    assert!(rows
        .iter()
        .any(|r| r.workspace == keysid() && r.source == "extracted"));
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- Q2（MR §4.1）项目记忆索引注入：两档数据源的优先关系与缺席条件 ----

/// 归一键位（Q2 起 `scope_of_session` 出来的串才是查行的键位）。测试用同一个
/// helper 取键位，跨平台两侧一致（Windows 上它是 `f:\ws\a`，POSIX 上仍是原串）。
fn keysid() -> String {
    crate::workspace::scope_key(Some("F:/ws/a"))
}

#[test]
fn project_index_uses_the_md_projection_when_it_has_pointer_lines() {
    let dir = temp_dir("idx-md-first");
    let store = wired(&dir, true);
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

#[test]
fn project_index_falls_back_to_sqlite_when_the_md_is_an_empty_shell() {
    // **本机实测的那一档**：`MEMORY.md` 在场但只有 `# 记忆索引` 标题（或整个文件
    // 不在）。这一档必须落 sqlite 现合成，而不是"注入一个空节"或"报没有货"。
    let dir = temp_dir("idx-fallback");
    let store = wired(&dir, false); // 文件化关 = 一份 md 都不写，正合现场形态
    let ws = keysid();
    store
        .write(&ws, "user.stack", "Rust", "偏好", "explicit")
        .unwrap();
    let out = store.project_index(Some("ws-session")).expect("兜底档有货");
    assert!(out.contains("- user.stack — 偏好"), "sqlite 现合成: {out}");
    assert!(!out.contains("Rust"), "兜底档也只给指针，正文不进: {out}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn project_index_is_absent_for_unattributed_and_empty_projects() {
    let dir = temp_dir("idx-absent");
    let store = wired(&dir, true);
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
    let store = wired(&dir, false);
    let ws = keysid();
    store.write(&ws, "k.one", "v", "t", "explicit").unwrap();
    assert_eq!(
        store.scope_id(Some("ws-session")),
        ws,
        "scope_id 与查行键位必须同串"
    );
    let out = store.project_index(Some("ws-session")).unwrap();
    assert!(out.contains("- k.one — t"), "{out}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_broken_database_degrades_to_no_section_without_touching_recall() {
    // 「库不可用」不许冒充「项目层空」在**索引这一侧**的形态：整节缺席 + 一次告警，
    // 而全局层召回（同库）的降级路径不受影响（`recall_quiet` 已有自己的告警位）。
    let dir = temp_dir("idx-unavailable");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("memory.sqlite3"), b"not a database").unwrap();
    let store = MemoryStore::new(&dir, test_resolver());
    assert!(store.project_index(Some("ws-session")).is_none());
    assert!(
        store.recall(Some("ws-session"), 5).is_empty(),
        "recall 照常降级为空"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
