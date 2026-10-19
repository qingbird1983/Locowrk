//! 自建 OpenAI 兼容客户端（LLM.md v3.0，替代 genai 三合一）：一条线格式
//! （`POST {base}/chat/completions` + SSE）+ 传输韧性（重试不设上限——限流
//! /网关抖动/断流对用户不可见，退避封顶、可随时停止）。非流式 `complete`
//! 与流式共用同一条管道（一套线上代码）。
//!
//! `client/` 目录只有一片：`test_probe.rs` = 逐模型连通测试那条旁路（台账第三节 · 第一批第五刀）。
//! 主环（装配 + 重试环 + 单次尝试）**刻意不再切** —— 切成两片就会出现第二条线路形状，
//! 违「同类只许一种形态」；所以本文件仍留在 `check_length` 名单里，三问理由写在
//! `specs/03` 第三节的判定表与提交正文里（名单只放路径，不放理由 = 不造第二个出处）。

use std::sync::atomic::AtomicU64;
use std::time::Duration;

use async_trait::async_trait;
use futures::StreamExt;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde_json::Value;

use kernel_agent::{LlmError, LlmRequest, LlmResponse, LlmService};

use crate::settings::LlmSettings;

use super::dialect::{self, Dialect};
use super::models::{
    apply_proxy, join_version, DEFAULT_ANTHROPIC_BASE, DEFAULT_OLLAMA_BASE, DEFAULT_OPENAI_BASE,
};
use super::retry::{self, Failure};
use super::sse::{SseEvent, SseParser};
use super::wire::{build_payload, Delta, StreamAcc, WireCaps};

mod test_probe;

pub use test_probe::*;

/// 连接建立上限（流式总时长不设限——长输出合法）。
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// 重试等待期间的停等轮询步长（响应「停止生成」）。
const STOP_POLL_MS: u64 = 200;
/// 逐模型连通测试总超时（§8.3：ping 一次，快失败）。
const TEST_TIMEOUT: Duration = Duration::from_secs(15);

/// 重试期间的宿主钩子（AppLlm 组装注入；`None` = 无停止查询/无重置通知，
/// 记录员/标题等非交互调用点重试天然不可见）。
pub struct RetryHooks {
    /// 重试等待期间轮询——true = 用户请求停止，中止重试。
    pub should_stop: Box<dyn Fn() -> bool + Send + Sync>,
    /// 重试前通知（attempt 从 1 计；delay_ms = 本次等待）——流式调用点用
    /// 它发重置事件，前端清掉本步未完成的流式缓冲。
    pub on_retry: Box<dyn Fn(u32, u64) + Send + Sync>,
}

/// 单次尝试的失败：Fatal 原样上抛；Retryable 带分类、是否已向外推过 delta
///（重置判定）、以及 stream_options 修复提示（网关点名拒绝时去掉重发）。
enum AttemptError {
    Fatal(String),
    Retryable {
        failure: Failure,
        emitted: bool,
        repair_stream_options: bool,
        /// 400 报文点名 max_tokens：剥字段重发一次（D5 窄修订的网关兼容
        /// 逃生舱，与 stream_options 同款先例）。
        repair_max_tokens: bool,
    },
}

pub struct CompatLlm {
    next_call: AtomicU64,
}

impl CompatLlm {
    pub fn new() -> Self {
        Self {
            next_call: AtomicU64::new(1),
        }
    }

