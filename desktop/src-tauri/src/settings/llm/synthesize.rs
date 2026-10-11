//! LLM 区的**目录合成视图**簇（审计 F5 拆自原 `settings/llm.rs`）：settings 钉死值 ∪
//! models.dev 目录 = 展示面（供应商稿 §4.5）。`synthesize_view` / `synthesize_view_raw`
//! 两个入口共享 `synthesize_core`，`PinFields` 是它们的钉死面入参。
//!
//! 这一簇的东西**永不落 settings**（目录部分只进内存）——与 `persist.rs` 的边界正在这
//! 里：一个管磁盘真相，一个管展示合成。原来同文件放一起时，改展示字段要滚过 300 行
//! 落盘结构才够得着，这就是拆它的理由。

use serde::Serialize;

use super::persist::ProviderModel;
use crate::llm::catalog::CatalogModel;

/// 每百万 token 单价（USD 原值，供应商稿 §4.6：货币如实标注不做换算；
/// 目录缺哪个分量就 None，不冒充 0）。
#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PricingView {
    pub input: Option<f64>,
    pub output: Option<f64>,
    pub cache_read: Option<f64>,
}

/// 字段来源标记（供应商稿 §4.5 SourcesView）：`"user"` = 用户钉死
/// （settings 存了值，刷新/快照永不覆盖）；`"catalog"` = 跟随目录
/// （编辑即钉死）；None = 未知（如实标注）。
#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SourcesView {
    pub window: Option<String>,
    pub max_output: Option<String>,
    pub modalities: Option<String>,
    pub reasoning: Option<String>,
}

/// 档位内一个模型的 IPC 合成视图（批 A，供应商稿 §4.5）：settings 钉死
/// 值 ∪ models.dev 目录 = 展示面；**目录部分只进内存不落 settings**。
/// 前端保存时只回传钉死字段（sources 标记为 catalog 的派生值会被剥除）。
#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ProviderModelView {
    pub id: String,
    pub alias: Option<String>,
    pub context_window: Option<u64>,
    /// 用户钉死值（与 `max_output` 同源；目录值只在展示字段）。
    pub max_output: Option<u64>,
    pub max_output_source: Option<String>,
    /// 用户钉死图像输入（tri-state）。
    pub supports_image: Option<bool>,
    /// 可用于子代理（批 F，用户钉死；None = 不参与）。
    pub available_for_subagents: Option<bool>,
    /// 展示用有效模态（钉死 ∪ 目录；未知 = ["text"] 纯文本缺省）。
    pub input_modalities: Vec<String>,
    /// 归一化推理梯子（目录；空 = 隐藏档位选择器——批 B 起接预设方言）。
    pub reasoning_ladder: Vec<String>,
    pub tool_call: Option<bool>,
    pub pricing: Option<PricingView>,
    pub sources: SourcesView,
}

/// settings 钉死值 ∪ 目录命中的合成（供应商稿 §4.5）。`hit` None =
/// 目录未命中（能力未知如实：模态回落纯文本、梯子空、无价格）。
pub fn synthesize_view(pm: &ProviderModel, hit: Option<&CatalogModel>) -> ProviderModelView {
    let pins = PinFields {
        id: &pm.id,
        alias: pm.alias.clone(),
        context_window: pm.context_window,
        max_output: pm.max_output,
        max_output_source: pm.max_output_source.clone(),
        supports_image: pm.supports_image,
        available_for_subagents: pm.available_for_subagents,
    };
    synthesize_core(pins, hit)
}

/// 同 `synthesize_view`，但钉死面取自已合成的无目录视图（命令层 enrich
/// 复用：settings/view.rs 先以 hit=None 铺底，这里挂上目录数据）。
pub fn synthesize_view_raw(
    pm: &ProviderModelView,
    hit: Option<&CatalogModel>,
) -> ProviderModelView {
    let pins = PinFields {
        id: &pm.id,
        alias: pm.alias.clone(),
        context_window: pm.context_window,
        max_output: pm.max_output,
        max_output_source: pm.max_output_source.clone(),
        supports_image: pm.supports_image,
        available_for_subagents: pm.available_for_subagents,
    };
    synthesize_core(pins, hit)
}

/// 合成的钉死面（两个入口共享）。
struct PinFields<'a> {
    id: &'a str,
    alias: Option<String>,
    context_window: Option<u64>,
    max_output: Option<u64>,
    max_output_source: Option<String>,
    supports_image: Option<bool>,
    available_for_subagents: Option<bool>,
}

fn synthesize_core(pm: PinFields<'_>, hit: Option<&CatalogModel>) -> ProviderModelView {
    let modalities_from_pin = |img: bool| {
        let mut v = vec!["text".to_string()];
        if img {
            v.push("image".to_string());
        }
        v
    };
    let input_modalities = match pm.supports_image {
        Some(img) => modalities_from_pin(img),
        None => match hit.filter(|h| !h.input_modalities.is_empty()) {
            Some(h) => h.input_modalities.clone(),
            None => vec!["text".into()],
        },
    };
    let catalog_window = hit.filter(|_| pm.context_window.is_none());
    let catalog_max = hit.filter(|_| pm.max_output.is_none());
    ProviderModelView {
        id: pm.id.to_string(),
        alias: pm.alias,
        context_window: pm.context_window.or(hit.and_then(|h| h.context_window)),
        max_output: pm.max_output.or(hit.and_then(|h| h.max_output)),
        max_output_source: pm.max_output_source,
        supports_image: pm.supports_image,
        available_for_subagents: pm.available_for_subagents,
        input_modalities,
        reasoning_ladder: hit.map(|h| h.reasoning_options.clone()).unwrap_or_default(),
        tool_call: hit.and_then(|h| h.tool_call),
        pricing: hit.and_then(|h| h.pricing.clone()).map(|p| PricingView {
            input: p.input,
            output: p.output,
            cache_read: p.cache_read,
        }),
        sources: SourcesView {
            window: pm.context_window.map(|_| "user".into()).or(catalog_window
                .filter(|h| h.context_window.is_some())
                .map(|_| "catalog".into())),
            max_output: pm.max_output.map(|_| "user".into()).or(catalog_max
                .filter(|h| h.max_output.is_some())
                .map(|_| "catalog".into())),
            modalities: pm.supports_image.map(|_| "user".into()).or(hit
                .filter(|h| !h.input_modalities.is_empty())
                .map(|_| "catalog".into())),
            reasoning: hit
                .filter(|h| !h.reasoning_options.is_empty())
                .map(|_| "catalog".into()),
        },
    }
}
