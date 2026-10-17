//! ㉘·B 权限模式决策表 + 外部路径门测试（tests.rs/tests_approval.rs 的
//! 400 行纪律续拆）：四档行为、external 卡载荷、扫描器单元。

use std::sync::Arc;

use kernel_agent::{ApprovalResolver, ToolCallReq};
use plugin_api::{CallId, SideEffect, ToolSpec};
use serde_json::json;

use crate::approval::ApprovalHub;
use crate::events::AgentEvent;
use crate::permission_mode::{PermissionMode, PermissionModeStore};
// 台账 T7：本文件三处「后台决断者」与「等人回填 oneshot 的那一步」都改用共享形状
// （上界由 `tests_approval/mod.rs` 统一定义，不再各写一份无界 loop）。
use crate::tests::{kind, Collector};
use crate::tests_approval::{bounded, Decider, DECIDER_BUDGET};

/// 临时会话目录 + 指定缺省档的存储。
fn store(tag: &str, default: PermissionMode) -> Arc<PermissionModeStore> {
    let dir = crate::testutil::work_dir(&format!("lw-perm-tests-{}", tag));
    Arc::new(PermissionModeStore::new(dir, default))
}

fn spec(side: SideEffect) -> ToolSpec {
    ToolSpec {
        name: "demo".into(),
        description: String::new(),
        schema: json!({ "type": "object" }),
        side_effect: side,
        exposure: Default::default(),
    }
}

fn req(tool: &str, args: serde_json::Value) -> ToolCallReq {
    ToolCallReq {
        id: CallId("c1".into()),
        tool: tool.into(),
        args,
        session_id: Some("s1".into()),
    }
}

/// 同步落卡即决（看到请求立刻放行/拒绝）的 hub 测试脚手架：返回 (hub, 事件收集器)。
fn hub_with_modes(
    tag: &str,
    mode: PermissionMode,
) -> (Arc<ApprovalHub>, Collector, Arc<PermissionModeStore>) {
    let sink = Collector::default();
    let modes = store(tag, mode);
    let hub = Arc::new(
        ApprovalHub::with_timeout(Arc::new(sink.clone()), std::time::Duration::ZERO)
            .with_modes(Arc::clone(&modes)),
    );
    (hub, sink, modes)
}

/// 决策表③ 模式放行：`完全` 档 Irreversible 直接放行（reason mode:auto），
/// 不推卡。
#[tokio::test]
async fn auto_mode_allows_without_card() {
    let (hub, sink, _m) = hub_with_modes("auto", PermissionMode::Auto);
    let d = hub
        .resolve(
            &req("shell", json!({ "command": "echo hi" })),
            &spec(SideEffect::Irreversible),
        )
        .await;
    assert!(d.allowed && d.reason.as_deref() == Some("mode:auto"));
    assert!(sink.events().iter().all(|e| kind(e) != "approval_request"));
}

/// 决策表③：`自动编辑` 档 Compensable 放行（mode:acceptEdits）；Irreversible
/// 仍落⑤ 决策卡。
#[tokio::test]
async fn accept_edits_allows_compensable_but_cards_irreversible() {
    let (hub, sink, _m) = hub_with_modes("accept", PermissionMode::AcceptEdits);
    let comp = hub
        .resolve(
            &req("fs_write", json!({ "path": "F:/ws/a/x.txt" })),
            &spec(SideEffect::Compensable {
                undo_tool: "fs_restore".into(),
            }),
        )
        .await;
    assert!(comp.allowed && comp.reason.as_deref() == Some("mode:acceptEdits"));

    let decider = Arc::clone(&hub);
    Decider::new(decider).deny().spawn(|_| {});
    // 本 hub 的超时是 ZERO（= 不起守夜），所以这一步也得有上界：
    // 卡片没出现 ⇒ 报"挂起"，而不是把整轮门禁等死。
    let irr = bounded(
        "hub.resolve",
        DECIDER_BUDGET,
        hub.resolve(
            &req("shell", json!({ "command": "echo hi" })),
            &spec(SideEffect::Irreversible),
        ),
    )
    .await;
    assert!(!irr.allowed, "自动编辑档不可逆动作仍要问");
    assert!(sink.events().iter().any(|e| kind(e) == "approval_request"));
}

/// 决策表① 计划档纵深防御：目录围栏漏网的非 Pure 调用硬拒、不推卡。
#[tokio::test]
async fn plan_mode_denies_without_card() {
    let (hub, sink, _m) = hub_with_modes("plan", PermissionMode::Plan);
    let d = hub
        .resolve(
            &req("shell", json!({ "command": "echo hi" })),
            &spec(SideEffect::Irreversible),
        )
        .await;
    assert!(!d.allowed);
    assert!(d.reason.unwrap_or_default().contains("计划模式只读"));
    assert!(sink.events().iter().all(|e| kind(e) != "approval_request"));
}

