//! memory_files 单测：tree.md 渲染（数值序/占位剔除/会话分组）、人改
//! 保护（文件新于投影不覆盖）、首启导出幂等（人编辑不被覆盖）。

use std::sync::Arc;

use memory_tree::{ChapterDraft, MemoryTree, OrganizeRecord, TreeStore};
use session_store::{Session, SessionEvent};

use super::*;
use crate::memory::MemoryStore;

fn tmp_dir(tag: &str) -> std::path::PathBuf {
    crate::testutil::work_dir(&format!("mfiles-{}", tag))
}

fn resolver() -> crate::workspace::WorkspaceResolver {
    Arc::new(|_: &str| None)
}

/// 两章节实树（b1 区间 0–3、b2 区间 4–9，草稿倒序给出以证渲染按章节号排序）。
fn real_tree(ws: &str, sid: &str) -> MemoryTree {
    let mut t = MemoryTree::new(ws, sid);
    let drafts = vec![
        ChapterDraft {
            title: "第一章节标题".into(),
            summary: "摘要一".into(),
            event_range: (0, 3),
        },
        ChapterDraft {
            title: "第二章节标题".into(),
            summary: "摘要二".into(),
            event_range: (4, 9),
        },
    ];
    let chapters = t.build_chapters(&drafts, "recorder", 1000);
    t.apply_organized(
        chapters,
        9,
        OrganizeRecord {
            ts: 1000,
            reason: "manual".into(),
            mode: "recorder".into(),
            status: "ok".into(),
            from: 0,
            to: 9,
            chapters: 2,
            manager_input: 0,
            manager_output: 0,
            duration_ms: 0,
        },
        None,
    );
    t
}

#[test]
fn render_tree_md_groups_sessions_and_skips_pending() {
    let ws = r"F:\proj\Render";
    let mut t = real_tree(ws, "s1");
    t.insert_placeholder(10, 12, 2000).expect("占位章节");
    let md = render_tree_md(ws, &[("s1".into(), "我的会话".into(), &t)]);
    assert!(
        md.contains("# 记忆树 · Render-"),
        "标题带项目 md 目录名：{md}"
    );
    assert!(md.contains("## 我的会话"), "会话小节（旁车标题）");
    assert!(md.contains("### b1 · 第一章节标题"));
    assert!(md.contains("### b2 · 第二章节标题"));
    assert!(!md.contains("b3"), "占位章节不进人读层：{md}");
    assert!(md.contains("<!-- range: 0-3 · source: recorder · session: s1 -->"));
    // 无标题会话回落 sid
    let md2 = render_tree_md(ws, &[("s9".into(), String::new(), &t)]);
    assert!(md2.contains("## s9"));
    // 全占位 = 不产出小节
    let mut only_pending = MemoryTree::new(ws, "s2");
    only_pending.insert_placeholder(0, 4, 3000).unwrap();
    let md3 = render_tree_md(ws, &[("s2".into(), "空".into(), &only_pending)]);
    assert!(!md3.contains("###"), "全占位不渲染章节节");
}

