//! L1 学习面两列（ML §3.2 · 后审计队列 Q5）：schema 迁移与"学习列不被写路径冲掉"。
//!
//! 本批**没有任何读取点**（拿 `hit_count` 做加权排序 = L2 / 队列 Q9），所以这些用例判的
//! 是两件将来会被依赖的性质：①存量库能自动补列且**不丢行**、②补列之后任何写路径都不许
//! 把学习量清零。
//!
//! ⚠ S1 刀②c-1 把取数通道从 `store.list()` 换成 `store.export_rows()`：这两件事说的都是
//! **库里那一行**，而 `list()` 的读源已是真源 md（拿它当"库里有哪几行"的代理在本仓红过
//! 两次，纪律见 `md_read.rs` 与本簇第 3 条用例里那句"分歧由刀②c-2 收"）。
//! 第 4 条（并发首访问）换通道还有一层必要理由：迁移挂在 `open_conn` 上，而 `list()`
//! 现在根本不开库 ⇒ 用它当触发器，测的就是"什么都没发生"。

use super::*;
use rusqlite::Connection;

/// 当前列名清单（`pragma_table_info` 的返回序 = 建表序，够用）。
fn cols(conn: &Connection) -> Vec<String> {
    let mut stmt = conn
        .prepare("SELECT name FROM pragma_table_info('memories')")
        .unwrap();
    stmt.query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
}

/// 读回学习量（本批只有测试在读，生产读取点属 Q9）。
fn learning(conn: &Connection, key: &str) -> (i64, Option<i64>) {
    conn.query_row(
        "SELECT hit_count, last_hit_at FROM memories WHERE key = ?1",
        [key],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .unwrap()
}

/// 手工造库：`extra` 是给旧版 6 列表追加的列定义（半迁夹具用）。
fn make_db(dir: &std::path::Path, rows: &str, extra: &str) {
    std::fs::create_dir_all(dir).unwrap();
    let conn = Connection::open(dir.join("memory.sqlite3")).unwrap();
    conn.execute_batch(&format!(
        "CREATE TABLE memories (
             workspace  TEXT NOT NULL DEFAULT '',
             key        TEXT NOT NULL,
             value      TEXT NOT NULL,
             tags       TEXT NOT NULL DEFAULT '',
             updated_at INTEGER NOT NULL,
             source     TEXT NOT NULL DEFAULT 'explicit',{extra}
             PRIMARY KEY (workspace, key)
         );
         {rows}"
    ))
    .unwrap();
}

const ONE_ROW: &str = "INSERT INTO memories (workspace, key, value, tags, updated_at, source) \
     VALUES ('', 'k.a', 'v-a', 't', 100, 'explicit');";

/// 旧库（本批之前的 6 列形状）首次访问补两列，**原行一个字节都不许丢**，且再访问一次
/// 不许有动静（幂等）。学习列缺省 = `0` / `NULL`（从未被召回过）。
#[test]
fn legacy_db_gains_learning_columns_without_losing_rows() {
    let dir = temp_dir("learn-migrate");
    make_db(&dir, ONE_ROW, "");
    let store = MemoryStore::new(&dir, test_resolver());
    let rows = store.export_rows().unwrap();
    assert_eq!(rows.len(), 1, "迁移不许丢行");
    assert_eq!(rows[0].value, "v-a", "原值不动");
    assert_eq!(rows[0].updated_at, 100, "原时间戳不动");
    let conn = Connection::open(dir.join("memory.sqlite3")).unwrap();
    let c = cols(&conn);
    assert!(
        c.iter().any(|x| x == "hit_count") && c.iter().any(|x| x == "last_hit_at"),
        "{c:?}"
    );
    assert_eq!(learning(&conn, "k.a"), (0, None), "新列缺省");
    // 二次访问：没有东西可迁，不许 Err、不许改字节
    assert_eq!(store.export_rows().unwrap().len(), 1);
    assert_eq!(learning(&conn, "k.a"), (0, None));
    let _ = std::fs::remove_dir_all(&dir);
}

