//! ⑬ 自动停用巡检（exposure 文档 §8.8 的唯一例外条款）：默认关闭；
//! 开启后把"N 天未使用"的安装态第三方插件逐条停用。
//!
//! 口径：距最近一次使用（从未使用 = 安装时点起算）≥ N 天，与 ⑩
//! IdleBadge 的闲置评估同源（usage.sqlite3 的 plugin_times + lockfile
//! installed_at）；内置插件不参与（随程序分发，⑩ 起就不评估闲置）。
//! 只停用不启用——关闭设置不会复活已停用插件（状态切换永远由用户显式
//! 触发，§8.8 三条理由）。

use plugin_service::PluginService;

use crate::timeutil::now_secs;
use crate::usage::UsageRecorder;

/// 巡检一遍并停用超期插件。`days` = 0 直接返回（关闭态，两个调用点都
/// 先判了这里再兜一道）。逐条失败只 warn，不中断其余插件。
pub(crate) async fn auto_disable_idle(plugins: &PluginService, usage: &UsageRecorder, days: u32) {
    if days == 0 {
        return;
    }
    let now = now_secs();
    let times = usage.plugin_times();
    for p in plugins.list_installed() {
        if matches!(p.source, plugin_service::InstallSource::Builtin) || !p.enabled {
            continue;
        }
        let anchor = times
            .get(&p.id)
            .map(|(_, last)| *last)
            .unwrap_or(p.installed_at);
        if now.saturating_sub(anchor) >= i64::from(days) * 86_400 {
            match plugins.set_enabled(&p.id, false).await {
                Ok(()) => {
                    tracing::info!(plugin = %p.id, days, "auto-disabled idle plugin")
                }
                Err(e) => tracing::warn!(plugin = %p.id, "auto-disable failed: {e}"),
            }
        }
    }
}
