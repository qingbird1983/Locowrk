//! git 三态用例：正常（status/diff/log/commit→undo/push 到本地 bare 远端）/
//! 边界（空仓库、缺暂存说明、HEAD 移动后拒绝）/ 恶意（允许根外仓库、仓库
//! 外路径、refname 注入、缺 output 契约）。涉 env 用例持锁串行；需要 PATH
//! 里有 git（Git Bash / Git for Windows 环境均预装）。

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use super::{read, write};
use plugin_api::serde_json::{Value, json};

static ENV_LOCK: Mutex<()> = Mutex::new(());

fn temp_root(tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir =
        std::env::temp_dir().join(format!("locowrk-git-{tag}-{nanos}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // 数据根一并指向临时目录：allowed_roots 缺失即创建，不能在真 HOME 里
    // 留垃圾；FS_ROOTS = 临时根，隔离真实环境
    // SAFETY: 全部用例经 ENV_LOCK 串行；Rust 2024 起 set_var 为 unsafe
    unsafe {
        std::env::set_var("LOCOWRK_DATA_DIR", dir.join("data"));
        std::env::set_var("LOCOWRK_FS_ROOTS", dir.canonicalize().unwrap());
    }
    dir
}

fn s(p: &Path) -> String {
    p.to_str().unwrap().to_string()
}

fn git(cwd: &Path, argv: &[&str]) -> String {
    let out = std::process::Command::new("git")
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

/// 初始化带身份配置的测试仓库（commit 不依赖全局 git config）。
fn init_repo(root: &Path, tag: &str) -> PathBuf {
    let repo = root.join(tag);
    std::fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-b", "main"]);
    git(&repo, &["config", "user.name", "Test"]);
    git(&repo, &["config", "user.email", "test@example.com"]);
    git(&repo, &["config", "commit.gpgsign", "false"]);
    repo
}

fn find_entry<'a>(st: &'a Value, path: &str) -> Option<&'a Value> {
    st["entries"]
        .as_array()?
        .iter()
        .find(|e| e["path"] == json!(path))
}

#[test]
fn status_diff_log_roundtrip() {
    let _g = ENV_LOCK.lock().unwrap();
    let root = temp_root("read");
    let repo = init_repo(&root, "r1");
    std::fs::write(repo.join("a.txt"), "hello\n").unwrap();
    write::git_commit(
        &json!({ "repo": s(&repo), "message": "c1", "add": ["a.txt"] }),
        std::ptr::null(),
    )
    .unwrap();

    let st = read::git_status(&json!({ "repo": s(&repo) }), std::ptr::null()).unwrap();
    assert_eq!(st["clean"], json!(true));
    assert_eq!(st["branch"], json!("main"));
    assert_eq!(st["ahead"], Value::Null);

    // 变更 + 新文件 → dirty，未跟踪条目 x/y 均为 '?'
    std::fs::write(repo.join("a.txt"), "changed\n").unwrap();
    std::fs::write(repo.join("new.txt"), "n\n").unwrap();
    let st = read::git_status(&json!({ "repo": s(&repo) }), std::ptr::null()).unwrap();
    assert_eq!(st["clean"], json!(false));
    let untracked = find_entry(&st, "new.txt").expect("untracked entry");
    assert_eq!(untracked["x"], json!("?"));
    assert_eq!(untracked["y"], json!("?"));

    let d = read::git_diff(&json!({ "repo": s(&repo) }), std::ptr::null()).unwrap();
    assert!(d["diff"].as_str().unwrap().contains("changed"));
    assert_eq!(d["truncated"], json!(false));

    let staged = read::git_diff(
        &json!({ "repo": s(&repo), "staged": true }),
        std::ptr::null(),
    )
    .unwrap();
    assert_eq!(staged["diff"], json!(""));

    let lg = read::git_log(&json!({ "repo": s(&repo) }), std::ptr::null()).unwrap();
    assert_eq!(lg["empty_repo"], json!(false));
    let commits = lg["commits"].as_array().unwrap();
    assert_eq!(commits.len(), 1);
    assert_eq!(commits[0]["subject"], json!("c1"));
    assert_eq!(commits[0]["author"], json!("Test"));
}

#[test]
fn log_on_empty_repo_is_empty_not_error() {
    let _g = ENV_LOCK.lock().unwrap();
    let root = temp_root("empty");
    let repo = init_repo(&root, "r2");
    let lg = read::git_log(&json!({ "repo": s(&repo) }), std::ptr::null()).unwrap();
    assert_eq!(lg["empty_repo"], json!(true));
    assert_eq!(lg["commits"].as_array().unwrap().len(), 0);
}

#[test]
fn commit_roundtrip_then_undo_restores_head() {
    let _g = ENV_LOCK.lock().unwrap();
    let root = temp_root("commit");
    let repo = init_repo(&root, "r3");

    std::fs::write(repo.join("a.txt"), "v0\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "base"]);
    let base = git(&repo, &["rev-parse", "HEAD"]).trim().to_string();

    std::fs::write(repo.join("b.txt"), "v1\n").unwrap();
    let out = write::git_commit(
        &json!({ "repo": s(&repo), "message": "second", "add": ["b.txt"] }),
        std::ptr::null(),
    )
    .unwrap();
    assert_eq!(
        out["commit"],
        json!(git(&repo, &["rev-parse", "HEAD"]).trim())
    );

    // undo（平铺形态）：HEAD 回到 base，工作区文件保留且 b.txt 留在暂存区
    let undo = write::git_undo_commit(&out, std::ptr::null()).unwrap();
    assert_eq!(undo["undone"], json!(true));
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]).trim(), base);
    assert!(repo.join("b.txt").is_file());
    let st = read::git_status(&json!({ "repo": s(&repo) }), std::ptr::null()).unwrap();
    let staged = find_entry(&st, "b.txt").expect("b.txt staged after undo");
    assert_eq!(staged["x"], json!("A"));
}

