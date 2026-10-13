//! M-18a 计数判据：`ApprovalHub::resolve` **一次决策**解析会话上下文几次。
//!
//! 为什么要有这条：`SessionContextResolver` 不是纯内存投影——生产实现
//! （`workspace.rs::session_context_resolver`）每次调用都 `read_meta` 读一遍
//! 会话旁车 `.meta.json`，再对 `[数据根] ∪ settings.fsRoots` 逐个
//! `canonicalize`（系统调用）。改前 `resolve` 里有**四个**独立解析点
//! （越界门 `scan_roots` / ③deny 的 `workspace_of` / ⑤allow 的
//! `workspace_of` / 决策卡载荷的 `workspace_of`），一次审批决策最多把同一份
//! 旁车读四遍。本文件把「解析次数」钉成断言，让 M-18a 的收益（4→1）有
//! 机器可判的真源，而不是靠读代码数。
//!
//! 判据形状：注入一个**带计数器**的 ctx 替身（`AtomicUsize`），五条代表路径
//! 各断言 `count == 1`。五条覆盖决策表的全部出口 = ③deny 命中 / ④模式放行 /
//! ⑤allow 命中 / ⑦决策卡（越界门命中版）/ ⑦决策卡（围栏内版 = 改前最坏路径）。
//!
//! **改前实测真值**（本文件先于生产码改动落盘并跑过一遍取的数，不是估算；
//! 口径 = `cargo test ctx_parse` 在生产码未改时的失败信息）：
//! deny 命中 **1** · 模式放行 **1** · allow 命中 **2** · 卡（external=true）
//! **2** · 卡（围栏内，external=false）**4**。改后五条一律 **1**。
//!
//! 两处与「读代码数出来的」不一样，记档以免下批照着错数改判据：
//! ① 卡路径带**出根**绝对路径时 `external=true` ⇒ `resolve` 的
//!   `if sensitive.is_none() && !external` 整块（③④⑤⑥）被跳过，只剩
//!   越界门 + 卡载荷两次 = **2**，不是四级全走的 4；
//! ② 要凑出「四级全走」的 **4** 次，命令必须带**围栏内**绝对路径
//!   （external=false）且档位不放行、规则不命中 ⇒ 第五条夹具专为它而设。

use super::*;

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

/// 带计数器的 ctx 替身：语义与 `mod.rs::ctx_stub` 一致（s1* → ws-A），只多
/// 一次自增，另可下发 `fence` 当调用期围栏根（`fs_roots`）。fence 空 =
/// 越界门走 env 回落分支（与 `ctx_stub` 同）；非空 = 越界门按声明根判定，
/// 用来构造「命令在围栏内 ⇒ external=false」那一档。
fn counting_ctx(
    n: Arc<AtomicUsize>,
    fence: Vec<String>,
) -> crate::workspace::SessionContextResolver {
    Arc::new(move |sid: &str| {
        n.fetch_add(1, Ordering::Relaxed);
        CallContext {
            session_id: Some(sid.to_string()),
            workspace: sid.starts_with("s1").then(|| "ws-A".to_string()),
            fs_roots: fence.clone(),
        }
    })
}

/// 全装配 hub（modes + ctx + rules 三件齐 = 决策表七序全通），ctx 换成计数版。
fn hub_counting(
    tag: &str,
    default: PermissionMode,
    fence: Vec<String>,
) -> (
    Arc<ApprovalHub>,
    Arc<PermissionRuleStore>,
    Arc<AtomicUsize>,
    PathBuf,
) {
    let n = Arc::new(AtomicUsize::new(0));
    let root = tmp_root(tag);
    let rules = Arc::new(PermissionRuleStore::new(&root.join("data")));
    let modes = Arc::new(PermissionModeStore::new(root.join("sessions"), default));
    let hub = Arc::new(
        ApprovalHub::with_timeout(
            Arc::new(Collector::default()),
            std::time::Duration::from_millis(80),
        )
        .with_modes(Arc::clone(&modes))
        .with_workspace(counting_ctx(Arc::clone(&n), fence))
        .with_rules(Arc::clone(&rules)),
    );
    (hub, rules, n, root)
}

/// 手改 JSON 注入一条 deny（UI 不产 deny，文件面生效——与
/// `project_rules::project_deny_rule_blocks_even_in_auto_mode` 同法）。
fn inject_deny(root: &Path, tool: &str, content: &str) {
    let f = root
        .join("data")
        .join("permissions")
        .join(rules_file_of("ws-A"));
    let mut v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&f).unwrap()).unwrap();
    v["deny"] = json!([{ "tool": tool, "content": content }]);
    std::fs::write(&f, serde_json::to_string_pretty(&v).unwrap()).unwrap();
}

