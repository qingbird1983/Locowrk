//! 设置驱动的模型服务分发（阶段八②）：model 已配置 → 自建 OpenAI 兼容
//! 客户端（LLM.md v3.0，含传输韧性重试）；未配置 → DemoLlm 脚本替身（无
//! 网络开箱即用）。
//!
//! 每次请求前重读设置：改 provider / model / key 立即生效，无需重启
//! （设置面板保存即可用，§13.1）。
//!
//! v2.58 会话级模型覆盖（§3.30）：可选注入 [`ModelOverrideStore`]——
//! `LlmRequest.session_id` 命中覆盖时按会话旁车解析：先按
//! `model_provider` 路由到档位（kind/base/key/窗口整档换），再以覆盖
//! 模型名替换 model；waterfall 契约的 `req.model`（§7.2 决策环改写）
//! 优先级最高。设置缺省完全不动。
//!
//! 重试面（LLM.md v3.0）：组装点注入停止标记与重试通知——重试等待期间
//! 轮询「停止生成」；已推过 delta 的重试先发 `agent://llm-retry` 让前端
//! 重置流式缓冲。未注入（测试/记录员）= 无钩子，重试静默进行。

use std::sync::Arc;

use async_trait::async_trait;
use kernel_agent::{estimate_tokens, LlmError, LlmRequest, LlmResponse, LlmService};
use serde_json::Value;

use super::client::{CompatLlm, RetryHooks};
use super::dialect;
use super::wire::WireCaps;
use crate::demo_llm::DemoLlm;
use crate::model_override::ModelOverrideStore;
use crate::settings::{LlmSettings, ProviderProfile, SettingsStore, SubagentPin};

/// 重试通知出口（组装点闭包 emit `agent://llm-retry`；v2.66 同款组装闭包
/// 模式——AppHandle 不进 llm 模块）。
pub type RetryNotify = Arc<dyn Fn(&str, u32, u64) + Send + Sync>;

/// 一次生效快照里顺手读出的**模型行钉死值**（台账 T5 的产物）。
///
/// 三个字段都只可能来自 settings（用户钉死面，save 时打 `source=user`）：
/// 档位里没有这个模型行 ⇒ 整组 None ⇒ 不发明能力。之所以不直接叫 `WireCaps`：
/// caps 还有一格来自 context 分区，而 llm 区的锁内不许跨区读（非重入 `inner`）。
struct RowFacts {
    max_output: Option<u64>,
    ctx_window: Option<u64>,
    supports_image: Option<bool>,
}

pub struct AppLlm {
    settings: Arc<SettingsStore>,
    /// 会话级模型覆盖（v2.58）；None = 不解析（测试路径）。
    overrides: Option<Arc<ModelOverrideStore>>,
    demo: DemoLlm,
    real: CompatLlm,
    /// 停止标记（v4.11）：重试等待期间轮询；None = 不轮询。
    stop: Option<Arc<crate::stop::StopFlags>>,
    /// 重试通知（流式重置）；None = 不通知。
    notify_retry: Option<RetryNotify>,
    /// 前缀断裂诊断（M6.1）：开关开时每 step 记分块 hash 链；命令层读。
    diagnoser: Arc<super::diagnose::Diagnoser>,
    /// 资产存储（批 D）：带附件请求的 wire 编码字节源；None = 附件请求
    /// Fatal（测试路径错误配置如实报错）。
    assets: Option<Arc<crate::assets::AssetStore>>,
    /// 模型能力目录（批 A）：发送前门控的目录模态数据源（用户钉死缺席时
    /// 目录说了算；两者都未知 = 放行，宁缺勿假）。
    catalog: Option<Arc<super::catalog::CatalogStore>>,
    /// 超预算工具输出的落盘句柄（S2 刀②）：None = 不落盘，行为退回改前的纯截断
    /// （headless 与测试装配就走这一档，所以它是 Option 而不是必需字段）。
    spill: Option<Arc<crate::spill::SpillStore>>,
    /// 超窗拒收读数（S2 刀⑧）：None = 不记（headless 与测试装配走这一档，与上面那枚
    /// 同形）。**写它的是这里**（只有发送侧知道自己这一次被拒了），读它的是
    /// `compact::deep::Compactor`——两个使用方，所以表由组装点构造、注给双方
    /// （[`crate::compact::Overflow`] 头注：为什么必须有这张表、代价是什么）。
    refusals: Option<Arc<crate::compact::Overflow>>,
}

