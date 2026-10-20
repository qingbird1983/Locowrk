//! 工具结果预算层的发送侧形状（v2.192 · 后审计队列 Q8 · CB §7.4）。
//! 从 `payload.rs` 单独立簇的理由 = 三问自检第②问：那文件管的是「请求体怎么装配」，
//! 本簇管的是「一条工具输出最多能带进上下文多少字节」这件独立的事，改预算不该翻到
//! 流式累积与 usage 映射那几节里去。簇名不叫 `wire`（被测子模块）= 形状闸 S4 的硬约束。

use super::*;

// ---- Q8 工具结果预算层（CB §7.4）：截断只发生在发送侧 ----

/// 预算 = None（设置里填 0「不限制」）时逐字节原样 = 改动前行为。这条是缺省
/// 等价判据：比的不是"看起来没截"，是**字节相同**。
#[test]
fn tool_result_budget_none_is_byte_identical() {
    let req = LlmRequest {
        messages: vec![tool_msg(json!("x".repeat(400_000)))],
        ..Default::default()
    };
    let free = build_payload("m", &req, false, Dialect::None, &WireCaps::default(), None).unwrap();
    let explicit_none = build_payload(
        "m",
        &req,
        false,
        Dialect::None,
        &WireCaps {
            tool_result_budget_bytes: None,
            ..Default::default()
        },
        None,
    )
    .unwrap();
    // 400_000 字符 + JSON 两端引号 = 400_002 字节（截断作用在 output.to_string() 上）
    let body = free["messages"][0]["content"].as_str().unwrap();
    assert_eq!(body.len(), 400_002);
    assert_eq!(
        body,
        explicit_none["messages"][0]["content"].as_str().unwrap()
    );
    // 预算在场但未超（小输出 + **生产那颗固定额度**）：见下方断言的口径说明。
    // 额度由生产常量给而不是这里手算 24 * 1024——手算会把"常量漂了"这件事
    // 伪装成"测试通过"（夹具必须照生产构造代码抄，同一口径批 F-24 立过）
    use crate::settings::context::TOOL_RESULT_BUDGET_BYTES;
    let within = build_payload(
        "m",
        &LlmRequest {
            messages: vec![tool_msg(json!("hello"))],
            ..Default::default()
        },
        false,
        Dialect::None,
        &WireCaps {
            tool_result_budget_bytes: Some(TOOL_RESULT_BUDGET_BYTES),
            ..Default::default()
        },
        None,
    )
    .unwrap();
    // 预算在场但未超（小输出 + 24 KiB 额度）：也不许多出任何一个字节的提示。
    // 注意 content 是 `output.to_string()` = JSON 文本，字符串值自带两端引号——
    // 截断作用在这一层字节上，不是裸正文（与超限支的 10_002/400_002 同一口径）。
    assert_eq!(within["messages"][0]["content"], "\"hello\"");
}

/// 超限：截到预算 + 一句可续读的提示，且**请求对象本身没被改写**（日志面
/// 保持全量真相源是这一刀的前提，不是附带效果）。
#[test]
fn tool_result_over_budget_is_clipped_with_a_live_recovery_path() {
    let req = LlmRequest {
        messages: vec![tool_msg(json!("y".repeat(10_000)))],
        ..Default::default()
    };
    let p = build_payload(
        "m",
        &req,
        false,
        Dialect::None,
        &WireCaps {
            tool_result_budget_bytes: Some(1_000),
            ..Default::default()
        },
        None,
    )
    .unwrap();
    let content = p["messages"][0]["content"].as_str().unwrap();
    // 只留前 1000 字节（全 ASCII，边界正好），加一句提示
    assert_eq!(&content[..1_000], "\"".to_owned() + &"y".repeat(999));
    assert!(content.len() < 10_002, "截断必须真的减小体积");
    // 提示必须给出「原输出多大」+「怎么拿回后面那段」+「上限是固定的」——
    // 少了任何一条，用户与模型都只能干看着一段被啃掉的输出。
    let note = &content[1_000..];
    assert!(note.contains("原输出 10002 字节"));
    assert!(note.contains("offset") && note.contains("max_bytes"));
    assert!(note.contains("本会话日志"));
    assert!(note.contains("发送侧上限固定"));
    // 发送侧动了字节，请求面一个字节没动 = 会话日志/回放/导出看到的仍是全量
    let Message::ToolResult { output, .. } = &req.messages[0] else {
        panic!("夹具被改写");
    };
    assert_eq!(output.as_str().unwrap().len(), 10_000);
}

