//! LLM 区的**策略与校验**簇（审计 F5 拆自原 `settings/llm.rs`）：provider 白名单、
//! 保留请求头表、请求头条数上限、代理档位，以及 `NetworkProxy` 的校验与归一。
//!
//! 为什么不并进持久态簇：这一簇是「什么可以存」的规则，持久态簇是「存成什么形状」。
//! 原文件的 `RESERVED_HEADERS` 注释就写着「批 C §8.1」、`PROXY_MODES` 写着「§8.5」，
//! 两条都是校验侧规格；把它和档位表放一起，改校验会牵动落盘结构（三问的第③问）。

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// LLM.md v3.0：统一适配器按 provider 路由（自建 OpenAI 兼容客户端）。
pub const PROVIDERS: [&str; 3] = ["openai", "anthropic", "ollama"];

/// 保留请求头（批 C §8.1）：鉴权 / 逐跳 / 受控头，保存时拒收（不区分
/// 大小写）——用户头可覆盖 User-Agent 等，不可碰鉴权与连接治理。
pub const RESERVED_HEADERS: [&str; 14] = [
    "authorization",
    "x-api-key",
    "api-key",
    "proxy-authorization",
    "host",
    "content-type",
    "content-length",
    "cookie",
    "set-cookie",
    "connection",
    "transfer-encoding",
    "te",
    "trailer",
    "upgrade",
];

/// 自定义请求头条数上限（§8.1）。
pub const MAX_CUSTOM_HEADERS: usize = 32;

/// 网络代理档位（§8.5）。
pub const PROXY_MODES: [&str; 3] = ["system", "direct", "custom"];

/// 自定义请求头保存校验：条数 ≤32；键 = 合法 HTTP token（可见 ASCII、
/// 无冒号、无空白）、不在保留表；值无控制字符、长度 ≤1024。
pub fn validate_headers(map: &BTreeMap<String, String>) -> Result<(), String> {
    if map.len() > MAX_CUSTOM_HEADERS {
        return Err(format!("自定义请求头最多 {} 条", MAX_CUSTOM_HEADERS));
    }
    for (k, v) in map {
        let key = k.trim();
        if key.is_empty() {
            return Err("请求头名不能为空".into());
        }
        if key.len() > 128 || !key.bytes().all(|b| b.is_ascii_graphic() && b != b':') {
            return Err(format!("请求头名不合法: {key}"));
        }
        if RESERVED_HEADERS.contains(&key.to_ascii_lowercase().as_str()) {
            return Err(format!(
                "请求头 {key} 是保留头（鉴权/逐跳/受控头不可自定义）"
            ));
        }
        if v.chars().any(char::is_control) {
            return Err(format!("请求头 {key} 的值含控制字符"));
        }
        if v.len() > 1024 {
            return Err(format!("请求头 {key} 的值过长（上限 1024 字节）"));
        }
    }
    Ok(())
}

/// 网络代理（批 C §8.5，裁定 D6）：system = reqwest 缺省（吃环境变量，
/// 现状）；direct = 强制直连；custom = 指定 url。`LlmConfig.network_proxy`
/// 缺省 None = system（零迁移零噪声）。
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct NetworkProxy {
    pub mode: String,
    pub url: Option<String>,
}

impl NetworkProxy {
    pub fn validate(&self) -> Result<(), String> {
        if !PROXY_MODES.contains(&self.mode.as_str()) {
            return Err(format!(
                "不支持的代理模式: {}（可选 system / direct / custom）",
                self.mode
            ));
        }
        if self.mode == "custom" {
            let url = self.url.as_deref().map(str::trim).unwrap_or("");
            if !(url.starts_with("http://") || url.starts_with("https://")) {
                return Err("自定义代理地址需以 http:// 或 https:// 开头".into());
            }
        }
        Ok(())
    }

    /// 归一：system / direct 不携带 url（settings 零噪声）。
    pub fn normalized(self) -> Self {
        let url = (self.mode == "custom")
            .then(|| self.url.clone().unwrap_or_default().trim().to_string())
            .filter(|u| !u.is_empty());
        Self {
            mode: self.mode,
            url,
        }
    }
}