impl AppLlm {
    pub fn new(settings: Arc<SettingsStore>) -> Self {
        Self {
            settings,
            overrides: None,
            demo: DemoLlm::new(),
            real: CompatLlm::new(),
            stop: None,
            notify_retry: None,
            diagnoser: Arc::new(super::diagnose::Diagnoser::default()),
            assets: None,
            catalog: None,
            spill: None,
            refusals: None,
        }
    }

    /// 组装点注入会话覆盖存储（init_state）。
    pub fn with_overrides(
        settings: Arc<SettingsStore>,
        overrides: Arc<ModelOverrideStore>,
    ) -> Self {
        Self {
            settings,
            overrides: Some(overrides),
            demo: DemoLlm::new(),
            real: CompatLlm::new(),
            stop: None,
            notify_retry: None,
            diagnoser: Arc::new(super::diagnose::Diagnoser::default()),
            assets: None,
            catalog: None,
            spill: None,
            refusals: None,
        }
    }

    /// 组装点注入资产存储与能力目录（批 D：wire 编码字节源 + 门控模态）。
    pub fn with_multimodal(
        mut self,
        assets: Arc<crate::assets::AssetStore>,
        catalog: Arc<super::catalog::CatalogStore>,
    ) -> Self {
        self.assets = Some(assets);
        self.catalog = Some(catalog);
        self
    }

    /// 组装点注入落盘句柄（S2 刀②：超预算工具输出写进 `<数据根>/spill/<会话>/`）。
    /// 与 `with_multimodal` 分开两枚而不并成一枚：两者服务的判定不同（附件字节源
    /// 缺席是 Fatal——用户附件不能静默丢，落盘句柄缺席是**降级**——退回纯截断），
    /// 把"必须在场"和"可以不在场"塞进同一个注入器，迟早有一侧的语义被读错。
    pub fn with_spill(mut self, spill: Arc<crate::spill::SpillStore>) -> Self {
        self.spill = Some(spill);
        self
    }

    /// 组装点注入拒收读数表（S2 刀⑧）：与压缩侧共用同一张 ⇒ 一次超窗被拒，下一趟
    /// step 边界的主层判定就读得到它（不这样这一层会被"上一次成功上报"那颗旧数钉死）。
    pub fn with_refusals(mut self, refusals: Arc<crate::compact::Overflow>) -> Self {
        self.refusals = Some(refusals);
        self
    }

    /// 组装点注入外部诊断账本（M6.1：AppState 持同一线程安全句柄供
    /// 命令层读取——不经 dyn LlmService downcast）。
    pub fn with_diagnoser(mut self, diagnoser: Arc<super::diagnose::Diagnoser>) -> Self {
        self.diagnoser = diagnoser;
        self
    }

    /// 组装点注入重试面（停止标记 + 通知出口）。链在 with_overrides 后。
    pub fn with_retry_surface(
        mut self,
        stop: Arc<crate::stop::StopFlags>,
        notify: RetryNotify,
    ) -> Self {
        self.stop = Some(stop);
        self.notify_retry = Some(notify);
        self
    }

    /// M6.1 断裂诊断：开关开且带会话身份才记（一次性请求纪律 6 不写
    /// 缓存，记了也无意义）；失败静默不拦请求。
    fn diagnose(&self, req: &LlmRequest) {
        if !self.settings.developer().cache_diagnostics {
            return;
        }
        if let Some(r) = self.diagnoser.record(req) {
            if !r.healthy() {
                tracing::info!(
                    session = ?req.session_id, step = r.step,
                    breaks = ?r.breaks,
                    "前缀缓存断裂（M6.1 诊断）"
                );
            }
        }
    }

    /// 本请求的重试钩子：会话身份 + 停止标记 + 通知出口齐备才生效。
    fn hooks(&self, session_id: Option<&str>) -> Option<RetryHooks> {
        let sid = session_id?;
        let stop = Arc::clone(self.stop.as_ref()?);
        let notify = Arc::clone(self.notify_retry.as_ref()?);
        let sid_stop = sid.to_string();
        let sid_notify = sid.to_string();
        Some(RetryHooks {
            should_stop: Box::new(move || stop.is_stopped(&sid_stop)),
            on_retry: Box::new(move |attempt, delay| notify(&sid_notify, attempt, delay)),
        })
    }