    /// 端点归一：kind 决定缺省端点，`join_version` 补版本段（LLM.md 覆盖
    /// 表）——ollama 落到 `/v1`（官方 OpenAI 兼容端点），智谱 /v4、方舟
    /// /v3 原样保留。
    pub(super) fn endpoint(cfg: &LlmSettings) -> String {
        let base = match cfg
            .base_url
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            Some(u) => u.to_string(),
            None => match cfg.provider.as_str() {
                "anthropic" => DEFAULT_ANTHROPIC_BASE.to_string(),
                "ollama" => DEFAULT_OLLAMA_BASE.to_string(),
                _ => DEFAULT_OPENAI_BASE.to_string(),
            },
        };
        format!("{}/chat/completions", join_version(&base))
    }

    /// 鉴权 + 自定义头装配（批 C §8.1 HeaderMap）：鉴权头先入，用户头
    /// **最后写入者覆盖**（HeaderMap::insert 替换语义——可覆盖
    /// anthropic-version 等非保留头；鉴权/逐跳保留键已在保存侧拒收）。
    pub(super) fn build_headers(cfg: &LlmSettings, key: &str) -> HeaderMap {
        let mut map = HeaderMap::new();
        let put = |map: &mut HeaderMap, name: &str, val: &str| {
            if let (Ok(n), Ok(v)) = (
                HeaderName::from_bytes(name.as_bytes()),
                HeaderValue::from_str(val),
            ) {
                map.insert(n, v);
            }
        };
        match cfg.provider.as_str() {
            // anthropic 官方兼容层两种鉴权头都收，双带最稳（v3.0 现状）
            "anthropic" => {
                put(&mut map, "authorization", &format!("Bearer {key}"));
                put(&mut map, "x-api-key", key);
                put(&mut map, "anthropic-version", "2023-06-01");
            }
            _ => put(&mut map, "authorization", &format!("Bearer {key}")),
        }
        for (k, v) in &cfg.headers {
            put(&mut map, k, v);
        }
        map
    }

    /// 缺 Key 先行报错（对桌面用户友好）；ollama 无需 Key。
    fn key_check(cfg: &LlmSettings) -> Result<(), LlmError> {
        let env_name = match cfg.provider.as_str() {
            "openai" => Some("OPENAI_API_KEY"),
            "anthropic" => Some("ANTHROPIC_API_KEY"),
            _ => None,
        };
        if let Some(env_name) = env_name {
            let has_key = cfg
                .api_key
                .as_deref()
                .map(str::trim)
                .filter(|k| !k.is_empty())
                .is_some();
            if !has_key && std::env::var(env_name).is_err() {
                return Err(LlmError::Backend(format!(
                    "provider `{}` 缺少 API Key：请在 设置 → 模型配置 里填写，\
                     或设置环境变量 {env_name}",
                    cfg.provider
                )));
            }
        }
        Ok(())
    }

    /// 非流式完成 = 同一条流式管道聚合（无回调、无钩子——重试对调用方
    /// 不可见，只是等待变长）。`assets`（批 D）：附件字节读取器。
    pub async fn complete_with(
        &self,
        cfg: &LlmSettings,
        caps: WireCaps,
        req: LlmRequest,
        assets: Option<&crate::assets::AssetStore>,
    ) -> Result<LlmResponse, LlmError> {
        self.complete_streaming_with(cfg, caps, req, assets, &|_| false, &|_| false, None)
            .await
    }

    /// 流式完成（⑳/v2.57/v4.11 语义同旧后端）：正文/推理 delta 逐段回调，
    /// 回调返回 true = 用户停止（部分正文原样收束）；传输失败自动重试
    /// （不设上限），已推过 delta 的重试先经 `hooks.on_retry` 让前端重置。
    #[allow(clippy::too_many_arguments)]
    pub async fn complete_streaming_with(
        &self,
        cfg: &LlmSettings,
        caps: WireCaps,
        req: LlmRequest,
        assets: Option<&crate::assets::AssetStore>,
        on_delta: &(dyn for<'a> Fn(&'a str) -> bool + Send + Sync),
        on_reasoning: &(dyn for<'a> Fn(&'a str) -> bool + Send + Sync),
        hooks: Option<&RetryHooks>,
    ) -> Result<LlmResponse, LlmError> {
        Self::key_check(cfg)?;
        let dialect = dialect::dialect_of(cfg.base_url.as_deref());
        let mut attempt: u32 = 0;
        let mut drop_stream_options = false;
        let mut drop_max_tokens = false;
        loop {
            attempt += 1;
            match self
                .attempt_stream(
                    cfg,
                    dialect,
                    &caps,
                    &req,
                    assets,
                    on_delta,
                    on_reasoning,
                    drop_stream_options,
                    drop_max_tokens,
                )
                .await
            {
                Ok(resp) => return Ok(resp),
                Err(AttemptError::Fatal(msg)) => return Err(LlmError::Backend(msg)),
                Err(AttemptError::Retryable {
                    failure,
                    emitted,
                    repair_stream_options,
                    repair_max_tokens,
                }) => {
                    // stream_options / max_tokens 被网关点名拒绝：立即去字段
                    // 重发，不算失败、不走退避（各自每次尝试只修一次——D5
                    // 窄修订的网关兼容逃生舱，与 stream_options 同款先例）
                    if repair_stream_options && !drop_stream_options {
                        drop_stream_options = true;
                        continue;
                    }
                    if repair_max_tokens && !drop_max_tokens {
                        drop_max_tokens = true;
                        continue;
                    }
                    let delay = match &failure {
                        Failure::RateLimit { retry_after_ms } => retry::rate_limit_delay_ms(
                            attempt,
                            *retry_after_ms,
                            retry::jitter_random(),
                        ),
                        _ => retry::transient_delay_ms(attempt, failure_retry_after(&failure)),
                    };
                    if emitted {
                        if let Some(h) = hooks {
                            (h.on_retry)(attempt, delay);
                        }
                    }
                    if self.sleep_stoppable(delay, hooks).await {
                        // 等待期间用户停止：重试轮尚无产出，空收束（前端
                        // 已显示「已停止」，不虚构结尾）
                        return Ok(LlmResponse::default());
                    }
                }
            }
        }
    }

    /// 单次尝试：POST + SSE 流循环。返回 Retryable 的分类（含 Retry-After
    /// 解析）与「已推 delta」标记。
    /// `assets`（批 D）：带附件请求的 asset 字节读取器（AppLlm 注入；
    /// build_payload 编码 content 数组时读，读失败 = Fatal）。
    /// `caps`（S2 刀②）：改吃借用——这份里现在住着一个 `Arc`（落盘句柄），
    /// `WireCaps` 因此从 `Copy` 降成 `Clone`；重试循环每轮复用同一个借用，
    /// 比每轮克隆一次更准（克隆会把"每轮一份上限快照"这种错觉留在代码里）。
    #[allow(clippy::too_many_arguments)]
    async fn attempt_stream(
        &self,
        cfg: &LlmSettings,
        dialect: Dialect,
        caps: &WireCaps,
        req: &LlmRequest,
        assets: Option<&crate::assets::AssetStore>,
        on_delta: &(dyn for<'a> Fn(&'a str) -> bool + Send + Sync),
        on_reasoning: &(dyn for<'a> Fn(&'a str) -> bool + Send + Sync),
        drop_stream_options: bool,
        drop_max_tokens: bool,
    ) -> Result<LlmResponse, AttemptError> {
        let broken = || AttemptError::Retryable {
            failure: Failure::Transient {
                retry_after_ms: None,
            },
            emitted: false,
            repair_stream_options: false,
            repair_max_tokens: false,
        };
        // 网络代理（批 C §8.5）：客户端逐请求新建（现状），代理随 cfg
        // 读取——设置保存即热换。
        let client = apply_proxy(
            reqwest::Client::builder().connect_timeout(CONNECT_TIMEOUT),
            cfg.network_proxy.as_ref(),
        )
        .build()
        .map_err(|e| AttemptError::Fatal(format!("HTTP 客户端构建失败: {e}")))?;
        let mut payload = build_payload(&cfg.model, req, true, dialect, caps, assets)
            .map_err(AttemptError::Fatal)?;
        if drop_stream_options {
            payload
                .as_object_mut()
                .expect("build_payload 产出 object")
                .remove("stream_options");
        }
        if drop_max_tokens {
            payload
                .as_object_mut()
                .expect("build_payload 产出 object")
                .remove("max_tokens");
        }
        let key = cfg.api_key.clone().unwrap_or_default();
        let resp = client
            .post(Self::endpoint(cfg))
            .headers(Self::build_headers(cfg, &key))
            .json(&payload)
            .send()
            .await;

        let resp = match resp {
            Ok(r) => r,
            // 连接失败 / 超时 / 传输层错误 = 瞬时
            Err(_) => return Err(broken()),
        };

        let status = resp.status();
        if !status.is_success() {
            let retry_after = parse_retry_after_ms(resp.headers());
            let body = resp.text().await.unwrap_or_default();
            return match retry::classify_status(status.as_u16(), &body) {
                Failure::Fatal(msg) => Err(AttemptError::Fatal(msg)),
                f => Err(AttemptError::Retryable {
                    repair_stream_options: status.as_u16() == 400
                        && !drop_stream_options
                        && body.contains("stream_options"),
                    repair_max_tokens: status.as_u16() == 400
                        && !drop_max_tokens
                        && payload.get("max_tokens").is_some()
                        && body.contains("max_tokens"),
                    emitted: false,
                    failure: merge_retry_after(f, retry_after),
                }),
            };
        }

        let mut acc = StreamAcc::new();
        let mut parser = SseParser::new();
        let mut emitted = false;
        let mut done = false;
        let mut eof = false;
        let mut stream = resp.bytes_stream();
        while !(done || eof) {
            let mut events: Vec<SseEvent> = Vec::new();
            let chunk = match tokio::time::timeout(
                Duration::from_millis(retry::IDLE_TIMEOUT_MS),
                stream.next(),
            )
            .await
            {
                Ok(Some(c)) => Some(c),
                Ok(None) => {
                    // 流自然结束：冲刷缺尾空行的残留事件
                    eof = true;
                    parser.finish(&mut events);
                    None
                }
                Err(_) => {
                    // 空闲看门狗：零事件超 180s = 断流（可重试），正常
                    // 吐字不限总时长
                    return Err(AttemptError::Retryable {
                        failure: Failure::Transient {
                            retry_after_ms: None,
                        },
                        emitted,
                        repair_stream_options: false,
                        repair_max_tokens: false,
                    });
                }
            };
            match chunk {
                Some(Ok(bytes)) => {
                    parser.feed(&String::from_utf8_lossy(&bytes), &mut events);
                }
                Some(Err(_)) => {
                    return Err(AttemptError::Retryable {
                        failure: Failure::Transient {
                            retry_after_ms: None,
                        },
                        emitted,
                        repair_stream_options: false,
                        repair_max_tokens: false,
                    });
                }
                None => {}
            }
            for ev in events {
                match ev {
                    SseEvent::Done => {
                        done = true;
                        break;
                    }
                    SseEvent::Data(data) => {
                        let Ok(chunk) = serde_json::from_str::<Value>(&data) else {
                            continue; // 单行损坏跳过（网关 keep-alive 变体等）
                        };
                        match acc.apply(&chunk) {
                            Delta::Content(d) => {
                                emitted = true;
                                if on_delta(&d) {
                                    // 停止生成（§3.39④）：部分正文原样收束
                                    return Ok(acc.finish(&self.next_call));
                                }
                            }
                            Delta::Reasoning(d) => {
                                emitted = true;
                                if on_reasoning(&d) {
                                    return Ok(acc.finish(&self.next_call));
                                }
                            }
                            Delta::None => {}
                        }
                    }
                }
            }
        }

        // G1 Turn Commit length 闸（可靠性稿 §13，随批 B）：finish_reason
        // = length = 输出在 max_tokens 处被截断——截断不收束（按 Fatal 如实
        // 报错而非当完整轮次）、工具不执行（部分 tool_calls 一律丢弃）、
        // 截断消息不入史（错误路径不落 AssistantMessage）。
        if acc.finish_reason() == Some("length") {
            return Err(AttemptError::Fatal("模型输出在最大输出上限处被截断（finish_reason=length），本轮按未完成收束：部分结果与未完成的工具调用不执行不保存。可在 设置 → 模型 调大该模型的「最大输出」后重试".into()));
        }
        // 收束判据：[DONE] 或 finish_reason 到场才算完整；供应商中途断流
        // （半截正文 / 空收束）按可重试断流处理，不当成功
        if done || acc.finish_reason().is_some() {
            return Ok(acc.finish(&self.next_call));
        }
        Err(AttemptError::Retryable {
            failure: Failure::Transient {
                retry_after_ms: None,
            },
            emitted,
            repair_stream_options: false,
            repair_max_tokens: false,
        })
    }

    /// 重试等待：200ms 步进轮询停止标记（有钩子时），返回 true = 用户停止。
    async fn sleep_stoppable(&self, delay_ms: u64, hooks: Option<&RetryHooks>) -> bool {
        let should_stop = hooks.map(|h| h.should_stop.as_ref());
        let mut waited = 0u64;
        loop {
            if should_stop.is_some_and(|f| f()) {
                return true;
            }
            if waited >= delay_ms {
                return false;
            }
            let step = STOP_POLL_MS.min(delay_ms - waited);
            tokio::time::sleep(Duration::from_millis(step)).await;
            waited += step;
        }
    }
}

