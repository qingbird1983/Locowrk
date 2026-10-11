//! 端到端集成测试：阶段八⑦第三批官方工具（真实 dlopen）。
//!
//! - memory-sqlite 1.2.0：memory_write→覆盖（previous 捕获）→ undo 契约
//!   还原、memory_delete→undo 整条还原、库文件落在数据根
//! - tool-git 1.1.0：真实 git 仓库 git_status/git_commit→undo_commit 契约、
//!   git_push 到本地 bare 远端、允许根外仓库拒绝
//! - tool-mcp 1.1.0：mcp.json → dlopen 后 describe 阶段发现 mcp_fake_*
//!   工具 → 调用回显 / 工具级错误作为数据 / mcp_status
//!
//! 与 tools_batch2.rs 同款模式：测试自建产物暂存到
//! target/locowrk-test/{pid}-{纳秒}/，且每次 stage() 落一个独占快照目录（F11b/F11c，
//! 见 `common/mod.rs` 头注）。
//! 样板来自 `mod common`（收敛前这两个文件的夹具区除一个前缀字符串外逐字相同）。
//! env 是进程级全局，全部用例串行（一把大锁包住整个测试体）。

use std::process::Command;
use std::sync::Arc;

use plugin_host::{PluginRegistry, RegistryToolSource};
use serde_json::{Value, json};
mod common;
use common::{call, env_lock, err_msg, load_activate, temp_root, workspace_dir};

fn git(cwd: &std::path::Path, argv: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(argv)
        .output()
        .expect("git spawn failed (is git on PATH?)");
    assert!(
        out.status.success(),
        "git {argv:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn memory_write_overwrite_undo_and_delete_undo_end_to_end() {
    let _env = env_lock().lock().unwrap();
    let root = temp_root("mem");
    std::fs::create_dir_all(&root).unwrap();
    unsafe { std::env::set_var("LOCOWRK_DATA_DIR", root.display().to_string()) };

    let registry = Arc::new(PluginRegistry::new());
    load_activate("memory-sqlite", "1.2.0", &registry);
    let source = RegistryToolSource::new(Arc::clone(&registry));

    // 库文件固定在数据根（模型无路径参数——无逃逸面）
    let db = root.join("memory.sqlite3");

    let write = |args: Value| {
        call(source.resolve("memory_write").unwrap().as_ref(), args).expect("memory_write ok")
    };
    let get = |key: &str| {
        call(
            source.resolve("memory_read").unwrap().as_ref(),
            json!({ "key": key }),
        )
        .expect("memory_read ok")
    };

    let out1 = write(json!({ "key": "user.lang", "value": "zh", "tags": "pref" }));
    assert_eq!(out1["created"], json!(true));
    assert_eq!(get("user.lang")["value"], json!("zh"));

    // 覆盖写携带被覆盖条目 → undo 契约还原
    let out2 = write(json!({ "key": "user.lang", "value": "en" }));
    assert_eq!(out2["previous"]["value"], json!("zh"));
    call(
        source.resolve("memory_undo").unwrap().as_ref(),
        json!({ "call_id": "c1", "args": { "key": "user.lang" }, "output": out2 }),
    )
    .expect("undo put ok");
    assert_eq!(get("user.lang")["value"], json!("zh"));

    // delete → undo 整条还原
    let del = call(
        source.resolve("memory_delete").unwrap().as_ref(),
        json!({ "key": "user.lang" }),
    )
    .expect("memory_delete ok");
    assert_eq!(del["deleted"], json!(true));
    assert_eq!(get("user.lang")["found"], json!(false));
    call(
        source.resolve("memory_undo").unwrap().as_ref(),
        json!({ "call_id": "c2", "args": { "key": "user.lang" }, "output": del }),
    )
    .expect("undo delete ok");
    assert_eq!(get("user.lang")["value"], json!("zh"));

    // 持久化实体在数据根生成
    assert!(db.is_file(), "memory db expected at {}", db.display());

    unsafe { std::env::remove_var("LOCOWRK_DATA_DIR") };
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn git_status_commit_undo_push_end_to_end() {
    let _env = env_lock().lock().unwrap();
    let root = temp_root("git");
    std::fs::create_dir_all(&root).unwrap();
    unsafe {
        std::env::set_var("LOCOWRK_DATA_DIR", root.join("data").display().to_string());
        std::env::set_var(
            "LOCOWRK_FS_ROOTS",
            root.canonicalize().unwrap().display().to_string(),
        );
    }

    let repo = root.join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-b", "main"]);
    git(&repo, &["config", "user.name", "Test"]);
    git(&repo, &["config", "user.email", "test@example.com"]);
    git(&repo, &["config", "commit.gpgsign", "false"]);
    std::fs::write(repo.join("a.txt"), "v0\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "base"]);
    let base = git(&repo, &["rev-parse", "HEAD"]).trim().to_string();
    let repo_str = repo.display().to_string();

    let registry = Arc::new(PluginRegistry::new());
    load_activate("tool-git", "1.1.0", &registry);
    let source = RegistryToolSource::new(Arc::clone(&registry));

    // status：干净基线
    let st = call(
        source.resolve("git_status").unwrap().as_ref(),
        json!({ "repo": repo_str }),
    )
    .expect("git_status ok");
    assert_eq!(st["clean"], json!(true));
    assert_eq!(st["branch"], json!("main"));

    // commit → undo 契约（soft reset：工作区保留、改动留暂存区）
    std::fs::write(repo.join("b.txt"), "v1\n").unwrap();
    let out = call(
        source.resolve("git_commit").unwrap().as_ref(),
        json!({ "repo": repo_str, "message": "second", "add": ["b.txt"] }),
    )
    .expect("git_commit ok");
    assert_eq!(
        out["commit"],
        json!(git(&repo, &["rev-parse", "HEAD"]).trim())
    );
    call(
        source.resolve("git_undo_commit").unwrap().as_ref(),
        json!({ "call_id": "c1", "args": {}, "output": out }),
    )
    .expect("git_undo_commit ok");
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]).trim(), base);
    assert!(
        repo.join("b.txt").is_file(),
        "soft reset keeps the worktree"
    );
    let st = call(
        source.resolve("git_status").unwrap().as_ref(),
        json!({ "repo": repo_str }),
    )
    .unwrap();
    assert_eq!(
        st["entries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["path"] == json!("b.txt"))
            .expect("b.txt staged after undo")["x"],
        json!("A")
    );

    // push 到本地 bare 远端（离线全链路）
    let bare = root.join("origin.git");
    git(&root, &["init", "--bare", &bare.display().to_string()]);
    git(
        &repo,
        &["remote", "add", "origin", &bare.display().to_string()],
    );
    let pushed = call(
        source.resolve("git_push").unwrap().as_ref(),
        json!({ "repo": repo_str, "branch": "main" }),
    )
    .expect("git_push ok");
    assert_eq!(pushed["pushed"], json!(true));
    assert!(
        bare.join("refs").join("heads").join("main").is_file(),
        "bare remote received main"
    );

    // 恶意：允许根外的仓库拒绝
    let outside = temp_root("git-outside");
    std::fs::create_dir_all(&outside).unwrap();
    git(&outside, &["init", "-b", "main"]);
    let err = err_msg(
        call(
            source.resolve("git_status").unwrap().as_ref(),
            json!({ "repo": outside.display().to_string() }),
        )
        .unwrap_err(),
    );
    assert!(err.contains("outside the allowed roots"), "{err}");

    unsafe {
        std::env::remove_var("LOCOWRK_DATA_DIR");
        std::env::remove_var("LOCOWRK_FS_ROOTS");
    }
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&outside);
}

