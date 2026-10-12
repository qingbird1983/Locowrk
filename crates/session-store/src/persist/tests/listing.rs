//! 列表与旁车面（自 `tests.rs` 批 F-23 拆出，用例本体逐字未改）：mtime 降序与非法 id
//! 不入列 / 旁车标题覆盖与回落 / v2.137 退役键只读不透出 / 删除会话清全部痕迹且幂等 /
//! 工作区关联往返与清除 / 空标题不清工作区。

use super::*;

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
