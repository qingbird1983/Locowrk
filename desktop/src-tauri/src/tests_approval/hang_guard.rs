//! 台账 T7 的自测面：钉的是 `mod.rs` 末尾那套 `Decider` / `bounded` 的**上界本身**。
//!
//! 为什么不顺手在每条审批用例里各写一份轮询：写 9 份就没有"上界"这件事可言——
//! 改前正是 9 份无界 `loop`，其中 3 份被调用方 `await`（卡片不出现就直接永挂），
//! 4 份所在的 hub 超时给 0（不起守夜 = 没人回填 oneshot，`resolve`/`run_turn` 永等）。
//! 挂起的测试既没有证明力，又把整轮门禁堵住（批 F-24a 曾把其中一条的 `ZERO`
//! 临时改成 500ms 来躲这件事，那是局部补丁不是收口）。
//!
//! 本文件三条 = 上界的正反两面：**没有卡片必须失败**、**永不收束的那一步必须报出来**、
//! **有卡片时照旧正常决议**（最后这条是防前两条退化成"永远 panic"的空壳）。

use super::*;

use std::time::Instant;

/// 自测用小上界：正常路径 2ms 轮询就能看到卡片，150ms 足够宽松；
/// 而一旦上界丢失，外层保护网（`SMALL * 10`）会在 1.5 秒内把它揪出来。
const SMALL: Duration = Duration::from_millis(150);

/// 外层保护网**故意不用 `bounded`**：自测的保护网必须与被测机制不同源，
/// 否则 `bounded` 自己失去上界时，这条自测会和它一起挂住（同义反复 = 零证明力）。
/// 这里用最原始的 `tokio::time::timeout`，被测的那一层挂了它照样报红。
const OUTER: Duration = Duration::from_millis(1500);

/// ① 卡片永远不出现 ⇒ 决断者必须**在预算内**失败。
/// 读数两条：`is_err`（有没有失败）+ 耗时（失败得及不及时）。把 `Decider::spawn`
/// 里那句 `assert!(waited < budget, …)` 删掉，用例红在保护网；把它改成静默
/// `return`，用例红在 `is_err`（变异凭证见提交正文）。
#[tokio::test]
async fn decider_fails_within_budget_when_no_card_is_ever_emitted() {
    // 完全档：Irreversible 也直接放行、根本不推卡 ⇒ pending 永远是空
    let (hub, _rules, _modes, root) = hub_v145("hang-none", PermissionMode::Auto);
    let started = Instant::now();
    let worker = Decider::new(hub).within(SMALL).spawn(|_| {});
    let res = tokio::time::timeout(OUTER, worker).await;
    let elapsed = started.elapsed();
    let _ = std::fs::remove_dir_all(&root);

    let res = res.expect("决断者在上界内没有收束 ⇒ 又退化成无界轮询了");
    assert!(
        res.is_err(),
        "没有卡片时决断者必须失败收场（改前这里是无限轮询），实测 {res:?}"
    );
    assert!(
        elapsed < Duration::from_secs(2),
        "有界等待失去上界：SMALL = {SMALL:?} 却等了 {elapsed:?}"
    );
}

/// ② `bounded` 自己也不能空转：喂一个永不完成的 future，它必须报出来而不是干等。
#[tokio::test]
async fn bounded_reports_a_step_that_never_converges() {
    let started = Instant::now();
    // panic 隔离在任务里，好让用例断言它的存在（而不是被它直接打死）
    let worker = tokio::spawn(async move {
        bounded("永不收束的那一步", SMALL, std::future::pending::<u32>()).await
    });
    let res = tokio::time::timeout(OUTER, worker).await;
    let elapsed = started.elapsed();

    let res = res.expect("bounded 失去上界：future 永不完成时它自己也一直等");
    assert!(
        res.is_err(),
        "future 永不收束时 bounded 必须 panic，实测 {res:?}"
    );
    assert!(
        elapsed < Duration::from_secs(2),
        "bounded 失去上界：SMALL = {SMALL:?} 却等了 {elapsed:?}"
    );
}

/// ③ 有卡片时上界不得碍事：决断者正常决议、`resolve` 正常放行、两边都在预算内返回。
/// 这条是 ①② 的阴性对照——证明那两条红不是因为"这套 helper 逢 panic 必红"。
#[tokio::test]
async fn decider_still_resolves_when_a_card_does_appear() {
    let (hub, _rules, _modes, root) = hub_v145("hang-ok", PermissionMode::Ask);
    let worker = Decider::new(Arc::clone(&hub)).spawn(|_| {});
    let d = bounded(
        "hub.resolve",
        DECIDER_BUDGET,
        hub.resolve(&shell_req("s1", "npm test"), &shell_spec()),
    )
    .await;
    let res = bounded("decider 句柄", DECIDER_BUDGET, worker).await;
    let _ = std::fs::remove_dir_all(&root);

    assert!(res.is_ok(), "有卡片时决断者必须正常决议收尾，实测 {res:?}");
    assert!(d.allowed, "缺省决议 = 放行（要拒绝得显式 .deny()）");
}
