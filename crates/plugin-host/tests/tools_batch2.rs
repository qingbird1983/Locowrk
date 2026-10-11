//! 端到端集成测试：阶段八⑥第二批官方工具（真实 dlopen）。
//!
//! - tool-fs 0.1.5：fs_read 回 revision → fs_write 守卫核对 + 备份 →
//!   fs_restore 撤销（undo 契约全链路）、fs_delete 守卫、允许根拒绝、
//!   陈旧观察拒绝（G3 Revision Guard）
//! - tool-shell 1.0.0：白名单放行 / 显式配置扩展 / denylist 与元字符拒绝 /
//!   真实 spawn（cargo --version）
//! - tool-edit 0.2.1：edit_replace（带 expected_revision）→ 跨插件
//!   fs_restore 撤销
//!
//! 与 fs_fetch_plugins.rs 同款模式：测试自建产物暂存到 target/locowrk-test/{pid}/。
//! 样板（build/load/call/env 锁/临时根）全部来自 `mod common`，本文件不再自带副本。
//! env 是进程级全局，全部用例串行（一把大锁包住整个测试体）。

use std::sync::Arc;

use plugin_host::{PluginRegistry, RegistryToolSource};
use serde_json::json;
mod common;
use common::{call, env_lock, err_msg, load_activate, temp_root};

