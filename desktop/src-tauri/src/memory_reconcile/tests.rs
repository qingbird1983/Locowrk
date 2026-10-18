//! memory_reconcile 单测：文件为准回写 / 新增吸纳 / 人删文件删行 /
//! **影子先行只点名不写文件** / 整层缺席不删行。全部走临时目录。
//!
//! S1 刀②c-2 把"索引先行愈合"那一格的正向断言（`sum.healed`）换成了反向判据：影子比
//! 真源新时文件一个字节不许变。理由见模块头注——旧形状在真源 = 文件的时代等于"把人删掉
//! 的记忆写回来"。同族另一条（启动导出不补事实文件）在 `memory_files::tests`，因为
//! 被删的那段代码住在那里。
//!
//! **取数通道写死 `export_rows()`，不许换成 `store.list()`**：本模块的职责是
//! 「以文件为准回写**行**」，而 S1 刀②b-1 起 `list()` 的读源 = md。用 list 当"库里有哪几行"
//! 的代理，会在那一刀落地当场失真——实测两条红就是证据：一条比出目录反查名 `proj-a-67ed0df8`
//! 而权威键位是 `F:/ws/proj-a`（md 那边只能给目录名），一条比出 0 行而库里明明还有 1 行
//! （文件被删了、行按"目录缺席不删行"保留）。断言的语义一个字没动，只是把眼睛挪回它本来
//! 该看的地方。

use super::run;
use crate::memory::MemoryStore;
use crate::memory_files::MemoryFiles;
use plugin_api::memlayout;
use std::path::PathBuf;
use std::sync::Arc;

fn temp_dir(tag: &str) -> PathBuf {
    crate::testutil::unique_dir(&format!("reconcile-{}", tag))
}

/// 宿主存储 + 同一数据根下的人类层句柄（刀②c-1 起 store 自己按数据根算 md 根，
/// 这里这份只是本模块函数签名要一个 `&MemoryFiles`；两份句柄同一算式 = 同一个目录）。
fn setup(tag: &str) -> (MemoryStore, MemoryFiles, PathBuf) {
    let dir = temp_dir(tag);
    std::fs::create_dir_all(&dir).unwrap();
    let files = MemoryFiles::new(&dir);
    let store = MemoryStore::new(&dir, Arc::new(|_| None));
    (store, files, dir)
}

/// md 层根（MemoryFiles root = 数据根/memory——测试里 sqlite 同根）。
fn md_root(dir: &std::path::Path) -> PathBuf {
    dir.join("memory")
}

/// 模拟人手改文件（直写盘面，绕过双写路径——frontmatter 原样、正文换掉）。
fn human_edit(dir: &std::path::Path, rel: &std::path::Path, body: &str) {
    std::thread::sleep(std::time::Duration::from_millis(25)); // mtime 推进
    let path = md_root(dir).join(rel);
    let text = std::fs::read_to_string(&path).unwrap();
    let front = text.split("---\n\n").next().unwrap().to_string();
    std::fs::write(path, format!("{front}---\n\n{body}\n")).unwrap();
}

#[test]
fn human_edit_flows_back_into_index() {
    let (store, files, dir) = setup("edit");
    store
        .write("", "pref.tool", "pnpm", "pref", "explicit")
        .unwrap();
    // 先把真源灌进影子（S1 刀③b-2 之后写路径不碰库 ⇒ 对账是唯一通路），下面那句
    // "文件为准**更新**一行"才有"更新"可谈；不补这一轮的话它变成 created=1，
    // 判的就是另一件事了。
    let seed = run(&store, &files, &[]).unwrap();
    assert_eq!(seed.created, 1, "对账第一轮从真源建行：{seed:?}");
    let rel = memlayout::fact_rel_path("", "pref.tool");
    human_edit(&dir, &rel, "永远用 pnpm，不要用 npm");
    let sum = run(&store, &files, &[]).unwrap();
    assert_eq!(sum.updated, 1, "文件为准更新一行：{sum:?}");
    let row = store
        .export_rows()
        .unwrap()
        .into_iter()
        .find(|r| r.key == "pref.tool")
        .unwrap();
    assert_eq!(row.value, "永远用 pnpm，不要用 npm");
    assert_eq!(row.tags, "pref", "tags 沿行");
    // 文件没被改写（人改保护——权威在文件）
    let text = std::fs::read_to_string(md_root(&dir).join(&rel)).unwrap();
    assert!(text.contains("永远用 pnpm，不要用 npm"));
    // 再跑一轮：已同步，零噪声
    let sum2 = run(&store, &files, &[]).unwrap();
    assert_eq!(sum2.updated, 0);
}

#[test]
fn human_created_file_is_ingested_as_new_row() {
    let (store, files, dir) = setup("create");
    let rel = memlayout::fact_rel_path("", "hand.made");
    std::fs::create_dir_all(md_root(&dir).join("global")).unwrap();
    std::fs::write(
        md_root(&dir).join(&rel),
        "---\nname: \"hand.made\"\ndescription: \"人建\"\ntags: \"人建\"\nmetadata:\n  type: fact\n  source: \"explicit\"\n  workspace: \"\"\n  updated: 1728100000\n---\n\n我手写的事实\n",
    )
    .unwrap();
    let sum = run(&store, &files, &[]).unwrap();
    assert_eq!(sum.created, 1, "{sum:?}");
    let row = store
        .export_rows()
        .unwrap()
        .into_iter()
        .find(|r| r.key == "hand.made")
        .unwrap();
    assert_eq!(row.value, "我手写的事实");
    assert_eq!(row.source, "explicit", "frontmatter source 优先");
    assert_eq!(row.updated_at, 1728100000, "frontmatter updated 优先");
    // S1 刀③b-2 之后这一行才是这条用例的承重断言：影子行的 tags **只**能从文件来
    // （四条 `shadow_*` 退役 ⇒ 再没有第二个出处）。写死空串时管理页看见的标签会集体消失，
    // 而这一维在 created 支落地之前没有任何用例盯着——旧世界里 shadow_upsert 把它盖住了。
    assert_eq!(row.tags, "人建", "文件里的 tags 是对账新增行的唯一来源");
}