impl Default for CompatLlm {
    fn default() -> Self {
        Self::new()
    }
}

/// 裸 `CompatLlm` 不作为服务注册（无设置来源）；入口一律走 AppLlm。
#[async_trait]
impl LlmService for CompatLlm {
    async fn complete(&self, _req: LlmRequest) -> Result<LlmResponse, LlmError> {
        Err(LlmError::Backend(
            "CompatLlm 需要设置来源，请通过 AppLlm 使用".into(),
        ))
    }
}

/// 把响应头里的 Retry-After 并入可重试分类（Fatal 不动）。
fn merge_retry_after(f: Failure, retry_after_ms: Option<u64>) -> Failure {
    match f {
        Failure::RateLimit { .. } => Failure::RateLimit { retry_after_ms },
        Failure::Transient { .. } => Failure::Transient { retry_after_ms },
        Failure::Fatal(msg) => Failure::Fatal(msg),
    }
}

fn failure_retry_after(f: &Failure) -> Option<u64> {
    match f {
        Failure::RateLimit { retry_after_ms } | Failure::Transient { retry_after_ms } => {
            *retry_after_ms
        }
        Failure::Fatal(_) => None,
    }
}

/// `retry-after-ms`（毫秒，浮点容许）→ `retry-after`（秒）。HTTP-date 解析
/// 需额外依赖，跳过（回落退避曲线——极少见）。
fn parse_retry_after_ms(headers: &reqwest::header::HeaderMap) -> Option<u64> {
    let get = |name: &str| headers.get(name).and_then(|v| v.to_str().ok());
    if let Some(ms) = get("retry-after-ms") {
        if let Ok(v) = ms.trim().parse::<f64>() {
            if v.is_finite() && v > 0.0 {
                return Some(v.ceil() as u64);
            }
        }
    }
    let secs = get("retry-after")?.trim();
    secs.parse::<f64>()
        .ok()
        .filter(|v| v.is_finite() && *v > 0.0)
        .map(|v| (v * 1000.0).ceil() as u64)
}
