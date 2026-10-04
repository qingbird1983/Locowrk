//! edit_diff 单测：正常（多 hunk / 增删改 / 尾换行保留）/ 边界（头行
//! 容忍、\ No newline 跳过）/ 恶意（上下文不匹配整体拒绝、越界、
//! old_start 0、hunk 外内容、重叠）。

use std::path::Path;

use super::edit_diff;
use crate::testutil::ENV_LOCK;
use plugin_api::HostVtable;
use plugin_api::serde_json::json;

/// 直呼函数的单测没有调用期 vtable——空指针 = 回落 env 全局围栏（与
/// 未接围栏前的语义一致）。
fn no_host() -> *const HostVtable {
    std::ptr::null()
}

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

fn write_file(root: &Path, name: &str, content: &str) -> String {
    std::fs::create_dir_all(root).unwrap();
    let p = root.join(name);
    std::fs::write(&p, content).unwrap();
    p.display().to_string()
}

#[test]
fn multi_hunk_apply_with_trailing_newline() {
    let _g = ENV_LOCK.lock().unwrap();
    let root = temp_root("apply");
    unsafe { std::env::set_var("LOCOWRK_DATA_DIR", &root) };
    let path = write_file(
        &root,
        "code.rs",
        "fn a() {}\nfn b() {}\nfn c() {}\nfn d() {}\nfn e() {}\n",
    );
    let patch = "\
--- a/code.rs
+++ b/code.rs
@@ -1,3 +1,3 @@ fn header
 fn a() {}
-fn b() {}
+fn b_changed() {}
 fn c() {}
@@ -5 +5,2 @@ tail
 fn e() {}
+fn f() {}
";
    let out = edit_diff(&json!({ "path": path, "patch": patch }), no_host()).expect("apply ok");
    assert_eq!(out["hunks_applied"], json!(2));
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "fn a() {}\nfn b_changed() {}\nfn c() {}\nfn d() {}\nfn e() {}\nfn f() {}\n"
    );
    unsafe { std::env::remove_var("LOCOWRK_DATA_DIR") };
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn mismatch_rejects_whole_patch_without_touching_file() {
    let _g = ENV_LOCK.lock().unwrap();
    let root = temp_root("mismatch");
    unsafe { std::env::set_var("LOCOWRK_DATA_DIR", &root) };
    let original = "alpha\nbeta\ngamma\n";
    let path = write_file(&root, "f.txt", original);
    let patch = "\
--- a/f.txt
+++ b/f.txt
@@ -2 +2 @@
-beta
+BETA
";
    // 恶意：上下文行与文件不符 → 整体拒绝、文件未动
    let bad_context = "\
--- a/f.txt
+++ b/f.txt
@@ -2 +2 @@
-delta
+BETA
";
    let err = edit_diff(&json!({ "path": path, "patch": bad_context }), no_host()).unwrap_err();
    assert!(err.contains("expected"), "got: {err}");
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        original,
        "文件未动"
    );

    // 正确 patch 应用成功
    edit_diff(&json!({ "path": path, "patch": patch }), no_host()).expect("正确 patch 应用成功");

    unsafe { std::env::remove_var("LOCOWRK_DATA_DIR") };
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn malformed_patches_are_rejected() {
    // 无 hunk
    assert!(
        edit_diff(
            &json!({ "path": "/x", "patch": "--- a\n+++ b\n" }),
            no_host()
        )
        .is_err()
    );
    // old_start 0（建新文件）
    let err = edit_diff(
        &json!({ "path": "/x", "patch": "@@ -0,0 +1,1 @@\n+x\n" }),
        no_host(),
    )
    .unwrap_err();
    assert!(err.contains("not supported"), "got: {err}");
    // hunk 外内容
    let err = edit_diff(
        &json!({ "path": "/x", "patch": "hello\n@@ -1 +1 @@\n" }),
        no_host(),
    )
    .unwrap_err();
    assert!(err.contains("outside a hunk"), "got: {err}");
    // 越界起始
    let err = edit_diff(
        &json!({ "path": "/x", "patch": "@@ -9 +9 @@\n+x\n" }),
        no_host(),
    )
    .is_err();
    assert!(err);
    // \\ No newline 标记不炸
    let parsed = super::parse_patch(
        "@@ -1 +1 @@\n-a\n\\ No newline at end of file\n+a\n\\ No newline at end of file\n",
    );
    assert_eq!(parsed.unwrap()[0].lines.len(), 2);
}
