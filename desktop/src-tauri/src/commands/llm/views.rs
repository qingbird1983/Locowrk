//! IPC 视图与目录合成的**共读段**（批 A）：`ProviderModelsView` 是选择器清单的行形状，
//! `enrich_with_catalog` = settings 钉死值 ∪ 目录 的统一合成（settings_get 与各档位命令都走这里，
//! 目录数据只进内存不落 settings），`apply_dialect_ladder` = 推理梯子落方言口径（批 B §5.1）。
//! 后两个被本目录多片共读 ⇒ 留在这里，`pub(super)` 只对本模块树可见，不抬成 crate 面。

use serde::Serialize;

use crate::llm::catalog::CatalogStore;
use crate::settings::LlmSettingsView;

/// 单档模型清单（§3.30 浮层分组行）。`error` = 该档探测失败（如实逐组
/// 报错，不拖垮其它组）。
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProviderModelsView {
    pub id: String,
    pub label: String,
    pub models: Vec<String>,
    pub error: Option<String>,
}

/// 推理梯子落方言口径（批 B）：梯子 = 预设方言的**可发送**档位集
/// （§5.1——目录档位在方言表里没有写法的一律不渲染）；无方言 = 清空
/// （选择器隐藏）。sources.reasoning 标 "preset"（方言来源）。
pub(super) fn apply_dialect_ladder(
    v: &mut crate::settings::ProviderModelView,
    base_url: Option<&str>,
) {
    let d = crate::llm::dialect_of(base_url);
    if d.ladder().is_empty() {
        v.reasoning_ladder.clear();
        v.sources.reasoning = None;
    } else {
        v.reasoning_ladder = d.ladder().iter().map(|s| s.to_string()).collect();
        v.sources.reasoning = Some("preset".into());
    }
}

/// settings 钉死值 ∪ 目录 = IPC 合成视图（批 A）。settings_get / 档位
/// 命令返回统一走这里——目录数据只进内存不落 settings。
pub fn enrich_with_catalog(catalog: &CatalogStore, llm_view: &mut LlmSettingsView) {
    for p in &mut llm_view.providers {
        p.models = p
            .models
            .iter()
            .map(|m| {
                let hit = catalog.lookup_model(&p.kind, p.base_url.as_deref(), &m.id);
                let mut v = crate::settings::synthesize_view_raw(m, hit.as_ref());
                apply_dialect_ladder(&mut v, p.base_url.as_deref());
                v
            })
            .collect();
    }
}
