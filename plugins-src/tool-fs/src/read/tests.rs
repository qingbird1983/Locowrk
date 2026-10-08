//! fs_read 单测（G3 Revision Guard 面）：revision 覆盖全量内容、与截断
//! 窗口解耦；CRLF/LF 归一口径。涉 env 用例串行（锁在 crate::testutil，
//! 与 write 模块共用）。

use std::time::{SystemTime, UNIX_EPOCH};

use super::fs_read;
use crate::testutil::ENV_LOCK;
use plugin_api::fsutil::content_revision;
use plugin_api::serde_json::json;

fn temp_root(tag: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "locowrk-tool-fs-read-{tag}-{}-{nanos}",
        std::process::id()
    ))
}

#[test]
fn read_revision_covers_full_content_beyond_truncation() {
    let _g = ENV_LOCK.lock().unwrap();
    let root = temp_root("rev");
    unsafe { std::env::set_var("LOCOWRK_DATA_DIR", &root) };
    std::fs::create_dir_all(&root).unwrap();
    let file = root.join("big.txt");
    // 100KB > 32KiB 缺省窗口 → truncated，但 revision 仍覆盖全量
    let big = format!("{}\n", "x".repeat(100_000));
    std::fs::write(&file, &big).unwrap();

    let out = fs_read(
        &json!({ "path": file.display().to_string() }),
        std::ptr::null(),
    )
    .expect("read ok");
    assert_eq!(out["truncated"], json!(true));
    assert_eq!(out["revision"], json!(content_revision(big.as_bytes())));

    // CRLF ↔ LF 归一：换行风格漂移不换 revision（与编辑容错同口径）
    let p = file.display().to_string();
    std::fs::write(&file, "a\nb\n").unwrap();
    let out = fs_read(&json!({ "path": p }), std::ptr::null()).unwrap();
    let rev_lf = out["revision"].as_str().unwrap().to_owned();
    std::fs::write(&file, "a\r\nb\r\n").unwrap();
    let out = fs_read(&json!({ "path": p }), std::ptr::null()).unwrap();
    assert_eq!(out["revision"].as_str(), Some(rev_lf.as_str()));
    // 其余内容差异照常换 revision
    std::fs::write(&file, "a\nb \n").unwrap();
    let out = fs_read(&json!({ "path": p }), std::ptr::null()).unwrap();
    assert_ne!(out["revision"].as_str(), Some(rev_lf.as_str()));

    unsafe { std::env::remove_var("LOCOWRK_DATA_DIR") };
    let _ = std::fs::remove_dir_all(&root);
}
