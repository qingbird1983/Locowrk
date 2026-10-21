//! specs/12 刀①：辅助调用的**总时长预算**与**可中止**。
//!
//! 被测面 = `llm::client` 的重试环（预算怎么掐）+ `llm::retry` 的预算判据
//! （压缩层靠**档位**把「预算耗尽」与「别的失败」分开，specs/13 刀⑦b）。
//!
//! 为什么起一个**本地 TCP 假服务器**而不是纯函数：预算的承重点恰恰是
//! 「**单次尝试也受约束**」——只查「重试前累计耗时」不够，一次失败尝试自己
//! 就能等满空闲看门狗（180s），90s 的预算会形同虚设。这条性质只有真发一次
//! 请求才测得出来（纯函数测不出"挂在读上的连接"）。
//!
//! 两个假端点形态（判据要能分开它们）：
//! - `Fail`：恒 500 ⇒ 瞬时故障 ⇒ 客户端会重试；
//! - `Hang`：接受连接后**一个字节都不写** ⇒ 客户端挂在读上。
//!
//! 假端点本体是**共享夹具**（`tests/mod.rs` 的 `fake_endpoint`）：两处各写一遍
//! = 两个形状。

use super::*;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// 恒 500 的应答（瞬时故障 ⇒ 客户端会重试）。
const FAIL_500: &str = "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\n\r\n";

/// AC-1：预算耗尽 = **一次失败**（Err），不是无限重试。
///
/// 这一条是整刀的根：改之前摘要请求失败会永远重试下去（界面表现 = 卡在
/// 「思考中」，且按「停止生成」也没用）。
///
/// ⚠ 外面套一层超时是**判据自己**的健壮性要求：把预算整个去掉（= 回到改前
/// 口径）时，这条用例会**永远等下去**——那不是"报红"，是挂死整个测试进程。
/// 超时把它变成一条红色的断言（"预算内就该回来"），变异才真的报红。
#[tokio::test]
async fn a_summary_call_over_budget_fails_instead_of_retrying_forever() {
    let base = fake_endpoint(Some((FAIL_500.to_string(), String::new()))).await;
    let cfg = cfg_to(&base);
    let llm = CompatLlm::new();
    let started = std::time::Instant::now();
    let err = tokio::time::timeout(
        Duration::from_secs(15),
        llm.complete_with(
            &cfg,
            WireCaps::default(),
            one_user_req(),
            None,
            None,
            Some(Duration::from_millis(600)),
        ),
    )
    .await
    .expect("预算内就该回来（没回来 = 预算被绕过了，这条会挂死）")
    .expect_err("预算耗尽必须是 Err（而不是继续转下去）");
    let msg = err.to_string();
    assert!(
        msg.contains(retry::BUDGET_PREFIX),
        "错误必须带预算前缀（**日志标记**，判据已改走档位）：{msg}"
    );
    assert_eq!(
        err.kind,
        FailureKind::Budget,
        "预算耗尽 ⇒ 档位 Budget（压缩层按**档**认这一档，不看文案）：{msg}"
    );
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "预算内就该回来，实测 {:?}",
        started.elapsed()
    );
}

/// AC-2：**单次尝试**也受预算约束——挂着不吐字的连接由预算接管，不等满空闲
/// 看门狗（180s）。
///
/// 判据的承重点：只看"重试前累计耗时"的话，这一次尝试自己就能挂 180s
/// ⇒ 90s 的预算形同虚设 ⇒ 卡死原样还在。
#[tokio::test]
async fn an_idle_summary_stream_is_aborted_by_the_budget_not_by_the_idle_watchdog() {
    let base = fake_endpoint(None).await;
    let cfg = cfg_to(&base);
    let llm = CompatLlm::new();
    let started = std::time::Instant::now();
    let err = tokio::time::timeout(
        Duration::from_secs(15),
        llm.complete_with(
            &cfg,
            WireCaps::default(),
            one_user_req(),
            None,
            None,
            Some(Duration::from_millis(700)),
        ),
    )
    .await
    .expect("预算内就该回来（没回来 = 预算没包住单次尝试，这条会挂死）")
    .expect_err("挂着不吐字的连接必须被预算掐掉");
    let msg = err.to_string();
    assert!(msg.contains(retry::BUDGET_PREFIX), "实得 {}", msg);
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "必须由预算（亚秒级）掐掉，不许等满空闲看门狗（180s），实测 {:?}",
        started.elapsed()
    );
}