#[test]
fn undo_refuses_when_head_has_moved() {
    let _g = ENV_LOCK.lock().unwrap();
    let root = temp_root("undo-moved");
    let repo = init_repo(&root, "r4");

    std::fs::write(repo.join("a.txt"), "v0\n").unwrap();
    write::git_commit(
        &json!({ "repo": s(&repo), "message": "c1", "add": ["a.txt"] }),
        std::ptr::null(),
    )
    .unwrap();
    std::fs::write(repo.join("a.txt"), "v2\n").unwrap();
    let out_c1 = write::git_commit(
        &json!({ "repo": s(&repo), "message": "c2", "add": ["a.txt"] }),
        std::ptr::null(),
    )
    .unwrap();

    // out_c1 记录的是 c2；之后又提交了 c3，HEAD 已不再是 c2——对错位
    // 提交执行 soft reset 会改写错误的历史，必须拒绝
    std::fs::write(repo.join("a.txt"), "v3\n").unwrap();
    write::git_commit(
        &json!({ "repo": s(&repo), "message": "c3", "add": ["a.txt"] }),
        std::ptr::null(),
    )
    .unwrap();
    let err = write::git_undo_commit(&json!({ "output": out_c1 }), std::ptr::null()).unwrap_err();
    assert!(err.contains("HEAD has moved"), "got: {err}");
}

#[test]
fn undo_refuses_root_commit_explicitly() {
    let _g = ENV_LOCK.lock().unwrap();
    let root = temp_root("undo-root");
    let repo = init_repo(&root, "r7");

    // 仓库的第一条提交 = 根提交（无父）：此前 `reset --soft HEAD~1` 隐性
    // 失败（git 报错晦涩，模型反复尝试）；现在必须显式拒绝并给出路
    std::fs::write(repo.join("a.txt"), "v0\n").unwrap();
    let out = write::git_commit(
        &json!({ "repo": s(&repo), "message": "root", "add": ["a.txt"] }),
        std::ptr::null(),
    )
    .unwrap();
    let err = write::git_undo_commit(&json!({ "output": out }), std::ptr::null()).unwrap_err();
    assert!(err.contains("ROOT commit"), "got: {err}");
    assert!(err.contains("update-ref"), "got: {err}");
    // 拒绝是无操作：HEAD 仍在根提交
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]).trim(), out["commit"]);
}

