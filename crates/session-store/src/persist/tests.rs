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
        matches!(session.events()[1], SessionEvent::UserMessage { ref content } if content == "二")
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
