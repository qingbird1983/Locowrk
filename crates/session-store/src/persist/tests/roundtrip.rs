//! 落盘往返面（自 `tests.rs` 批 F-23 拆出，用例本体逐字未改）：会话 id 白名单与
//! 追加-恢复往返 / 带点 id / undo 截断重写与滚动备份 / 坏行容忍。

use super::*;

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
