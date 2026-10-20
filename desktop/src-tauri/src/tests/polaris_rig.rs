//! 北极星长跑的**装置**（S2 刀⑦b）：一台带窗口闸门的替身供应商 + 一整套压缩面 +
//! 一条"可再生大输出"的工具替身。判读与判据在 [`polaris_longrun`](super::polaris_longrun)。
//!
//! 为什么用模拟桩而不是真模型：spec 原文写的就是"**模拟**重负载长会话"，而这轮要证的
//! 性质是上下文预算这一面的算术（到线就压、压得住就不超窗、后半程不比前半程胀），与
//! "模型答得好不好"无关。真跑 44 轮要烧额度、不可重复、也没法把"每轮真实发了多少"
//! 钉成判据。
//!
//! ⚠ **覆盖缺口要说破，不许当成三层都测到了**：落盘层（spill）住在发送侧
//! `llm::wire::payload`，是 `AppLlm` 把消息序列化进 HTTP body 时那一刀；这条模拟链走
//! 的是桩 LLM，结构上碰不到它。spill 自己的证据在 `llm/tests/spill.rs`（刀② 两条各钉
//! 一半）。这里能测到的是挂在 `agent/request` 环上的那两层（微压缩、滑窗保护网，注册
//! 序照 [`crate::state::waterfall`]）与挂在 runner step 边界的主层。

use super::*;

use crate::compact::deep::Compactor;
use crate::compact::Failures;
use crate::context_budget::effective_threshold;
use crate::microcompact::{MainThreshold, MicroCompact};
use crate::runner::UsagePolicy;
use crate::settings::session_capacity;
use crate::slide::Slide;
use async_trait::async_trait;
use cordis_core::Context;
use kernel_agent::{
    estimate_tokens, points, Kernel, LlmError, LlmRequest, LlmResponse, LlmService, LlmUsage,
    ToolSource,
};
use memory_tree::{MemoryManagerClient, MemoryManagerUsage};
use plugin_api::{CallCtx, SideEffect, Tool, ToolError, ToolSpec};
use semver::Version;
use session_store::{Message, SessionEvent};
use std::sync::atomic::{AtomicUsize, Ordering};

/// 跑多少轮。**spec 的下限是 ≥40，这里取 100，而且是判据③ 逼出来的**：主层的形状是
/// 锯齿（压到阈值一半再涨回线），44 轮那一版实测爬坡段（第 1→20 轮）横跨了整个前半程，
/// 前半均值被爬坡拉低到 13142、后半是稳态 18507 ⇒ 比值 1.408 红。那条红不是"越跑越胀"，
/// 是"爬坡段 vs 稳态段"——判据③ 要证的是稳态不胀，就得让分点两侧都落在稳态里。
/// 读数与代价都写进 specs/09 刀⑦b 条目（这条口径 spec 原文没写，是实测补的）。
pub(crate) const RUNS: u32 = 100;
/// 替身模型的上下文窗口。
pub(crate) const WINDOW_TOKENS: u64 = 32_768;
/// 模型行钉死的输出上限 ⇒ 输出侧预留 = `min(8192, 21000)` = 8192，有效阈值随之退到
/// 24_576。**这颗数不许取得比一轮的净增还小**：触发线吃锚点（上一次请求上报的数），
/// 而请求是这一次的，中间差一轮增量；预留小过增量就必然撞窗（本轮第一版实测到，
/// 已按台账 ⓜ 登记）。
pub(crate) const MAX_OUTPUT_TOKENS: u64 = 8_192;
/// 每轮那条工具输出往参数里塞多少个「细节」（两字一节）。800 节 = 1 600 个 CJK
/// = 4 800 字节，`estimate_tokens` 按字节 /4 ⇒ 一条结果 ≈ 1 200 token，连同骨架
/// 一轮净增实测就是这个量。为什么不许把这一枚调大当"更重的负载"：
/// · 微压缩要求「保留 5 组」之后还有可清的组，单组 size × 5 必须明显小于那条线
///   （22_118）——4000 节那一版每组 6 115、五组 30 575 已超线，微压缩整跑 0 次落账，
///   等于把这层从被测面上摘掉了（台账 ⓜ 记的就是这两版的读数）；
/// · 判据① 的 ×1.1 容差是给"锚点比本次请求滞后一轮"留的，同样要求一轮增量 ≤ 阈值一成
///   （24_576 的那版是 2 457），6 115 那一版破到 1.24 而没撞窗（32_768 还挡得住）。
pub(crate) const PAYLOAD_CHARS: usize = 800;
/// 每轮那句**用户提问**自己带多少节「要求」（同样两字一节 ⇒ 600 节 ≈ 900 token）。
/// 这一枚不是装饰：微压缩只清"可由同一调用重取得到"的工具正文，用户说过的话它一条
/// 不动 ⇒ 提问太轻的那版配方里每轮净留 112 token 的骨架，100 轮涨到 17 193 仍够不着
/// 主层那条线（24_576），于是"后半程比前半程胀 1.42 倍"那条红测的其实是骨架线性积累，
/// 而不是主层接管后的稳态形状（台账 ⓝ）。真实长会话里用户每轮都在写东西，所以给它分量。
pub(crate) const QUESTION_CHARS: usize = 600;
/// 工具替身用**白名单里的真名**（`microcompact::COMPACTABLE_TOOLS`）：微压缩只清"结果
/// 可由同一调用重取得到"的工具，换成别的名字这一层结构上就不会动手，ⓔ 那笔账也就取不到。
pub(crate) const TOOL_NAME: &str = "fs_read";
/// `llm/retry.rs:53` 那句话的开头（判据② 的"果"：整轮日志里命中必须为 0）。
pub(crate) const OVERFLOW_SENTENCE: &str = "上下文超出模型窗口";

