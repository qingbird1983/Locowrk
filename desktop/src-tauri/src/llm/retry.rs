//! 传输韧性（LLM.md v3.0「传输韧性」节）：错误分类与退避的纯函数。
//! 退避曲线参照 PI-Desktop `provider-retry`（限流 2s×2 指数 + 抖动封顶
//! 30s / 瞬时 1-8s 翻倍封顶 / 服务器 `Retry-After` 优先 / 180s 空闲看门
//! 狗）；**重试不设次数上限**（用户裁定"半次失败都不可接受"）——瞬时故障
//! 一直重试到成功或用户点停止，单次等待始终封顶。

/// 限流退避：2s 起步 ×2 指数 + 25% 抖动，单次封顶 30s。
pub const RATE_LIMIT_INITIAL_MS: u64 = 2_000;
pub const RATE_LIMIT_MAX_MS: u64 = 30_000;
pub const RATE_LIMIT_JITTER: f64 = 0.25;
/// 瞬时故障退避：1s 起步翻倍，封顶 8s（此后维持 8s，节奏可预期）。
pub const TRANSIENT_INITIAL_MS: u64 = 1_000;
pub const TRANSIENT_MAX_MS: u64 = 8_000;
/// 流空闲看门狗：零事件超过此时长 = 断流（可重试），防止挂在半死的连接
/// 上永远等。正常的长输出只要持续吐字就不限总时长。
pub const IDLE_TIMEOUT_MS: u64 = 180_000;

/// 一次请求失败的分类（LLM.md 分类表）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// 供应商限流（429）。`retry_after_ms` = 服务器声明的等待（如有）。
    RateLimit { retry_after_ms: Option<u64> },
    /// 瞬时故障（408 / 5xx / 网络错误 / 超时 / 断流）。
    Transient { retry_after_ms: Option<u64> },
    /// 重试无意义的真错误——原样报给用户（鉴权失败 / 上下文超限 / 请求
    /// 错误）。
    Fatal(String),
}

/// HTTP 状态 + 响应体 → 分类。`Retry-After` 头由调用方解析后并入
/// （`Failure::retry_after_ms`），本函数只看状态与报文语义。
pub fn classify_status(status: u16, body: &str) -> Failure {
    match status {
        401 | 403 => Failure::Fatal(format!(
            "鉴权失败（HTTP {status}）：API Key 无效或无权限，请在 设置 → 模型配置 里检查"
        )),
        429 => Failure::RateLimit {
            retry_after_ms: None,
        },
        408 => Failure::Transient {
            retry_after_ms: None,
        },
        500..=599 => Failure::Transient {
            retry_after_ms: None,
        },
        400 | 413 if is_context_overflow(body) => Failure::Fatal(
            // ⚠ 这句文案原先指「设置 → 上下文 的**压缩阈值**」（队列 Q7 · CB §7.3）。那个设置项
            // 已随 v2.137 退役（`CtxCompact` 桌面链整条删掉，`settings/context.rs` 只剩
            // `threshold_tokens`），文案等于把用户指向一个空设置项。S2 那一批之后它又漂过一次：
            // 刀⑥ 按裁定 B 把那枚滑杆**改判为整条压缩面的全局阈值**并在界面上改名「压缩阈值」，
            // 于是这句指「整理阈值」变成指一个界面上已经不存在的名字。现在只许点名**当下真存在**
            // 的手段：压缩阈值（调低 ⇒ 更早触发压缩）、新开会话、减少单次工具输出。
            // 判据 = `llm::tests::transport` 里那条"文案只指现役控件"（正向钉三个现存
            // 手段、反向钉退役字样），缺任何一侧都能被改回去而测试不红。
            "上下文超出模型窗口：请新开会话继续，或把 设置 → 上下文 的压缩阈值调低（更早触发压缩），并减少单次工具输出（如 shell / fs_read 的返回量）。".into(),
        ),
        _ => {
            let snippet: String = body.chars().take(300).collect();
            Failure::Fatal(format!("HTTP {status}: {snippet}"))
        }
    }
}