#[test]
fn push_to_local_bare_remote() {
    let _g = ENV_LOCK.lock().unwrap();
    let root = temp_root("push");
    let repo = init_repo(&root, "r5");

    std::fs::write(repo.join("a.txt"), "v0\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "base"]);

    let bare = root.join("origin.git");
    git(&root, &["init", "--bare", &s(&bare)]);
    git(&repo, &["remote", "add", "origin", &s(&bare)]);

    let out = write::git_push(
        &json!({ "repo": s(&repo), "branch": "main" }),
        std::ptr::null(),
    )
    .unwrap();
    assert_eq!(out["pushed"], json!(true));
    assert_eq!(out["remote"], json!("origin"));
    assert!(bare.join("refs").join("heads").join("main").is_file());
}

#[test]
fn push_rejects_malformed_refnames() {
    let _g = ENV_LOCK.lock().unwrap();
    let root = temp_root("push-evil");
    let repo = init_repo(&root, "r6");

    // argv 选项注入形态：`-oProxyCommand`、空、含点段 / 空格 / .lock 结尾
    for branch in ["-oProxyCommand=calc", "", "a..b", "has space", "main.lock"] {
        let err = write::git_push(
            &json!({ "repo": s(&repo), "branch": branch }),
            std::ptr::null(),
        )
        .unwrap_err();
        assert!(
            err.contains("unsupported characters"),
            "branch `{branch}`: {err}"
        );
    }
    let err = write::git_push(
        &json!({ "repo": s(&repo), "remote": "$(x)", "branch": "main" }),
        std::ptr::null(),
    )
    .unwrap_err();
    assert!(err.contains("unsupported characters"), "got: {err}");
    assert!(write::git_push(&json!({ "repo": s(&repo) }), std::ptr::null()).is_err());
}

#[test]
fn repo_outside_allowed_roots_rejected() {
    let _g = ENV_LOCK.lock().unwrap();
    let root = temp_root("outside-root");

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let outside = std::env::temp_dir().join(format!("locowrk-git-noroot-{nanos}"));
    std::fs::create_dir_all(&outside).unwrap();
    git(&outside, &["init", "-b", "main"]);

    let err = read::git_status(&json!({ "repo": s(&outside) }), std::ptr::null()).unwrap_err();
    assert!(err.contains("outside the allowed roots"), "got: {err}");
    // 清理不被 git 误伤的空仓库
    let _ = std::fs::remove_dir_all(&outside);
    let _ = root;
}

#[test]
fn commit_rejects_paths_outside_repo() {
    let _g = ENV_LOCK.lock().unwrap();
    let root = temp_root("outside-repo");
    let repo = init_repo(&root, "r7");

    // 在允许根内、仓库外放一个文件，`../` 相对路径不允许把仓库外的东西
    // 提交进来
    std::fs::write(root.join("outside.txt"), "x\n").unwrap();
    let err = write::git_commit(
        &json!({
            "repo": s(&repo), "message": "evil", "add": ["../outside.txt"]
        }),
        std::ptr::null(),
    )
    .unwrap_err();
    assert!(err.contains("outside the repository"), "got: {err}");

    // diff/log 的 path 限定同样拒绝越界
    let err = read::git_diff(
        &json!({ "repo": s(&repo), "path": "../outside.txt" }),
        std::ptr::null(),
    )
    .unwrap_err();
    assert!(err.contains("outside the repository"), "got: {err}");
}

#[test]
fn commit_arg_validation() {
    let _g = ENV_LOCK.lock().unwrap();
    let root = temp_root("commit-args");
    let repo = init_repo(&root, "r8");
    std::fs::write(repo.join("a.txt"), "v\n").unwrap();

    // 缺暂存说明 / 双暂存说明 / 空提交说明 / 缺 repo 字段
    assert!(
        write::git_commit(
            &json!({ "repo": s(&repo), "message": "x" }),
            std::ptr::null()
        )
        .is_err()
    );
    assert!(
        write::git_commit(
            &json!({ "repo": s(&repo), "message": "x", "add": ["a.txt"], "add_all": true }),
            std::ptr::null()
        )
        .is_err()
    );
    assert!(
        write::git_commit(
            &json!({ "repo": s(&repo), "message": "  ", "add_all": true }),
            std::ptr::null()
        )
        .is_err()
    );
    assert!(
        write::git_commit(
            &json!({ "message": "x", "add_all": true }),
            std::ptr::null()
        )
        .is_err()
    );
    // 校验失败时不得有任何提交发生
    assert_eq!(
        read::git_log(&json!({ "repo": s(&repo) }), std::ptr::null()).unwrap()["empty_repo"],
        json!(true)
    );
}

#[test]
fn undo_without_contract_rejected() {
    let _g = ENV_LOCK.lock().unwrap();
    let root = temp_root("undo-args");
    let repo = init_repo(&root, "r9");

    assert!(write::git_undo_commit(&json!({}), std::ptr::null()).is_err());
    let _ = repo;
}
