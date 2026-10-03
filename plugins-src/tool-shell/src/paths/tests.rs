//! 路径校验单测：根内放行（含 `..` 归一）/ 相对与绝对逃逸 / flag 贴值
//! 与 `--flag=值` 两条注入形态 / 不存在候选放行 / cwd 出根整条拒绝。
//!
//! env（LOCOWRK_FS_ROOTS / LOCOWRK_DATA_DIR）是进程全局，用锁串行；
//! cwd 一律显式传参，绝不动进程 cwd（并行测试共享它）。

use super::ensure_readonly_scope;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

static ENV_LOCK: Mutex<()> = Mutex::new(());

/// 一次性沙箱：允许根 root（内含 sub/ 与 file.txt）、根外 outside
/// （内含 secret.txt）、隔离数据根 data（避免 allowed_roots 碰真实
/// HOME）。Drop 时摘 env 并清目录——先声明守卫后声明 env，保证摘 env
/// 发生在解锁之前。
struct Env {
    root: PathBuf,
    outside: PathBuf,
}

impl Env {
    fn new(tag: &str) -> Env {
        let base = std::env::temp_dir().join(format!(
            "locowrk-shell-paths-{}-{}",
            tag,
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let root = base.join("root");
        std::fs::create_dir_all(root.join("sub")).unwrap();
        std::fs::write(root.join("file.txt"), "hello").unwrap();
        let outside = base.join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("secret.txt"), "x").unwrap();
        let data = base.join("data");
        std::fs::create_dir_all(&data).unwrap();
        unsafe {
            std::env::set_var("LOCOWRK_DATA_DIR", &data);
            std::env::set_var("LOCOWRK_FS_ROOTS", &root);
        }
        Env { root, outside }
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        unsafe {
            std::env::remove_var("LOCOWRK_FS_ROOTS");
            std::env::remove_var("LOCOWRK_DATA_DIR");
        }
        let _ = std::fs::remove_dir_all(self.root.parent().unwrap());
    }
}

fn s(v: &str) -> String {
    v.to_string()
}

#[test]
fn inside_paths_pass_including_dotdot_normalization() {
    let _g = ENV_LOCK.lock().unwrap();
    let env = Env::new("inside");
    let ok = ensure_readonly_scope(
        &[
            s("-la"),
            s("sub"),
            s("file.txt"),
            s("--include=*.rs"),
            s("missing-pattern"),
        ],
        Some(&env.root),
        std::ptr::null(),
    );
    assert!(ok.is_ok(), "{ok:?}");
    // `..` 归一后仍在根内（cwd 在子目录，参数回父级）
    let ok = ensure_readonly_scope(
        &[s(".."), s("file.txt")],
        Some(&env.root.join("sub")),
        std::ptr::null(),
    );
    assert!(ok.is_ok(), "{ok:?}");
}

#[test]
fn relative_escape_is_denied() {
    let _g = ENV_LOCK.lock().unwrap();
    let env = Env::new("rel-escape");
    let err = ensure_readonly_scope(
        &[s("../outside/secret.txt")],
        Some(&env.root),
        std::ptr::null(),
    )
    .unwrap_err();
    assert!(err.contains("outside the allowed roots"), "{err}");
}

#[test]
fn absolute_outside_is_denied() {
    let _g = ENV_LOCK.lock().unwrap();
    let env = Env::new("abs-escape");
    let abs = env.outside.join("secret.txt");
    let err = ensure_readonly_scope(
        &[abs.display().to_string()],
        Some(&env.root),
        std::ptr::null(),
    )
    .unwrap_err();
    assert!(err.contains("outside the allowed roots"), "{err}");
    // 绝对路径在根内 → 放行
    let abs_in = env.root.join("file.txt");
    assert!(
        ensure_readonly_scope(
            &[abs_in.display().to_string()],
            Some(&env.root),
            std::ptr::null()
        )
        .is_ok()
    );
}

#[test]
fn flag_attached_and_eq_value_forms_are_denied() {
    let _g = ENV_LOCK.lock().unwrap();
    let env = Env::new("flag-forms");
    // 短选项贴值（规则 3：带分隔符即拒，不论存在与否）
    let err = ensure_readonly_scope(&[s("-f/etc/passwd")], Some(&env.root), std::ptr::null())
        .unwrap_err();
    assert!(err.contains("path separator"), "{err}");
    let err = ensure_readonly_scope(
        &[s("-fC:\\Windows\\win.ini")],
        Some(&env.root),
        std::ptr::null(),
    )
    .unwrap_err();
    assert!(err.contains("path separator"), "{err}");
    // `--flag=值` 无分隔符逃逸（`..`）：resolve 出根即拒（带分隔符的
    // 值会被上面的规则 3 先拦，这里测的是 = 值校验自己的职责面）
    let err =
        ensure_readonly_scope(&[s("--file=..")], Some(&env.root), std::ptr::null()).unwrap_err();
    assert!(err.contains("outside the allowed roots"), "{err}");
    // 对照：= 值在根内（无分隔符的相对文件）放行
    assert!(
        ensure_readonly_scope(&[s("--file=file.txt")], Some(&env.root), std::ptr::null()).is_ok()
    );
}

#[test]
fn cwd_outside_roots_denies_whole_call() {
    let _g = ENV_LOCK.lock().unwrap();
    let env = Env::new("cwd-outside");
    // 裸 ls 列的就是 cwd——cwd 出根整条拒绝，即使没有任何路径参数
    let err = ensure_readonly_scope(&[], Some(&env.outside), std::ptr::null()).unwrap_err();
    assert!(err.contains("working directory"), "{err}");
    // 降级文案要教模型换工具
    assert!(err.contains("`shell` tool"), "{err}");
}