    /// 拒收现场记账（S2 刀⑧）。三条发送收口各调一次（与 [`Self::diagnose`] 同一条
    /// 惯例），算式因此只有一份。
    ///
    /// 只认**超窗**：判别复用 `llm::retry` 那一份 `is_overflow_message`（不许有第二种
    /// "这算超窗"的说法——限流、鉴权、模态拒发都各有自己的收口，把它们也记进来就等于
    /// 让压缩层去追一堆跟窗口无关的失败）。占用尺用发送侧同一把
    /// `estimate_tokens(&req.messages)`（与 `wire::payload` 那处预检钳制、与机械层的
    /// `from_tokens` 同尺 ⇒ 不是第四把尺）。
    pub(crate) fn refusal_note(
        &self,
        sid: Option<&str>,
        est: u64,
        out: &Result<LlmResponse, LlmError>,
    ) {
        let (Some(table), Some(sid), Some(err)) =
            (self.refusals.as_deref(), sid, out.as_ref().err())
        else {
            return;
        };
        if super::is_overflow_message(&err.to_string()) {
            tracing::info!(
                session = sid,
                tokens = est,
                "这一趟被按超窗拒收：把占用读数交给压缩面（它不会有用量上报）"
            );
            table.note(sid, est);
        }
    }

    /// 批 F 路由面：`params["route"] = {provider, model, thinking?}`（§7.2
    /// opaque params sanctioned 通道，内核零改动；build_payload 只读白名单
    /// params，route 不进请求体）。子代理模型面（SubagentLlm）写，本函数
    /// 只读不解释来源。
    fn route_of(req: &LlmRequest) -> Option<SubagentPin> {
        let r = req.params.get("route")?;
        Some(SubagentPin {
            provider_id: r.get("provider").and_then(Value::as_str)?.to_string(),
            model: r.get("model").and_then(Value::as_str)?.to_string(),
            thinking_level: r
                .get("thinking")
                .and_then(Value::as_str)
                .map(str::to_string),
        })
    }

    /// 本请求的生效配置（只有解析结果那一半）。
    ///
    /// T5 之后生产路径不再单独调它——`prepare` 要的是"配置 + 该档钉死值"，
    /// 一次快照就够。留着是因为 `llm/tests/dispatch.rs` 那 4 条用例钉的正是
    /// **解析次序**（会话覆盖 / 子代理路由 / 决策环改写 / 无覆盖），它们应当
    /// 不被钉死值那三个字段干扰；把测试面与本函数的存在理由写死在这里，
    /// 下次谁想把钉死值塞回 `LlmSettings` 就知道动的是谁的判据。
    #[cfg(test)]
    pub(crate) fn effective_cfg(&self, req: &LlmRequest) -> LlmSettings {
        self.effective_cfg_facts(req).0
    }

    /// 生效配置 **+ 同一次快照里顺手读到的模型行钉死值**（台账 T5）。
    ///
    /// 为什么要这一层：`prepare` 以前为了拿 `max_output` / `context_window` /
    /// `supports_image` 三个值，在 `effective_cfg` 之后**又取了一次锁、又把整个
    /// 档位区克隆了一遍**（`llm_config()` 返回的是含各档明文 Key 的全量副本）。
    /// 这三个值本来就长在"最后一次生效的那个档位"上——而那个档位正是
    /// `effective_cfg` 闭包里已经选定、正要丢掉的那个借用。同一次快照里读出来，
    /// 第二次取锁就没有存在的理由。
    ///
    /// 返回 `(LlmSettings, RowFacts)` 而不是直接返回 `WireCaps`：`caps` 里还有一格
    /// `tool_result_budget_bytes` 来自 `settings::context` 的固定常量，不属于档位快照，
    /// 而 `with_llm_config` 持着非重入的 `inner` 锁，闭包内不许再读别的区（自死锁，
    /// 见下方既有警告注释）。
    fn effective_cfg_facts(&self, req: &LlmRequest) -> (LlmSettings, RowFacts) {
        // 批 F-24b / M-18c：一次取锁、零 `LlmConfig` 副本。改前实测本函数最多 **5 次取锁**
        // （167 的 `llm()` + 170/171 与 179/180 两对 `llm_config()`）——其中每一对里的第二
        // 次克隆纯属浪费：`profile()` 返回的是**借用**，第一份快照本就活到 `if let` 块尾。
        // ⚠ 覆盖与路由先在闭包外取好：`with_llm_config` 持着非重入的 `inner` 锁，闭包内
        //   再调 `self.settings.*` = 自死锁。
        let covered = match (&self.overrides, &req.session_id) {
            (Some(store), Some(sid)) => store.get(sid),
            _ => None,
        };
        let route = Self::route_of(req);
        self.settings.with_llm_config(|llm| {
            let mut cfg = llm.flat();
            // `chosen` = 本请求最终生效的那个档位（与 `flat_profile` 的选取同步记下来）；
            // 没走任何档位改写时它留空，收尾回落到活动档——与 `prepare` 改前三段
            // `or_else` 的次序（route → 覆盖 → 活动档）逐字对应。
            let mut chosen: Option<&ProviderProfile> = None;
            if let Some((provider_id, model)) = covered {
                if let Some(p) = llm.profile(&provider_id) {
                    cfg = llm.flat_profile(p);
                    chosen = Some(p);
                }
                cfg.model = model;
            }
            // 子代理路由（批 F）：整档换 + 模型名替换。在决策环 req.model 之前
            // ——route 是组装点意志，决策环改写仍最高优先级。
            if let Some(route) = route {
                if let Some(p) = llm.profile(&route.provider_id) {
                    cfg = llm.flat_profile(p);
                    chosen = Some(p);
                }
                cfg.model = route.model;
            }
            let chosen = chosen.or_else(|| llm.active_profile());
            // §7.2 决策环的 model 改写是最高优先级（会话覆盖只在缺省层生效）
            if let Some(m) = &req.model {
                cfg.model = m.clone();
            }
            // 钉死值按**生效模型名**在选定的档位里找；找不到行就整组留 None
            // （不发明能力：目录里没有的、档位里没钉的，一律不发）。
            let row = chosen.and_then(|p| p.models.iter().find(|m| m.id == cfg.model));
            let facts = RowFacts {
                max_output: row.and_then(|m| m.max_output),
                ctx_window: row
                    .and_then(|m| m.context_window)
                    .or_else(|| chosen.and_then(|p| p.window_tokens)),
                supports_image: row.and_then(|m| m.supports_image),
            };
            (cfg, facts)
        })
    }

