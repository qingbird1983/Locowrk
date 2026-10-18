//! 既有形状（v2.136 建簇以来累积，本批自 `memory/tests.rs` 整块搬入）。
//!
//! ⚠ S1 刀②c-1 对本簇做了一次**观测点订正**，不是放宽断言：`list` / `recall` 的读源
//! 已是真源 md，所以"验证库里落了几行""验证旧库迁移动作"这两类用例一律改吃
//! `export_rows()` 那条直读影子的通路——`memory_reconcile/tests.rs` 早就为同一件事改过
//! 一次，纪律写在 `md_read.rs` 头注（拿管理页当"库里有哪几行"的代理，读源一翻就失真）。
//! 断言的**语义**没动；动了语义的是那几条"测的就是本刀取消的那一档"的用例，逐条去向
//! 写在提交正文与 `specs/05`。
//!
//! 另外三条用例的造数据从"往库里 INSERT"改成走 `write()`：真源 = 文件之后，只有这条
//! 路造出来的东西才看得见（`a_row_in_the_db_without_a_file_is_not_a_fact` 钉的正是反面）。

use super::*;

/// 旧库（插件早期版本建的 4 列表）第一次被访问时必须整表重建到新列形状，并把两行
/// 迁进未归属键位、补上 `source`。**观测点 = `export_rows()`**：这一段验的是影子迁移，
/// 拿 `list()` 看会读到 md（那儿当然没有这些行）——那是观测点错位，不是迁移坏了。
/// 迁移本身挂在 `open_conn` 这条线上（两侧同源），所以这里一次访问就够触发。
#[test]
fn an_old_four_column_db_migrates_into_the_shadow() {
    let dir = temp_dir("old-db");
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

    let rows = store.export_rows().unwrap();
    assert_eq!(rows.len(), 2, "旧库两行迁入未归属键位");
    assert!(rows.iter().all(|r| r.workspace.is_empty()));
    assert!(
        rows.iter().all(|r| r.source == "explicit"),
        "旧行迁移补 explicit 来源"
    );
    let a = rows.iter().find(|r| r.key == "a").expect("a 那行还在");
    assert_eq!(a.value, "old");
    assert_eq!(a.tags, "t", "tags 列跟着迁过来");
    assert_eq!(a.updated_at, 100);
    let _ = std::fs::remove_dir_all(&dir);
}

/// 管理页四条动作（写 / 编辑 / 删除 / 清空）在**真源**上的往返。断言形状与搬来之时
/// 逐字相同，只有两处随语义订正：① 造数据走 `write()`（见本簇头注）；② 排序那一维
/// 不在这里钉——同一秒写两条时 `updated_at` 相同，新→旧的判据由 `md_read.rs`
/// 那条带时间差用例管（在这儿钉会得到一条"同秒按 key 升序"的假判据）。
#[test]
fn crud_roundtrip() {
    let dir = temp_dir("crud");
    let store = MemoryStore::new(&dir, test_resolver());
    store.write("", "a", "old", "t", "explicit").unwrap();
    store.write("", "b", "bee", "", "explicit").unwrap();

    assert_eq!(store.list("", 200).unwrap().len(), 2);
    assert!(store
        .list("", 200)
        .unwrap()
        .iter()
        .all(|r| r.workspace.is_empty()));

    // 经 update 语义没有"新增"，编辑不存在的键返回 false
    assert!(!store.update_value("", "ghost", "v1").unwrap());

    // 过滤：子串命中 value，通配符按字面量
    assert_eq!(store.list("bee", 200).unwrap().len(), 1);
    assert_eq!(store.list("%", 200).unwrap().len(), 0);

    // 编辑：只改 value + 时间，key/source 不动
    assert!(store.update_value("", "a", "new").unwrap());
    let rows = store.list("new", 200).unwrap();
    assert_eq!(rows.len(), 1, "编辑之后按新值命中：{rows:?}");
    assert_eq!(rows[0].value, "new");
    assert_eq!(rows[0].key, "a");
    assert!(rows[0].updated_at > 100);

    // 删除 + 清空（全按键位）
    assert!(store.delete("", "a").unwrap());
    assert!(!store.delete("", "a").unwrap(), "重复删除 = 不存在");
    assert_eq!(store.clear_all().unwrap(), 1, "清空报的是真源里剩的那几条");
    assert!(store.list("", 200).unwrap().is_empty());

    let _ = std::fs::remove_dir_all(&dir);
}

