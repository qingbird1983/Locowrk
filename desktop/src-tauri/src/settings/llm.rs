//! LLM 主模型区类型（阶段八②/S2 窗口迁入）：设置原值 / IPC 投影 / 部分更新。
//! 校验与应用逻辑在 settings.rs 的 `update` / `llm`。

use serde::{Deserialize, Serialize};

/// LLM.md 方案一：统一适配器按 provider 路由（genai 封装）。
pub const PROVIDERS: [&str; 3] = ["openai", "anthropic", "ollama"];

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct LlmSettings {
    pub provider: String,
    /// 空串 = 未配置。
    pub model: String,
    /// None = 用 provider 默认端点。
    pub base_url: Option<String>,
    /// 仅落盘，永不出 IPC。
    pub api_key: Option<String>,
    /// 模型上下文窗口（token，S2 迁入——窗口是模型的属性，归属模型配置）。
    /// None = 未设置（Ollama 可经 /api/show 探测，云端查静态档案或手填），
    /// 阈值回落 FALLBACK_WINDOW_TOKENS 保守保护。
    pub window_tokens: Option<u64>,
}

impl Default for LlmSettings {
    fn default() -> Self {
        Self {
            provider: "openai".into(),
            model: String::new(),
            base_url: None,
            api_key: None,
            window_tokens: None,
        }
    }
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LlmSettingsView {
    pub provider: String,
    pub model: String,
    pub base_url: Option<String>,
    pub api_key_set: bool,
    /// 模型上下文窗口（S2 迁入；None = 未设置，回落保守保护）。
    pub window_tokens: Option<u64>,
}

/// 部分更新（settings_set 入参）。字段 None = 保持不变；
/// base_url / api_key 传空串 = 清除（回落默认端点 / 删除已存 Key）；
/// window_tokens 双层 Option：null = 清除（回落保守保护），缺省 = 不变。
#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct LlmUpdate {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub window_tokens: Option<Option<u64>>,
}