#[test]
fn sync_writes_tree_md_and_guards_human_edits() {
    let root = tmp_dir("sync");
    let files = MemoryFiles::new(&root);
    let store = TreeStore::new(&root);
    let sessions_dir = root.join("sessions");
    std::fs::create_dir_all(&sessions_dir).unwrap();

    // 无实章节（纯占位/无投影）= 跳过
    let mut pending_only = MemoryTree::new(r"F:\proj\Sync", "s-empty");
    pending_only.insert_placeholder(0, 4, 1000).unwrap();
    store.save(&pending_only).unwrap();
    assert!(!sync_workspace_tree_md(&files, &store, &sessions_dir, r"F:\proj\Sync").unwrap());

    store.save(&real_tree(r"F:\proj\Sync", "s1")).unwrap();
    // 旁车标题（v2.57 重命名同路径）
    let mut session = Session::new();
    session.append(SessionEvent::UserMessage {
        content: "hi".into(),
        parts: Vec::new(),
    });
    crate::session_log::append_slice(&sessions_dir, "s1", &session, 0);
    crate::session_log::rename(&sessions_dir, "s1", "会话标题甲").unwrap();

    assert!(sync_workspace_tree_md(&files, &store, &sessions_dir, r"F:\proj\Sync").unwrap());
    let path = root
        .join("memory")
        .join("projects")
        .join(plugin_api::memlayout::project_dir_name(r"F:\proj\Sync"))
        .join("tree.md");
    assert!(path.exists());
    assert!(std::fs::read_to_string(&path)
        .unwrap()
        .contains("## 会话标题甲"));

    // 人改保护：文件比投影新 → 跳过重建
    std::thread::sleep(std::time::Duration::from_millis(30));
    std::fs::write(&path, "HUMAN EDIT").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(30));
    assert!(!sync_workspace_tree_md(&files, &store, &sessions_dir, r"F:\proj\Sync").unwrap());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "HUMAN EDIT");

    // 投影再次更新（新于文件）→ 恢复重建
    std::thread::sleep(std::time::Duration::from_millis(30));
    store.save(&real_tree(r"F:\proj\Sync", "s1")).unwrap();
    assert!(sync_workspace_tree_md(&files, &store, &sessions_dir, r"F:\proj\Sync").unwrap());
    assert!(std::fs::read_to_string(&path).unwrap().contains("### b1"));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn project_dir_is_the_handle_view_of_the_same_formula_and_holds_tree_md() {
    // 队列 Q15 第一刀：`memory_reveal_project` 走 MemoryFiles::project_dir，
    // 而它必须与 tree.md 的实际落点**同一个目录**——否则按钮打开的是空文件夹，
    // 用户看到的就是"我的记忆呢"。期望路径手抄，不拿被测函数自证。
    let root = tmp_dir("projdir");
    let files = MemoryFiles::new(&root);
    let ws = r"F:\proj\Reveal";
    let expect = root
        .join("memory")
        .join("projects")
        .join(plugin_api::memlayout::project_dir_name(ws));
    assert_eq!(files.project_dir(ws).unwrap(), expect);
    assert!(
        files.project_dir("").is_err(),
        "未归属（看板下拉末位）没有项目目录：按钮要报错而不是打开一个空壳"
    );

    let store = TreeStore::new(&root);
    store.save(&real_tree(ws, "s1")).unwrap();
    let sessions_dir = root.join("sessions");
    std::fs::create_dir_all(&sessions_dir).unwrap();
    assert!(sync_workspace_tree_md(&files, &store, &sessions_dir, ws).unwrap());
    let names: Vec<String> = std::fs::read_dir(files.project_dir(ws).unwrap())
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        names.contains(&"tree.md".to_string()),
        "tree.md 住在家目录里: {names:?}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn initial_export_skips_existing_files_and_regenerates_trees() {
    let root = tmp_dir("export");
    let files = MemoryFiles::new(&root);
    let store = TreeStore::new(&root);
    let sessions_dir = root.join("sessions");
    std::fs::create_dir_all(&sessions_dir).unwrap();
    let memory = MemoryStore::new(&root, resolver());
    memory
        .write("", "pref.pnpm", "always pnpm", "", "explicit")
        .unwrap();
    memory
        .write(r"F:\proj\Exp", "db.choice", "sqlite", "", "explicit")
        .unwrap();
    store.save(&real_tree(r"F:\proj\Exp", "s1")).unwrap();

    // 人在 sqlite 之外已有一份编辑（同键位）——导出不得覆盖
    let human = root.join("memory").join("global").join("pref.pnpm.md");
    std::fs::create_dir_all(human.parent().unwrap()).unwrap();
    std::fs::write(&human, "HUMAN EDIT").unwrap();

    let (facts, trees) = run_initial_export(&memory, &store, &files, &sessions_dir).unwrap();
    assert_eq!((facts, trees), (1, 1), "全局键被跳过 = 只导出项目事实");
    assert_eq!(
        std::fs::read_to_string(&human).unwrap(),
        "HUMAN EDIT",
        "人编辑不动"
    );
    let exported = root
        .join("memory")
        .join("projects")
        .join(plugin_api::memlayout::project_dir_name(
            // v2.186：宿主 `write()` 在入口归一键位，md 目录名跟着归一形走
            // （`write_fact` 拿的是 `write` 里同一个串）。目录名必须由**同一个
            // 归一串**算出来，否则这条判据钉的是"原始串的落点"而不是生产落点。
            &crate::workspace::scope_key(Some(r"F:\proj\Exp")),
        ))
        .join("facts")
        .join("db.choice.md");
    assert!(exported.exists());

    // 幂等：重跑零新增（含 tree.md 已在、投影无更新）
    let (facts2, trees2) = run_initial_export(&memory, &store, &files, &sessions_dir).unwrap();
    assert_eq!((facts2, trees2), (0, 0), "重跑幂等");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn render_tree_md_places_run_summary_above_its_chapter_group() {
    let mut t = real_tree("ws", "s-sum");
    t.summaries.push(memory_tree::RunSummary {
        from: 0,
        to: 9,
        text: "第一段总结。\n\n第二段总结。".into(),
        created_at: 1000,
    });
    let md = render_tree_md("ws", &[("s-sum".into(), "会话题".into(), &t)]);
    let sum_pos = md.find("段落总结").expect("总结块在场");
    let b1_pos = md.find("### b1").expect("b1 在场");
    let b2_pos = md.find("### b2").expect("b2 在场");
    // 字面上层：总结块在该轮全部章节组之前
    assert!(sum_pos < b1_pos && sum_pos < b2_pos);
    assert!(md.contains("> 📋 **段落总结**（事件 0-9）"));
    assert!(md.contains("> 第一段总结。"));
    assert!(md.contains("> 第二段总结。"));
}
