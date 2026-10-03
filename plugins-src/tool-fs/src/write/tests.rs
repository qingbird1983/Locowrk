//! fs_write / fs_delete / fs_restore 单测（进程内直接调 PluginState 层，
//! 不经 FFI）。涉及 LOCOWRK_DATA_DIR 的用例串行（env 是进程级全局）。

use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use super::{fs_delete, fs_restore, fs_write};
use plugin_api::serde_json::{Value, json};

/// env 是进程级全局：改数据根的用例先抢锁再 set_var。
static ENV_LOCK: Mutex<()> = Mutex::new(());

fn temp_root(tag: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "locowrk-tool-fs-{tag}-{}-{nanos}",
        std::process::id()
    ))
}

fn undo_payload(write_output: &Value) -> Value {
    json!({
        "call_id": "call-test",
        "args": { "path": write_output["path"], "content": "whatever" },
        "output": write_output.clone(),
    })
}

#[test]
fn write_overwrite_backup_restore_roundtrip() {
    let _guard = ENV_LOCK.lock().unwrap();
    let root = temp_root("roundtrip");
    unsafe { std::env::set_var("LOCOWRK_DATA_DIR", &root) };

    let file = root.join("work").join("note.txt");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, "v1").unwrap();

    // 覆盖写：existed=true 且备份落进数据根
    let out = fs_write(
        &json!({ "path": file.display().to_string(), "content": "v2 longer" }),
        std::ptr::null(),
    )
    .expect("write ok");
    assert_eq!(out["existed"], json!(true));
    assert!(out["backup"].as_str().is_some(), "backup path recorded");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "v2 longer");
    let backup = out["backup"].as_str().unwrap();
    let backup = std::path::Path::new(backup);
    let root_canon = root.canonicalize().unwrap();
    assert!(
        backup.canonicalize().unwrap().starts_with(&root_canon),
        "backup lives under data root"
    );
    assert_eq!(std::fs::read_to_string(backup).unwrap(), "v1");

    // undo：恢复 v1，备份自消费
    let restored = fs_restore(&undo_payload(&out), std::ptr::null()).expect("restore ok");
    assert_eq!(restored["restored"], json!(true));
    assert_eq!(restored["already"], json!(false));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "v1");
    assert!(!backup.exists(), "backup consumed by rename");

    // 重复恢复：no-op 幂等
    let again = fs_restore(&undo_payload(&out), std::ptr::null()).expect("idempotent restore");
    assert_eq!(again["already"], json!(true));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "v1");

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn write_new_file_then_restore_removes_it() {
    let _guard = ENV_LOCK.lock().unwrap();
    let root = temp_root("newfile");
    unsafe { std::env::set_var("LOCOWRK_DATA_DIR", &root) };

    let file = root.join("fresh.txt");
    let out = fs_write(
        &json!({ "path": file.display().to_string(), "content": "hello", "create_dirs": false }),
        std::ptr::null(),
    )
    .expect("write ok");
    assert_eq!(out["existed"], json!(false));
    assert!(out["backup"].is_null(), "new file has nothing to back up");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "hello");

    let restored = fs_restore(&undo_payload(&out), std::ptr::null()).expect("restore ok");
    assert_eq!(restored["removed"], json!(true));
    assert!(!file.exists(), "undo deletes the created file");
    // 再恢复：已删 → 幂等 no-op
    let again = fs_restore(&undo_payload(&out), std::ptr::null()).expect("idempotent");
    assert_eq!(again["already"], json!(true));

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn write_guards_and_delete_matrix() {
    let _guard = ENV_LOCK.lock().unwrap();
    let root = temp_root("guards");
    unsafe { std::env::set_var("LOCOWRK_DATA_DIR", &root) };

    // 目录写入拒绝；超限拒绝；缺 content 拒绝
    let dir = root.join("adir");
    std::fs::create_dir_all(&dir).unwrap();
    assert!(
        fs_write(
            &json!({ "path": dir.display().to_string(), "content": "x" }),
            std::ptr::null()
        )
        .is_err()
    );
    assert!(
        fs_write(
            &json!({ "path": root.join("big.txt").display().to_string(),
        "content": "x".repeat(2 * 1024 * 1024 + 1) }),
            std::ptr::null()
        )
        .is_err()
    );
    assert!(
        fs_write(
            &json!({ "path": root.join("nocontent.txt").display().to_string() }),
            std::ptr::null()
        )
        .is_err()
    );

    // create_dirs=false 时父目录缺失拒绝；true 时创建
    let deep = root.join("a/b/c.txt");
    assert!(
        fs_write(
            &json!({ "path": deep.display().to_string(), "content": "x" }),
            std::ptr::null()
        )
        .is_err()
    );
    let out = fs_write(
        &json!({ "path": deep.display().to_string(), "content": "x", "create_dirs": true }),
        std::ptr::null(),
    )
    .expect("create_dirs ok");
    assert!(deep.is_file());
    // 撤销新建 → 文件删除（父目录保留）
    fs_restore(&undo_payload(&out), std::ptr::null()).unwrap();
    assert!(!deep.exists());

    // delete 矩阵：删文件 ok；删目录 / 删不存在 报错
    let file = root.join("doomed.txt");
    std::fs::write(&file, b"bye").unwrap();
    let deleted = fs_delete(
        &json!({ "path": file.display().to_string() }),
        std::ptr::null(),
    )
    .expect("delete ok");
    assert_eq!(deleted["deleted"], json!(true));
    assert!(!file.exists());
    assert!(
        fs_delete(
            &json!({ "path": dir.display().to_string() }),
            std::ptr::null()
        )
        .is_err()
    );
    assert!(
        fs_delete(
            &json!({ "path": file.display().to_string() }),
            std::ptr::null()
        )
        .is_err()
    );

    // existed=true 但 output 缺 backup → 明确报错，不静默
    assert!(
        fs_restore(
            &json!({ "output": { "path": file.display().to_string(),
        "existed": true } }),
            std::ptr::null()
        )
        .is_err()
    );

    unsafe { std::env::remove_var("LOCOWRK_DATA_DIR") };
    let _ = std::fs::remove_dir_all(&root);
}

