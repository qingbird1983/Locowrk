//! sub_transcript 单测：自身段写盘 / 覆盖重写 / 非法 id 跳过。

use super::*;
use session_store::load_session;
use session_store::SessionEvent::UserMessage;
use std::path::Path;

fn tmp_dir(tag: &str) -> PathBuf {
    crate::testutil::work_dir(&format!("lw-subtx-{}", tag))
}

/// 造一个会话：父前缀 1 条 + 自身段 2 条（任务 prompt + 回复）。
fn fixture() -> Session {
    let mut s = Session::new();
    s.append(UserMessage {
        content: "父的旧消息（fork 前缀）".into(),
        parts: Vec::new(),
    });
    s.append(UserMessage {
        content: "子任务 prompt".into(),
        parts: Vec::new(),
    });
    s.append(session_store::SessionEvent::AssistantMessage {
        content: "子完成".into(),
        tool_calls: Vec::new(),
    });
    s
}

#[test]
fn close_writes_own_segment_and_overwrites() {
    let dir = tmp_dir("write");
    let store = SubTranscriptStore::new(dir.clone());
    let child = fixture();
    store.close("p1-sub-1", 1, &child);

    let path = session_file(&dir, "p1-sub-1").unwrap();
    assert!(path.exists());
    let (loaded, skipped) = load_session(&path).unwrap();
    assert_eq!(skipped, 0);
    assert_eq!(loaded.len(), 2, "只写自身段（fork 前缀不带）");
    assert!(matches!(
        loaded.events().first(),
        Some(UserMessage { content, .. }) if content == "子任务 prompt"
    ));

    // 同 id 重落 = 覆盖（不残留旧行）
    store.close("p1-sub-1", 2, &child);
    let (loaded, _) = load_session(&path).unwrap();
    assert_eq!(loaded.len(), 1);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn close_skips_invalid_id() {
    let dir = tmp_dir("invalid");
    let store = SubTranscriptStore::new(dir.clone());
    let child = fixture();
    // 路径穿越 id：不落盘、不 panic
    store.close("../evil", 0, &child);
    assert!(!Path::new(&dir.join("evil.jsonl")).exists());
    assert!(!dir.join("evil.jsonl").exists());
    std::fs::remove_dir_all(&dir).ok();
}
