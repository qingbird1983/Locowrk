//! 子会话审批改道（自 `tests_approval.rs` 批 F-14 拆出，用例本体逐字未改）：子会话请求上溯
//! 父根审批 / 子会话继承父权限档。

use super::*;

use crate::permission_mode::PermissionMode;

/// 子会话审批改道：{parent}-sub-{n} 的豁免/规则/权限档全部取父根会话。
#[tokio::test]
async fn child_session_routes_to_parent_root() {
    let (hub, rules, _modes, root) = hub_v145("child", PermissionMode::Ask);
    rules
        .add_allow("ws-A", "shell", Some("node server.js".into()))
        .unwrap();

    // 豁免种在父会话上——改道前子会话 id 全新永不命中（v2.144 缺陷）
    hub.decide_seed_for_test("s1", "shell");
    let d = hub
        .resolve(&shell_req("s1-sub-1", "cargo build"), &shell_spec())
        .await;
    assert!(d.allowed, "子会话必须命中父会话豁免");

    // 规则同理：子会话发起的 shell 调用命中 ws-A 的粘性规则
    let d = hub
        .resolve(&shell_req("s1-sub-2", "node server.js"), &shell_spec())
        .await;
    assert!(d.allowed);
    assert_eq!(d.reason.as_deref(), Some("rule:project-allow"));
    let _ = std::fs::remove_dir_all(&root);
}

/// 权限档继承：父会话显式 Plan，子会话非 Pure 调用按父档硬拒
/// （v2.144 缺陷 = 回落设置缺省 Ask，弹卡）。
#[tokio::test]
async fn child_session_inherits_parent_mode() {
    let (hub, _rules, modes, root) = hub_v145("childmode", PermissionMode::Ask);
    modes.set_session_mode("s1", PermissionMode::Plan).unwrap();
    let d = hub
        .resolve(&shell_req("s1-sub-1", "npm test"), &shell_spec())
        .await;
    assert!(!d.allowed, "子会话必须继承父会话 Plan 档");
    let _ = std::fs::remove_dir_all(&root);
}
