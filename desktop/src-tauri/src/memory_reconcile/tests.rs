//! memory_reconcile 单测：文件为准回写 / 新增吸纳 / 人删文件删行 /
//! 索引先行愈合 / 目录缺席不删行。全部走临时目录。
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

/// 开着文件层的宿主存储（sqlite 与 md 同根）。
fn setup(tag: &str) -> (MemoryStore, MemoryFiles, PathBuf) {
    let dir = temp_dir(tag);
    std::fs::create_dir_all(&dir).unwrap();
    let files = MemoryFiles::new(&dir);
    let store = MemoryStore::new(&dir, Arc::new(|_| None))
        .with_files(Arc::new(files.clone()), Arc::new(|| true));
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
        "---\nname: \"hand.made\"\ndescription: \"人建\"\nmetadata:\n  type: fact\n  source: \"explicit\"\n  workspace: \"\"\n  updated: 1728100000\n---\n\n我手写的事实\n",
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
}

#[test]
fn deleted_file_removes_row_but_missing_dir_keeps_rows() {
    let (store, files, dir) = setup("delete");
    store.write("", "gone.key", "v", "", "explicit").unwrap();
    let rel = memlayout::fact_rel_path("", "gone.key");
    // 单文件删除（标记在 = 我们写过）→ 行删
    std::fs::remove_file(md_root(&dir).join(&rel)).unwrap();
    let sum = run(&store, &files, &[]).unwrap();
    assert_eq!(sum.deleted, 1, "{sum:?}");
    assert!(store.export_rows().unwrap().is_empty());
    // 整层缺席 = 复位手势：行保留（首启导出按索引重建文件）
    store.write("", "kept.key", "v", "", "explicit").unwrap();
    let global = md_root(&dir).join("global");
    std::fs::remove_dir_all(&global).unwrap();
    let sum2 = run(&store, &files, &[]).unwrap();
    assert_eq!(sum2.deleted, 0, "目录缺席不删行：{sum2:?}");
    assert_eq!(store.export_rows().unwrap().len(), 1);
}

#[test]
fn stale_file_is_healed_from_index() {
    let (store, files, dir) = setup("heal");
    store.write("", "k", "old", "", "explicit").unwrap();
    let rel = memlayout::fact_rel_path("", "k");
    // 索引先行（模拟 md 同步失败的历史）：行更新而文件未动（标记仍 == mtime）
    store
        .reconcile_upsert("", "k", "new value", "", "explicit", 1728200000)
        .unwrap();
    let sum = run(&store, &files, &[]).unwrap();
    assert_eq!(sum.healed, 1, "{sum:?}");
    let text = std::fs::read_to_string(md_root(&dir).join(&rel)).unwrap();
    assert!(text.contains("new value"), "文件从索引愈合");
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
