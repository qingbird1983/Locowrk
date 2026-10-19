//! S2 刀② 发送侧形状：一条判定、两种后果（落盘 or 降级），以及"日志面没被改写"。
//!
//! 与 `tool_budget.rs` 的分工：那簇钉的是**改前那半**（预算缺席/未超/超限降级三条
//! 字节等价关系，本刀一字未改仍绿 = 等价性证据）；本簇钉的是新后果——超预算时
//! 全文进 `<数据根>/spill/<会话>/`，上下文里只留路径 + 前段预览。

use std::sync::Arc;

use session_store::Message;

use super::*;
use crate::spill::SpillStore;

/// 夹具照生产构造抄：`output` 是 `json!("y".repeat(n))`，进 wire 的是
/// `output.to_string()` = 带两端引号的 JSON 文本（`tool_budget.rs:57-59` 同一条口径，
/// 手算裸正文长度会把"算式漂了"伪装成"测试通过"）。
fn request(body: &str, session: Option<&str>) -> LlmRequest {
    LlmRequest {
        messages: vec![tool_msg(json!(body))],
        session_id: session.map(str::to_string),
        ..Default::default()
    }
}

fn caps(store: Option<Arc<SpillStore>>, budget: Option<usize>) -> WireCaps {
    WireCaps {
        tool_result_budget_bytes: budget,
        spill: store,
        ..Default::default()
    }
}

fn tmp_store(tag: &str) -> Arc<SpillStore> {
    Arc::new(SpillStore::new(&crate::testutil::unique_dir(&format!(
        "spill-wire-{tag}"
    ))))
}

/// 超限 + 有会话身份 ⇒ 上下文里是 `<persisted-output>`，而**全文在盘上**。
#[test]
fn over_budget_spills_the_whole_output_to_disk() {
    let store = tmp_store("full");
    let body = "y".repeat(20_000);
    let req = request(&body, Some("s-wire-1"));
    let p = build_payload(
        "m",
        &req,
        false,
        Dialect::None,
        &caps(Some(Arc::clone(&store)), Some(1_000)),
        None,
    )
    .unwrap();
    let content = p["messages"][0]["content"].as_str().unwrap();
    assert!(content.starts_with("<persisted-output"), "{content}");
    assert!(content.contains("</persisted-output>"));
    assert!(content.contains("preview_bytes=\"1000\""), "{content}");
    // total_bytes = 进 wire 的那份字节数：20_000 正文 + JSON 两端引号
    assert!(content.contains("total_bytes=\"20002\""), "{content}");

    // 落的是全文，不是预览——模型读回时拿得到尾巴（改前那种提示对 shell 输出等于没了）
    let path = content
        .split("path=\"")
        .nth(1)
        .and_then(|s| s.split('"').next())
        .expect("占位文本必须带路径");
    let on_disk = std::fs::read_to_string(path).expect("落盘文件应在场");
    assert_eq!(on_disk.len(), 20_002, "盘上该是带引号的完整 wire 正文");
    let tail = "y".repeat(20_000) + "\"";
    assert!(on_disk.ends_with(tail.as_str()));
}

/// **前缀缓存判据**（AC-S2-4 那一半在这一刀就能立）：同一份内容投两次，
/// 得到的字符串逐字节相同 ⇒ 被清前缀稳定，不打断缓存。
#[test]
fn the_projection_is_byte_stable_across_two_builds() {
    let store = tmp_store("stable");
    let body = "z".repeat(9_000);
    let req = request(&body, Some("s-wire-2"));
    let caps = caps(Some(Arc::clone(&store)), Some(500));
    let first = build_payload("m", &req, false, Dialect::None, &caps, None).unwrap();
    let second = build_payload("m", &req, false, Dialect::None, &caps, None).unwrap();
    assert_eq!(
        first["messages"][0]["content"].as_str().unwrap(),
        second["messages"][0]["content"].as_str().unwrap(),
        "同内容必须同字节（内容寻址文件名是这条性质的来源）"
    );
}