/// **库里有一行、真源里没这个文件 ⇒ 这条记忆不存在**（刀②c-1 的语义反转本体）。
///
/// 它是"存在性判据从 SQL 影响行数换成文件在不在"这句话的唯一机械证明：`update_value`
/// 与 `delete` 都必须在这一形状上返回 `false`（旧实现一个返回 true、一个返回 false，
/// 两者说的都是影子的话）。存量库里正有一批这个形状（2026-10-18 只读探针：库里 10 条
/// 全带 tags，md 侧一条 `tags:` 行都没有），决策人裁定不回填、由他清理 ⇒ 这条用例
/// 钉的是"清理之前它按真源算没有"，不是"回填之后"。
#[test]
fn a_row_in_the_db_without_a_file_is_not_a_fact() {
    let dir = temp_dir("shadow-only");
    let store = wired(&dir);
    store.write("", "shadow.row", "v", "", "explicit").unwrap();
    // 只删文件、不动影子 ⇒ 造出"库里有行、真源没有"的形状
    std::fs::remove_file(dir.join("memory").join("global").join("shadow.row.md")).unwrap();
    assert_eq!(
        store.export_rows().unwrap().len(),
        1,
        "前提：影子里那一行还在（我们只删了文件）"
    );

    assert!(
        store.list("", 200).unwrap().is_empty(),
        "列表不许从库里把它捞回来"
    );
    assert!(
        !store.update_value("", "shadow.row", "v2").unwrap(),
        "文件不在 = 没这条记忆，编辑不该把它当已存在"
    );
    assert!(!store.delete("", "shadow.row").unwrap(), "删除同理");
    assert_eq!(store.clear_all().unwrap(), 0, "清空的计数只认真源");
    let _ = std::fs::remove_dir_all(&dir);
}

