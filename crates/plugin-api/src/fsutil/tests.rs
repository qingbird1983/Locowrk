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
