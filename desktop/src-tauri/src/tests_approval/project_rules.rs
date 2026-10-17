//! 项目粘性规则（自 `tests_approval.rs` 批 F-14 拆出，用例本体逐字未改）：allow 规则在 Ask 档免卡 /
//! deny 规则在 Auto 档仍拦 / 精确命令规则落盘 / 无工作区时跳过持久化。

use super::*;

use crate::permission_mode::PermissionMode;
use serde_json::json;
use std::sync::Arc;

/// ⑤ 项目允许规则：Ask 档下命中的命令免卡直行；未命中的照常弹卡。
#[tokio::test]
async fn project_allow_rule_skips_card_in_ask_mode() {
    let (hub, rules, _modes, root) = hub_v145("allow", PermissionMode::Ask);
    rules
        .add_allow("ws-A", "shell", Some("npm test".into()))
        .unwrap();

    // 命中：直行（rule:project-allow 审计理由，不带 mode: 前缀 = 前端照常渲染记录条）
    let d = hub
        .resolve(&shell_req("s1", "npm test"), &shell_spec())
        .await;
    assert!(d.allowed);
    assert_eq!(d.reason.as_deref(), Some("rule:project-allow"));

    // 未命中：弹卡（守夜 80ms 后自动拒绝）
    let d = hub
        .resolve(&shell_req("s1", "npm run build"), &shell_spec())
        .await;
    assert!(!d.allowed);
    let _ = std::fs::remove_dir_all(&root);
}

/// ③ 项目拒绝规则先于模式放行：Auto 档也拦（比 ZCode 的 yolo 先于
/// deny 更安全，有意偏离记档）。
#[tokio::test]
async fn project_deny_rule_blocks_even_in_auto_mode() {
    let (hub, rules, _modes, root) = hub_v145("deny", PermissionMode::Auto);
    rules
        .add_allow("ws-A", "shell", Some("rm -rf build".into()))
        .unwrap();
    // UI 不产 deny——手改 JSON 注入（文件面生效语义）
    let f = root
        .join("data")
        .join("permissions")
        .join(rules_file_of("ws-A"));
    let mut v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&f).unwrap()).unwrap();
    v["deny"] = json!([{ "tool": "shell", "content": "rm -rf build" }]);
    std::fs::write(&f, serde_json::to_string_pretty(&v).unwrap()).unwrap();

    let d = hub
        .resolve(&shell_req("s1", "rm -rf build"), &shell_spec())
        .await;
    assert!(!d.allowed, "deny 规则必须拦下 Auto 档");
    assert!(d.reason.unwrap_or_default().contains("项目权限规则拒绝"));
    let _ = std::fs::remove_dir_all(&root);
}

/// 「本项目始终允许」落盘：scope=project 决策持久化精确命令规则（shell），
/// 第二次同命令免卡直行。
#[tokio::test]
async fn project_scope_persists_exact_command_rule() {
    let (hub, rules, _modes, root) = hub_v145("persist", PermissionMode::Ask);

    // 第一次：弹卡（后台抓 request_id 后按 project 放行）
    let decider = Arc::clone(&hub);
    let worker = Decider::new(decider).scope("project").spawn(|_| {});
    let d = hub
        .resolve(&shell_req("s1", "node server.js"), &shell_spec())
        .await;
    worker.await.unwrap();
    assert!(d.allowed, "决策后 step 醒来放行");
    // 精确命令规则落盘
    assert!(
        rules
            .verdicts("ws-A", "shell", Some("node server.js"))
            .allowed,
        "scope=project 必须持久化规则"
    );
    assert!(
        !rules
            .verdicts("ws-A", "shell", Some("node other.js"))
            .allowed,
        "v1 粘性 = 精确命令，不同命令不得被放宽"
    );
    // 第二次同命令：免卡直行
    let d = hub
        .resolve(&shell_req("s1", "node server.js"), &shell_spec())
        .await;
    assert!(d.allowed);
    assert_eq!(d.reason.as_deref(), Some("rule:project-allow"));
    let _ = std::fs::remove_dir_all(&root);
}

/// 未归属会话的 project 决策 = 跳过持久化（当次放行仍生效，ZCode 同款）。
#[tokio::test]
async fn project_scope_without_workspace_skips_persistence() {
    let (hub, rules, _modes, root) = hub_v145("nows", PermissionMode::Ask);
    let decider = Arc::clone(&hub);
    let worker = Decider::new(decider).scope("project").spawn(|_| {});
    let d = hub
        .resolve(&shell_req("other-session", "node server.js"), &shell_spec())
        .await;
    worker.await.unwrap();
    assert!(d.allowed, "当次放行不受持久化缺席影响");
    assert!(
        !rules
            .verdicts("ws-A", "shell", Some("node server.js"))
            .allowed
    );
    let _ = std::fs::remove_dir_all(&root);
}