/// **md 写失败 = 操作失败，且影子一行都不写**（刀②c-1 的顺序证据）。
///
/// 夹具 = 在 `memory/global` 这个位置放一个普通文件冒充目录 ⇒ 写侧
/// `create_dir_all` 必失败（两侧同形，不需要权限、不需要平台特判）。
/// 后半句"影子里没有那行"才是这条用例的承重部分：只断言 `write()` 返回 Err 的话，
/// "先写影子、再写 md、md 失败照样 Err"那种实现也能绿——而那种顺序里库里已经躺着
/// 一条用户以为没写进去的记忆，下次启动还会被对账写回文件。
#[test]
fn a_failed_md_write_is_an_error_and_writes_no_shadow() {
    let dir = temp_dir("md-write-fails");
    std::fs::create_dir_all(dir.join("memory")).unwrap();
    std::fs::write(dir.join("memory").join("global"), "not a directory").unwrap();
    let store = wired(&dir);

    let e = store
        .write("", "never.lands", "v", "", "explicit")
        .expect_err("真源写不进去就必须报错，不许 warn 之后当成功");
    assert!(e.contains("never.lands"), "报错得点名是哪条：{e}");
    assert!(
        store.export_rows().unwrap().is_empty(),
        "顺序判据：真源没落成 ⇒ 影子一行都不许落"
    );
    assert!(
        store.delete("", "never.lands").is_err(),
        "同一处层级故障在删除这一侧也得报错，不许读成「没这条记忆」然后谎称删除成功"
    );
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

/// 召回交付的是**行形状**（key / value / tags 三件都从 frontmatter 读回），整根扫不动
/// 时降级为空、不 panic（"注入不拦聊天"那条裁定：读不到最多是不注入，不许打断聊天）。
///
/// 造数据从"往库里 INSERT"改走 `write()`（本簇头注那条观测点订正）。坏根夹具 =
/// md 根的位置上坐着一个普通文件 ⇒ 刀②c-1 起这种层级故障会进 `unreadable` 账
/// （⇒ partial ⇒ 那一轮不写埋点），但**交付面仍是空而不是 Err**——两半各管一半。
#[test]
fn recall_projects_memory_entries_and_degrades_to_empty() {
    let dir = temp_dir("recall");
    let store = MemoryStore::new(&dir, test_resolver());
    store.write("", "k", "v", "tg", "explicit").unwrap();

    let got = store.recall(None, 5);
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].key, "k");
    assert_eq!(got[0].value, "v");
    assert_eq!(got[0].tags, "tg", "tags 从 frontmatter 那行读回");

    // 打不开的根（用一个文件当目录）→ 空表不 panic
    let bad = MemoryStore::new(&dir.join("file-not-dir"), test_resolver());
    std::fs::write(dir.join("file-not-dir"), "x").unwrap();
    assert!(bad.recall(None, 5).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

/// 列举条数钳制（下限 1、上限 `LIST_MAX`）。与 `md_read.rs` 那条同判据的两份夹具
/// 都留着：这里钉"0 收敛到 1"，那里钉"钳到上限但不许丢真货"。
#[test]
fn limit_is_clamped() {
    let dir = temp_dir("limit");
    let store = MemoryStore::new(&dir, test_resolver());
    store.write("", "a", "v", "", "explicit").unwrap();
    store.write("", "b", "v", "", "explicit").unwrap();
    assert_eq!(store.list("", 0).unwrap().len(), 1, "0 收敛到下限 1");
    assert_eq!(store.list("", 1).unwrap().len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- 真源落盘 + 影子跟随（《Memory File-ization Design v1.1》，刀②c-1 定形）----

/// 四条宿主写路径都**先落真源**，影子（sqlite）随后跟到同一个归一键位。
/// 传**原始串**（v2.186 起 `write()` 在入口归一键位），落点用 `keysid()` 取：
/// 目录名与 sqlite 的 workspace 列必须是同一个归一串——这正是那批接上的欠账
/// （Q2 留的「宿主 write 未归一 ⇒ 同项目双键位」）。
#[test]
fn host_writes_land_in_md_and_the_db_shadows_them() {
    let dir = temp_dir("md-sync");
    let store = wired(&dir);
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
    assert!(md.exists(), "宿主写路径落真源");
    assert!(
        store
            .export_rows()
            .unwrap()
            .iter()
            .any(|r| r.workspace == ws && r.key == "db.choice"),
        "影子跟着落到**同一个归一键位**（键位不同源 = 对账永远对不平）"
    );
    // 管理页编辑（管理页拿到的是 list 读回来的归一串 ⇒ 按归一键位寻址）
    store.update_value(&ws, "db.choice", "postgres").unwrap();
    assert!(std::fs::read_to_string(&md).unwrap().contains("postgres"));
    assert!(store
        .list("postgres", 200)
        .unwrap()
        .iter()
        .any(|r| r.value == "postgres"));
    // 管理页删除
    store.delete(&ws, "db.choice").unwrap();
    assert!(!md.exists());
    assert!(store.export_rows().unwrap().is_empty(), "删除也跟着影子");
    // 清空（全局层）
    store.write("", "pref.x", "y", "", "explicit").unwrap();
    let g = dir.join("memory").join("global").join("pref.x.md");
    assert!(g.exists());
    store.clear_all().unwrap();
    assert!(!g.exists(), "清空删真源");
    assert!(store.export_rows().unwrap().is_empty(), "清空也清影子");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn export_rows_covers_both_scopes() {
    let dir = temp_dir("md-export");
    let store = wired(&dir);
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
