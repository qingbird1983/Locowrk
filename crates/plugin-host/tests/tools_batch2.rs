//! 端到端集成测试：阶段八⑥第二批官方工具（真实 dlopen）。
//!
//! - tool-fs 1.2.1：fs_write 备份 → fs_restore 撤销（undo 契约全链路）、
//!   fs_delete、允许根拒绝
//! - tool-shell 1.0.0：白名单放行 / 显式配置扩展 / denylist 与元字符拒绝 /
//!   真实 spawn（cargo --version）
//! - tool-edit 1.1.1：edit_replace → 跨插件 fs_restore 撤销
//!
//! 与 fs_fetch_plugins.rs 同款模式：测试自建产物暂存到 target/locowrk-test/。
//! env 是进程级全局，全部用例串行（一把大锁包住整个测试体）。

use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex, OnceLock};

use plugin_api::{Tool, ToolError};
use plugin_host::{PluginRegistry, RegistryToolSource};
use semver::Version;
use serde_json::{Value, json};

fn build_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn workspace_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root")
}

fn artifact_name(plugin_id: &str) -> String {
    let stem = plugin_id.replace('-', "_");
    if cfg!(windows) {
        format!("{stem}.dll")
    } else if cfg!(target_os = "macos") {
        format!("lib{stem}.dylib")
    } else {
        format!("lib{stem}.so")
    }
}

fn build_and_stage(plugin_id: &str, version: &str) -> PathBuf {
    let _guard = build_lock().lock().unwrap();
    let status = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
        .args(["build", "-p", plugin_id, "--quiet"])
        .env("LOCOWRK_PLUGIN_VERSION", version)
        .status()
        .expect("spawn cargo");
    assert!(status.success(), "cargo build -p {plugin_id} failed");

    let src = workspace_dir()
        .join("target/debug")
        .join(artifact_name(plugin_id));
    assert!(src.exists(), "artifact missing at {}", src.display());
    let dst_dir = workspace_dir()
        .join("target/locowrk-test")
        .join(plugin_id)
        .join(version);
    std::fs::create_dir_all(&dst_dir).unwrap();
    let dst = dst_dir.join(artifact_name(plugin_id));
    std::fs::copy(&src, &dst).unwrap();
    dst
}

fn load_activate(plugin_id: &str, version: &str, registry: &Arc<PluginRegistry>) {
    let ver: Version = version.parse().unwrap();
    let path = build_and_stage(plugin_id, version);
    registry.load(plugin_id, &ver, &path).expect("load");
    registry.activate(plugin_id, &ver).expect("activate");
}

fn call(tool: &dyn Tool, args: Value) -> Result<Value, ToolError> {
    let ctx = plugin_api::CallCtx::new();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(tool.call(&ctx, args))
}

/// env 串行锁：本文件所有用例都动进程级 env。
fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn temp_root(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "locowrk-batch2-{tag}-{}-{nanos}",
        std::process::id()
    ))
}

fn err_msg(e: ToolError) -> String {
    match e {
        ToolError::Failed { message, .. } => message,
        other => other.to_string(),
    }
}