    /// 发送前门控的图像输入三态（批 D）：用户钉死（`pin`，生效模型行的
    /// supports_image）最高 → 目录命中次之 → 都未知 = None（放行——目录
    /// 没有的能力不发明）。
    fn image_allowed(&self, cfg: &LlmSettings, pin: Option<bool>) -> Option<bool> {
        if let Some(ok) = pin {
            return Some(ok);
        }
        let catalog = self.catalog.as_ref()?;
        let hit = catalog.lookup_model(&cfg.provider, cfg.base_url.as_deref(), &cfg.model)?;
        if hit.input_modalities.is_empty() {
            return None;
        }
        Some(hit.input_modalities.iter().any(|m| m == "image"))
    }

    /// 批 B 请求计划：生效配置 + 模型能力上限（钉死 max_output / 生效窗口）
    /// 与推理档位落 params。会话旁车 thinking_level 经方言 clamp（就近降档
    /// 或剥除）；`params["aux"]=true`（辅助调用标记）则取方言最低档。内核
    /// 零改动——params 是 opaque Value（§7.2 决策环同通道）。
    fn prepare(&self, mut req: LlmRequest) -> (LlmRequest, LlmSettings, WireCaps, Option<bool>) {
        // T5：生效配置与该档里钉死的三个能力值**同一次快照读出**，本函数不再为
        // 读几个字段而第二次取锁、第二次克隆含明文 Key 的档位区。
        let (cfg, facts) = self.effective_cfg_facts(&req);
        // Q8 工具结果预算：组装点带下来（内核读不到 settings，与 instruction 链预算同款
        // 注入形状）。写进初始化器而不是 `let mut caps = default()` 之后再赋一格：
        // clippy 的 field_reassign_with_default 在 `-D warnings` 下是硬错（本批首跑红过）。
        // 这里只取"是否限制"，KiB→字节的换算不在本文件做第二次。
        let mut caps = WireCaps {
            tool_result_budget_bytes: Some(crate::settings::context::TOOL_RESULT_BUDGET_BYTES),
            max_output: facts.max_output,
            ctx_window: facts.ctx_window,
            // 刀②：句柄是 Arc，clone 一次进 caps（WireCaps 从 Copy 改成 Clone 的
            // 全部代价）。这里只带句柄，"要不要真落盘"由发送侧按会话身份判定。
            spill: self.spill.clone(),
        };
        let image_pin = facts.supports_image;
        let route = Self::route_of(&req);
        // 推理档位解析：辅助降档优先（aux 标记），否则子代理钉档档位
        // （批 F，路由后档位的方言 clamp），否则会话旁车档
        let dialect = dialect::dialect_of(cfg.base_url.as_deref());
        let aux = req
            .params
            .get("aux")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        // 辅助调用不发送 max_tokens（§5.4 宁缺勿假；ZCode 发 min(5000,max)
        // 的做法记档不跟）
        if aux {
            caps.max_output = None;
        }
        let level = if aux {
            dialect.aux_level().map(str::to_string)
        } else {
            let requested = route
                .as_ref()
                .and_then(|r| r.thinking_level.clone())
                .or_else(|| {
                    req.session_id
                        .as_deref()
                        .and_then(|sid| self.overrides.as_ref().and_then(|o| o.get_thinking(sid)))
                });
            requested.and_then(|lvl| dialect.clamp(&lvl).map(str::to_string))
        };
        if let Some(lvl) = level {
            req.params["thinking_level"] = Value::String(lvl);
        }
        let image_allowed = self.image_allowed(&cfg, image_pin);
        // 批 E 工具结果图片投影（§8.1，D4）：在 image_allowed 解析后、
        // 估算前改写消息列——maxOutput 钳制估算吃投影后的形状
        req.messages = super::wire::project_tool_assets(req.messages, image_allowed);
        (req, cfg, caps, image_allowed)
    }
}