/// 决策表② 越界门：`完全` 档下越出全局声明根的命令仍强制弹卡（豁免也
/// 拦不住），载荷 external=true 且带会话 workspace（环境行如实）；
/// 声明根内的命令不弹卡（㉘·C 围栏让路——跨项目畅通）。
#[tokio::test]
async fn external_path_forces_card_even_in_auto() {
    let sink = Collector::default();
    let modes = store("external", PermissionMode::Auto);
    let sessions = crate::testutil::unique_dir("lw-perm-ext-sessions");
    let _ = std::fs::create_dir_all(sessions.join("ws"));
    // 归属 s1 → 工作区；围栏 = 全局声明根（数据根 + fsRoots），resolver 手工构造
    let ws_path = sessions.join("ws").canonicalize().unwrap();
    let ws_clone = ws_path.clone();
    let resolver: crate::workspace::SessionContextResolver = Arc::new(move |sid: &str| {
        let attributed = sid == "s1";
        plugin_api::ffi::CallContext {
            session_id: Some(sid.to_string()),
            workspace: attributed.then(|| ws_clone.to_string_lossy().into_owned()),
            fs_roots: if attributed {
                vec![ws_clone.to_string_lossy().into_owned()]
            } else {
                Vec::new()
            },
        }
    });
    // 超时从 `ZERO` 改成 500ms（批 F-24a）：ZERO = 不起守夜任务，一旦「声明
    // 根内不该弹卡」这条断言因为围栏逻辑退化而落进决策卡，本测试会**挂起**
    // 而不是失败（决断者无界轮询 + 没人回填 oneshot）——挂起的测试是零证明力
    // 还堵住整轮门禁。
    // 台账 T7 订正：躲挂起不该靠调短超时（那是把通用缺陷摊成一条用例的私人补丁），
    // 上界现在由 `Decider` + `bounded` 统一提供 ⇒ 这里恢复 `ZERO`（= 生产缺省那一档），
    // 500ms 这个魔法数从本文件消失；挂起风险由下面两处 bounded 兜住。
    let hub = Arc::new(
        ApprovalHub::with_timeout(Arc::new(sink.clone()), std::time::Duration::ZERO)
            .with_modes(Arc::clone(&modes))
            .with_workspace(Arc::clone(&resolver)),
    );
    // 先种豁免：模拟此前「允许本次对话」已发生——越界门必须无视它
    hub.decide_seed_for_test("s1", "shell");

    // 声明根内（工作区自身）：不弹卡，`完全` 档直接放行
    let inside = ws_path.to_string_lossy().replace('\\', "/");
    let d = bounded(
        "hub.resolve（声明根内）",
        DECIDER_BUDGET,
        hub.resolve(
            &req(
                "shell",
                json!({ "command": format!("type {inside}/a.txt") }),
            ),
            &spec(SideEffect::Irreversible),
        ),
    )
    .await;
    assert!(d.allowed, "声明根内命令在完全档直接放行（围栏让路）");

    Decider::new(Arc::clone(&hub))
        .deny()
        .spawn(|r| assert!(r.external, "decider 侧也看到 external 标记"));
    let outside = r"X:\outside\secret.txt";
    let d = bounded(
        "hub.resolve（越界卡）",
        DECIDER_BUDGET,
        hub.resolve(
            &req("shell", json!({ "command": format!("type {outside}") })),
            &spec(SideEffect::Irreversible),
        ),
    )
    .await;
    assert!(!d.allowed, "external 卡被拒收束（测试决断者拒绝）");
    let p = sink.events().iter().find_map(|e| match e {
        AgentEvent::ApprovalRequest(p) => Some(p.clone()),
        _ => None,
    });
    let p = p.expect("external card emitted");
    assert!(p.external, "载荷必须标 external");
    assert!(p.workspace.as_deref().is_some(), "载荷带会话真实工作区");
    assert!(hub.pending().is_empty());
}

