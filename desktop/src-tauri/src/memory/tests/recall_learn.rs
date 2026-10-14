//! 队列 Q9 宿主端到端：`MemorySource::recall_relevant` 的**生产实现**——
//! 真库、真 SQL、真写回。与 `memory_learn/tests/`（纯函数面）分工：
//! 那里判"四条式子对不对"，这里判"式子接没接到数据库上、写回有没有只碰该碰的列"。

use super::*;
use kernel_agent::{LearnWeights, RecallQuery};
use rusqlite::Connection;

/// 固定时间轴（不取系统时钟）：三档年龄差要用整数天算，现取时间会让断言随时钟漂。
/// ⚠ 时间基准必须**取生产的那个钟**（`now_secs()`），不能写死一个漂亮常量：
/// 本批第一次就跑出 `["mid-hot","new-noise"]` —— 夹具用 2023 年的固定秒数、`rank`
/// 用真实当前时间，于是两个时间信号在测试里一起塌到接近 0，`strength` 反而赢了，
/// 断言判的已经不是"相关性 vs 新鲜度"而是"夹具与生产不同源"。写死常量在这里看着
/// 更可控，实际是把被测关系测没了（本批判据自纠的一条，注释留在这里当墓碑）。
const DAY: i64 = 86_400;

/// 手工建 8 列表（含 Q5 的学习两列）并塞全局层行，返回夹具的时间基准。
///
/// ⚠ 为什么不走 `store.write`：那颗函数把 `updated_at` 恒取 `now_secs()`，
/// 三行会全部同刻 ⇒ `recency`/`freshness` 两档在测试里退化，"老而相关 vs 新而无干"
/// 这一对**正是本刀要判的那件事**造不出来。夹具照生产写路径的列集抄（Q5 的
/// `learning.rs` 同法），不发明列。
fn seeded(tag: &str) -> (std::path::PathBuf, MemoryStore, i64) {
    let base = crate::timeutil::now_secs();
    let dir = temp_dir(tag);
    std::fs::create_dir_all(&dir).unwrap();
    {
        let conn = Connection::open(dir.join("memory.sqlite3")).unwrap();
        conn.execute_batch(&format!(
            "CREATE TABLE memories (
                 workspace  TEXT NOT NULL DEFAULT '',
                 key        TEXT NOT NULL,
                 value      TEXT NOT NULL,
                 tags       TEXT NOT NULL DEFAULT '',
                 updated_at INTEGER NOT NULL,
                 source     TEXT NOT NULL DEFAULT 'explicit',
                 hit_count  INTEGER NOT NULL DEFAULT 0,
                 last_hit_at INTEGER,
                 PRIMARY KEY (workspace, key));
             INSERT INTO memories VALUES ('', 'old-rel', '部署用 docker compose', 'ops', {n}, 'explicit', 0, NULL);
             INSERT INTO memories VALUES ('', 'new-noise', '今天天气不错', '', {n}, 'explicit', 0, NULL);
             INSERT INTO memories VALUES ('', 'mid-hot', '别的东西', '', {d}, 'explicit', 30, {n});
             INSERT INTO memories VALUES ('F:/ws/a', 'project-only', 'docker 在项目层', '', {n}, 'explicit', 0, NULL);",
            n = base,
            d = base - 60 * DAY
        ))
        .unwrap();
    }
    let store = MemoryStore::new(&dir, test_resolver());
    (dir, store, base)
}

fn read_learning(conn: &Connection, key: &str) -> (i64, Option<i64>, i64) {
    conn.query_row(
        "SELECT hit_count, last_hit_at, updated_at FROM memories WHERE key = ?1",
        [key],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )
    .unwrap()
}

fn q(weights: LearnWeights, terms: &[&str]) -> RecallQuery {
    RecallQuery {
        terms: terms.iter().map(|s| s.to_string()).collect(),
        weights,
    }
}

/// 学习路径的排序 + 写回 + 层隔离，三条一起判（同一个库、同一次调用）。
#[test]
fn learned_recall_reorders_and_reinforces_only_what_it_delivered() {
    let (dir, store, base) = seeded("learn-recall");
    let on = LearnWeights {
        enabled: true,
        ..LearnWeights::default()
    };
    let got = store.recall_relevant(None, 2, &q(on, &["docker"]));
    let keys: Vec<&str> = got.iter().map(|e| e.key.as_str()).collect();
    // 相关性档（400）把"老但命中 docker"的这条抬到第一。
    assert_eq!(keys, ["old-rel", "new-noise"], "实得 {keys:?}");
    // ⚠ 第二位是 **E5 保留槽顶上来的**，不是自己考上的：mid-hot（今天刚被命中过、
    // 累计 30 次）按分数排第二 = 491,700 > new-noise 的 350,000，而"最近更新那条"
    // 是 new-noise ⇒ `limit >= 2` 的顶替把它换进末位。所以这一例同时端到端判了
    // 三件事：加权排序生效（old-rel 上位）、保留槽生效（mid-hot 落选）、
    // **落选的不写回**（下面 `hit_count` 仍是 30）。
    assert!(!keys.contains(&"mid-hot"), "保留槽没顶掉分数第二：{keys:?}");
    // 项目层那一行也含 "docker"，但恒不得进注入面（v2.136 裁定，学习路径同样适用）
    assert!(!keys.contains(&"project-only"), "项目层混进注入：{keys:?}");
    // 层标口径与现役一致
    assert!(got.iter().all(|e| e.scope == "global"));

    let conn = Connection::open(dir.join("memory.sqlite3")).unwrap();
    // **交付的两条** +1，没交付的两条一字不动（写回单位 = 这次真正进块了的）
    let (hits, last, updated) = read_learning(&conn, "old-rel");
    assert_eq!(hits, 1, "交付即命中");
    assert!(
        last.is_some_and(|t| t >= base),
        "写回时间由调用方的 now 给（>= 夹具基准，允许跨一秒）"
    );
    assert_eq!(updated, base, "⚠ 写回不得刷新 updated_at——它是 recency 的唯一来源，一旦被召回刷新，被读到就冒充成刚编辑过");
    assert_eq!(read_learning(&conn, "new-noise").0, 1);
    assert_eq!(
        read_learning(&conn, "mid-hot").0,
        30,
        "未交付的行不许被计数"
    );
    assert_eq!(
        read_learning(&conn, "project-only").0,
        0,
        "项目层不参与写回"
    );
    drop(conn);
    let _ = std::fs::remove_dir_all(&dir);
}

