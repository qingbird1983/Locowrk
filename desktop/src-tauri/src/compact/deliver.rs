//! 收缩矩阵（上游 `packages/core/src/session/compaction.ts:397-476` `deliver`）。
//!
//! 装不下时依次尝试（上游 `compaction.ts:417-441` 逐条）：
//!
//! 1. **原样整段**（估算 ≤ `fit`）——前缀缓存还能命中；
//! 2. **去掉保留区**（估算 ≤ `target`）——同款，只是短一截；
//! 3. **压成文本并整轮丢最老**（瞄准 `target`）；
//! 4. 上面都拼不出来 ⇒ 再按 [`SHRINK_STEPS`] 收窄目标（70% / 50% / 35%）。
//!
//! 用尽 ⇒ 失败（判据 `cannot-shrink`）。

use kernel_agent::LlmRequest;
use plugin_api::ToolSpec;

use super::recent;
use super::SHRINK_STEPS;

/// 一段纯文本的估算（上游 `Token.estimate` = 字节 / 4）。
pub(crate) fn estimate_text(text: &str) -> u64 {
    (text.len() as u64) / 4
}

/// 一条请求的估算大小（上游 `estimateRequest`）。
///
/// 上游那份把 system / tools / messages 分开算；本仓的 messages 走仓库唯一那把
/// 尺（`estimate_tokens`），system / tools 按同一把尺补齐。
pub(crate) fn estimate_request(req: &LlmRequest) -> u64 {
    let system = req.system.as_deref().map(estimate_text).unwrap_or(0);
    let tools: u64 = req.tools.iter().map(estimate_tool).sum();
    system + tools + kernel_agent::estimate_tokens(&req.messages)
}

/// 一枚工具定义的估算（上游 `estimateTool`：只算 provider 真收到的那三样）。
fn estimate_tool(tool: &ToolSpec) -> u64 {
    let schema = serde_json::to_string(&tool.schema).unwrap_or_default();
    estimate_text(&tool.name) + estimate_text(&tool.description) + estimate_text(&schema)
}

/// 请求里"原样保留的那一段"的条数（上游的 `lead`）。
///
/// 上游 = 上一个 checkpoint 之后的那一段；本仓的 checkpoint 就是窗口第一条 ⇒
/// lead = 1，没有 checkpoint ⇒ lead = 0。
pub(crate) fn lead_len(request: &LlmRequest) -> usize {
    match request.messages.first() {
        Some(m) if super::plan::is_checkpoint(m) => 1,
        _ => 0,
    }
}

/// 收缩状态（上游 `deliver` 里的那几枚局部量）。
pub(crate) struct Shrink {
    /// 目标大小：这趟想发多大。
    pub target: u64,
    /// "装得下"的尺：整段请求的估算 ≤ 它就原样发。
    pub fit: u64,
    /// 已经发生的"太长"拒收次数。
    pub rejections: u32,
    /// 被拒那一次的请求估算（后续各档都瞄准它的百分比）。
    pub rejected: Option<u64>,
    /// 已经改成纯文本（`payload-too-large` 那一档）。
    pub as_text: bool,
}

impl Shrink {
    /// 初始状态。
    pub(crate) fn new(target: u64, fit: u64) -> Self {
        Shrink {
            target,
            fit,
            rejections: 0,
            rejected: None,
            as_text: false,
        }
    }

    /// 一次"太长"拒收之后的下一档（上游 `compaction.ts:455-463`）。
    ///
    /// 返回 `None` = 递减带用尽（上游那句 `if (step === undefined) return fail`）。
    pub(crate) fn on_too_long(&mut self, request_estimate: u64) -> Option<f64> {
        let step = SHRINK_STEPS.get(self.rejections as usize).copied()?;
        let rejected = *self.rejected.get_or_insert(request_estimate);
        self.target = (rejected as f64 * step).floor() as u64;
        self.fit = self.target;
        self.rejections += 1;
        Some(step)
    }
}

/// 挑出来的那份请求（连同"它是不是被压成文本了"）。
///
/// 那一枚标记有用：`payload-too-large` 那一档只对**原样**与**去保留区**那两种
/// 生效（上游 `compaction.ts:450`），压成文本的请求再报"太大"就按"太长"处理。
pub(crate) struct Candidate {
    pub request: LlmRequest,
    pub flattened: bool,
}

/// 挑这一趟要发的那份请求（上游 `compaction.ts:417-432` 逐条）。
///
/// `prepared` = 整段（真实窗口）；`history` = 去掉保留区的那一段。
pub(crate) fn pick(
    prepared: &LlmRequest,
    history: &LlmRequest,
    shrink: &Shrink,
) -> Option<Candidate> {
    let unchanged = if shrink.as_text {
        None
    } else if estimate_request(prepared) <= shrink.fit {
        Some((prepared.clone(), false))
    } else if estimate_request(history) <= shrink.target {
        Some((history.clone(), false))
    } else {
        None
    };
    if let Some((request, flattened)) = unchanged {
        return Some(Candidate { request, flattened });
    }
    if let Some(request) = flatten(history, shrink.target) {
        return Some(Candidate {
            request,
            flattened: true,
        });
    }
    if shrink.rejections > 0 {
        // 已经拒过：拼不出来就是"再也缩不动了"。
        return None;
    }
    if shrink.as_text {
        return flatten(history, u64::MAX).map(|request| Candidate {
            request,
            flattened: true,
        });
    }
    Some(Candidate {
        request: history.clone(),
        flattened: false,
    })
}

/// 压成文本并丢最老（额度 = 目标 − lead 的估算 − 那句省略标记的估算）。
///
/// 上游那句省略标记按**最坏情况**（全部都要省略）预留位置：先留够再挑留哪些，
/// 否则"留到最后发现省略标记放不下"就得回头再算一轮。
fn flatten(history: &LlmRequest, target: u64) -> Option<LlmRequest> {
    let lead = lead_len(history);
    let groups = recent::exchanges(&history.messages[lead..]);
    let room = target
        .saturating_sub(estimate_lead(history, lead))
        .saturating_sub(estimate_text(&recent::omitted_note(groups.len())));
    recent::flatten_and_drop_oldest(history, lead, room)
}

/// lead 那一段（system + tools + 头部消息）的估算。
fn estimate_lead(history: &LlmRequest, lead: usize) -> u64 {
    let system = history.system.as_deref().map(estimate_text).unwrap_or(0);
    let tools: u64 = history.tools.iter().map(estimate_tool).sum();
    system + tools + kernel_agent::estimate_tokens(&history.messages[..lead])
}
