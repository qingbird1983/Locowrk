//! specs/12 刀①：辅助调用的**总时长预算**与**可中止**。
//!
//! 被测面 = `llm::client` 的重试环（预算怎么掐）+ `llm::retry` 的预算判据
//! （压缩层靠它把「预算耗尽」与「别的失败」分开）。
//!
//! 为什么起一个**本地 TCP 假服务器**而不是纯函数：预算的承重点恰恰是
//! 「**单次尝试也受约束**」——只查「重试前累计耗时」不够，一次失败尝试自己
//! 就能等满空闲看门狗（180s），90s 的预算会形同虚设。这条性质只有真发一次
//! 请求才测得出来（纯函数测不出"挂在读上的连接"）。
//!
//! 两个假端点形态（判据要能分开它们）：
//! - `Fail`：恒 500 ⇒ 瞬时故障 ⇒ 客户端会重试；
//! - `Hang`：接受连接后**一个字节都不写** ⇒ 客户端挂在读上。

use super::*;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::settings::{LlmSettings, NetworkProxy};

/// 假端点的应答形态。
#[derive(Clone, Copy)]
enum Mode {
    /// 恒 500：瞬时故障 ⇒ 客户端会重试。
    Fail,
    /// 接受连接后一个字节都不写：连接挂在读上（空连接挂死）。
    Hang,
}

/// 起一个本地假端点，返回它的 base url（端点路径照客户端的归一走 `/v1/...`，
/// 假服务器不关心路径）。
async fn fake_endpoint(mode: Mode) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        while let Ok((mut sock, _)) = listener.accept().await {
            tokio::spawn(async move {
                // 读掉请求（客户端在等响应；读不完也不影响它收到响应）
                let mut buf = vec![0u8; 16 * 1024];
                let _ = sock.read(&mut buf).await;
                match mode {
                    Mode::Fail => {
                        let _ = sock
                            .write_all(
                                b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\n\r\n",
                            )
                            .await;
                    }
                    Mode::Hang => {
                        // 不写、不关：客户端挂在读上，直到它自己的看门狗或预算接管
                        std::future::pending::<()>().await;
                    }
                }
            });
        }
    });
    format!("http://{addr}")
}

/// 指向本地假端点的配置。
///
/// ⚠ `network_proxy = direct` 是**必须**的：缺省是 system 档（吃环境变量），
/// 开发机上有 `HTTP_PROXY` 时请求会被代理接管 ⇒ 测的根本不是本地端点。
fn cfg_to(base: &str) -> LlmSettings {
    LlmSettings {
        provider: "openai".into(),
        model: "m".into(),
        base_url: Some(base.into()),
        api_key: Some("sk-test".into()),
        window_tokens: None,
        headers: Default::default(),
        network_proxy: Some(NetworkProxy {
            mode: "direct".into(),
            url: None,
        }),
    }
}

fn one_user_req() -> LlmRequest {
    LlmRequest {
        messages: vec![Message::User {
            content: "hi".into(),
            parts: Vec::new(),
        }],
        ..Default::default()
    }
}

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
    let base = fake_endpoint(Mode::Fail).await;
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
        "错误必须带预算前缀（压缩层靠它认这一档）：{msg}"
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
    let base = fake_endpoint(Mode::Hang).await;
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
    let base = fake_endpoint(Mode::Fail).await;
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

/// 预算判据只认自己那枚前缀——压缩层靠它把「预算耗尽」与「别的失败」分开，
/// 认错档的表现是界面说错话（把"超预算"说成"网络或额度"）。
#[test]
fn the_budget_predicate_only_matches_its_own_prefix() {
    assert!(retry::is_budget_exhausted(
        "[budget] 调用超出总时长预算（90000ms）"
    ));
    assert!(
        !retry::is_budget_exhausted("网络断了"),
        "普通失败不许被认成预算耗尽"
    );
    assert!(
        !retry::is_budget_exhausted("上下文超出模型窗口"),
        "超窗是另一档（它有自己的递减带），不许被预算判据吃掉"
    );
}

/// AC-6 的**接线半**：主对话轮的实际调用点**真的**没有传预算。
///
/// 上一条证的是「`CompatLlm` 传 `None` 时仍无限重试」（能力面）；这一条证的是
/// **主对话轮走的那条路**传的就是 `None`（接线面）。少这一半，将来谁在
/// `app_llm.rs` 里手滑填了一个 `Some(...)`，上一条照样绿——而那正是
/// 「用户 2026-08 的裁定被悄悄推翻」的地方。
#[tokio::test]
async fn the_main_turn_call_site_passes_no_budget() {
    let base = fake_endpoint(Mode::Fail).await;
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
