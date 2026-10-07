//! tree.md 对账单测：人改回写投影（打手动标记）/ 机器新文件跳过（零噪声）
//! / 结构改动忽略不删捆。

use std::path::Path;

use memory_tree::{ChapterId, MemoryChapter, MemoryTree, OrganizeRecord, TreeStore};

use super::run;
use crate::memory_files::MemoryFiles;

fn temp_root(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("mtree-rec-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn record() -> OrganizeRecord {
    OrganizeRecord {
        ts: 1,
        reason: "manual".into(),
        mode: "recorder".into(),
        status: "ok".into(),
        from: 0,
        to: 19,
        chapters: 1,
        manager_input: 100,
        manager_output: 50,
        duration_ms: 10,
    }
}

fn tree_with(ws: &str, sid: &str, ranges: &[(u64, u64)]) -> MemoryTree {
    let mut t = MemoryTree::new(ws, sid);
    let chapters = ranges
        .iter()
        .enumerate()
        .map(|(i, &(a, b))| {
            let title = format!("自动标题{}", i + 1);
            let summary = format!("自动摘要{}", i + 1);
            let draft = memory_tree::sanitize_draft(&title, &summary, (a as usize, b as usize))
                .unwrap();
            MemoryChapter::from_draft(ChapterId(format!("b{}", i + 1)), &draft, "recorder", 1)
        })
        .collect();
    t.apply_organized(chapters, ranges.last().map(|r| r.1).unwrap_or(0) as usize, record());
    t
}

/// 标准渲染格式的 tree.md（小节内容可由调用方改写后再落盘）。
fn md_text(rows: &[(&str, &str, &str, u64, u64, &str)]) -> String {
    let mut out = String::from("# 记忆树 · 测试\n\n> 由整理系统生成的人读层\n");
    for (sid, title, summary, a, b, source) in rows {
        out.push_str(&format!("\n## 会话{sid}\n\n### b1 · {title}\n\n{summary}\n\n"));
        out.push_str(&format!(
            "<!-- range: {a}-{b} · source: {source} · session: {sid} -->\n"
        ));
    }
    out
}

fn write_md(root: &Path, ws: &str, text: &str) -> std::path::PathBuf {
    let dir = root
        .join("memory")
        .join("projects")
        .join(plugin_api::memlayout::project_dir_name(ws));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("tree.md");
    std::fs::write(&p, text).unwrap();
    p
}

/// mtime 前拨（同毫秒双写会让"文件新于投影"判据抖动，测试里显式拉开门槛）。
fn bump_forward(path: &Path, secs: u64) {
    let f = std::fs::OpenOptions::new().write(true).open(path).unwrap();
    f.set_times(
        std::fs::FileTimes::new()
            .set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(secs)),
    )
    .unwrap();
}

#[test]
fn human_edits_flow_back_with_manual_flags() {
    let root = temp_root("human");
    let ws = "F:\\ws\\rectest-human";
    let store = TreeStore::new(&root);
    store.save(&tree_with(ws, "sess1", &[(10, 19)])).unwrap();

    let files = MemoryFiles::new(&root);
    let p = write_md(
        &root,
        ws,
        &md_text(&[("sess1", "人改标题", "人改摘要\n第二行", 10, 19, "recorder")]),
    );
    bump_forward(&p, 2);

    let sum = run(&store, &files, &[ws.to_string()]);
    assert_eq!(sum.titles, 1);
    assert_eq!(sum.summaries, 1);
    let t = store.load(ws, "sess1").unwrap();
    let b = t.chapters.values().next().unwrap();
    assert_eq!(b.title, "人改标题");
    assert!(b.title_edited_by_user);
    assert!(b.summary.contains("第二行"));
    assert!(b.summary_edited_by_user);
}

#[test]
fn machine_fresh_file_is_skipped() {
    let root = temp_root("fresh");
    let ws = "F:\\ws\\rectest-fresh";
    let store = TreeStore::new(&root);
    let files = MemoryFiles::new(&root);
    // 先写 md（人题样），投影后落 = 投影更新 → 机器写的，跳过
    let p = write_md(
        &root,
        ws,
        &md_text(&[("sess1", "不该吸呐的题", "不该吸呐的摘要", 10, 19, "recorder")]),
    );
    store.save(&tree_with(ws, "sess1", &[(10, 19)])).unwrap();
    let _ = p;

    let sum = run(&store, &files, &[ws.to_string()]);
    assert_eq!(sum.titles + sum.summaries, 0);
    let t = store.load(ws, "sess1").unwrap();
    let b = t.chapters.values().next().unwrap();
    assert_eq!(b.title, "自动标题1");
    assert!(!b.title_edited_by_user);
}

#[test]
fn structural_changes_are_ignored_not_deleted() {
    let root = temp_root("struct");
    let ws = "F:\\ws\\rectest-struct";
    let store = TreeStore::new(&root);
    store
        .save(&tree_with(ws, "sess1", &[(10, 14), (15, 19)]))
        .unwrap();
    let files = MemoryFiles::new(&root);
    // 人删了 b2 小节 + b1 的 range 被改坏：对账不删捆、坏区间忽略
    let mut text = md_text(&[("sess1", "人改标题", "摘要", 10, 14, "recorder")]);
    text.push_str("\n### b2 · 孤儿小节\n\n无指针\n\n<!-- range: 99-100 · source: rule · session: sess1 -->\n");
    let p = write_md(&root, ws, &text);
    bump_forward(&p, 2);

    let sum = run(&store, &files, &[ws.to_string()]);
    assert_eq!(sum.titles, 1); // 只有 b1 标题吸呐
    let t = store.load(ws, "sess1").unwrap();
    assert_eq!(t.chapters.len(), 2, "结构改动不删捆");
    let b1 = t.chapters.values().find(|b| b.chapter_id.0 == "b1").unwrap();
    assert_eq!(b1.title, "人改标题");
}

#[test]
fn parse_sections_reads_render_format() {
    let text = md_text(&[("s1", "标题·带点", "摘\n要", 3, 9, "rule")]);
    let secs = super::parse_sections(&text);
    assert_eq!(secs.len(), 1);
    let s = &secs[0];
    assert_eq!(s.title, "标题·带点");
    assert_eq!(s.summary, "摘\n要");
    assert_eq!(s.range, (3, 9));
    assert_eq!(s.session, "s1");
    // 无指针小节（被截断的文件）丢弃
    let bad = "# 记忆树\n\n### b1 · 无指针\n\n摘要\n";
    assert!(super::parse_sections(bad).is_empty());
}