/// 大输出替身工具：纯读、无副作用，返回一段可控大小的正文。
struct RegenTool;

fn regen_spec() -> &'static ToolSpec {
    static SPEC: OnceLock<ToolSpec> = OnceLock::new();
    SPEC.get_or_init(|| ToolSpec {
        name: TOOL_NAME.into(),
        description: "test regen".into(),
        schema: serde_json::json!({ "type": "object" }),
        side_effect: SideEffect::Pure,
        exposure: Default::default(),
    })
}

#[async_trait]
impl Tool for RegenTool {
    fn spec(&self) -> &ToolSpec {
        regen_spec()
    }
    fn plugin_version(&self) -> &Version {
        static V: Version = Version::new(1, 0, 0);
        &V
    }
    async fn call(&self, _ctx: &CallCtx, args: Value) -> Result<Value, ToolError> {
        let path = args.get("path").and_then(Value::as_str).unwrap_or("?");
        Ok(serde_json::json!({
            "content": format!("{path} 正文：") + &"细节".repeat(PAYLOAD_CHARS),
        }))
    }
}

struct RegenSource;

#[async_trait]
impl ToolSource for RegenSource {
    async fn resolve(&self, _sid: Option<&str>, _name: &str) -> Result<Arc<dyn Tool>, ToolError> {
        Ok(Arc::new(RegenTool) as Arc<dyn Tool>)
    }
    async fn list(&self, _sid: Option<&str>) -> Vec<ToolSpec> {
        vec![regen_spec().clone()]
    }
}

/// 带窗口闸门的测量桩。它做三件事：量每次请求的 input（判据的取数口）、替供应商拒收
/// 超窗请求（判据② 的"因"）、照真链路把用量如实报成 `LlmUsage`（锚点因此吃"上一次真
/// 发了多少"，而不是 `None` 回落估算）。
///
/// 决策规则与 [`crate::demo_llm::DemoLlm`] 同形但工具名换成白名单里那枚：新问题→发一次
/// `fs_read`；拿到工具结果→汇总作答（本轮收束）。
pub(crate) struct Gate {
    window: u64,
    /// 拒收读数表（刀⑧）：真链路里写它的是 `AppLlm`，替身链路里就是这里——
    /// 同一件事（被拒那一次没有用量上报，得把读数交给压缩面），同一个尺。
    refusals: Arc<crate::compact::Overflow>,
    inputs: StdMutex<Vec<u64>>,
    rejected: AtomicUsize,
    next_call: AtomicUsize,
}