/// 决策表② 隐私门：`完全` 档下触及凭据目标（含 Pure 只读工具）仍强制
/// 弹卡——privacy.rs 清单压过模式放行与豁免，载荷带类别键。
#[tokio::test]
async fn privacy_gate_forces_card_even_in_auto_and_pure() {
    let sink = Collector::default();
    let modes = store("privacy", PermissionMode::Auto);
    let hub = Arc::new(
        ApprovalHub::with_timeout(Arc::new(sink.clone()), std::time::Duration::ZERO)
            .with_modes(Arc::clone(&modes)),
    );
    // 豁免已发生也拦不住
    hub.decide_seed_for_test("s1", "shell_read");

    Decider::new(Arc::clone(&hub))
        .deny()
        .spawn(|r| assert_eq!(r.sensitive.as_deref(), Some("ssh")));
    // Pure 工具（决策表③ 原本直通）也被隐私门拦下
    // 本 hub 超时 = ZERO（不起守夜）⇒ 这一步必须有界，否则没人点卡就永远等。
    let d = bounded(
        "hub.resolve（隐私卡）",
        DECIDER_BUDGET,
        hub.resolve(
            &req("shell_read", json!({ "path": "C:/Users/x/.ssh/id_rsa" })),
            &spec(SideEffect::Pure),
        ),
    )
    .await;
    assert!(!d.allowed, "隐私卡被拒收束（测试决断者拒绝）");
    let p = sink.events().iter().find_map(|e| match e {
        AgentEvent::ApprovalRequest(p) => Some(p.clone()),
        _ => None,
    });
    let p = p.expect("privacy card emitted");
    assert_eq!(p.sensitive.as_deref(), Some("ssh"), "载荷带类别键");
    assert!(!p.external, "隐私门 ≠ 越界门");
    assert!(hub.pending().is_empty());
}

/// 决策表③ 只读直通（㉘·C 新面）：Pure 在任何档位（含询问/计划）不经卡
/// 直通，reason mode:pure / mode:plan-readonly。
#[tokio::test]
async fn pure_tools_pass_through_all_modes() {
    for (tag, mode, reason) in [
        ("pure-ask", PermissionMode::Ask, "mode:pure"),
        ("pure-plan", PermissionMode::Plan, "mode:plan-readonly"),
        ("pure-accept", PermissionMode::AcceptEdits, "mode:pure"),
    ] {
        let (hub, sink, _m) = hub_with_modes(tag, mode);
        let d = hub
            .resolve(
                &req("fs_read", json!({ "path": "F:/ws/a/x.txt" })),
                &spec(SideEffect::Pure),
            )
            .await;
        assert!(d.allowed, "{tag}: Pure 直通");
        assert_eq!(d.reason.as_deref(), Some(reason), "{tag}");
        assert!(sink.events().iter().all(|e| kind(e) != "approval_request"));
    }
}

/// 扫描器单元：盘符/UNC 提取、分量边界、围栏内外判定。
#[test]
fn scanner_extracts_and_judges_paths() {
    use crate::approval::{extract_windows_abs_paths, is_outside};
    let toks = extract_windows_abs_paths(
        r#"cd "F:\ws\a" && type F:\ws\a\x.txt C:\win\system.ini \\srv\share\f"#,
    );
    assert_eq!(toks.len(), 4, "{toks:?}");
    let roots = vec![
        std::path::PathBuf::from("F:\\ws\\a"),
        std::path::PathBuf::from("C:\\Users\\x\\.Locowrk"),
    ];
    assert!(!is_outside("F:/ws/a/x.txt", &roots));
    assert!(!is_outside("F:\\WS\\A\\sub\\y", &roots), "大小写不敏感");
    assert!(is_outside("C:\\Windows\\system.ini", &roots));
    // 分量边界：F:\ab 不算 F:\a 内
    assert!(is_outside("F:\\ws\\ab", &roots));

    // CJK 回归：多字节字符不得打崩扫描（续字节切片 panic），纯中文命令
    // 无路径 token，含中文的路径 token 照常提取
    assert!(extract_windows_abs_paths("echo 你好").is_empty());
    assert!(extract_windows_abs_paths(r#"git commit -m "修复""#).is_empty());
    assert!(extract_windows_abs_paths("type 中文文件.txt").is_empty());
    assert_eq!(
        extract_windows_abs_paths(r#"type F:\数据\文件.txt"#),
        vec![r"F:\数据\文件.txt".to_string()]
    );
    assert_eq!(
        extract_windows_abs_paths(r#"\\服务器\share\f"#),
        vec![r"\\服务器\share\f".to_string()]
    );
}

/// 决策表④ 豁免仍按既有语义放行（非 external 路径）。
#[tokio::test]
async fn exemption_still_allows_internal_calls() {
    let (hub, sink, _m) = hub_with_modes("exempt", PermissionMode::Ask);
    hub.decide_seed_for_test("s1", "shell");
    let d = hub
        .resolve(
            &req("shell", json!({ "command": "echo hi" })),
            &spec(SideEffect::Irreversible),
        )
        .await;
    assert!(d.allowed);
    assert!(sink.events().iter().all(|e| kind(e) != "approval_request"));
}