/// 半迁状态（有 `hit_count`、无 `last_hit_at` = 两条 ALTER 中途被杀的形状）必须**自愈**。
/// ⚠ 这条是该设计的**独有点**：判据"两列齐就跳过、缺一就整批两条都跑"的实现会在第一条
/// 撞 `duplicate column name` ⇒ Err → ROLLBACK ⇒ 这个库永远打不开（比漏列更坏的缺陷形状）。
#[test]
fn half_migrated_library_self_heals_instead_of_locking() {
    let dir = temp_dir("learn-half");
    make_db(
        &dir,
        ONE_ROW,
        "\n         hit_count INTEGER NOT NULL DEFAULT 0,",
    );
    let conn = Connection::open(dir.join("memory.sqlite3")).unwrap();
    assert!(cols(&conn).iter().any(|x| x == "hit_count"));
    let store = MemoryStore::new(&dir, test_resolver());
    let rows = store.export_rows().unwrap();
    assert_eq!(rows.len(), 1, "半迁库照样能读");
    let c = cols(&conn);
    assert!(
        c.iter().any(|x| x == "last_hit_at"),
        "缺的那条要补上：{c:?}"
    );
    assert_eq!(learning(&conn, "k.a"), (0, None));
    let _ = std::fs::remove_dir_all(&dir);
}

/// 学习列不许被写路径冲掉（ML §6.4 的白名单口径）。
///
/// 三条路径里现在**只有 `reconcile_upsert` 还会写这张表**（S1 刀③b-2 摘掉四条
/// `shadow_*` ⇒ 常规 `write` 与管理页 `update_value` 一条 SQL 都不发）。那两段留在
/// 这里不是恒真的装饰：它们各自防的是"将来有人往真源写路径加回一条 upsert、顺手把
/// 学习列整行 REPLACE 掉"——那时 `learning()` 这两个断言立刻红。三条现在都靠
/// `ON CONFLICT DO UPDATE SET` 的**显式列**天然成立，本条钉的是将来有人把它改成
/// 整行 REPLACE / `INSERT OR REPLACE` 的那一刻。
#[test]
fn learning_columns_survive_every_write_path() {
    let dir = temp_dir("learn-survive");
    std::fs::create_dir_all(&dir).unwrap();
    let store = MemoryStore::new(&dir, test_resolver());
    store.write("", "k.a", "v1", "t", "explicit").unwrap();
    // 影子那一行经生产入口落（它同时把表建出来 —— 写路径不开库之后，
    // 没有这一步就没有 `memories` 这张表可查）
    store
        .reconcile_upsert("", "k.a", "v1", "t", "explicit", 1)
        .unwrap();
    let conn = Connection::open(dir.join("memory.sqlite3")).unwrap();
    conn.execute(
        "UPDATE memories SET hit_count = 7, last_hit_at = 999 WHERE key = 'k.a'",
        [],
    )
    .unwrap();

    store.write("", "k.a", "v2", "t", "explicit").unwrap();
    assert_eq!(
        learning(&conn, "k.a"),
        (7, Some(999)),
        "write 覆盖值不清学习量"
    );
    store.update_value("", "k.a", "v3").unwrap();
    assert_eq!(
        learning(&conn, "k.a"),
        (7, Some(999)),
        "管理页编辑不清学习量"
    );
    store
        .reconcile_upsert("", "k.a", "v4", "t", "organized", 1234)
        .unwrap();
    assert_eq!(
        learning(&conn, "k.a"),
        (7, Some(999)),
        "reconcile 直写不清学习量"
    );
    assert_eq!(
        store.export_rows().unwrap()[0].value,
        "v4",
        "影子跟着 reconcile 直写走到 v4"
    );
    // ⚠ 说破一处**已知的分歧**（不是本条的判据，也不许被读成"对齐了文件"）：
    // `reconcile_upsert` 是"索引向文件对齐"的专用直写面，它只动影子、**故意不反写 md**
    // （那条纪律原文在 `memory/reconcile.rs` 中间的 `----` 注释里）⇒ 这一跑之后真源停在
    // v3、库里是 v4。两边不再同值正是 `memory_reconcile` 整项要退役的理由（v1.1 §8 已判、
    // 刀②c-2 收口），本条只负责"学习量不被任何写路径冲掉"那一维。
    let _ = std::fs::remove_dir_all(&dir);
}

