//! persist 模块测试：往返 / undo 截断重写 + 备份留痕 / 坏行容忍 / 列表。

use std::path::PathBuf;

use super::*;
use crate::event::SessionEvent;

fn tmp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("locowrk-persist-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn user_msg(content: &str) -> SessionEvent {
    SessionEvent::UserMessage {
        content: content.into(),
        parts: Vec::new(),
    }
}

#[test]
fn session_id_validation_and_roundtrip() {
    let dir = tmp_dir("roundtrip");
    // 非法 id（路径穿越 / 空）拒绝
    assert!(session_file(&dir, "../escape").is_err());
    assert!(session_file(&dir, "a/b").is_err());
    assert!(session_file(&dir, "").is_err());

    let path = session_file(&dir, "desktop-1").unwrap();
    append_event(&path, &user_msg("一")).unwrap();
    append_event(&path, &user_msg("二")).unwrap();
    append_event(&path, &user_msg("三")).unwrap();

    let (session, skipped) = load_session(&path).unwrap();
    assert_eq!(skipped, 0);
    assert_eq!(session.len(), 3);
    assert_eq!(session.cursor(), 3, "恢复后游标在末尾 = 停机时状态");
    assert!(
        matches!(session.events()[1], SessionEvent::UserMessage { ref content, .. } if content == "二")
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn dotted_session_id_round_trips() {
    // 回归锁（审计 M-23）：含 `.` 的会话 id 曾在 memory-tree 侧被拒、这边
    // 被收 = 「日志落盘成功、树投影静默失效」。规则单点化后两侧必须都认，
    // 且旁车仍落 `<id>.meta.json`（with_extension 只换 jsonl 那一段）。
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    // 目录名带纳秒并先清（历批 F11 教训：只带 pid 会读到上次运行残留）
    let dir = std::env::temp_dir().join(format!("locowrk-persist-dotted-{nanos}"));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();

    let id = "a.b_c-1";
    assert!(valid_session_id(id), "plugin-api 单点规则必须放行点号");
    let path = session_file(&dir, id).unwrap();
    append_event(&path, &user_msg("一")).unwrap();
    let (session, skipped) = load_session(&path).unwrap();
    assert_eq!(session.len(), 1, "带点 id 的日志读得回");
    assert_eq!(skipped, 0);

    write_title(&dir, id, "带点标题").unwrap();
    assert_eq!(
        read_meta(&dir, id).unwrap().title.as_deref(),
        Some("带点标题")
    );
    assert!(
        dir.join("a.b_c-1.meta.json").exists(),
        "旁车文件名只替换 jsonl 段，id 内的点不动"
    );

    // 收紧面照旧拒（全点串 / 穿越 / 空）
    assert!(session_file(&dir, "..").is_err(), "整串全点必须拒");
    assert!(session_file(&dir, ".").is_err());
    let long = "x".repeat(65);
    assert!(
        session_file(&dir, &long).is_err(),
        "超过 64 字符上限必须拒（新增判据）"
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn rewrite_truncates_and_leaves_backup() {
    let dir = tmp_dir("rewrite");
    let path = session_file(&dir, "s").unwrap();
    let mut full = Session::new();
    for i in 0..3 {
        full.append(user_msg(&format!("m{i}")));
    }
    rewrite_log(&path, &full).unwrap();
    assert_eq!(load_session(&path).unwrap().0.len(), 3);

    // undo 语义：fork 截断到 1 条 → 重写后磁盘 1 行，备份保留 3 行原文
    full.rewind_to(0);
    let mut truncated = full.fork(0);
    truncated.append(user_msg("新轮"));
    rewrite_log(&path, &truncated).unwrap();

    let (restored, skipped) = load_session(&path).unwrap();
    assert_eq!(restored.len(), 1);
    assert_eq!(skipped, 0, "被撤销的事件经重写移出主文件，不留在坏行里");
    let bak = path.with_file_name("s.jsonl.undo.bak");
    assert!(bak.exists(), "重写前必须滚动备份");
    assert_eq!(load_session(&bak).unwrap().0.len(), 3);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn load_tolerates_bad_lines() {
    let dir = tmp_dir("badline");
    let path = session_file(&dir, "s").unwrap();
    append_event(&path, &user_msg("好一")).unwrap();
    // 模拟崩溃半行 / 手改损坏
    use std::io::Write;
    std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(b"{not-json\n")
        .unwrap();
    append_event(&path, &user_msg("好二")).unwrap();

    let (session, skipped) = load_session(&path).unwrap();
    assert_eq!(session.len(), 2, "前缀事件完整可用");
    assert_eq!(skipped, 1);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn list_sessions_orders_by_mtime_desc() {
    let dir = tmp_dir("list");
    let early = session_file(&dir, "a-early").unwrap();
    append_event(&early, &user_msg("早")).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(30));
    let late = session_file(&dir, "b-late").unwrap();
    append_event(&late, &user_msg("晚")).unwrap();
    // 非法 id 文件不入列
    std::fs::write(dir.join("../x.jsonl"), "").ok();
    std::fs::write(dir.join("bad id.jsonl"), "").ok();
    std::fs::write(dir.join("note.txt"), "").ok();

    let metas = list_sessions(&dir);
    let ids: Vec<&str> = metas.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(ids, vec!["b-late", "a-early"], "mtime 降序");
    assert_eq!(metas[1].events, 1);
    assert_eq!(metas[1].title.as_deref(), Some("早"), "首条用户消息做标题");
    assert!(metas[0].updated_at >= metas[1].updated_at);

    // 目录不存在 = 空表不报错
    assert!(list_sessions(&dir.join("absent")).is_empty());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn meta_title_overrides_list_and_resets() {
    let dir = tmp_dir("meta");
    let path = session_file(&dir, "s1").unwrap();
    append_event(&path, &user_msg("第一封信")).unwrap();
    // 无旁车：自动标题 = 首条用户消息
    assert_eq!(list_sessions(&dir)[0].title.as_deref(), Some("第一封信"));
    // 重命名覆盖（trim）
    write_title(&dir, "s1", "  新标题  ").unwrap();
    assert_eq!(list_sessions(&dir)[0].title.as_deref(), Some("新标题"));
    assert!(read_meta(&dir, "s1").unwrap().title.is_some());
    // 空串 = 删旁车回落自动标题
    write_title(&dir, "s1", "   ").unwrap();
    assert!(read_meta(&dir, "s1").is_none());
    assert_eq!(list_sessions(&dir)[0].title.as_deref(), Some("第一封信"));
    // 非法 id 拒绝
    assert!(write_title(&dir, "../x", "t").is_err());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn meta_organizer_key_is_phantom_on_read() {
    let dir = tmp_dir("organizer");
    let path = session_file(&dir, "s1").unwrap();
    append_event(&path, &user_msg("hello")).unwrap();
    // v2.137 退役键：旧旁车里的存量 `organizer` 照常解析（serde 忽略），
    // 列表投影不再透出
    std::fs::write(
        dir.join("s1.meta.json"),
        r#"{"organizer":"legacy","title":"旧会话"}"#,
    )
    .unwrap();
    let meta = read_meta(&dir, "s1").expect("旧旁车读得进");
    assert_eq!(meta.title.as_deref(), Some("旧会话"));
    assert!(
        serde_json::to_value(&list_sessions(&dir)[0])
            .unwrap()
            .get("organizer")
            .is_none()
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn delete_session_removes_all_files_and_is_idempotent() {
    let dir = tmp_dir("delete");
    let path = session_file(&dir, "s2").unwrap();
    append_event(&path, &user_msg("hello")).unwrap();
    write_title(&dir, "s2", "t").unwrap();
    // 造一份 undo 备份（rewrite_log 对已存在文件滚动备份）
    let mut full = Session::new();
    full.append(user_msg("x"));
    rewrite_log(&path, &full).unwrap();
    assert!(dir.join("s2.jsonl.undo.bak").exists());

    assert!(delete_session(&dir, "s2").unwrap());
    assert!(!session_file(&dir, "s2").unwrap().exists());
    assert!(!dir.join("s2.meta.json").exists());
    assert!(!dir.join("s2.jsonl.undo.bak").exists());
    assert!(list_sessions(&dir).iter().all(|m| m.id != "s2"));
    // 幂等：再删 Ok(false)
    assert!(!delete_session(&dir, "s2").unwrap());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn search_finds_content_with_snippet_and_respects_limit() {
    let dir = tmp_dir("search");
    for (id, msg) in [
        ("a", " discussion about rust internals "),
        ("b", "完全无关的中文内容"),
        ("c", "another RUST talk and more RUST notes"),
    ] {
        let p = session_file(&dir, id).unwrap();
        append_event(&p, &user_msg(msg)).unwrap();
    }
    let hits = search_sessions(&dir, "RUST", 10);
    assert_eq!(hits.len(), 2, "大小写不敏感，两个会话命中");
    assert!(
        hits.iter()
            .all(|h| h.snippet.to_lowercase().contains("ust"))
    );
    let c = hits.iter().find(|h| h.id == "c").unwrap();
    assert_eq!(c.match_count, 2, "两行消息各命中一次");
    let limited = search_sessions(&dir, "rust", 1);
    assert_eq!(limited.len(), 1);
    assert_eq!(search_sessions(&dir, "  ", 10).len(), 0);
    assert_eq!(search_sessions(&dir, "不存在的词", 10).len(), 0);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn workspace_association_roundtrip_and_clear() {
    let dir = tmp_dir("ws");
    let path = session_file(&dir, "w1").unwrap();
    append_event(&path, &user_msg("关联前的消息")).unwrap();
    // 未关联：列表行 workspace = None
    assert_eq!(list_sessions(&dir)[0].workspace, None);
    // 关联 → 列表行透出；标题字段不受影响
    write_workspace(&dir, "w1", Some("F:\\Warehouse\\AIwork\\Locowrk")).unwrap();
    let row = &list_sessions(&dir)[0];
    assert_eq!(
        row.workspace.as_deref(),
        Some("F:\\Warehouse\\AIwork\\Locowrk")
    );
    assert_eq!(row.title.as_deref(), Some("关联前的消息"));
    // trim + 空串视同清除
    write_workspace(&dir, "w1", Some("  ")).unwrap();
    assert_eq!(list_sessions(&dir)[0].workspace, None);
    // 两字段全空 → 旁车文件删除（回落全自动）
    assert!(read_meta(&dir, "w1").is_none());
    // 非法 id 拒绝
    assert!(write_workspace(&dir, "../x", Some("F:/ws")).is_err());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn rename_to_empty_preserves_workspace() {
    let dir = tmp_dir("ws-title");
    let path = session_file(&dir, "w2").unwrap();
    append_event(&path, &user_msg("自动标题")).unwrap();
    write_workspace(&dir, "w2", Some("F:/ws")).unwrap();
    write_title(&dir, "w2", "手起的名").unwrap();
    // 改名与关联并存（合并写入）
    let meta = read_meta(&dir, "w2").unwrap();
    assert_eq!(meta.title.as_deref(), Some("手起的名"));
    assert_eq!(meta.workspace.as_deref(), Some("F:/ws"));
    // 空标题只清标题字段——workspace 保留（v2.49 会整文件删除，v2.50 修）
    write_title(&dir, "w2", "  ").unwrap();
    let meta = read_meta(&dir, "w2").unwrap();
    assert_eq!(meta.title, None);
    assert_eq!(meta.workspace.as_deref(), Some("F:/ws"));
    assert_eq!(list_sessions(&dir)[0].workspace.as_deref(), Some("F:/ws"));
    std::fs::remove_dir_all(&dir).ok();
}
