//! 传输韧性：SSE 分帧跨块缓冲 + 错误分类（状态码 × 报文）与退避曲线。

use super::*;

// ---------- sse：分帧 ----------

// ---------- retry：分类与退避 ----------

/// 按行缓冲跨网络分块拼帧；注释/keep-alive 跳过；[DONE] 哨兵；缺尾空行
/// 由 finish 冲刷。
#[test]
fn sse_parser_buffers_lines_across_chunks() {
    let mut p = SseParser::new();
    let mut out = Vec::new();
    p.feed("data: {\"a\"", &mut out);
    p.feed(":1}\n\n: keep-alive\ndata: [DON", &mut out);
    assert_eq!(out.len(), 1, "第一条跨块凑齐");
    assert_eq!(out[0], SseEvent::Data("{\"a\":1}".into()));
    p.feed("E]\n", &mut out); // [DONE] 行无尾空行
    assert_eq!(out.len(), 2);
    assert_eq!(out[1], SseEvent::Done);
    p.finish(&mut out); // 残留（无）——幂等
    assert_eq!(out.len(), 2);
}

/// 分类表：鉴权 fatal / 429 限流 / 5xx·408 瞬时 / 上下文超限 fatal /
/// 普通 4xx fatal（带报文片段）。
#[test]
fn classify_follows_status_and_body() {
    assert!(matches!(
        retry::classify_status(401, ""),
        Failure::Fatal { .. }
    ));
    assert!(matches!(
        retry::classify_status(403, ""),
        Failure::Fatal { .. }
    ));
    assert!(matches!(
        retry::classify_status(429, "rate limited"),
        Failure::RateLimit {
            retry_after_ms: None
        }
    ));
    assert!(matches!(
        retry::classify_status(502, ""),
        Failure::Transient { .. }
    ));
    assert!(matches!(
        retry::classify_status(408, ""),
        Failure::Transient { .. }
    ));
    // 400 + 上下文超限语义 → fatal（重试不会变小）
    assert!(matches!(
        retry::classify_status(400, "maximum context length exceeded"),
        Failure::Fatal { .. }
    ));
    assert!(matches!(
        retry::classify_status(413, "too many tokens"),
        Failure::Fatal { .. }
    ));
    // 普通 400 = 请求错误，如实透传
    let f = retry::classify_status(400, "invalid field 'foo'");
    assert!(matches!(f, Failure::Fatal { .. }));
}

/// 超限文案只许点名**现役**控件（队列 Q7 · CB §7.3）。反向半边同样承重，而且它在
/// S2 刀⑥ 换过一次方向：「压缩阈值」曾经是 v2.137 退役掉的旧设置项（那时反向钉它、
/// 正向钉「整理阈值」）；裁定 B 把 `threshold_tokens` 那枚滑杆**改判为整条压缩面的
/// 全局阈值**并在界面上改名「压缩阈值」之后，被退役的那一侧换成了「整理阈值」⇒
/// 正反两侧随之对调。这一格就是"文案与界面同名"的机器证明：改了界面名而文案没跟上，
/// 这里当场红。而只判"不含退役字样"的话，把三个现存手段删掉仍然绿 ⇒ 两侧都钉。
#[test]
fn context_overflow_message_points_at_live_controls_only() {
    let msg = match retry::classify_status(400, "maximum context length exceeded") {
        Failure::Fatal { msg: m, .. } => m,
        other => panic!("上下文超限必须判 Fatal（重试不会变小），实得 {other:?}"),
    };
    for live in ["新开会话", "压缩阈值", "shell"] {
        assert!(
            msg.contains(live),
            "现存手段「{live}」必须出现在文案里：{msg}"
        );
    }
    for retired in ["整理阈值", "压缩策略", "稳定期"] {
        assert!(
            !msg.contains(retired),
            "界面上已不存在的名字「{retired}」不许再出现在文案里：{msg}"
        );
    }
    assert!(
        msg.contains("设置 → 上下文"),
        "要给出现役设置项的可点名路径：{msg}"
    );
}

/// 上下文超限语义识别：中文 Fatal 文案与英文并集必须由**同一个谓词**认出。
///
/// ⚠ **它现在只服务跨内核边界的那一处**（specs/13 刀⑦b）：压缩那条路改走
/// **档位**（`FailureKind::ContextOverflow`，传输层分好档随错误上来），主对话轮
/// 的「超窗 ⇒ 触发压缩」拿到的是内核那枚**字符串**错误 ⇒ 仍按文本认。
/// 中文文案与英文并集必须同源可判：改了文案而谓词没跟上，表现是"超窗时不递减、
/// 直接放弃"，也就是把唯一能自愈的那一档悄悄删掉，而且没有任何一条其它用例会红。
#[test]
fn the_overflow_message_is_recognised_by_the_same_predicate() {
    let Failure::Fatal { msg: text, .. } = retry::classify_status(413, "This prompt is too long")
    else {
        panic!("413 + 超限措辞该判 Fatal");
    };
    assert!(
        retry::is_overflow_message(&text),
        "中文 Fatal 文案要与英文并集同源可判：{text}"
    );
    // 正向另一半：直呼通路原样透传的供应商报文（还没进 classify 的那一档）
    assert!(retry::is_overflow_message(
        "HTTP 400: This model's maximum context length is 8192 tokens"
    ));
    // 反向两条：认错了方向就是白花几条往返 / 把能自愈的那一档当死错
    assert!(
        !retry::is_overflow_message("网络断了"),
        "普通失败不许触发递减"
    );
    assert!(
        !retry::is_overflow_message("鉴权失败（HTTP 401）：API Key 无效"),
        "鉴权失败不是超窗，不许被认成'缩一缩内容就好'的事"
    );
}

/// 退避曲线：限流 2s×2 指数 + 抖动上界、封顶 30s、Retry-After 优先；
/// 瞬时 1/2/4/8s 后维持 8s。
#[test]
fn backoff_curves_match_policy() {
    // 限流：attempt=1 → 2s..2.5s；attempt=3 → 8s..10s
    let d = retry::rate_limit_delay_ms(1, None, 1.0);
    assert_eq!(d, 2_500);
    let d = retry::rate_limit_delay_ms(1, None, 0.0);
    assert_eq!(d, 2_000);
    let d = retry::rate_limit_delay_ms(3, None, 0.0);
    assert_eq!(d, 8_000);
    assert_eq!(
        retry::rate_limit_delay_ms(20, None, 1.0),
        retry::RATE_LIMIT_MAX_MS
    );
    // 服务器声明优先（封顶 30s）
    assert_eq!(retry::rate_limit_delay_ms(1, Some(5_000), 0.0), 5_000);
    assert_eq!(
        retry::rate_limit_delay_ms(1, Some(999_999), 0.0),
        retry::RATE_LIMIT_MAX_MS
    );
    // 瞬时：1/2/4/8/8...
    assert_eq!(retry::transient_delay_ms(1, None), 1_000);
    assert_eq!(retry::transient_delay_ms(2, None), 2_000);
    assert_eq!(retry::transient_delay_ms(3, None), 4_000);
    assert_eq!(retry::transient_delay_ms(4, None), 8_000);
    assert_eq!(retry::transient_delay_ms(9, None), 8_000);
    assert_eq!(retry::transient_delay_ms(2, Some(3_000)), 3_000);
}