/// **等价性的生产面**：学习档关掉 ⇒ 交付集合与现役 `recall` 逐字段相同，
/// 且**一次写回都不发生**（稿 §5.4 只要求排序回退，本批把写回收进同一档——
/// 对照组若仍在累积 `hit_count`，A/B 比的就不是同一份库）。
#[test]
fn learn_disabled_returns_current_ordering_and_writes_nothing() {
    let (dir, store, base) = seeded("learn-off");
    let off = LearnWeights {
        enabled: false,
        ..LearnWeights::default()
    };
    // 关掉时词表也传空（内核调用点的形状），但**即使传了词表也不得生效**——
    // 等价性只由 `enabled` 一个条件决定，这样宿主侧没有"半开"分支可写。
    let got = store.recall_relevant(None, 3, &q(off, &["docker"]));
    let legacy = store.recall(None, 3);
    let keys: Vec<&str> = got.iter().map(|e| e.key.as_str()).collect();
    let legacy_keys: Vec<&str> = legacy.iter().map(|e| e.key.as_str()).collect();
    assert_eq!(
        keys, legacy_keys,
        "关掉学习档必须走现役那条 SQL，实得 {keys:?}"
    );
    assert_eq!(
        got.iter().map(|e| e.value.clone()).collect::<Vec<_>>(),
        legacy.iter().map(|e| e.value.clone()).collect::<Vec<_>>()
    );
    let conn = Connection::open(dir.join("memory.sqlite3")).unwrap();
    for key in ["old-rel", "new-noise", "mid-hot", "project-only"] {
        let (hits, last, _) = read_learning(&conn, key);
        assert_eq!(hits, if key == "mid-hot" { 30 } else { 0 }, "{key} 被写了");
        assert_eq!(last, if key == "mid-hot" { Some(base) } else { None });
    }
    drop(conn);
    let _ = std::fs::remove_dir_all(&dir);
}

/// 权重全 0（合法档，见 `settings/tests/memory.rs`）⇒ 走的是学习路径、写回照做，
/// 但顺序由 tie-break 决定 = 现役序。这是"滑条全拉到 0"的用户可见形状。
#[test]
fn zero_weights_keep_current_order_yet_still_reinforce() {
    let (dir, store, _base) = seeded("learn-zero");
    let zero = LearnWeights {
        enabled: true,
        relevance: 0,
        strength: 0,
        freshness: 0,
        recency: 0,
    };
    let got = store.recall_relevant(None, 2, &q(zero, &["docker"]));
    let keys: Vec<&str> = got.iter().map(|e| e.key.as_str()).collect();
    // 现役序 = `updated_at DESC, key ASC`：old-rel 与 new-noise 同刻（夹具就是），
    // key 升序把 **new-noise 排在前面**（'n' < 'o'）——写反过一次，这条注释留着
    // 是给下一个手搓期望序的人：tie-break 的第二键是 key 升序，不是插入序。
    assert_eq!(keys, ["new-noise", "old-rel"], "实得 {keys:?}");
    let conn = Connection::open(dir.join("memory.sqlite3")).unwrap();
    assert_eq!(
        read_learning(&conn, "old-rel").0,
        1,
        "全 0 仍要写回（档是开的）"
    );
    drop(conn);
    let _ = std::fs::remove_dir_all(&dir);
}

/// 库打不开 ⇒ 学习路径与现役同档降级（空表、不 panic、**不记埋点也不写库**）。
/// 这条补的是 Q1 那张"注入不可用 vs 库是空的"的假指标防线在学习路径上的对应面。
#[test]
fn unreadable_database_degrades_to_empty_on_the_learned_path() {
    let dir = temp_dir("learn-broken");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("file-not-dir"), "x").unwrap();
    let store = MemoryStore::new(&dir.join("file-not-dir"), test_resolver());
    let on = LearnWeights {
        enabled: true,
        ..LearnWeights::default()
    };
    assert!(store
        .recall_relevant(Some("ws-session"), 5, &q(on, &["docker"]))
        .is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}
