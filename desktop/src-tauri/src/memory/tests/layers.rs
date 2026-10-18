//! 每层一行与回收区的宿主侧装配（S1 刀④c · `memory/layers.rs`）。
//!
//! 判的都是"界面拿得到什么"这一层：数据面（搬移、防撞、越界拒、索引刷新）已经由
//! `plugin_api::memlayout::tests::{obsolete,read_back}` 两簇钉住，这里不重复证那些，
//! 只证**转出去的形状**没错——字段映射、排序方向、两个"读不到"的数、hint 对不上时
//! 报什么。命令壳（`commands/memory.rs`）没有单测：`State<'_, AppState>` 不在可构造
//! 面内，与 `memory_reveal_project` 同例（Q15 那批立的就是这条分工）。

use super::*;
use crate::memory::{
    clear_layer, layer_dir_to_open, layers_view, obsolete_view, purge_obsolete, restore_obsolete,
};

/// 「每层一行」：按目录给数，两个"读不到"的数各归各的位置。
#[test]
fn a_layer_row_counts_by_directory_and_reports_what_cannot_be_read() {
    let dir = temp_dir("layers-view");
    let store = wired(&dir);
    let ws = keysid();
    let root = store.md_root().to_path_buf();
    store
        .write("", "pref.a", "全局一条", "", "explicit")
        .unwrap();
    store
        .write("", "pref.b", "全局两条", "", "explicit")
        .unwrap();
    store
        .write(&ws, "build.cmd", "项目一条", "构建", "explicit")
        .unwrap();

    let view = layers_view(&root);
    assert_eq!(view.blocked_layers, 0, "{view:?}");
    assert_eq!(view.rows.len(), 2, "全局 + 那一层：{:?}", view.rows);
    let global = view
        .rows
        .iter()
        .find(|r| r.hint.is_empty())
        .expect("全局行");
    assert_eq!(global.fact_count, 2, "{global:?}");
    let proj = view
        .rows
        .iter()
        .find(|r| r.hint == plugin_api::memlayout::project_dir_name(&ws))
        .expect("项目行");
    assert_eq!(proj.fact_count, 1, "{proj:?}");
    assert!(proj.newest_updated_at > 0, "写进去那条带着 updated");
    assert_eq!(view.unreadable_files, 0);

    // 事实位上放一个目录冒充文件 ⇒ 那一层读得动、这一篇读不动：只进 `unreadable_files`，
    // **不许**混进 `fact_count`（虚报），也不许让它从读数里消失（瞒报）。
    std::fs::create_dir_all(root.join("global").join("broken.md")).unwrap();
    let after = layers_view(&root);
    let g = after
        .rows
        .iter()
        .find(|r| r.hint.is_empty())
        .expect("全局行");
    assert_eq!((g.fact_count, g.unreadable), (2, 1), "{g:?}");
    assert_eq!(after.unreadable_files, 1);

    // 整层扫不动那一档：那一层**不在这份清单里**（`layers()` 只枚举读得动的目录），所以它
    // 只能靠 `blocked_layers` 交代出去。把那个数写成 0（"层数不往外传"）在这一句红——
    // 少了它，界面上的"有一层读不到"就变成"这台机器只记过两层"，那是瞒报。
    let fake = root.join("projects").join("Gamma-00000000");
    std::fs::create_dir_all(&fake).unwrap();
    std::fs::write(fake.join("facts"), "not a directory").unwrap();
    let blocked_view = layers_view(&root);
    assert_eq!(blocked_view.blocked_layers, 1, "{:?}", blocked_view.rows);
    assert_eq!(
        blocked_view.rows.len(),
        2,
        "扫不动那层不进清单，所以清单少一行：{:?}",
        blocked_view.rows
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// 「删除该层」只搬那一层；hint 对不上 ⇒ 搬 0 条而不是报错。
#[test]
fn clearing_a_layer_moves_only_that_layer_into_the_trash() {
    let dir = temp_dir("layers-clear");
    let store = wired(&dir);
    let ws = keysid();
    let root = store.md_root().to_path_buf();
    let hint = plugin_api::memlayout::project_dir_name(&ws);
    store
        .write("", "pref.a", "全局一条", "", "explicit")
        .unwrap();
    store
        .write(&ws, "build.a", "项目一条", "", "explicit")
        .unwrap();
    store
        .write(&ws, "build.b", "项目两条", "", "explicit")
        .unwrap();

    assert_eq!(clear_layer(&root, &hint).unwrap(), 2, "那层两条都搬走");
    let view = layers_view(&root);
    let g = view
        .rows
        .iter()
        .find(|r| r.hint.is_empty())
        .expect("全局行");
    assert_eq!(g.fact_count, 1, "另一层一个字都没动：{g:?}");
    assert_eq!(
        view.rows
            .iter()
            .find(|r| r.hint == hint)
            .unwrap()
            .fact_count,
        0,
        "被清那层现在 0 条"
    );
    let trash = obsolete_view(&root);
    assert_eq!(trash.rows.len(), 2, "{:?}", trash.rows);
    assert_eq!(trash.unreadable, 0);
    // 对不上任何一层 ⇒ 0 条（"删一个没建过的层"与"那层本来空着"是同一件事，不弹错误）
    assert_eq!(clear_layer(&root, "no-such-layer").unwrap(), 0);
    let _ = std::fs::remove_dir_all(&dir);
}

/// 恢复：搬回原位、回收区少一条；两种"没有"要分开——列表里没有 = `false`，
/// 越界形状 = `Err`（刀④c 把 `rel` 暴露到 IPC 面上，这道闸是它唯一的防线）。
#[test]
fn a_restored_entry_comes_back_and_a_bad_shape_is_refused() {
    let dir = temp_dir("layers-restore");
    let store = wired(&dir);
    let root = store.md_root().to_path_buf();
    store
        .write("", "pref.a", "要被删的那条", "", "explicit")
        .unwrap();
    store.delete("", "pref.a").unwrap();
    let trash = obsolete_view(&root);
    assert_eq!(trash.rows.len(), 1, "{:?}", trash.rows);

    // 正路：原样把清单给的 rel 传回去
    assert!(restore_obsolete(&root, &trash.rows[0].rel).unwrap());
    assert_eq!(layers_view(&root).rows[0].fact_count, 1, "回到原来那一层");
    assert_eq!(obsolete_view(&root).rows.len(), 0, "回收区少一条");
    // 列表过期（同一条再恢复一次）⇒ `false`，不是 Err：界面上是"这条已经不在了"
    assert!(
        !restore_obsolete(&root, &trash.rows[0].rel).unwrap(),
        "没有这条 ⇒ false"
    );
    // 越界形状 ⇒ Err，且回收区不许被这次尝试改动。三个形状都挑在**两侧同解**的写法：
    // `..\\escape.md` 在 POSIX 上是一个合法普通文件名（反斜杠不是分隔符），拿它做断言
    // 会让 WSL 侧红在实现上（AGENTS §7：跨平台的性质不许靠 cfg 单侧跳过）。
    for bad in ["../../outside.md", "", "."] {
        let e = restore_obsolete(&root, bad).expect_err("越界必须被拒");
        assert!(e.contains("不合法"), "{bad} 的报错要点名形状：{e}");
    }
    assert_eq!(obsolete_view(&root).rows.len(), 0);
    let _ = std::fs::remove_dir_all(&dir);
}

/// 只有清空回收区那一次是真删；此后既列不出来、磁盘上也没有那份镜像了。
#[test]
fn purging_the_trash_is_the_only_step_that_really_deletes() {
    let dir = temp_dir("layers-purge");
    let store = wired(&dir);
    let root = store.md_root().to_path_buf();
    store
        .write("", "pref.a", "先删一条", "", "explicit")
        .unwrap();
    store.delete("", "pref.a").unwrap();
    assert_eq!(obsolete_view(&root).rows.len(), 1, "前提：软删进得去");

    assert_eq!(purge_obsolete(&root).unwrap(), 1);
    assert_eq!(obsolete_view(&root).rows.len(), 0);
    // 真删之后磁盘上那份镜像也不该在。这里**不猜文件名**（删除戳是 `~<毫秒>`，写死一个
    // 具体名字的用例只会恒真地通过）：直接数回收区那一层里还剩几项——`purge_obsolete`
    // 会把搬空的子目录收掉，所以目录不在（计数 0）与目录在而里面空，两种都算通过。
    let leftover = std::fs::read_dir(root.join("obsolete").join("global"))
        .map(|rd| rd.flatten().count())
        .unwrap_or(0);
    assert_eq!(leftover, 0, "真删之后镜像不该还在：{leftover} 项");
    // 空回收区再清 ⇒ 0，不报错（幂等）
    assert_eq!(purge_obsolete(&root).unwrap(), 0);
    // 真源那条早就没了：清空回收区不该把全局层带出什么新东西
    assert_eq!(layers_view(&root).rows[0].fact_count, 0);
    let _ = std::fs::remove_dir_all(&dir);
}

/// 「打开该层文件夹」：全局层开 `global/`，项目层开它的项目文件夹（`facts/` 的上一级，
/// 与 `memory_reveal_project` 同形状）；对不上任何一层 ⇒ Err 且**不回落到别处**。
#[test]
fn opening_a_layer_points_at_that_layer_and_never_at_a_neighbor() {
    let dir = temp_dir("layers-open");
    let store = wired(&dir);
    let ws = keysid();
    let root = store.md_root().to_path_buf();
    let hint = plugin_api::memlayout::project_dir_name(&ws);
    store
        .write("", "pref.a", "全局一条", "", "explicit")
        .unwrap();
    store
        .write(&ws, "build.a", "项目一条", "", "explicit")
        .unwrap();

    assert_eq!(layer_dir_to_open(&root, "").unwrap(), root.join("global"));
    assert_eq!(
        layer_dir_to_open(&root, &hint).unwrap(),
        root.join("projects").join(&hint),
        "项目层开的是那一层的项目文件夹，不是 facts/ 也不是隔壁那层"
    );
    let e = layer_dir_to_open(&root, "no-such-layer").expect_err("对不上就报错");
    assert!(
        e.contains("no-such-layer"),
        "报错要带上用户能看到的那个名字：{e}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
