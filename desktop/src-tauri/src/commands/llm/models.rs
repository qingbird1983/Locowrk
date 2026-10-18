//! 全部已存档位的模型清单（§3.30 选择器数据源）：并行探测、5s 超时逐组收口、
//! 停用档不进清单。**选择器路径不做端点扫描**（§8.2 记档：扫出来的端点无处持久化 = 谎报），
//! 扫描只在有采纳面的表单 probe 发生。

use tauri::State;

use crate::llm::{keyring, models};
use crate::state::AppState;

use super::views::ProviderModelsView;

/// 全部已存档位的模型清单（并行探测，5s 超时逐组收口）。**选择器路径
/// 不做端点扫描**（§8.2 记档）——扫描胜出的端点无处持久化，清单成功了
/// 聊天仍走原端点必败 = 谎报；扫描只在表单 probe（有采纳面）发生。
#[tauri::command]
pub async fn llm_list_models(
    state: State<'_, AppState>,
) -> Result<Vec<ProviderModelsView>, String> {
    let cfg = state.settings.llm_config();
    // 停用档不进选择器清单（§3.30）：与 active 回落同一裁决口径。
    let probes = cfg.providers.iter().filter(|p| p.enabled).map(|p| {
        let id = p.id.clone();
        let label = p.label.clone();
        let key = keyring::resolve(p);
        let headers = p.headers.clone();
        let proxy = cfg.network_proxy.clone();
        async move {
            let res = models::list_models(
                &p.kind,
                p.base_url.as_deref(),
                key.as_deref(),
                &headers,
                proxy.as_ref(),
            )
            .await;
            (id, label, res)
        }
    });
    let done = futures::future::join_all(probes).await;
    Ok(done
        .into_iter()
        .map(|(id, label, res)| match res {
            Ok(m) => ProviderModelsView {
                id,
                label,
                models: m,
                error: None,
            },
            Err(e) => ProviderModelsView {
                id,
                label,
                models: vec![],
                error: Some(e),
            },
        })
        .collect())
}