#[async_trait]
impl LlmService for AppLlm {
    async fn complete(&self, req: LlmRequest) -> Result<LlmResponse, LlmError> {
        self.diagnose(&req);
        let (req, cfg, caps, image_allowed) = self.prepare(req);
        if cfg.model.trim().is_empty() {
            tracing::debug!("LLM model 未配置，走演示替身（设置 → 模型配置后走真实模型）");
            return self.demo.complete(req).await;
        }
        // 发送前模态门控（批 D §5.3 第二道）：拒发 = Fatal（重试无意义）
        super::wire::check_modalities(&req, image_allowed).map_err(LlmError::Backend)?;
        let assets = self.assets.as_deref();
        let sid = req.session_id.clone();
        let est = estimate_tokens(&req.messages);
        let out = self.real.complete_with(&cfg, caps, req, assets).await;
        self.refusal_note(sid.as_deref(), est, &out);
        out
    }

    /// ⑳ 流式分发：同 complete 的设置驱动——未配置走演示替身（缺省实现
    /// = 整段一次回调，流式形状一致），真实模型走兼容客户端流式（含重试）。
    async fn complete_streaming(
        &self,
        req: LlmRequest,
        on_delta: &(dyn for<'a> Fn(&'a str) -> bool + Send + Sync),
    ) -> Result<LlmResponse, LlmError> {
        self.diagnose(&req);
        let (req, cfg, caps, image_allowed) = self.prepare(req);
        if cfg.model.trim().is_empty() {
            tracing::debug!("LLM model 未配置，走演示替身（设置 → 模型配置后走真实模型）");
            return self.demo.complete_streaming(req, on_delta).await;
        }
        super::wire::check_modalities(&req, image_allowed).map_err(LlmError::Backend)?;
        let hooks = self.hooks(req.session_id.as_deref());
        let assets = self.assets.as_deref();
        let sid = req.session_id.clone();
        let est = estimate_tokens(&req.messages);
        let out = self
            .real
            .complete_streaming_with(
                &cfg,
                caps,
                req,
                assets,
                on_delta,
                &|_| false,
                hooks.as_ref(),
            )
            .await;
        self.refusal_note(sid.as_deref(), est, &out);
        out
    }

    /// v2.57 推理通道分发：演示替身走缺省回落（无推理），真实模型把
    /// reasoning_content delta 逐段交给 `on_reasoning`。
    async fn complete_streaming_reasoning(
        &self,
        req: LlmRequest,
        on_delta: &(dyn for<'a> Fn(&'a str) -> bool + Send + Sync),
        on_reasoning: &(dyn for<'a> Fn(&'a str) -> bool + Send + Sync),
    ) -> Result<LlmResponse, LlmError> {
        self.diagnose(&req);
        let (req, cfg, caps, image_allowed) = self.prepare(req);
        if cfg.model.trim().is_empty() {
            tracing::debug!("LLM model 未配置，走演示替身（设置 → 模型配置后走真实模型）");
            return self.demo.complete_streaming(req, on_delta).await;
        }
        super::wire::check_modalities(&req, image_allowed).map_err(LlmError::Backend)?;
        let hooks = self.hooks(req.session_id.as_deref());
        let assets = self.assets.as_deref();
        let sid = req.session_id.clone();
        let est = estimate_tokens(&req.messages);
        let out = self
            .real
            .complete_streaming_with(
                &cfg,
                caps,
                req,
                assets,
                on_delta,
                on_reasoning,
                hooks.as_ref(),
            )
            .await;
        self.refusal_note(sid.as_deref(), est, &out);
        out
    }
}
