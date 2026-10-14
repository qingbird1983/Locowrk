//! fsutil 单测：三态（正常 / 边界 / 恶意）——根内放行、`..` 与根外拒绝、
//! 不存在的写目标按最深存在祖先校验。涉及 env 的用例串行。

use std::sync::Mutex;

use super::{allowed_roots, backup_original, data_root, resolve_existing, resolve_for_write};

static ENV_LOCK: Mutex<()> = Mutex::new(());

fn temp_root(tag: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "locowrk-fsutil-{tag}-{}-{nanos}",
        std::process::id()
    ))
}

#[test]
fn inside_paths_pass_and_escapes_fail() {
    let _guard = ENV_LOCK.lock().unwrap();
    let root = temp_root("roots");
    let sub = root.join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    unsafe { std::env::set_var("LOCOWRK_DATA_DIR", &root) };
    unsafe { std::env::set_var("LOCOWRK_FS_ROOTS", root.display().to_string()) };
    let roots = allowed_roots().unwrap();
    assert!(roots.len() >= 2, "数据根 + 配置根");

    // 正常：根内已存在路径放行（canonicalize 后仍根内）
    let file = sub.join("a.txt");
    std::fs::write(&file, b"x").unwrap();
    assert!(resolve_existing(&file, &roots).is_ok());

    // 恶意：`..` 归一后落到根外
    let escape = sub.join("..").join("..").join("etc-passwd");
    assert!(resolve_existing(&escape, &roots).is_err());

    // 恶意：绝对路径指向根外（.ssh 模拟）——存在且可 canonicalize，靠前缀拒绝
    let outside = temp_root("outside-ssh");
    let ssh_key = outside.join("id_rsa");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(&ssh_key, b"secret").unwrap();
    assert!(resolve_existing(&ssh_key, &roots).is_err());

    // 边界：写目标不存在 → 按最深存在祖先校验
    let new_file = sub.join("new").join("b.txt");
    assert!(resolve_for_write(&new_file, &roots).is_ok());
    let outside_new = temp_root("outside").join("c.txt");
    assert!(resolve_for_write(&outside_new, &roots).is_err());

    // 未设置 FS_ROOTS → 只放行数据根：根内仍放行，根外仍拒绝
    unsafe { std::env::remove_var("LOCOWRK_FS_ROOTS") };
    let only_data = allowed_roots().unwrap();
    let in_data = data_root().unwrap().join("whatever.txt");
    assert!(resolve_for_write(&in_data, &only_data).is_ok());
    assert!(
        resolve_for_write(&outside_new, &only_data).is_err(),
        "FS_ROOTS 撤销后根外目标不放行"
    );

    // 不存在的配置根 → 报错不静默
    unsafe {
        std::env::set_var(
            "LOCOWRK_FS_ROOTS",
            root.join("no-such-dir").display().to_string(),
        )
    };
    assert!(allowed_roots().is_err());

    unsafe { std::env::remove_var("LOCOWRK_FS_ROOTS") };
    unsafe { std::env::remove_var("LOCOWRK_DATA_DIR") };
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn backup_lands_under_data_root_and_is_readable() {
    let _guard = ENV_LOCK.lock().unwrap();
    let root = temp_root("backup");
    std::fs::create_dir_all(&root).unwrap();
    unsafe { std::env::set_var("LOCOWRK_DATA_DIR", &root) };

    let file = root.join("note.txt");
    std::fs::write(&file, b"v1").unwrap();
    let backup = backup_original(&file, "tool-fs").expect("backup ok");
    assert!(backup.starts_with(root.join("backups").join("tool-fs")));
    assert_eq!(std::fs::read(&backup).unwrap(), b"v1");

    unsafe { std::env::remove_var("LOCOWRK_DATA_DIR") };
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn missing_path_outside_roots_reports_outside_not_os_error() {
    // 2026-09 冒烟误读教训：根外且不存在的路径，拒绝理由必须仍是
    // "outside the allowed roots"，不能长得像读失败的 OS NotFound
    let root = temp_root("missing-guard");
    let sub = root.join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    let roots = vec![root.canonicalize().unwrap()];

    // 根外且不存在 → 守卫裁决（outside），不透传裸 OS 错误
    let outside_missing = temp_root("missing-out").join("no_such").join("f.txt");
    let err = resolve_existing(&outside_missing, &roots).unwrap_err();
    assert!(err.contains("outside the allowed roots"), "got: {err}");

    // 根内但不存在 → 原样报找不到，不冒充越界
    let inside_missing = sub.join("no_such_file.txt");
    let err = resolve_existing(&inside_missing, &roots).unwrap_err();
    assert!(!err.contains("outside the allowed roots"), "got: {err}");
    // 报错教学（审计 F12）：光报 OS 原文模型会拿同一路径反复重试，报文必须给出路
    // （指 `fs_list` 上级），且原始路径仍在场供自查
    assert!(
        err.contains("fs_list"),
        "根内 NotFound 要指「先 fs_list 上级」，got: {err}"
    );
    assert!(
        err.contains("no_such_file.txt"),
        "报文要带上查不到的路径，got: {err}"
    );

    // 借不存在组件 + `..` 逃逸：缺失尾部的 `..` 归一后判前缀。
    // sub/no_such/../.. = root（两级）→ 根内 NotFound；
    // 再多一级 = root 外 → 必须报 outside（归一前词法检查会漏放）
    let two_up = sub.join("no_such").join("..").join("..").join("evil.txt");
    let err = resolve_existing(&two_up, &roots).unwrap_err();
    assert!(!err.contains("outside the allowed roots"), "got: {err}");
    let three_up = sub
        .join("no_such")
        .join("..")
        .join("..")
        .join("..")
        .join("evil.txt");
    let err = resolve_existing(&three_up, &roots).unwrap_err();
    assert!(err.contains("outside the allowed roots"), "got: {err}");
    // fs_write 同一归一：越界写目标在"校验后落盘"前就被拒
    let write_escape = sub
        .join("no_such")
        .join("..")
        .join("..")
        .join("..")
        .join("evil.txt");
    assert!(resolve_for_write(&write_escape, &roots).is_err());
}

// ---------- Revision Guard（G3） ----------

use super::{check_revision, content_line_count, content_revision, file_revision};

#[test]
fn line_count_shares_the_revision_normalization() {
    // ±N 的唯一尺子（后审计队列 Q12·C1）：三个写工具报的行数全靠它，所以口径
    // 必须与 content_revision 同一条归一支——一次 CRLF 保存不许变出整文件的行数。
    assert_eq!(content_line_count("a\nb\n"), 2); // 末尾换行不产生空行
    assert_eq!(content_line_count("a\nb"), 2); // 无尾换行同数（had_nl 不参与）
    assert_eq!(content_line_count("a\r\nb\r\n"), 2);
    assert_eq!(content_line_count(""), 0);
    assert_eq!(content_line_count("\n\n"), 0); // 纯换行 = 空内容，不是 3 行
    assert_eq!(content_line_count("一\n二\n三"), 3); // 只按换行切，多字节不参与
}

#[test]
fn revision_hash_is_lf_normalized_and_stable() {
    // 稳定：同内容同 revision；格式 sha256:<64 hex>
    let rev = content_revision(b"hello\nworld\n");
    assert_eq!(content_revision(b"hello\nworld\n"), rev);
    assert!(rev.starts_with("sha256:"));
    assert_eq!(rev[7..].len(), 64);
    assert!(rev[7..].bytes().all(|b| b.is_ascii_hexdigit()));

    // CRLF ↔ LF 归一：换行风格漂移不判变更
    assert_eq!(content_revision(b"a\r\nb\r\n"), content_revision(b"a\nb\n"));
    // 孤 \r 是内容，照常参与哈希
    assert_ne!(content_revision(b"a\rb\n"), content_revision(b"ab\n"));
    // 其余差异照常判变更
    assert_ne!(content_revision(b"a\nb\n"), content_revision(b"a\nb \n"));
}

#[test]
fn file_revision_three_states() {
    let root = temp_root("revision");
    let file = root.join("f.txt");
    // 不存在 = None（fs_write 新建语义判定依据）
    assert_eq!(file_revision(&file).unwrap(), None);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(&file, b"v1").unwrap();
    let rev1 = file_revision(&file).unwrap().unwrap();
    std::fs::write(&file, b"v2").unwrap();
    assert_ne!(file_revision(&file).unwrap().unwrap(), rev1);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn check_revision_mismatch_teaches_reread() {
    // 一致放行
    assert!(check_revision("edit_replace", "f.txt", "sha256:a", "sha256:a").is_ok());
    // 不一致：报文带双方 revision 并指路重读（§3.3）
    let err = check_revision("edit_replace", "f.txt", "sha256:a", "sha256:b").unwrap_err();
    assert!(err.contains("stale observation"), "got: {err}");
    assert!(
        err.contains("sha256:a") && err.contains("sha256:b"),
        "got: {err}"
    );
    assert!(err.contains("fs_read"), "got: {err}");
}
