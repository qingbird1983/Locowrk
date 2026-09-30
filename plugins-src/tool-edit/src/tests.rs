//! edit_replace / edit_lines 单测：正常 / 边界（多重命中、越界）/ 恶意
//! （根外路径拒绝）三态。涉 env 用例串行。

use super::lines::edit_lines;
use super::replace::edit_replace;
use crate::testutil::ENV_LOCK;
use plugin_api::serde_json::json;

fn temp_root(tag: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "locowrk-tool-edit-{tag}-{}-{nanos}",
        std::process::id()
    ))
}

#[test]
fn replace_matrix() {
    let _g = ENV_LOCK.lock().unwrap();
    let root = temp_root("replace");
    unsafe { std::env::set_var("LOCOWRK_DATA_DIR", &root) };
    let path = root.join("app.rs");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(&path, "fn main() {\n    todo!()\n}\n").unwrap();
    let p = path.display().to_string();

    // 正常：唯一命中替换 + 备份产出
    let out = edit_replace(
        &json!({ "path": p, "old_string": "todo!()", "new_string": "println!(\"hi\");" }),
    )
    .expect("replace ok");
    assert_eq!(out["edits"], json!(1));
    assert!(out["backup"].as_str().is_some());
    assert!(std::fs::read_to_string(&path).unwrap().contains("println!"));

    // 边界：多重命中必须 replace_all
    std::fs::write(&path, "x\nx\n").unwrap();
    let err =
        edit_replace(&json!({ "path": p, "old_string": "x", "new_string": "y" })).unwrap_err();
    assert!(err.contains("matches 2 times"), "got: {err}");
    let out = edit_replace(
        &json!({ "path": p, "old_string": "x", "new_string": "y", "replace_all": true }),
    )
    .unwrap();
    assert_eq!(out["edits"], json!(2));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "y\ny\n");

    // 恶意：old 不存在 / 空 old / 根外路径
    assert!(edit_replace(&json!({ "path": p, "old_string": "nope", "new_string": "y" })).is_err());
    assert!(edit_replace(&json!({ "path": p, "old_string": "", "new_string": "y" })).is_err());
    let outside = temp_root("elsewhere").join("s.txt");
    assert!(
        edit_replace(&json!({ "path": outside.display().to_string(),
        "old_string": "a", "new_string": "b" }))
        .is_err()
    );

    unsafe { std::env::remove_var("LOCOWRK_DATA_DIR") };
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn lines_matrix() {
    let _g = ENV_LOCK.lock().unwrap();
    let root = temp_root("lines");
    unsafe { std::env::set_var("LOCOWRK_DATA_DIR", &root) };
    let path = root.join("list.txt");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(&path, "one\ntwo\nthree\nfour\n").unwrap();
    let p = path.display().to_string();

    // 正常：2..3 替换为两行
    let out = edit_lines(&json!({ "path": p, "start_line": 2, "end_line": 3,
        "content": "TWO-A\nTWO-B" }))
    .expect("lines ok");
    assert_eq!(out["replaced"], json!(2));
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "one\nTWO-A\nTWO-B\nfour\n"
    );

    // 边界：空 content = 删除区间；尾换行保持
    edit_lines(&json!({ "path": p, "start_line": 2, "end_line": 3, "content": "" })).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "one\nfour\n");
    // 单文件行校验
    let err = edit_lines(&json!({ "path": p, "start_line": 5, "end_line": 6, "content": "x" }))
        .unwrap_err();
    assert!(err.contains("beyond the last line"), "got: {err}");
    assert!(
        edit_lines(&json!({ "path": p, "start_line": 3, "end_line": 2, "content": "x" })).is_err()
    );

    // 恶意：根外拒绝
    let outside = temp_root("elsewhere2").join("t.txt");
    assert!(
        edit_lines(&json!({ "path": outside.display().to_string(),
            "start_line": 1, "end_line": 1, "content": "x" }))
        .is_err()
    );

    unsafe { std::env::remove_var("LOCOWRK_DATA_DIR") };
    let _ = std::fs::remove_dir_all(&root);
}