/// AC-6：主对话轮**不设预算** ⇒ 「重试不设上限」那条裁定逐字不变，
/// 且停止标记仍能把它中止。
///
/// 两半都要证：① 不传预算时它**不自己回来**（还在重试）；② 停止标记置位后
/// 它能收束。少任何一半，这条用例都证明不了"主路径没被这一刀动到"。
#[tokio::test]
async fn the_main_turn_keeps_its_unbounded_retry_and_its_stop_hook() {
    let base = fake_endpoint(Some((FAIL_500.to_string(), String::new()))).await;
    let cfg = cfg_to(&base);
    let llm = CompatLlm::new();
    let stop = Arc::new(AtomicBool::new(false));
    let hooks = RetryHooks {
        should_stop: {
            let stop = Arc::clone(&stop);
            Box::new(move || stop.load(Ordering::Relaxed))
        },
        on_retry: Box::new(|_attempt, _delay_ms| {}),
    };
    let early = tokio::time::timeout(
        Duration::from_millis(1500),
        llm.complete_with(
            &cfg,
            WireCaps::default(),
            one_user_req(),
            None,
            Some(&hooks),
            None,
        ),
    )
    .await;
    assert!(
        early.is_err(),
        "不传预算时不该自己回来（主对话轮那条裁定就是「重试不设上限」）"
    );
    // 停止标记置位 ⇒ 下一次重试等待的轮询命中 ⇒ 空收束
    stop.store(true, Ordering::Relaxed);
    let resp = tokio::time::timeout(
        Duration::from_secs(20),
        llm.complete_with(
            &cfg,
            WireCaps::default(),
            one_user_req(),
            None,
            Some(&hooks),
            None,
        ),
    )
    .await
    .expect("停止后必须收束")
    .expect("停止后是空收束，不是错误");
    assert!(resp.content.is_empty(), "停止后不虚构正文");
}

/// 预算这一档的判据**住在接入面**（specs/13 刀⑦b）：压缩层按**档位**认它，
/// 不再看文案前缀——`is_budget_exhausted` 那枚前缀判据随本刀退役（连同压缩层
/// 那三枚同款文本判据一起删）。
///
/// 正向那一半由上面那条端到端用例钉住（预算耗尽 ⇒ 档位 `Budget`）；这里钉
/// **反向**：别的档不许被认成预算耗尽——认错档的表现是界面说错话（把"超预算"
/// 说成"网络或额度"）。
#[test]
fn no_other_failure_claims_the_budget_kind() {
    // 反向：普通失败（400 请求错误）不许被认成预算耗尽。
    let Failure::Fatal { kind, .. } = retry::classify_status(400, "invalid field 'foo'") else {
        panic!("普通 400 是致命错误");
    };
    assert_ne!(kind, FailureKind::Budget, "普通失败不许被认成预算耗尽");
    // 反向：超窗是另一档（它有自己的收缩带），不许被预算档吃掉。
    let Failure::Fatal { kind, .. } = retry::classify_status(400, "context length exceeded") else {
        panic!("超窗是致命错误");
    };
    assert_eq!(kind, FailureKind::ContextOverflow, "超窗归超窗那一档");
    assert_ne!(kind, FailureKind::Budget, "超窗不许被预算档吃掉");
}

/// AC-6 的**接线半**：主对话轮的实际调用点**真的**没有传预算。
///
/// 上一条证的是「`CompatLlm` 传 `None` 时仍无限重试」（能力面）；这一条证的是
/// **主对话轮走的那条路**传的就是 `None`（接线面）。少这一半，将来谁在
/// `app_llm.rs` 里手滑填了一个 `Some(...)`，上一条照样绿——而那正是
/// 「用户 2026-08 的裁定被悄悄推翻」的地方。
#[tokio::test]
async fn the_main_turn_call_site_passes_no_budget() {
    let base = fake_endpoint(Some((FAIL_500.to_string(), String::new()))).await;
    let root = temp_root("main-turn-budget");
    std::fs::write(
        root.join("settings.json"),
        serde_json::json!({
            "llm": {
                "active": "main",
                "providers": [{
                    "id": "main",
                    "label": "主档",
                    "kind": "openai",
                    "baseUrl": base,
                    "apiKey": "sk-test",
                    "model": "m",
                    "windowTokens": 131072,
                    "enabled": true
                }],
                "networkProxy": { "mode": "direct" }
            }
        })
        .to_string(),
    )
    .unwrap();
    let store = Arc::new(SettingsStore::open(&root).unwrap());
    let llm = AppLlm::new(store);
    let early = tokio::time::timeout(
        Duration::from_millis(1500),
        llm.complete_streaming(one_user_req(), &|_| false),
    )
    .await;
    assert!(
        early.is_err(),
        "主对话轮不该自己回来——它不传预算 ⇒ 「重试不设上限」那条裁定仍然成立"
    );
}