/// 上下文超限语义识别（各家措辞不一，取并集小写匹配；400/413 且命中才
/// 判超限——普通 400 是请求错误，如实透传）。
fn is_context_overflow(body: &str) -> bool {
    const PATTERNS: [&str; 7] = [
        "context length",
        "maximum context",
        "context window",
        "too many tokens",
        "prompt is too long",
        "input length exceeds",
        "reduce the length",
    ];
    let lower = body.to_lowercase();
    PATTERNS.iter().any(|p| lower.contains(p))
}

/// 预算耗尽错误的前缀（specs/12 刀①）：`llm/client.rs` 的预算环打这一枚。
///
/// 为什么要有这一枚而不是让调用方自己判断：压缩层要把「预算耗尽」与「别的失败」
/// 分开——界面上要说的是两句不同的真话（"摘要模型没答上" vs "摘要调用超出了预算"）。
/// 判据只有这一个出处：前缀串与判据函数同处一文件，改一边必须改另一边。
pub(crate) const BUDGET_PREFIX: &str = "[budget]";

/// 判一条**错误文本**是不是"预算耗尽"（specs/12 刀①：压缩层用它决定要给哪一档 why）。
///
/// 形态与 [`is_overflow_message`] 同款（同款先例）：判据住在模型接入面，压缩层
/// 不自备关键词表——两处各写一份，改一边另一边会漂。
pub(crate) fn is_budget_exhausted(msg: &str) -> bool {
    msg.starts_with(BUDGET_PREFIX)
}

/// 判一条**错误文本**是不是"上下文超限"（S2 刀④d 的压缩层用它决定要不要递减
/// 重试）。
///
/// 为什么不许压缩层自己写一份关键词表：那是同一件事的第二个出处——上面那张
/// `PATTERNS` 改一字（新接一家供应商），压缩层不会跟着改，表现是"摘要请求超窗时
/// 不递减、直接放弃"，而这恰恰是唯一能自愈的那一档。直呼通路
/// （`llm/aux_client.rs`）把 [`classify_status`] 的结果 `to_string()` 之后交回来，
/// 交回来的是**中文文案**而不是原始报文，所以这里在并集之外还要认那句中文；
/// 两者的关系由用例 `the_overflow_message_is_recognised_by_the_same_predicate` 钉住。
pub(crate) fn is_overflow_message(msg: &str) -> bool {
    is_context_overflow(msg) || msg.contains("上下文超出模型窗口")
}

/// 限流退避：服务器声明优先（封顶 30s），否则 2s × 2^(attempt-1) 指数 +
/// 25% 抖动（同 PI：`random ∈ [0,1)`）。`attempt` 从 1 计。
pub fn rate_limit_delay_ms(attempt: u32, retry_after_ms: Option<u64>, random: f64) -> u64 {
    if let Some(ms) = bounded_server(retry_after_ms, RATE_LIMIT_MAX_MS) {
        return ms;
    }
    let attempt = attempt.max(1);
    let base = RATE_LIMIT_INITIAL_MS as f64 * 2f64.powi(attempt.saturating_sub(1) as i32);
    let jitter = base * RATE_LIMIT_JITTER * random.clamp(0.0, 1.0);
    (base + jitter).min(RATE_LIMIT_MAX_MS as f64).ceil() as u64
}

/// 瞬时退避：服务器声明优先（封顶 8s——网关说何时就绪就听它的），否则
/// 1s/2s/4s/8s 翻倍后维持 8s。不加抖动：单请求重试不需要去同步。
pub fn transient_delay_ms(attempt: u32, retry_after_ms: Option<u64>) -> u64 {
    if let Some(ms) = bounded_server(retry_after_ms, TRANSIENT_MAX_MS) {
        return ms;
    }
    let attempt = attempt.max(1);
    let base = TRANSIENT_INITIAL_MS.saturating_mul(1u64 << (attempt - 1).min(3));
    base.min(TRANSIENT_MAX_MS)
}

/// 服务器声明延迟：非有限值丢弃，钳到 [0, cap]。
fn bounded_server(ms: Option<u64>, cap: u64) -> Option<u64> {
    ms.map(|v| v.min(cap))
}

/// 抖动熵源：无需 rand 依赖，纳秒亚秒段够用（抖动只求去同步不求密码学）。
pub fn jitter_random() -> f64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    f64::from(nanos % 1000) / 1000.0
}