/// 切点必须落在 UTF-8 字符边界：中文输出按字节硬切会当场 panic，而 `fs_read`
/// 读一个中文文件就足以撞上——这不是假设中的风险。
#[test]
fn tool_result_clip_never_splits_a_multibyte_char() {
    // 每字 3 字节；output.to_string() = 引号(1) + 正文(2400) + 引号(1) = 2402 字节。
    // 预算 8 落在「测」的第 2 字节上 ⇒ 必须回退到 7。
    let req = LlmRequest {
        messages: vec![tool_msg(json!("边界测试".repeat(200)))],
        ..Default::default()
    };
    let p = build_payload(
        "m",
        &req,
        false,
        Dialect::None,
        &WireCaps {
            tool_result_budget_bytes: Some(8),
            ..Default::default()
        },
        None,
    )
    .unwrap();
    let content = p["messages"][0]["content"].as_str().unwrap();
    let head = &content[..content.find("\n\n【").unwrap()];
    assert_eq!(head, "\"边界");
    assert!(content.contains("前 7 字节"), "提示里的切点得是真切点");
    assert!(std::str::from_utf8(content.as_bytes()).is_ok());
}

/// 错误支同闸：错误正文常带一整段 stderr/堆栈，只截成功支等于留一条绕开闸门
/// 的侧门。
#[test]
fn tool_result_error_branch_is_not_a_side_door() {
    let req = LlmRequest {
        messages: vec![Message::ToolResult {
            call_id: CallId("call-2".into()),
            tool: "shell".into(),
            args: json!({}),
            output: json!(null),
            error: Some(format!("boom {}", "e".repeat(9_000))),
        }],
        ..Default::default()
    };
    let p = build_payload(
        "m",
        &req,
        false,
        Dialect::None,
        &WireCaps {
            tool_result_budget_bytes: Some(500),
            ..Default::default()
        },
        None,
    )
    .unwrap();
    let content = p["messages"][0]["content"].as_str().unwrap();
    assert!(content.starts_with(r#"{"error":"boom eee"#));
    assert!(content.contains("工具结果预算"));
    assert!(content.len() < 9_010, "错误支也得真的被截");
}

/// CB §7.4 自带的验收判据原文 = 「构造 500 KB 工具结果，断言进上下文的 ≤ 预算，
/// 且截断标注格式统一」——稿里的判据是**下限不是参考**（队列 §5 第 5 条），所以
/// 这条按原话打：512_000 字节 + **生产固定档**（额度由 `TOOL_RESULT_BUDGET_BYTES`
/// 给，不是测试里另写一个数）。标注格式统一由"只出现一次"承担：两处贴同一句话
/// 等于把预算又顶高一份，且模型会以为是两段独立截断。
#[test]
fn tool_result_500kb_input_is_capped_at_the_default_budget() {
    use crate::settings::context::TOOL_RESULT_BUDGET_BYTES;
    let limit = TOOL_RESULT_BUDGET_BYTES;
    // ⚠ 这一处是**刻意的手算对照哨兵**（与"夹具不许手算"相反的一侧）：额度本身由
    // 生产常量给，这里独立对一次数——若有人把常量改了，全链路其它
    // 断言会跟着一起漂，只有这一条会红。
    assert_eq!(limit, 24 * 1024, "固定档 = 24 KiB");
    let req = LlmRequest {
        messages: vec![tool_msg(json!("z".repeat(512_000)))],
        ..Default::default()
    };
    let p = build_payload(
        "m",
        &req,
        false,
        Dialect::None,
        &WireCaps {
            tool_result_budget_bytes: Some(limit),
            ..Default::default()
        },
        None,
    )
    .unwrap();
    let content = p["messages"][0]["content"].as_str().unwrap();
    let mark = content.find("\n\n【").unwrap();
    // 正文段严格在预算内（提示句是给模型看的元信息，占预算外，长度由格式统一那条兜住）
    assert_eq!(mark, limit);
    assert!(content.len() < 512_002);
    // 标注格式统一 = 整条内容里只有一句截断说明
    assert_eq!(count(content, "工具结果已按"), 1);
    assert_eq!(count(content, "【"), 1);
    // 原输出字节数如实报（模型据此判断还差多少，虚报会让它少续读）
    assert!(content.contains("原输出 512002 字节"));
}

fn count(hay: &str, needle: &str) -> usize {
    hay.split(needle).count() - 1
}