/// 并发首访问（4 线程 Barrier 对齐起跑）：S1 刀③b 之前这挡的是"宿主与插件两个进程族
/// 开同一个 `memory.sqlite3`，插件退场之后它挡的是**宿主自己**的多线（记忆命令线程 +
/// `memory_learn` 后台线程）。四方同时首次访问一个未迁的旧库，都必须成功、都不许丢行。
/// ⚠ 这条用例的**证据强度依赖并发压力**，实测两次都要如实记：
///   · 摘掉 `BEGIN IMMEDIATE`（不排队）⇒ 单独跑它也红（那次 `COMMIT;` 失配即 Err）= 稳定独有点；
///   · 摘掉等锁后的二次探针 ⇒ **整簇并发跑时它红**，但**只跑它这一条时不红**——外层探针
///     与 BEGIN 之间的窗口需要别的线程同时压着才撞得到。所以"M1 抓到了"这件事成立，
///     但它是**条件依赖**证据，不是稳定独有点（登记在 ROADMAP 本节边界②，不夸成闭环）。
#[test]
fn first_touches_race_on_one_library_all_succeed() {
    let dir = temp_dir("learn-race");
    make_db(&dir, ONE_ROW, "");
    let store = Arc::new(MemoryStore::new(&dir, test_resolver()));
    // 起点对齐：四个线程都在"库存在、列还没补"这一态上同时冲进首次访问，
    // 否则撞车窗口太窄（2 线程时对方可能已经迁完，第二个直接走外层早退 = 空过）。
    let start = Arc::new(std::sync::Barrier::new(4));
    let handles = (0..4)
        .map(|_| {
            let s = Arc::clone(&store);
            let b = Arc::clone(&start);
            std::thread::spawn(move || {
                b.wait();
                s.export_rows().map(|r| r.len())
            })
        })
        .collect::<Vec<_>>();
    for hd in handles {
        assert_eq!(hd.join().unwrap(), Ok(1), "并发首访问不许失败、也不许丢行");
    }
    let conn = Connection::open(dir.join("memory.sqlite3")).unwrap();
    let c = cols(&conn);
    assert!(
        c.iter().any(|x| x == "hit_count") && c.iter().any(|x| x == "last_hit_at"),
        "{c:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// 如实边界：删一行 = 连带删掉它的学习量（语义如此，不是漏）。刀③b-2 之后**删影子只有
/// 一条通路** = 对账（`reconcile_delete`：人删了文件 ⇒ 行跟着走）；管理页 `delete` 只删
/// 真源，库里那行要等下一轮对账（那一维由 `host_writes_land_in_md_and_never_touch_the_shadow`
/// 钉着）。本条把它写下来，免得后来人把"行没了"当成迁移缺陷。
#[test]
fn deleting_a_row_takes_its_learning_with_it() {
    let dir = temp_dir("learn-delete");
    std::fs::create_dir_all(&dir).unwrap();
    let store = MemoryStore::new(&dir, test_resolver());
    store.write("", "k.a", "v", "t", "explicit").unwrap();
    store
        .reconcile_upsert("", "k.a", "v", "t", "explicit", 1)
        .unwrap();
    let conn = Connection::open(dir.join("memory.sqlite3")).unwrap();
    conn.execute(
        "UPDATE memories SET hit_count = 3, last_hit_at = 77 WHERE key = 'k.a'",
        [],
    )
    .unwrap();
    assert!(store.reconcile_delete("", "k.a").unwrap());
    let n: i64 = conn
        .query_row("SELECT COUNT(*) FROM memories WHERE key = 'k.a'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(n, 0, "行与学习量一起走");
    let _ = std::fs::remove_dir_all(&dir);
}