#[test]
fn deleted_file_removes_row_but_missing_dir_keeps_rows() {
    let (store, files, dir) = setup("delete");
    store.write("", "gone.key", "v", "", "explicit").unwrap();
    // 两段都要先跑一轮对账把行建出来：S1 刀③b-2 之后 `write` 不再落库，
    // "删掉一行"这个动作的前提（行在）现在只能由 reconcile 自己给。
    run(&store, &files, &[]).unwrap();
    let rel = memlayout::fact_rel_path("", "gone.key");
    // 单文件删除（标记在 = 我们写过）→ 行删
    std::fs::remove_file(md_root(&dir).join(&rel)).unwrap();
    let sum = run(&store, &files, &[]).unwrap();
    assert_eq!(sum.deleted, 1, "{sum:?}");
    assert!(store.export_rows().unwrap().is_empty());
    // 整层缺席 = 复位手势：行保留（不删行是因为"目录没了"无法区分"人清空了记忆"
    // 与"这层还没建过"；刀②c-2 起也**不补文件**，两份都不动才是安全的那一档）
    store.write("", "kept.key", "v", "", "explicit").unwrap();
    run(&store, &files, &[]).unwrap();
    let global = md_root(&dir).join("global");
    std::fs::remove_dir_all(&global).unwrap();
    let sum2 = run(&store, &files, &[]).unwrap();
    assert_eq!(sum2.deleted, 0, "目录缺席不删行：{sum2:?}");
    assert_eq!(store.export_rows().unwrap().len(), 1);
}

#[test]
fn a_shadow_ahead_of_the_file_is_reported_but_never_written() {
    let (store, files, dir) = setup("shadow-ahead");
    store.write("", "k", "old", "", "explicit").unwrap();
    let rel = memlayout::fact_rel_path("", "k");
    let path = md_root(&dir).join(&rel);
    let before = std::fs::read(&path).unwrap();
    // 影子先行：库落笔而文件没跟上（标记仍 == mtime = 我们没写过这个文件的新形态）
    store
        .reconcile_upsert("", "k", "new value", "", "explicit", 1728200000)
        .unwrap();
    let sum = run(&store, &files, &[]).unwrap();
    assert_eq!(sum.warnings, 1, "只点名：{sum:?}");
    assert_eq!(
        (sum.created, sum.updated, sum.deleted),
        (0, 0, 0),
        "一个对齐动作都不许有：{sum:?}"
    );
    let after = std::fs::read(&path).unwrap();
    assert_eq!(
        before, after,
        "真源一个字节都不改（旧形状在这里会把 new value 写进文件）"
    );
    assert!(String::from_utf8_lossy(&after).contains("old"));
    // 再跑一轮：同一状态照样只点名，不许"看第二次就顺手补上"
    let sum2 = run(&store, &files, &[]).unwrap();
    assert_eq!(sum2.warnings, 1, "{sum2:?}");
    assert_eq!(std::fs::read(&path).unwrap(), before);
    // 负对照：同一文件推进 mtime（= 人动过）⇒ 走「文件为准」那格，
    // 证明上面命中"影子先行"靠的是标记而不是别的原因
    human_edit(&dir, &rel, "人写的版本");
    let sum3 = run(&store, &files, &[]).unwrap();
    assert_eq!(sum3.updated, 1, "人改过 ⇒ 文件为准：{sum3:?}");
    assert_eq!(sum3.warnings, 0, "{sum3:?}");
}

#[test]
fn project_file_without_frontmatter_workspace_resolves_via_dir() {
    let (store, files, dir) = setup("proj");
    let ws = "F:/ws/proj-a";
    let dir_name = memlayout::project_dir_name(ws);
    let facts = md_root(&dir).join("projects").join(&dir_name).join("facts");
    std::fs::create_dir_all(&facts).unwrap();
    // 人建文件：无 frontmatter，靠目录反查（known_workspaces 提供 fsRoots）
    std::fs::write(facts.join("bare.md"), "纯文本事实，无 frontmatter\n").unwrap();
    let sum = run(&store, &files, &[ws.to_string()]).unwrap();
    assert_eq!(sum.created, 1, "{sum:?}");
    let row = store
        .export_rows()
        .unwrap()
        .into_iter()
        .find(|r| r.key == "bare")
        .expect("按目录名反查到项目键位");
    assert_eq!(row.workspace, ws);
    assert_eq!(row.source, "human", "无 frontmatter source = human");
    // 反查不到的目录 = 跳过并计警告，不 panic（known 照传真实形态：
    // fsRoots ∪ sqlite 键位——bare.md 由此继续可解析）
    let unknown = md_root(&dir)
        .join("projects")
        .join("ghost-12345678")
        .join("facts");
    std::fs::create_dir_all(&unknown).unwrap();
    std::fs::write(unknown.join("x.md"), "---\nname: \"x\"\n---\n\nv\n").unwrap();
    let sum2 = run(&store, &files, &[ws.to_string()]).unwrap();
    assert_eq!(sum2.warnings, 1, "{sum2:?}");
    assert_eq!(sum2.created, 0, "bare.md 已同步零噪声");
}
