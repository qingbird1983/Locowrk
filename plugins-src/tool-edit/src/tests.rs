//! edit_replace / edit_lines 单测：正常 / 边界（多重命中、越界）/ 恶意
//! （根外路径拒绝）三态。涉 env 用例串行。

use super::lines::edit_lines;
use super::replace::edit_replace;
use crate::testutil::ENV_LOCK;
use plugin_api::HostVtable;
use plugin_api::serde_json::json;

/// 直呼函数的单测没有调用期 vtable——空指针 = `context_roots` 返回
/// None，回落 env 全局围栏（与未接围栏前的语义一致）。
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
        no_host(),
    )
    .expect("replace ok");
    assert_eq!(out["edits"], json!(1));
    assert!(out["backup"].as_str().is_some());
    assert!(std::fs::read_to_string(&path).unwrap().contains("println!"));

    // 边界：多重命中必须 replace_all
    std::fs::write(&path, "x\nx\n").unwrap();
    let err = edit_replace(
        &json!({ "path": p, "old_string": "x", "new_string": "y" }),
        no_host(),
    )
    .unwrap_err();
    assert!(err.contains("matches 2 times"), "got: {err}");
    let out = edit_replace(
        &json!({ "path": p, "old_string": "x", "new_string": "y", "replace_all": true }),
        no_host(),
    )
    .unwrap();
    assert_eq!(out["edits"], json!(2));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "y\ny\n");

    // 恶意：old 不存在 / 空 old / 根外路径
    assert!(
        edit_replace(
            &json!({ "path": p, "old_string": "nope", "new_string": "y" }),
            no_host()
        )
        .is_err()
    );
    assert!(
        edit_replace(
            &json!({ "path": p, "old_string": "", "new_string": "y" }),
            no_host()
        )
        .is_err()
    );
    let outside = temp_root("elsewhere").join("s.txt");
    assert!(
        edit_replace(
            &json!({ "path": outside.display().to_string(),
            "old_string": "a", "new_string": "b" }),
            no_host()
        )
        .is_err()
    );

    unsafe { std::env::remove_var("LOCOWRK_DATA_DIR") };
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn replace_fuzzy_matrix() {
    let _g = ENV_LOCK.lock().unwrap();
    let root = temp_root("fuzzy");
    unsafe { std::env::set_var("LOCOWRK_DATA_DIR", &root) };
    let path = root.join("win.rs");
    std::fs::create_dir_all(&root).unwrap();
    let p = path.display().to_string();

    // 精确优先：完全一致时 match_type=exact
    std::fs::write(&path, "aaa\n").unwrap();
    let out = edit_replace(
        &json!({ "path": p, "old_string": "aaa", "new_string": "b" }),
        no_host(),
    )
    .unwrap();
    assert_eq!(out["match_type"], json!("exact"));
    assert_eq!(out["byte_range"], json!([0, 3]));

    // CRLF 文件 + LF old_string → 模糊命中；new_string 行尾适配成 CRLF
    std::fs::write(&path, "fn a() {\r\n    old();\r\n}\r\n").unwrap();
    let out = edit_replace(
        &json!({ "path": p, "old_string": "fn a() {\n    old();\n}",
                 "new_string": "fn a() {\n    new();\n}" }),
        no_host(),
    )
    .unwrap();
    assert_eq!(out["match_type"], json!("fuzzy"));
    assert_eq!(out["edits"], json!(1));
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "fn a() {\r\n    new();\r\n}\r\n"
    );

    // 行尾空白：old 跨行、文件行带尾随空格 → 精确失败、模糊命中；
    // 尾随空格属于命中区域的「不可见尾部」，随替换一起被换掉
    std::fs::write(&path, "let x = 1;   \nlet y = 2;\n").unwrap();
    let out = edit_replace(
        &json!({ "path": p, "old_string": "let x = 1;\nlet y = 2;",
                 "new_string": "let x = 0;\nlet y = 0;" }),
        no_host(),
    )
    .unwrap();
    assert_eq!(out["match_type"], json!("fuzzy"));
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "let x = 0;\nlet y = 0;\n"
    );

    // BOM：old 头部带 BOM（从旧读取结果照抄的残迹）而文件没有 → 模糊命中
    std::fs::write(&path, "hello\n").unwrap();
    let out = edit_replace(
        &json!({ "path": p, "old_string": "\u{feff}hello", "new_string": "hi" }),
        no_host(),
    )
    .unwrap();
    assert_eq!(out["match_type"], json!("fuzzy"));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "hi\n");

    // 缩进不放宽：多行 old 行首空白差异不容错（精确与容错都失败）
    std::fs::write(&path, "fn a() {\n        old();\n}\n").unwrap();
    assert!(
        edit_replace(
            &json!({ "path": p, "old_string": "fn a() {\n    old();\n}", "new_string": "x" }),
            no_host()
        )
        .is_err()
    );

    // 模糊多重候选：报错列出候选行号（每处都有行尾差异，精确零命中）
    std::fs::write(&path, "a \nb \na\t\nb\t\n").unwrap();
    let err = edit_replace(
        &json!({ "path": p, "old_string": "a\nb\n", "new_string": "z" }),
        no_host(),
    )
    .unwrap_err();
    assert!(
        err.contains("matches 2 times") && err.contains("lines 1, 3"),
        "got: {err}"
    );

    // replace_all=true 模糊全换（new 带 LF 保持 LF——文件是 LF 风格）
    let out = edit_replace(
        &json!({ "path": p, "old_string": "a\nb\n", "new_string": "z\n", "replace_all": true }),
        no_host(),
    )
    .unwrap();
    assert_eq!(out["edits"], json!(2));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "z\nz\n");

    // 全都找不到：报错教下一步（fs_read 重读逐字复制）
    let err = edit_replace(
        &json!({ "path": p, "old_string": "nope", "new_string": "x" }),
        no_host(),
    )
    .unwrap_err();
    assert!(err.contains("fs_read"), "got: {err}");

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
    let out = edit_lines(
        &json!({ "path": p, "start_line": 2, "end_line": 3,
        "content": "TWO-A\nTWO-B" }),
        no_host(),
    )
    .expect("lines ok");
    assert_eq!(out["replaced"], json!(2));
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "one\nTWO-A\nTWO-B\nfour\n"
    );

    // 边界：空 content = 删除区间；尾换行保持
    edit_lines(
        &json!({ "path": p, "start_line": 2, "end_line": 3, "content": "" }),
        no_host(),
    )
    .unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "one\nfour\n");
    // 单文件行校验
    let err = edit_lines(
        &json!({ "path": p, "start_line": 5, "end_line": 6, "content": "x" }),
        no_host(),
    )
    .unwrap_err();
    assert!(err.contains("beyond the last line"), "got: {err}");
    assert!(
        edit_lines(
            &json!({ "path": p, "start_line": 3, "end_line": 2, "content": "x" }),
            no_host()
        )
        .is_err()
    );

    // 恶意：根外拒绝
    let outside = temp_root("elsewhere2").join("t.txt");
    assert!(
        edit_lines(
            &json!({ "path": outside.display().to_string(),
                "start_line": 1, "end_line": 1, "content": "x" }),
            no_host()
        )
        .is_err()
    );

    unsafe { std::env::remove_var("LOCOWRK_DATA_DIR") };
    let _ = std::fs::remove_dir_all(&root);
}
