//! 软删与回收（S1 刀④a · AC-S1-7）：删除 = 搬进 `obsolete/`、恢复 = 原路搬回、
//! 清空回收区才是真删。这一簇只管磁盘上那些字节**去了哪儿**——索引对不对由
//! `write_delete` 那簇同批盯（每条软删/恢复都会刷那一层的 `MEMORY.md`）。
//!
//! 夹具纪律照 `tests/mod.rs`：全部正文经生产 `write_fact` 渲染，不手写 md。
//! 断言一律不猜目录名算法（项目层用 `project_dir_name` 取，回收区按 `list_obsolete` 读）。

use super::super::obsolete::shift::slot_stamp;
use super::*;

/// 软删的三件同时成立：原位置没了、回收区里有一条镜像、读面与索引都不再看见它。
/// 最后那半是 AC-S1-7 的判据本体——"注入零命中"就是 `iter_facts` 与索引双双不含。
#[test]
fn a_delete_moves_the_file_into_obsolete_and_out_of_the_read_face() {
    let root = tmp_root("ob-del");
    let ws = r"F:\proj\Alpha";
    write_fact(&root, &entry(ws, "db.choice", "postgres")).unwrap();
    write_fact(&root, &entry("", "pref.pnpm", "always pnpm")).unwrap();

    let moved = soft_delete_fact(&root, ws, "db.choice").expect("软删成功");
    assert!(moved, "文件在场就该搬走");

    let origin = fact_rel_path(ws, "db.choice");
    assert!(!root.join(&origin).exists(), "原位置必须没有");
    let scan = iter_facts(&root);
    let keys: Vec<&str> = scan.facts.iter().map(|f| f.key.as_str()).collect();
    assert_eq!(keys, ["pref.pnpm"], "读面只剩没被删的那条：{keys:?}");
    let index = std::fs::read_to_string(
        root.join("projects")
            .join(project_dir_name(ws))
            .join("MEMORY.md"),
    )
    .unwrap();
    assert!(
        !index.contains("db.choice"),
        "索引行跟着消失（否则注入还会带它）：{index}"
    );

    let ob = list_obsolete(&root);
    assert_eq!(ob.entries.len(), 1, "{:?}", ob.entries);
    let e = &ob.entries[0];
    assert_eq!(e.key, "db.choice");
    assert_eq!(e.origin, origin, "镜像 rel 必须指回它来的地方");
    assert_eq!(e.workspace, ws, "展示用的那一维从 frontmatter 读回");
    assert!(e.deleted_at_ms > 0, "删除戳在场：{e:?}");
    // 回收区里那份内容就是原文件（一字节都没重写 ⇒ 恢复之后 sha 必然相同）
    let in_obsolete = obsolete_root(&root).join(&e.rel);
    assert!(
        in_obsolete.is_file(),
        "搬过去不是复制：{}",
        in_obsolete.display()
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// 恢复 = 原路搬回，**字节与内容逐字相同**、索引那一行重新在场、回收区清空。
#[test]
fn restoring_puts_the_bytes_back_at_the_origin() {
    let root = tmp_root("ob-restore");
    let before = write_fact(&root, &entry("", "pref.x", "原话")).unwrap();
    let bytes = std::fs::read(&before).unwrap();
    soft_delete_fact(&root, "", "pref.x").unwrap();
    let rel = list_obsolete(&root).entries[0].rel.clone();

    assert!(restore_obsolete(&root, &rel).expect("恢复不该失败"));
    assert_eq!(
        std::fs::read(&before).unwrap(),
        bytes,
        "恢复回来的就是那一份字节"
    );
    assert_eq!(
        read_fact(&root, "", "pref.x")
            .unwrap()
            .expect("读面要看得见")
            .value,
        "原话"
    );
    let index = std::fs::read_to_string(root.join("global").join("MEMORY.md")).unwrap();
    assert!(index.contains("pref.x"), "索引行搬回来了：{index}");
    assert!(
        list_obsolete(&root).entries.is_empty(),
        "回收区必须清空（恢复不是复制）"
    );
    // 幂等：同一条再恢复一次 = 回收区里没有 ⇒ false，不报错也不新建空文件
    assert!(
        !restore_obsolete(&root, &rel).unwrap(),
        "没有那条就是 no-op"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// 原位置已被占用（删掉之后又写回同一个 key）⇒ 恢复必须**报错并原地留着**。
/// 静默覆盖会把用户新写的那一条吃掉——那比"恢复失败让他看一眼"严重得多。
#[test]
fn a_restore_never_overwrites_what_came_back_in_its_place() {
    let root = tmp_root("ob-clash");
    write_fact(&root, &entry("", "k.dup", "第一版")).unwrap();
    soft_delete_fact(&root, "", "k.dup").unwrap();
    let rel = list_obsolete(&root).entries[0].rel.clone();
    write_fact(&root, &entry("", "k.dup", "第二版")).unwrap();

    let e = restore_obsolete(&root, &rel).expect_err("原位置有货就该报错");
    assert!(e.contains("原位置"), "报错要说清楚为什么：{e}");
    assert_eq!(
        read_fact(&root, "", "k.dup").unwrap().unwrap().value,
        "第二版",
        "报错这一趟不许动真源"
    );
    assert_eq!(
        list_obsolete(&root).entries.len(),
        1,
        "被拒的那条必须还留在回收区，等他处置"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// 同一 key 反复软删不许互相吃掉（`rename` 两侧都覆盖既有目标）。
///
/// ⚠ 这条**不是**防撞算法的红证：它要撞出名，得靠两次 `now_ms()` 落在同一毫秒里——那是
/// 运气，跑一次绿一次等于零证明力（第一版拿它做变异，实测打空）。防撞算法本体由下面那两条
/// 纯函数用例确定性地钉。这一条留着管端到端那一层：搬移、索引、`list_obsolete` 三件事在
/// "同一条被删两次"这个形状上还自洽（两个条目、同一个 origin、原位置空）。
#[test]
fn two_soft_deletes_of_the_same_key_both_survive_in_the_trash() {
    let root = tmp_root("ob-twice");
    write_fact(&root, &entry("", "k.a", "v1")).unwrap();
    soft_delete_fact(&root, "", "k.a").unwrap();
    write_fact(&root, &entry("", "k.a", "v2")).unwrap();
    soft_delete_fact(&root, "", "k.a").unwrap();

    let ob = list_obsolete(&root);
    assert_eq!(ob.entries.len(), 2, "两条都要找得回：{:?}", ob.entries);
    assert_ne!(ob.entries[0].rel, ob.entries[1].rel, "落点不许相同");
    assert!(
        ob.entries
            .iter()
            .all(|e| e.origin == fact_rel_path("", "k.a")),
        "剥戳之后都指回同一个原位：{:?}",
        ob.entries
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// 防撞的算法本体（纯函数 ⇒ 可以确定性地测）：槽被占就往后挪 1ms，而不是给文件名加后缀。
/// 红证 = `tmp/mut-clamp.mjs --case 42`（把那句"没被占就用"改成恒真 ⇒ 本条与下一条都红）。
#[test]
fn a_taken_stamp_walks_forward_one_millisecond_at_a_time() {
    // 占住 T..T+2 ⇒ 该拿到 T+3
    let got = slot_stamp(1_000, |t| t < 1_003).expect("第三个槽之后是空的");
    assert_eq!(got, 1_003, "挪的是戳，不是名字：{got}");
    // 没被占 ⇒ 一格都不挪（时钟粒度不是可以浪费的：戳就是"什么时候删的"）
    assert_eq!(slot_stamp(2_000, |_| false), Some(2_000));
}

/// 撞满上限 = 报不出来，而不是**静默覆盖**已经在那儿的那一篇。
/// 静默覆盖的形态是"用户删过两条、回收区只有一条"——那种丢失在界面上看不出来。
#[test]
fn a_full_hundred_taken_stamps_refuses_instead_of_overwriting() {
    assert_eq!(
        slot_stamp(5_000, |_| true),
        None,
        "全占 ⇒ None，由调用方转成 Err"
    );
    // 边界说破：偏移 0..=100 共 101 格是允许的，第 102 格才放弃
    assert_eq!(slot_stamp(5_000, |t| t < 5_100), Some(5_100));
    assert_eq!(slot_stamp(5_000, |t| t <= 5_100), None);
}

/// 整层软删（管理页"清空"）：两层一起搬空、两个索引都刷、返回的是搬动条数。
#[test]
fn a_soft_clear_moves_every_layer_and_reports_the_count() {
    let root = tmp_root("ob-clear");
    let ws = r"F:\proj\Beta";
    write_fact(&root, &entry("", "g1", "v")).unwrap();
    write_fact(&root, &entry("", "g2", "v")).unwrap();
    write_fact(&root, &entry(ws, "p1", "v")).unwrap();

    let n = soft_clear_facts(&root).expect("清空成功");
    assert_eq!(n, 3, "搬动的条数 = 真源里的事实文件数");
    assert!(iter_facts(&root).facts.is_empty(), "读面一条不剩");
    assert_eq!(list_obsolete(&root).entries.len(), 3);
    for idx in [
        root.join("global").join("MEMORY.md"),
        root.join("projects")
            .join(project_dir_name(ws))
            .join("MEMORY.md"),
    ] {
        let text = std::fs::read_to_string(&idx).unwrap();
        assert!(
            !text.contains(".md)",),
            "两层索引都得重写空：{}｜{text}",
            idx.display()
        );
    }
    // 再清一次 = 0（幂等，且不报错）
    assert_eq!(soft_clear_facts(&root).unwrap(), 0);
    let _ = std::fs::remove_dir_all(&root);
}

/// 回收区那枚"恢复"按钮把 `rel` 暴露到了 IPC 面上（刀④c），所以越界形状必须当场被拒：
/// `..` 往上走、绝对路径、空串三种都要拦。放过去的话，一次点击就成了"把数据根外一个
/// 文件搬进真源"——那已经不是删除动作，而是任意路径读写。
#[test]
fn a_restore_refuses_anything_that_is_not_a_trash_relative_path() {
    let root = tmp_root("ob-restore-escape");
    write_fact(&root, &entry("", "k.a", "v1")).unwrap();
    soft_delete_fact(&root, "", "k.a").unwrap();
    let outside = root
        .parent()
        .expect("tmp_root 有父目录")
        .join("ob-escape-victim.md");
    std::fs::write(&outside, b"i live outside the data root").unwrap();

    for bad in [
        std::path::Path::new("../../ob-escape-victim.md"),
        outside.as_path(),
        std::path::Path::new(""),
    ] {
        let e = restore_obsolete(&root, bad).expect_err("三种形状都要拒");
        assert!(e.contains("不合法"), "报错要点名是路径不合法：{e}");
    }
    assert!(
        std::fs::read(&outside).unwrap() == b"i live outside the data root"[..],
        "根外那个文件一个字都没被碰"
    );
    assert_eq!(list_obsolete(&root).entries.len(), 1, "回收区也没少一条");
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_file(&outside);
}

/// **单层软删**（刀④c 界面上"删除该层"那一枚动作）。这一档是上面那条给不出的：
/// 那条只证"全都搬走"，把这一条的实现改成"永远搬全局层"或"顺手把所有层索引都刷一遍"
/// 它照样绿。所以两边都要断言：被清那层重写空索引，**没被碰那层连索引都不许动**。
#[test]
fn clearing_one_layer_leaves_the_other_layer_and_its_index_alone() {
    let root = tmp_root("ob-clear-layer");
    let ws = r"F:\proj\Beta";
    let hint = project_dir_name(ws);
    write_fact(&root, &entry("", "g1", "v")).unwrap();
    write_fact(&root, &entry("", "g2", "v")).unwrap();
    write_fact(&root, &entry(ws, "p1", "v")).unwrap();
    write_fact(&root, &entry(ws, "p2", "v")).unwrap();
    let global_index = std::fs::read_to_string(root.join("global").join("MEMORY.md")).unwrap();

    let n = soft_clear_layer(&root, &hint).expect("清项目层");
    assert_eq!(n, 2, "只搬那一层的两条");
    let left: Vec<String> = iter_facts(&root)
        .facts
        .iter()
        .map(|f| f.key.clone())
        .collect();
    assert_eq!(
        left,
        vec!["g1".to_string(), "g2".to_string()],
        "全局层两条照旧：{left:?}"
    );
    assert_eq!(list_obsolete(&root).entries.len(), 2, "回收区只多了那两条");
    assert_eq!(
        std::fs::read_to_string(root.join("global").join("MEMORY.md")).unwrap(),
        global_index,
        "没被碰的那层，索引一个字都不许改"
    );
    let idx = std::fs::read_to_string(root.join("projects").join(&hint).join("MEMORY.md")).unwrap();
    assert!(!idx.contains(".md)"), "被清那层的索引必须重写空：{idx}");
    // hint 对不上 = 那层不存在 ⇒ Ok(0) 而不是 Err：删一个没建过的层不该弹错误给人看
    assert_eq!(soft_clear_layer(&root, "no-such-layer").unwrap(), 0);
    // 再清同一层 = 0（幂等）
    assert_eq!(soft_clear_layer(&root, &hint).unwrap(), 0);
    let _ = std::fs::remove_dir_all(&root);
}

/// 单层版**同样**吃那道"有层扫不动就中止"的闸（两个入口共用 `check_blocked` 那一份判据）。
/// 夹具用 `projects/<x>/facts` 位置上放一个普通文件：那正是 [`layers`] 第二个返回值的
/// 进账口，与上面那条 `global` 被冒充（走的是软删自己的那句"层不是目录"）不是同一档。
#[test]
fn clearing_one_layer_also_refuses_when_a_layer_is_unreadable() {
    let root = tmp_root("ob-clear-blocked");
    write_fact(&root, &entry("", "g1", "v")).unwrap();
    write_fact(&root, &entry(r"F:\proj\Beta", "p1", "v")).unwrap();
    let fake = root.join("projects").join("Gamma-00000000");
    std::fs::create_dir_all(&fake).unwrap();
    std::fs::write(fake.join("facts"), "not a directory").unwrap();

    let e = soft_clear_layer(&root, "").expect_err("有一层扫不动 ⇒ 单层版也必须中止");
    assert!(e.contains("扫不动"), "报错要说清为什么没删：{e}");
    assert_eq!(
        iter_facts(&root).facts.len(),
        2,
        "中止 = 一条都不搬（不许留下半空的状态）"
    );
    assert_eq!(
        list_obsolete(&root).entries.len(),
        0,
        "回收区也不许多出东西"
    );
    // 同一道闸对"全清"也生效：撤掉 `soft_clear_facts` 里那一句调用，这条当场红
    assert!(soft_clear_facts(&root).is_err(), "全清同样中止");
    let _ = std::fs::remove_dir_all(&root);
}

/// 软删**不许动 `.sync/` 标记**（纪律③）：留着它，reconcile 才会看见"标记在 + 行在 +
/// 文件缺"从而删掉影子行。标记一起删掉 ⇒ 影子行成为没有主人的幽灵（界面上 = 删了又冒出来）。
#[test]
fn a_soft_delete_keeps_the_sync_marker_for_reconcile() {
    let root = tmp_root("ob-marker");
    write_fact(&root, &entry("", "pref.m", "v")).unwrap();
    let rel = fact_rel_path("", "pref.m");
    assert!(read_marker(&root, &rel).is_some(), "前提：写入时落了标记");

    soft_delete_fact(&root, "", "pref.m").unwrap();
    assert!(
        read_marker(&root, &rel).is_some(),
        "软删之后标记必须还在（对账靠它删影子行）"
    );
    // 对照：同一条走硬删（`memory_undo` 撤销新建那一支）时标记是跟着没的
    write_fact(&root, &entry("", "pref.h", "v")).unwrap();
    let rel2 = fact_rel_path("", "pref.h");
    delete_fact(&root, "", "pref.h").unwrap();
    assert!(
        read_marker(&root, &rel2).is_none(),
        "硬删连标记一起清（那是'当作没发生过'的那一档）"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// **层级故障 ≠ 这条不存在**：那一层的位置被一个普通文件冒充时，软删必须 `Err`。
/// 与 `write_delete.rs` 里硬删那族同一判据（Windows 上 `ERROR_PATH_NOT_FOUND` 与
/// `ENOENT` 同码，不分开就得到一条"Linux 报错、Windows 谎称没这条记忆"的假性质）。
#[test]
fn a_blocked_layer_is_an_error_not_a_missing_fact() {
    let root = tmp_root("ob-blocked");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("global"), "not a directory").unwrap();

    let e = soft_delete_fact(&root, "", "pref.x").expect_err("层级故障必须报错");
    assert!(e.contains("层不是目录"), "报错要指名是哪一层坏了：{e}");
    // 回收区那侧不该因为一次失败报错就留下半搬的状态
    assert!(
        list_obsolete(&root).entries.is_empty(),
        "{:?}",
        list_obsolete(&root)
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// 真删只发生在清空回收区那一次：`purge_obsolete` 把条目清干净、报条数，
/// 并把 `obsolete/` 本身留着（它是布局的一部分）。
#[test]
fn only_purging_the_trash_really_deletes() {
    let root = tmp_root("ob-purge");
    write_fact(&root, &entry("", "k.1", "v")).unwrap();
    write_fact(&root, &entry("", "k.2", "v")).unwrap();
    soft_delete_fact(&root, "", "k.1").unwrap();
    soft_delete_fact(&root, "", "k.2").unwrap();
    assert_eq!(list_obsolete(&root).entries.len(), 2);

    assert_eq!(purge_obsolete(&root).expect("清空回收区"), 2);
    assert!(list_obsolete(&root).entries.is_empty(), "真删之后一条不剩");
    assert!(obsolete_root(&root).is_dir(), "目录本身留着");
    assert_eq!(purge_obsolete(&root).unwrap(), 0, "再清一次 = 0，不报错");
    // 回收区不在 = 空，不是故障（首启之后从没删过东西就是这个形状）
    std::fs::remove_dir_all(obsolete_root(&root)).unwrap();
    assert_eq!(purge_obsolete(&root).unwrap(), 0);
    assert_eq!(list_obsolete(&root).entries.len(), 0);
    let _ = std::fs::remove_dir_all(&root);
}