/// 断言「一次决策只解析一次上下文」，失败信息带上实测次数（改前跑这一遍
/// 就是取真值的手段）。
fn assert_one_parse(n: &AtomicUsize, path: &str) {
    let got = n.load(Ordering::Relaxed);
    assert_eq!(
        got, 1,
        "{path}: 一次决策解析了 {got} 次会话上下文（期望 1）"
    );
}

/// 出口③：deny 命中即返回。改前 1 次（deny 那一级解析）→ 改后 1 次。
#[tokio::test]
async fn resolve_parses_session_context_once_on_deny_hit() {
    let (hub, rules, n, root) = hub_counting("cnt-deny", PermissionMode::Auto, vec![]);
    rules
        .add_allow("ws-A", "shell", Some("rm -rf build".into()))
        .unwrap();
    inject_deny(&root, "shell", "rm -rf build");

    let d = hub
        .resolve(&shell_req("s1", "rm -rf build"), &shell_spec())
        .await;
    assert!(!d.allowed, "deny 规则必须拦下 Auto 档（既有语义）");
    assert_one_parse(&n, "deny 命中");
    let _ = std::fs::remove_dir_all(&root);
}

/// 出口④：模式放行（Auto）。改前 1 次 → 改后 1 次（这条是**回归护栏**：
/// memo 化之后不许把「本来不解析的路径」变成解析，也不许把已解析的丢掉）。
#[tokio::test]
async fn resolve_parses_session_context_once_on_mode_allow() {
    let (hub, _rules, n, root) = hub_counting("cnt-mode", PermissionMode::Auto, vec![]);
    let d = hub
        .resolve(&shell_req("s1", "echo hi"), &shell_spec())
        .await;
    assert!(d.allowed && d.reason.as_deref() == Some("mode:auto"));
    assert_one_parse(&n, "模式放行");
    let _ = std::fs::remove_dir_all(&root);
}

/// 出口⑤：项目 allow 规则命中。改前 **2** 次（deny 一级 + allow 一级各解析
/// 一遍同一份旁车）→ 改后 1 次。
#[tokio::test]
async fn resolve_parses_session_context_once_on_project_allow_rule() {
    let (hub, rules, n, root) = hub_counting("cnt-allow", PermissionMode::Ask, vec![]);
    rules
        .add_allow("ws-A", "shell", Some("npm test".into()))
        .unwrap();

    let d = hub
        .resolve(&shell_req("s1", "npm test"), &shell_spec())
        .await;
    assert!(d.allowed);
    assert_eq!(d.reason.as_deref(), Some("rule:project-allow"));
    assert_one_parse(&n, "allow 命中");
    let _ = std::fs::remove_dir_all(&root);
}

/// 出口⑦（越界门命中版）：命令带**出根**绝对路径 ⇒ `external=true`，
/// ③④⑤⑥ 整块被跳过，改前 **2** 次（越界门 `scan_roots` + 卡载荷）→ 改后
/// 1 次。守夜 80ms 后 fail-closed 拒绝。
#[tokio::test]
async fn resolve_parses_session_context_once_on_card_path() {
    let (hub, _rules, n, root) = hub_counting("cnt-card", PermissionMode::Ask, vec![]);
    let d = hub
        .resolve(
            &shell_req("s1", r"type X:\outside\secret.txt"),
            &shell_spec(),
        )
        .await;
    assert!(
        !d.allowed,
        "无人决策 ⇒ 守夜超时拒绝（既有 fail-closed 语义）"
    );
    assert_one_parse(&n, "决策卡路径");
    let _ = std::fs::remove_dir_all(&root);
}

/// 出口⑦（围栏内版 = 改前**最坏路径**）：命令带**围栏内**绝对路径 ⇒
/// `external=false` ⇒ 决策表四级全走一遍（越界门 `scan_roots` + ③deny 的
/// `workspace_of` + ⑤allow 的 `workspace_of` + 卡载荷的 `workspace_of`），
/// 改前实测 **4** 次同一份旁车 → 改后 1 次。M-18a 的收益上限就是这一档。
#[tokio::test]
async fn resolve_parses_session_context_once_on_card_path_inside_fence() {
    // 围栏根用不存在的路径即可：`scan_roots`/`is_outside` 只做字符串归一
    // 比较（`/`→`\`、去 verbatim、小写、去尾分隔），不碰文件系统。
    let fence = vec![r"F:\lw-fence".to_string()];
    let (hub, _rules, n, root) = hub_counting("cnt-card-in", PermissionMode::Ask, fence);
    let d = hub
        .resolve(&shell_req("s1", r"type F:\lw-fence\a.txt"), &shell_spec())
        .await;
    assert!(
        !d.allowed,
        "无人决策 ⇒ 守夜超时拒绝（既有 fail-closed 语义）"
    );
    assert_one_parse(&n, "决策卡路径（围栏内，四级全走）");
    let _ = std::fs::remove_dir_all(&root);
}
