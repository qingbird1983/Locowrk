//! SSRF 防线 = `plugin_api::netsafe` 的本插件参数位（审计 F7 · 批 F-21b 上收）。
//! 本插件的防线是 tool-fetch v2.14 那份照搬来的（用户 2026-09-29 指定纪律不复核），
//! 上收后两处字面量差异收进参数位：报文前缀 `notify` + 后门 `LOCOWRK_NOTIFY_ALLOW_PRIVATE`
//! （单测打本进程服务要靠它）。
use plugin_api::netsafe::Ssrf;

pub(crate) const SSRF: Ssrf = Ssrf {
    label: "notify",
    env_allow: "LOCOWRK_NOTIFY_ALLOW_PRIVATE",
};

#[cfg(test)]
mod tests;