/// 没有会话身份 ⇒ 退回改前那句截断提示，且**一个字节都不许多**。
/// 这条是"刀② 不许改变现网降级路径"的判据：比的不是相似，是相等。
#[test]
fn without_a_session_identity_the_bytes_are_what_they_were() {
    let store = tmp_store("degrade");
    let body = "w".repeat(5_000);
    let with_store_but_no_session = build_payload(
        "m",
        &request(&body, None),
        false,
        Dialect::None,
        &caps(Some(Arc::clone(&store)), Some(800)),
        None,
    )
    .unwrap();
    let without_store = build_payload(
        "m",
        &request(&body, Some("s-wire-3")),
        false,
        Dialect::None,
        &caps(None, Some(800)),
        None,
    )
    .unwrap();
    for (label, p) in [
        ("无会话身份", &with_store_but_no_session),
        ("无句柄", &without_store),
    ] {
        let content = p["messages"][0]["content"].as_str().unwrap();
        assert!(
            content.contains("工具结果已按「工具结果预算」截断"),
            "{label} 该走降级支: {content}"
        );
        assert!(
            !content.contains("persisted-output"),
            "{label} 不许声称落了盘"
        );
    }
    // 两种缺件形状的文本相同（都不该多带一个字节的"我本来能落盘"之类废话）
    assert_eq!(
        with_store_but_no_session["messages"][0]["content"],
        without_store["messages"][0]["content"]
    );
    // 切点仍是预算那一格，且切在字符边界上。这里不重算 800 个 w——
    // 进 wire 的是带引号的 JSON 文本，800 字节切点落在第 799 个 w 之后，
    // 手算这种"差一个字符"的期望值正是把夹具写错的地方。比长度就够了，
    // 降级文案与改前逐字节相同那半由 `tool_budget.rs` 三条未改的用例钉着。
    let clipped = with_store_but_no_session["messages"][0]["content"]
        .as_str()
        .unwrap();
    assert!(clipped.starts_with('\"'), "预览从 JSON 引号起: {clipped}");
    assert!(
        clipped.len() < body.len() + 2 + 200,
        "降级文本该是「预览 + 固定提示」，不该把全文带进去"
    );
}

/// 预算关掉（设置填 0 = 不限制）时既不截也不落盘：句柄在场也不许动手。
/// 这条挡住"加了落盘层就总想写点东西"那种实现倾向。
#[test]
fn an_unlimited_budget_touches_nothing() {
    let store = tmp_store("unlimited");
    let body = "v".repeat(50_000);
    let p = build_payload(
        "m",
        &request(&body, Some("s-wire-4")),
        false,
        Dialect::None,
        &caps(Some(store), None),
        None,
    )
    .unwrap();
    assert_eq!(
        p["messages"][0]["content"].as_str().unwrap().len(),
        body.len() + 2,
        "原样 = 正文 + JSON 两端引号"
    );
}

/// 错误支也过同一处判定：错误正文可能带一整段 stderr/堆栈，只放成功支等于
/// 留一个绕开闸门的侧门（`messages.rs` 头注那条纪律在这一刀原样继承）。
#[test]
fn the_error_branch_spills_too() {
    let store = tmp_store("error");
    // 夹具从 `tool_msg` 起步，只把 error 那一格填上（字段照生产构造抄，
    // 手写一份 ToolResult 会把"夹具字段与实际形状不符"带进测试）。
    let msg = match tool_msg(json!("不该在场")) {
        Message::ToolResult {
            call_id,
            tool,
            args,
            ..
        } => Message::ToolResult {
            call_id,
            tool,
            args,
            output: json!(null),
            error: Some("e".repeat(6_000)),
        },
        other => panic!("夹具应是 ToolResult，实际 {other:?}"),
    };
    let req = LlmRequest {
        messages: vec![msg],
        session_id: Some("s-wire-5".into()),
        ..Default::default()
    };
    let p = build_payload(
        "m",
        &req,
        false,
        Dialect::None,
        &caps(Some(store), Some(300)),
        None,
    )
    .unwrap();
    let content = p["messages"][0]["content"].as_str().unwrap();
    assert!(
        content.starts_with("<persisted-output"),
        "错误支超预算也该落盘: {content}"
    );
    assert!(content.contains("fs_read"), "读回通路要点名");
}

/// 投影不改请求对象：会话日志与内核持有的那份 messages 仍是全量真相源
/// （AC-S2-3 第三条"日志原文不变"在发送侧的对应面）。
#[test]
fn the_request_object_is_not_rewritten_by_the_projection() {
    let store = tmp_store("no-rewrite");
    let body = "u".repeat(12_000);
    let req = request(&body, Some("s-wire-6"));
    let p = build_payload(
        "m",
        &req,
        false,
        Dialect::None,
        &caps(Some(store), Some(600)),
        None,
    )
    .unwrap();
    assert!(p["messages"][0]["content"]
        .as_str()
        .unwrap()
        .contains("persisted-output"));
    match &req.messages[0] {
        Message::ToolResult { output, .. } => {
            assert_eq!(output.as_str().unwrap().len(), 12_000, "请求里该还是全文");
        }
        other => panic!("夹具应是 ToolResult，实际 {other:?}"),
    }
}
