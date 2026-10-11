//! SSRF 防线 = `plugin_api::netsafe` 的本插件参数位（审计 F7 · 批 F-21b 上收）。
//! 实现只有一处（基座），这里只留两个既有对外形态：报文前缀 `fetch` 与显式后门
//! `LOCOWRK_FETCH_ALLOW_PRIVATE`（集成测试的 fetch 用例打本进程 TCP 服务要靠它）。
//! 语义纪律（fail-closed、全地址判定、重定向在 fetch 侧关闭、DNS rebinding 残余风险）
//! 写在基座模块头注，不在这里复述。
use plugin_api::netsafe::Ssrf;

pub(crate) const SSRF: Ssrf = Ssrf {
    label: "fetch",
    env_allow: "LOCOWRK_FETCH_ALLOW_PRIVATE",
};

#[cfg(test)]
mod tests;