#[test]
fn fs_write_backup_restore_delete_end_to_end() {
    let _env = env_lock().lock().unwrap();
    let root = temp_root("fs");
    std::fs::create_dir_all(&root).unwrap();
    unsafe { std::env::set_var("LOCOWRK_DATA_DIR", root.display().to_string()) };

    let registry = Arc::new(PluginRegistry::new());
    load_activate("tool-fs", "1.2.1", &registry);
    let source = RegistryToolSource::new(Arc::clone(&registry));

    let file = root.join("note.txt");
    std::fs::write(&file, "v1").unwrap();

    // 覆盖写：备份进数据根
    let out = call(
        source.resolve("fs_write").unwrap().as_ref(),
        json!({ "path": file.display().to_string(), "content": "v2 longer content" }),
    )
    .expect("fs_write ok");
    assert_eq!(out["existed"], json!(true));
    let backup = out["backup"].as_str().expect("backup recorded").to_string();
    assert_eq!(std::fs::read_to_string(&backup).unwrap(), "v1");

    // undo 契约：内核以 {call_id, args, output} 调 fs_restore
    let undo = json!({
        "call_id": "call-it-1",
        "args": { "path": file.display().to_string(), "content": "v2 longer content" },
        "output": out,
    });
    call(source.resolve("fs_restore").unwrap().as_ref(), undo.clone()).expect("restore ok");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "v1");
    assert!(!std::path::Path::new(&backup).exists(), "备份自消费");
    // 幂等：重复恢复 no-op
    call(source.resolve("fs_restore").unwrap().as_ref(), undo).expect("idempotent restore");

    // delete（Irreversible 的审批在内核层，这里验证工具本体）
    call(
        source.resolve("fs_delete").unwrap().as_ref(),
        json!({ "path": file.display().to_string() }),
    )
    .expect("fs_delete ok");
    assert!(!file.exists());

    // 恶意：根外写拒绝
    let outside = temp_root("elsewhere").join("x.txt");
    let err = call(
        source.resolve("fs_write").unwrap().as_ref(),
        json!({ "path": outside.display().to_string(), "content": "x" }),
    )
    .unwrap_err();
    assert!(err_msg(err).contains("outside the allowed roots"));

    unsafe { std::env::remove_var("LOCOWRK_DATA_DIR") };
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn shell_whitelist_and_spawn_end_to_end() {
    let _env = env_lock().lock().unwrap();
    let root = temp_root("shell");
    unsafe {
        // init 时读一次：必须在 load 之前设好
        std::env::set_var("LOCOWRK_SHELL_ALLOW", "cargo --version");
        std::env::remove_var("LOCOWRK_DATA_DIR");
    }
    std::fs::create_dir_all(&root).unwrap();

    let registry = Arc::new(PluginRegistry::new());
    load_activate("tool-shell", "1.0.0", &registry);
    let source = RegistryToolSource::new(Arc::clone(&registry));
    let shell = source.resolve("shell").unwrap();

    // 显式配置的条目：真实 spawn（本机必有 cargo）
    let out = call(
        shell.as_ref(),
        json!({ "command": "cargo --version", "timeout_secs": 60 }),
    )
    .expect("whitelisted command runs");
    assert_eq!(out["timed_out"], json!(false));
    assert!(
        out["stdout"].as_str().unwrap().contains("cargo"),
        "stdout: {:?}",
        out["stdout"]
    );

    // 子命令限定：cargo publish 不在条目内
    let err = err_msg(call(shell.as_ref(), json!({ "command": "cargo publish" })).unwrap_err());
    assert!(err.contains("not whitelisted"), "{err}");
    // denylist：不进审批直接拒
    let err = err_msg(call(shell.as_ref(), json!({ "command": "rm -rf /" })).unwrap_err());
    assert!(err.contains("denylist"), "{err}");
    // 元字符：管道拒绝
    let err = err_msg(call(shell.as_ref(), json!({ "command": "ls | cat" })).unwrap_err());
    assert!(err.contains("metacharacters"), "{err}");
    // 内置白名单：git 只放行 status
    assert!(call(shell.as_ref(), json!({ "command": "git push" })).is_err());

    unsafe { std::env::remove_var("LOCOWRK_SHELL_ALLOW") };
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn edit_replace_then_cross_plugin_restore() {
    let _env = env_lock().lock().unwrap();
    let root = temp_root("edit");
    std::fs::create_dir_all(&root).unwrap();
    unsafe { std::env::set_var("LOCOWRK_DATA_DIR", root.display().to_string()) };

    // tool-edit 与 tool-fs 同 registry：undo 跨插件解析
    let registry = Arc::new(PluginRegistry::new());
    load_activate("tool-fs", "1.2.1", &registry);
    load_activate("tool-edit", "1.1.1", &registry);
    let source = RegistryToolSource::new(Arc::clone(&registry));

    let file = root.join("app.rs");
    let original = "fn main() {\n    todo!()\n}\n";
    std::fs::write(&file, original).unwrap();

    let out = call(
        source.resolve("edit_replace").unwrap().as_ref(),
        json!({
            "path": file.display().to_string(),
            "old_string": "todo!()",
            "new_string": "println!(\"hi\");",
        }),
    )
    .expect("edit_replace ok");
    assert_eq!(out["edits"], json!(1));
    assert!(std::fs::read_to_string(&file).unwrap().contains("println!"));

    // 撤销：edit 的 output 直接喂给 tool-fs 的 fs_restore
    let undo = json!({
        "call_id": "call-it-2",
        "args": json!({ "path": file.display().to_string() }),
        "output": out,
    });
    call(source.resolve("fs_restore").unwrap().as_ref(), undo).expect("cross-plugin restore");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), original);

    unsafe { std::env::remove_var("LOCOWRK_DATA_DIR") };
    let _ = std::fs::remove_dir_all(&root);
}