/// 恶意路径三态（安全底线）：允许根外的读/写/删/恢复一律拒绝。
#[test]
fn outside_root_paths_are_rejected() {
    let _guard = ENV_LOCK.lock().unwrap();
    let root = temp_root("guardroot");
    unsafe { std::env::set_var("LOCOWRK_DATA_DIR", &root) };
    // 不设 LOCOWRK_FS_ROOTS → 只放行数据根

    let outside = temp_root("elsewhere");
    std::fs::create_dir_all(&outside).unwrap();
    let outside_file = outside.join("secret.txt");
    std::fs::write(&outside_file, b"secret").unwrap();
    let outside_str = outside_file.display().to_string();

    // 读：canonicalize 成功但在根外 → 拒（不是"找不到"）
    let err = crate::read::fs_read(&json!({ "path": outside_str }), std::ptr::null()).unwrap_err();
    assert!(err.contains("outside the allowed roots"), "got: {err}");
    // 写 / 删 / 恢复目标同样拒绝
    assert!(
        fs_write(
            &json!({ "path": outside_str, "content": "x" }),
            std::ptr::null()
        )
        .is_err()
    );
    assert!(fs_delete(&json!({ "path": outside_str }), std::ptr::null()).is_err());

    // restore 的 backup 来源校验：伪造 output 把根外文件当备份 → 拒
    // （根外先被允许根检查拦下；根内非 vault 路径由 provenance 拦下——两道都算拒绝）
    let inside = root.join("victim.txt");
    let forged = json!({ "path": inside.display().to_string(),
                         "backup": outside_str, "existed": true });
    let err = fs_restore(&json!({ "output": forged }), std::ptr::null()).unwrap_err();
    assert!(
        err.contains("outside the allowed roots") || err.contains("not a tool-fs backup"),
        "got: {err}"
    );
    // 根内目标文件未被伪造恢复污染
    assert!(!inside.exists());

    // `..` 逃逸：归一后落根外
    let escape = root.join("sub").join("..").join("..").join("evil.txt");
    assert!(
        fs_write(
            &json!({ "path": escape.display().to_string(), "content": "x" }),
            std::ptr::null()
        )
        .is_err()
    );

    unsafe { std::env::remove_var("LOCOWRK_DATA_DIR") };
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&outside);
}