#[test]
fn mcp_bridge_discovers_and_calls_fake_server() {
    let _env = env_lock().lock().unwrap();

    // example 替身要单独构建（cargo build 默认不含 examples）
    let status = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
        .args([
            "build",
            "-p",
            "tool-mcp",
            "--example",
            "fake_mcp_server",
            "--quiet",
        ])
        .status()
        .expect("spawn cargo");
    assert!(status.success(), "build fake_mcp_server failed");
    let fake = workspace_dir()
        .join("target/debug/examples")
        .join(if cfg!(windows) {
            "fake_mcp_server.exe"
        } else {
            "fake_mcp_server"
        });
    assert!(fake.is_file(), "fake server missing at {}", fake.display());

    let root = temp_root("mcp");
    std::fs::create_dir_all(&root).unwrap();
    unsafe { std::env::set_var("LOCOWRK_DATA_DIR", root.display().to_string()) };
    std::fs::write(
        root.join("mcp.json"),
        json!({
            "servers": {
                "fake": {
                    "command": fake.display().to_string(),
                    "args": [],
                    "side_effect": "pure",
                    "tool_effects": { "boom": "irreversible" }
                }
            }
        })
        .to_string(),
    )
    .unwrap();

    // env 必须在 load 之前就位：describe 阶段完成发现并冻结工具清单
    let registry = Arc::new(PluginRegistry::new());
    load_activate("tool-mcp", "1.1.0", &registry);
    let source = RegistryToolSource::new(Arc::clone(&registry));

    let tools = source.list_tools();
    assert!(
        tools.iter().any(|t| t.name == "mcp_fake_echo"),
        "echo tool should be discovered, got: {:?}",
        tools.iter().map(|t| &t.name).collect::<Vec<_>>()
    );
    let echo = tools.iter().find(|t| t.name == "mcp_fake_echo").unwrap();
    assert!(matches!(echo.side_effect, plugin_api::SideEffect::Pure));
    let boom = tools.iter().find(|t| t.name == "mcp_fake_boom").unwrap();
    assert!(matches!(
        boom.side_effect,
        plugin_api::SideEffect::Irreversible
    ));

    // 调用：回显 + 工具级错误作为数据（tool-fetch 惯例）
    let out = call(
        source.resolve("mcp_fake_echo").unwrap().as_ref(),
        json!({ "text": "hi" }),
    )
    .expect("mcp call ok");
    assert_eq!(out["content"], json!("echo: hi"));
    let out = call(source.resolve("mcp_fake_boom").unwrap().as_ref(), json!({}))
        .expect("tool-level error is data");
    assert_eq!(out["is_error"], json!(true));
    assert_eq!(out["content"], json!("boom!"));

    // 管理工具
    let st =
        call(source.resolve("mcp_status").unwrap().as_ref(), json!({})).expect("mcp_status ok");
    assert_eq!(st["servers"][0]["name"], json!("fake"));
    assert_eq!(st["servers"][0]["connected"], json!(true));

    unsafe { std::env::remove_var("LOCOWRK_DATA_DIR") };
    let _ = std::fs::remove_dir_all(&root);
}