impl Gate {
    fn new(window: u64, refusals: Arc<crate::compact::Overflow>) -> Arc<Self> {
        Arc::new(Self {
            window,
            refusals,
            inputs: StdMutex::new(Vec::new()),
            rejected: AtomicUsize::new(0),
            next_call: AtomicUsize::new(0),
        })
    }
    pub(crate) fn inputs(&self) -> Vec<u64> {
        self.inputs.lock().unwrap().clone()
    }
    pub(crate) fn rejected(&self) -> usize {
        self.rejected.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl LlmService for Gate {
    async fn complete(&self, req: LlmRequest) -> Result<LlmResponse, LlmError> {
        let input = estimate_tokens(&req.messages);
        self.inputs.lock().unwrap().push(input);
        if input > self.window {
            self.rejected.fetch_add(1, Ordering::SeqCst);
            // ⓜ 那条链的替身现场：被拒的这一次**永远不会有用量上报**（供应商在处理
            // 之前就关门），所以得像 `AppLlm::refusal_note` 那样把这颗读数交给压缩面，
            // 否则触发线追不上已经涨过的窗口 = 整面自锁。
            if let Some(sid) = req.session_id.as_deref() {
                self.refusals.note(sid, input);
            }
            return Err(LlmError::Backend(format!(
                "模拟供应商 413：请求 {input} token 超过窗口 {}",
                self.window
            )));
        }
        let (content, tool_calls) = match req.messages.last() {
            Some(Message::ToolResult { output, .. }) => (
                format!("已读到内容（{} 字），答复如下。", output.to_string().len()),
                vec![],
            ),
            _ => {
                let n = self.next_call.fetch_add(1, Ordering::SeqCst) + 1;
                (
                    String::new(),
                    vec![kernel_agent::ToolCallReq {
                        id: plugin_api::CallId(format!("polaris-call-{n}")),
                        tool: TOOL_NAME.to_string(),
                        args: serde_json::json!({ "path": format!("/notes/note-{n}.md") }),
                        session_id: None,
                    }],
                )
            }
        };
        Ok(LlmResponse {
            content,
            tool_calls,
            usage: Some(LlmUsage {
                input,
                output: 24,
                ..Default::default()
            }),
        })
    }
}

/// 主层摘要客户端桩：数得出趟数，回一份七节齐全的摘要（形状出处 = `compact::sample_summary`，
/// 与判据侧同一个构造函数，不在夹具里再抄一遍节名表）。
#[derive(Default)]
pub(crate) struct Summarizer {
    calls: AtomicUsize,
}

#[async_trait]
impl MemoryManagerClient for Summarizer {
    async fn complete(
        &self,
        _system: &str,
        user: &str,
        _model: Option<&str>,
    ) -> Result<(String, MemoryManagerUsage), String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok((
            crate::compact::sample_summary(),
            MemoryManagerUsage {
                input: user.len() as u64 / 4,
                output: 200,
            },
        ))
    }
}

impl Summarizer {
    /// 摘要往返趟数（"这一跑花了几条模型往返写摘要"的读数）。
    pub(crate) fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

/// 一整套压缩面 + 窗口读数。两枚开关各自可关：腿 2 两枚全关（造对照红），腿 3 只关
/// 微压缩那一枚（专证主层单独也扛得住）。
pub(crate) struct Rig {
    pub(crate) kernel: Kernel,
    pub(crate) compactor: Arc<Compactor>,
    pub(crate) gate: Arc<Gate>,
    pub(crate) summarizer: Arc<Summarizer>,
    /// 长跑里"被拒现场"的读数表（腿 4 判它有没有把主层顶起步）
    pub(crate) refusals: Arc<crate::compact::Overflow>,
    pub(crate) threshold: u64,
    pub(crate) window: u64,
    /// 扣减链的原料（S3 刀④b）：滑杆、模型输出上限、设置里的预留上限。
    pub(crate) slider: u64,
    pub(crate) max_output: u64,
    pub(crate) reserve_cap: u64,
    pub(crate) root: std::path::PathBuf,
}

impl Rig {
    /// 与真组装点同形（S3 刀④a）：拒收读数那一格也从**同一张表**取，
    /// 这样长跑里进度条落的 `ContextUsage` 与压缩判定吃的是同一个分子。
    /// 腿 4 的靶就是"被拒之后这一档顶不顶得起步"，夹具若恒填 0 就等于把
    /// 被测形状绕过去。
    pub(crate) fn policy(&self, session_id: &str) -> UsagePolicy {
        // 预留量与来源和阈值同用 `context_budget` 的纯函数（S3 刀④b）：
        // 夹具替真组装点填链上那几枚，也不许自己写第二份 min。
        let (output_reserve_tokens, source) =
            crate::context_budget::reserve_reading(Some(self.max_output), self.reserve_cap);
        UsagePolicy {
            threshold_tokens: self.threshold,
            window_tokens: self.window,
            tools_est_tokens: 0,
            refused_tokens: self.refusals.reading(session_id),
            slider_tokens: self.slider,
            output_reserve_tokens,
            output_reserve_source: source.as_str(),
        }
    }
}

/// `reserve` = 档位上钉死的输出上限（也就是输出侧预留，见 `context_budget::output_reserve`）。
/// 腿 4 用它造出"预留小于一轮增量"那一版配方——ⓜ 登记的自锁现场。
pub(crate) fn rig(tag: &str, micro_on: bool, deep_on: bool, reserve: u64) -> Rig {
    let root = crate::testutil::work_dir(tag);
    std::fs::write(
        root.join("settings.json"),
        serde_json::json!({
            "llm": {"active": "p", "providers": [{
                "id": "p", "kind": "openai", "model": "m",
                "windowTokens": WINDOW_TOKENS,
                "models": [{"id": "m", "maxOutput": reserve}]
            }]},
            "context": {"microEnabled": micro_on, "compactEnabled": deep_on}
        })
        .to_string(),
    )
    .unwrap();
    let settings = Arc::new(crate::settings::SettingsStore::open(&root).unwrap());
    let (window, max_output) = settings.with_llm_config(|llm| session_capacity(llm, None, None));
    // 夹具自证：档位没被读进来的话阈值会退成滑杆缺省 100K，那这一跑测的是另一件事。
    assert_eq!(
        window,
        Some(WINDOW_TOKENS),
        "窗口必须从 llm 档位真读进来（读不到 = 夹具坏了，不是压缩面坏了）"
    );
    assert_eq!(max_output, Some(reserve));
    // 阈值走生产那把尺，不在这里手填（取数纪律：一个出处）。S3 刀④b：预留上限
    // 也从设置读——夹具不替生产决定这颗数。
    let slider = settings.organize_threshold_tokens();
    let reserve_cap = settings.output_reserve_cap_tokens();
    let threshold = effective_threshold(slider, window, max_output, reserve_cap);
    let main: MainThreshold = {
        let settings = Arc::clone(&settings);
        Arc::new(move |_sid: Option<&str>| {
            let (w, mo) = settings.with_llm_config(|llm| session_capacity(llm, None, None));
            w.map(|w| {
                effective_threshold(
                    settings.organize_threshold_tokens(),
                    Some(w),
                    mo,
                    settings.output_reserve_cap_tokens(),
                )
            })
        })
    };
    let failures = Arc::new(Failures::default());
    let refusals = Arc::new(crate::compact::Overflow::default());
    let gate = Gate::new(window.unwrap(), Arc::clone(&refusals));
    let summarizer = Arc::new(Summarizer::default());
    let ctx = Arc::new(Context::new());
    ctx.provide_dyn::<dyn ToolSource>(Arc::new(RegenSource));
    ctx.provide_dyn::<dyn LlmService>(Arc::clone(&gate) as Arc<dyn LlmService>);
    ctx.events.on_waterfall(
        points::REQUEST,
        Arc::new(MicroCompact::new(
            Arc::clone(&settings),
            root.join("sessions"),
            Arc::clone(&main),
        )) as Arc<_>,
    );
    ctx.events.on_waterfall(
        points::REQUEST,
        Arc::new(Slide::new(
            Arc::clone(&settings),
            Arc::clone(&failures),
            main,
        )) as Arc<_>,
    );
    let compactor = Arc::new(Compactor::new(
        settings,
        Arc::clone(&summarizer) as Arc<dyn MemoryManagerClient>,
        failures,
        Arc::clone(&refusals),
    ));
    Rig {
        kernel: Kernel::new(ctx, vec![]),
        compactor,
        gate,
        summarizer,
        refusals,
        threshold,
        window: window.unwrap(),
        slider,
        max_output: reserve,
        reserve_cap,
        root,
    }
}

/// 一批事件里两层各自的落账条数。内存与磁盘两个观察点都走这一式（不留第二份口径）。
/// `"micro"` 这枚字面量在生产面只有 `microcompact.rs` 一处（判据 SS18c 钉着），主层走
/// 它的公开常量 `STRATEGY`。
pub(crate) fn count_kinds(events: &[SessionEvent]) -> (usize, usize) {
    let (mut micro, mut deep) = (0usize, 0usize);
    for e in events {
        if let SessionEvent::ContextCompacted { strategy, .. } = e {
            if strategy == "micro" {
                micro += 1;
            } else if strategy == crate::compact::deep::STRATEGY {
                deep += 1;
            }
        }
    }
    (micro, deep)
}

pub(crate) fn mean(xs: &[u64]) -> f64 {
    xs.iter().sum::<u64>() as f64 / xs.len().max(1) as f64
}
