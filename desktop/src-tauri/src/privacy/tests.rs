//! 隐私门扫描器单元测试：类别命中、组件边界、误报压制、宁漏勿错杀。

use serde_json::json;

use super::check;

#[test]
fn credential_directories_hit_by_component() {
    // 组件边界：目录内 / 目录本身命中；同名后缀目录不算
    assert_eq!(
        check("shell_read", &json!({"path": "C:/Users/x/.ssh/id_rsa"})),
        Some("ssh")
    );
    assert_eq!(check("fs_list", &json!({"path": "~\\.ssh"})), Some("ssh"));
    assert_eq!(
        check("fs_read", &json!({"path": "/home/x/project/x.ssh"})),
        None
    );
    assert_eq!(
        check("fs_read", &json!({"path": "C:/u/.aws/credentials"})),
        Some("aws")
    );
    assert_eq!(
        check("fs_read", &json!({"path": "C:/u/.gnupg/pubring.kbx"})),
        Some("gpg")
    );
    assert_eq!(
        check("fs_read", &json!({"path": "C:/u/.kube/config"})),
        Some("kube")
    );
}

#[test]
fn standalone_credential_files_hit() {
    assert_eq!(
        check("fs_read", &json!({"path": "C:/u/.git-credentials"})),
        Some("creds_file")
    );
    assert_eq!(
        check("fs_read", &json!({"path": "~/.netrc"})),
        Some("creds_file")
    );
    assert_eq!(
        check("fs_read", &json!({"path": "C:/u/.docker/config.json"})),
        Some("docker")
    );
    assert_eq!(
        check("fs_read", &json!({"path": "C:/u/.docker/cli.json"})),
        None
    );
}

#[test]
fn browser_stores_need_vendor_scope() {
    let cookies = r"C:\Users\x\AppData\Local\Google\Chrome\User Data\Default\Network\Cookies";
    assert_eq!(check("fs_read", &json!({"path": cookies})), Some("browser"));
    let logins = r"C:\Users\x\AppData\Roaming\Mozilla\Firefox\Profiles\abc.default\logins.json";
    assert_eq!(
        check("shell_read", &json!({"path": logins})),
        Some("browser")
    );
    // 无厂商目录限定 = 不拦（宁漏勿错杀）
    assert_eq!(
        check("fs_read", &json!({"path": "F:/proj/assets/cookies"})),
        None
    );
}

#[test]
fn command_scans_catch_credential_and_destructive() {
    // 命令文本里的凭据路径也要拦（shell 是 Irreversible，但完全档会放行）
    assert_eq!(
        check("shell", &json!({"command": "cat ~/.ssh/id_rsa"})),
        Some("ssh")
    );
    assert_eq!(
        check("shell", &json!({"command": "vaultcmd /list"})),
        Some("os_store")
    );
    assert_eq!(
        check("shell", &json!({"command": "procdump -ma lsass lsass.dmp"})),
        Some("cred_dump")
    );
    assert_eq!(
        check("shell", &json!({"command": "format c: /q"})),
        Some("catastrophic")
    );
    assert_eq!(
        check("shell", &json!({"command": "dd if=/dev/zero of=/dev/sda"})),
        Some("catastrophic")
    );
    assert_eq!(
        check("shell", &json!({"command": "rm -rf / --no-preserve-root"})),
        Some("catastrophic")
    );
    assert_eq!(
        check("shell", &json!({"command": "rd /s /q C:\\ /q"})),
        Some("catastrophic")
    );
    // 普通词组不误伤
    assert_eq!(
        check(
            "shell",
            &json!({"command": "cargo run -- format json output"})
        ),
        None
    );
    assert_eq!(
        check("shell", &json!({"command": "rm -rf target && echo done"})),
        None,
        "工作区内的递归删除不拦（档位兜底）"
    );
}

#[test]
fn env_dev_workflows_stay_silent() {
    // .env 是开发日常（ZCode 同款不拦）；node_modules 清理不拦
    assert_eq!(check("fs_read", &json!({"path": "F:/proj/.env"})), None);
    assert_eq!(
        check("shell", &json!({"command": "rm -rf node_modules dist"})),
        None
    );
}

#[test]
fn file_body_fields_are_skipped() {
    // 内容里提到 .ssh ≠ 碰凭据：content/patch 等文件体字段不扫
    let args = json!({
        "path": "F:/proj/deploy.sh",
        "content": "scp id_rsa backup to /tmp\nssh-keygen -f ~/.ssh/x"
    });
    assert_eq!(check("fs_write", &args), None);
}

#[test]
fn nested_args_are_scanned() {
    let args = json!({
        "command": "run-job",
        "env": { "KEY": "C:/users/x/.aws/credentials" }
    });
    assert_eq!(check("shell", &args), Some("aws"));
    // 超长串跳过（参数区大 JSON 不因一句历史文本误触）
    let long = format!("pad {}", "x".repeat(600));
    assert_eq!(
        check("shell", &json!({"command": "echo", "note": long})),
        None
    );
}
