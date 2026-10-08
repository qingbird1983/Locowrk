//! fs_write / fs_delete / fs_restore 单测（进程内直接调 PluginState 层，
//! 不经 FFI）。涉及 LOCOWRK_DATA_DIR 的用例串行（env 是进程级全局；
//! 锁在 crate::testutil，与 read 模块共用）。

use std::time::{SystemTime, UNIX_EPOCH};

use super::{fs_delete, fs_restore, fs_write};
use crate::testutil::ENV_LOCK;
use plugin_api::fsutil::{content_revision, file_revision};
use plugin_api::serde_json::{Value, json};

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

    // 覆盖写（带当前 revision）：existed=true 且备份落进数据根
    let out = fs_write(
        &json!({
            "path": file.display().to_string(),
            "content": "v2 longer",
            "expected_revision": file_revision(&file).unwrap().unwrap(),
        }),
        std::ptr::null(),
    )
    .expect("write ok");
    assert_eq!(out["existed"], json!(true));
    assert_eq!(out["revision"], json!(content_revision(b"v2 longer")));
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

    // undo：恢复 v1，备份自消费；输出带恢复后文件的 revision（守卫不断链）
    let restored = fs_restore(&undo_payload(&out), std::ptr::null()).expect("restore ok");
    assert_eq!(restored["restored"], json!(true));
    assert_eq!(restored["already"], json!(false));
    assert_eq!(restored["revision"], json!(content_revision(b"v1")));
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

    // delete 矩阵：删文件 ok（带当前 revision）；删目录 / 删不存在 报错
    let file = root.join("doomed.txt");
    std::fs::write(&file, b"bye").unwrap();
    let deleted = fs_delete(
        &json!({
            "path": file.display().to_string(),
            "expected_revision": file_revision(&file).unwrap().unwrap(),
        }),
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

/// Revision Guard 矩阵（可靠性稿 §3）：新建语义 / 盲覆盖拒绝 / 陈旧观察
/// 拒绝并指路重读 / 链式 revision / 删除守卫。
#[test]
fn write_and_delete_revision_guard_matrix() {
    let _guard = ENV_LOCK.lock().unwrap();
    let root = temp_root("revguard");
    unsafe { std::env::set_var("LOCOWRK_DATA_DIR", &root) };
    let p = root.join("g.txt").display().to_string();

    // 新建：缺省（不带 expected_revision）= 期望文件不存在 → 放行；
    // 输出带新 revision
    let out =
        fs_write(&json!({ "path": p, "content": "v1" }), std::ptr::null()).expect("create ok");
    assert_eq!(out["existed"], json!(false));
    assert_eq!(out["revision"], json!(content_revision(b"v1")));

    // 盲覆盖拒绝：文件已存在而不带 revision（缺省与显式 null 同判）
    for extra in [json!({}), json!({ "expected_revision": null })] {
        let mut args = json!({ "path": p, "content": "v2" });
        for (k, v) in extra.as_object().unwrap() {
            args[k] = v.clone();
        }
        let err = fs_write(&args, std::ptr::null()).unwrap_err();
        assert!(err.contains("already exists"), "got: {err}");
        assert!(err.contains("fs_read"), "got: {err}");
    }
    assert_eq!(std::fs::read_to_string(&p).unwrap(), "v1", "目标未动");

    // 陈旧观察拒绝：报文带双方 revision + fs_read 指路（§3.3）
    let err = fs_write(
        &json!({ "path": p, "content": "v2", "expected_revision": "sha256:deadbeef" }),
        std::ptr::null(),
    )
    .unwrap_err();
    assert!(err.contains("stale observation"), "got: {err}");
    assert!(err.contains("sha256:deadbeef"), "got: {err}");
    assert!(
        err.contains(&content_revision(b"v1")),
        "报文带当前 revision: {err}"
    );

    // 链式：上一次写输出的 revision 直接续用 → 放行
    let out = fs_write(
        &json!({ "path": p, "content": "v2", "expected_revision": out["revision"] }),
        std::ptr::null(),
    )
    .expect("chained overwrite ok");
    assert_eq!(std::fs::read_to_string(&p).unwrap(), "v2");
    assert_eq!(out["revision"], json!(content_revision(b"v2")));

    // 给了 revision 但文件已消失：指路新建语义
    std::fs::remove_file(&p).unwrap();
    let err = fs_write(
        &json!({ "path": p, "content": "v3", "expected_revision": out["revision"] }),
        std::ptr::null(),
    )
    .unwrap_err();
    assert!(err.contains("does not exist"), "got: {err}");

    // 删除守卫：缺 revision 指路报错；陈旧拒绝；一致才删
    std::fs::write(&p, b"bye").unwrap();
    let err = fs_delete(&json!({ "path": p }), std::ptr::null()).unwrap_err();
    assert!(err.contains("missing `expected_revision`"), "got: {err}");
    let err = fs_delete(
        &json!({ "path": p, "expected_revision": "sha256:deadbeef" }),
        std::ptr::null(),
    )
    .unwrap_err();
    assert!(err.contains("stale observation"), "got: {err}");
    fs_delete(
        &json!({ "path": p, "expected_revision": content_revision(b"bye") }),
        std::ptr::null(),
    )
    .expect("delete with fresh revision ok");
    assert!(!std::path::Path::new(&p).exists());

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
