//! 拒收记账面（S2 刀⑧ · 台账 ⓜ）：一次请求被供应商按**超窗**拒掉时，发送侧要把这一次
//! 的占用读数交给压缩面。为什么非记不可、代价与自愈路径都写在 `compact::Overflow` 头注，
//! 长跑那一腿的实测形状写在 `tests/polaris_longrun.rs`。
//!
//! ⚠ 这一簇证的是"**该写的时候写不写、不该写的时候别乱写**"，证不了"三条发送收口都
//! 真的调了它"——那是跨文件关系，按本仓口径由结构闸 **SS26** 钉两侧文件集合（少了那条
//! 判据，把三处调用删光这一簇照样全绿，正是队列 §6 第 10 条点名的形状）。真 HTTP 那一趟
//! 的报文形状也不在这里：`llm/tests` 没有 server 替身，而"什么算超窗"这一格已经由
//! `transport.rs` 那两条（分类表 + 同一条判定谓词）钉住，本簇复用同一个谓词，不另判。

use super::*;
use kernel_agent::LlmResponse;

use crate::compact::Overflow;

/// 装了一张表的 AppLlm（与组装点同形：表由组装点构造、注进来）。
fn llm_with(table: Arc<Overflow>) -> AppLlm {
    let root = temp_root("refusal-table");
    let store = Arc::new(SettingsStore::open(&root).unwrap());
    AppLlm::new(store).with_refusals(table)
}

fn backend(msg: &str) -> Result<LlmResponse, LlmError> {
    Err(LlmError::Backend(msg.to_string()))
}

/// 超窗那一次要落表，用的就是发送侧那把估算尺（`estimate_tokens(&req.messages)` 的
/// 结果由调用方给，本函数只负责"记不记、记多少"）。中文与英文报文各一条：那条判定
/// 认两家，本簇也得两条都过，否则日后文案改一侧就悄悄不记账了。
#[test]
fn an_overflow_refusal_is_booked_with_the_size_we_sent() {
    let t = Arc::new(Overflow::default());
    let llm = llm_with(Arc::clone(&t));
    llm.refusal_note(
        Some("s-cn"),
        9_000,
        &backend("上下文超出模型窗口：请新开会话"),
    );
    assert_eq!(t.of("s-cn"), Some(9_000), "中文那句要认");
    llm.refusal_note(
        Some("s-en"),
        7_000,
        &backend("HTTP 400: prompt is too long"),
    );
    assert_eq!(t.of("s-en"), Some(7_000), "供应商英文报文也要认");
}

/// 普通失败**不许**记：鉴权、限流、网络断跟窗口无关，把它们也交给压缩面 = 让压缩层去
/// 追一堆压不动的失败，还会把这一层永久顶在"该压"那一侧。成功返回同样不记。
#[test]
fn an_ordinary_failure_is_not_a_refusal() {
    let t = Arc::new(Overflow::default());
    let llm = llm_with(Arc::clone(&t));
    for msg in [
        "HTTP 401: invalid api key",
        "HTTP 429: rate limit exceeded",
        "请求超时",
        "HTTP 400: reduce the temperature",
    ] {
        llm.refusal_note(Some("s-plain"), 9_000, &backend(msg));
        assert_eq!(t.of("s-plain"), None, "「{msg}」不该被当成超窗");
    }
    llm.refusal_note(
        Some("s-ok"),
        9_000,
        &Ok(LlmResponse {
            content: "正常回答".into(),
            tool_calls: Vec::new(),
            usage: None,
        }),
    );
    assert_eq!(t.of("s-ok"), None, "成功那一趟没有拒收可言");
}

/// 两格早退：没有会话身份（一次性请求，`LlmRequest.session_id = None`）与**没装表**
/// （headless / 测试装配那一档，与 `with_spill` 同形）都不许写、也不许 panic。
/// 前一半必须先有一条"这台 llm 确实会写这张表"的对照，再证"没身份那一次一格都不许多
/// 出来"——只断言"某个键读回 None"是空转（它本来就恒为 None，什么也判不出来）。
#[test]
fn a_refusal_without_a_session_or_without_a_table_books_nothing() {
    let t = Arc::new(Overflow::default());
    let llm = llm_with(Arc::clone(&t));
    llm.refusal_note(Some("s-known"), 500, &backend("prompt is too long"));
    assert_eq!(t.of("s-known"), Some(500), "对照：这一台真的会写这张表");
    llm.refusal_note(None, 9_000, &backend("prompt is too long"));
    assert_eq!(t.entries(), 1, "没有会话身份那一次一格都不许多出来");
    assert_eq!(
        t.of("s-known"),
        Some(500),
        "也不许把无主的读数并进已有那一格"
    );

    let root = temp_root("refusal-no-table");
    let store = Arc::new(SettingsStore::open(&root).unwrap());
    let plain = AppLlm::new(store);
    plain.refusal_note(Some("s-none"), 9_000, &backend("prompt is too long"));
}