#[test]
fn fs_write_backup_restore_delete_end_to_end() {
    let _env = env_lock().lock().unwrap();
    let root = temp_root("fs");
    std::fs::create_dir_all(&root).unwrap();
    unsafe { std::env::set_var("LOCOWRK_DATA_DIR", root.display().to_string()) };

    let registry = Arc::new(PluginRegistry::new());
    load_activate("tool-fs", "0.1.5", &registry);
    let source = RegistryToolSource::new(Arc::clone(&registry));

    let file = root.join("note.txt");
    std::fs::write(&file, "v1").unwrap();

    // fs_read 拿 revision（G3 链条的第一环，过 FFI 验证）
    let read_out = call(
        source.resolve("fs_read").unwrap().as_ref(),
        json!({ "path": file.display().to_string() }),
    )
    .expect("fs_read ok");
    let revision = read_out["revision"].as_str().expect("revision").to_string();

    // 盲覆盖拒绝（无 expected_revision）
    let err = err_msg(
        call(
            source.resolve("fs_write").unwrap().as_ref(),
            json!({ "path": file.display().to_string(), "content": "v2 longer content" }),
        )
        .unwrap_err(),
    );
    assert!(err.contains("already exists"), "{err}");

    // 覆盖写（带 revision）：备份进数据根，输出带新 revision
    let out = call(
        source.resolve("fs_write").unwrap().as_ref(),
        json!({
            "path": file.display().to_string(),
            "content": "v2 longer content",
            "expected_revision": revision,
        }),
    )
    .expect("fs_write ok");
    assert_eq!(out["existed"], json!(true));
    assert!(out["revision"].as_str().is_some(), "新 revision 回传");
    let backup = out["backup"].as_str().expect("backup recorded").to_string();
    assert_eq!(std::fs::read_to_string(&backup).unwrap(), "v1");

    // 陈旧观察拒绝：伪造过期 revision → StaleObservation 报文带双方
    let err = err_msg(
        call(
            source.resolve("fs_write").unwrap().as_ref(),
            json!({
                "path": file.display().to_string(),
                "content": "v3",
                "expected_revision": revision,
            }),
        )
        .unwrap_err(),
    );
    assert!(err.contains("stale observation"), "{err}");
    assert!(err.contains("fs_read"), "{err}");

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

    // delete（Irreversible 的审批在内核层，这里验证工具本体）：重读拿
    // 新 revision 后才放行
    let read_out = call(
        source.resolve("fs_read").unwrap().as_ref(),
        json!({ "path": file.display().to_string() }),
    )
    .expect("fs_read ok");
    call(
        source.resolve("fs_delete").unwrap().as_ref(),
        json!({
            "path": file.display().to_string(),
            "expected_revision": read_out["revision"],
        }),
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
fn shell_any_simple_command_runs_end_to_end() {
    // v2.145 无命令级禁令（《Permission Model Redesign (ZCode Alignment)
    // v1》）：结构合法即执行——审批在内核侧（工具 Irreversible），FFI 面
    // 直调工具 = 已过审批的形状。曾「not whitelisted」拒绝的命令现在真跑。
    let _env = env_lock().lock().unwrap();
    let root = temp_root("shell");
    unsafe {
        std::env::remove_var("LOCOWRK_DATA_DIR");
        // 退役 env 幻影化：设置了也无效（插件只启动 warn），此处顺带证明
        std::env::set_var("LOCOWRK_SHELL_ALLOW", "cargo --version");
    }
    std::fs::create_dir_all(&root).unwrap();

    let registry = Arc::new(PluginRegistry::new());
    load_activate("tool-shell", "1.0.0", &registry);
    let source = RegistryToolSource::new(Arc::clone(&registry));
    let shell = source.resolve("shell").unwrap();

    // 白名单外的命令（v2.144 前被拒绝）现在真跑（whoami 各平台必有）
    let out = call(
        shell.as_ref(),
        json!({ "command": "whoami", "timeout_secs": 60 }),
    )
    .expect("non-whitelisted command runs since v2.145");
    assert_eq!(out["timed_out"], json!(false));
    assert_eq!(out["success"], json!(true));

    // 前硬锁类（python -c）：策略放行——python 在则真跑，不在则 spawn
    // 错误；两种结果都证明政策层不再拒绝（报文无 "permanently locked"）
    match call(
        shell.as_ref(),
        json!({ "command": "python --version", "timeout_secs": 60 }),
    ) {
        Ok(out) => assert_eq!(out["success"], json!(true)),
        Err(e) => {
            let err = err_msg(e);
            assert!(
                !err.contains("permanently locked") && !err.contains("not whitelisted"),
                "policy must not reject interpreters anymore: {err}"
            );
        }
    }

    // 元字符：结构性拒绝保留（唯一幸存的 deny 面）
    let err = err_msg(call(shell.as_ref(), json!({ "command": "ls | cat" })).unwrap_err());
    assert!(err.contains("metacharacters"), "{err}");
    // 空命令
    assert!(call(shell.as_ref(), json!({ "command": "   " })).is_err());

    unsafe { std::env::remove_var("LOCOWRK_SHELL_ALLOW") };
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn shell_readonly_path_guard_end_to_end() {
    let _env = env_lock().lock().unwrap();
    let root = temp_root("shell_ro");
    let fs_root = root.join("ws");
    std::fs::create_dir_all(&fs_root).unwrap();
    std::fs::write(root.join("secret.txt"), "s").unwrap();
    unsafe {
        // 只读豁免的硬前提：路径面受 fsutil 允许根约束（与 fs_* 同源）
        std::env::set_var("LOCOWRK_DATA_DIR", root.join("data"));
        std::env::set_var("LOCOWRK_FS_ROOTS", &fs_root);
    }

    let registry = Arc::new(PluginRegistry::new());
    load_activate("tool-shell", "1.0.0", &registry);
    let source = RegistryToolSource::new(Arc::clone(&registry));
    let shell_read = source.resolve("shell_read").unwrap();

    // 只读表内命令 + cwd 在根内 → 免审批直接跑（git 在任何目录都能 spawn）
    let out = call(
        shell_read.as_ref(),
        json!({ "command": "git status", "cwd": fs_root.display().to_string() }),
    )
    .expect("readonly command runs without approval");
    assert_eq!(out["program"], json!("git"));

    // 路径逃逸：候选 resolve 出根 → 拒绝并引导改用 shell（降级审批）
    let err = err_msg(
        call(
            shell_read.as_ref(),
            json!({ "command": "git log -- ../secret.txt", "cwd": fs_root.display().to_string() }),
        )
        .unwrap_err(),
    );
    assert!(err.contains("outside the allowed roots"), "{err}");
    assert!(err.contains("`shell` tool"), "{err}");

    // 只读表外命令：不进只读面
    let err = err_msg(call(shell_read.as_ref(), json!({ "command": "git push" })).unwrap_err());
    assert!(err.contains("not a read-only command"), "{err}");

    // G4 只读证明层（可靠性稿 §6）：白名单内但证明不了 → 拒绝并带出路，
    // 端到端过 FFI。结构性元字符 / git 配置覆盖 / 写旗标 / 敏感路径各一。
    let err =
        err_msg(call(shell_read.as_ref(), json!({ "command": "ls $(rm -rf /)" })).unwrap_err());
    assert!(err.contains("not a single simple command"), "{err}");
    let err = err_msg(
        call(
            shell_read.as_ref(),
            json!({ "command": "git -c core.pager=x status" }),
        )
        .unwrap_err(),
    );
    assert!(err.contains("global options"), "{err}");
    let err = err_msg(
        call(
            shell_read.as_ref(),
            json!({ "command": "git diff --output=x" }),
        )
        .unwrap_err(),
    );
    assert!(err.contains("not read-only"), "{err}");
    let err = err_msg(call(shell_read.as_ref(), json!({ "command": "cat .env" })).unwrap_err());
    assert!(err.contains("sensitive path"), "{err}");

    unsafe {
        std::env::remove_var("LOCOWRK_FS_ROOTS");
        std::env::remove_var("LOCOWRK_DATA_DIR");
    }
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
    load_activate("tool-fs", "0.1.5", &registry);
    load_activate("tool-edit", "0.2.1", &registry);
    let source = RegistryToolSource::new(Arc::clone(&registry));

    let file = root.join("app.rs");
    let original = "fn main() {\n    todo!()\n}\n";
    std::fs::write(&file, original).unwrap();

    // fs_read → revision → edit_replace（G3 全链过 FFI）
    let read_out = call(
        source.resolve("fs_read").unwrap().as_ref(),
        json!({ "path": file.display().to_string() }),
    )
    .expect("fs_read ok");
    let out = call(
        source.resolve("edit_replace").unwrap().as_ref(),
        json!({
            "path": file.display().to_string(),
            "old_string": "todo!()",
            "new_string": "println!(\"hi\");",
            "expected_revision": read_out["revision"],
        }),
    )
    .expect("edit_replace ok");
    assert_eq!(out["edits"], json!(1));
    assert!(out["revision"].as_str().is_some(), "新 revision 回传");
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
