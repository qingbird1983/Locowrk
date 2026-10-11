//! LLM 区的 **IPC 形状**簇（审计 F5 拆自原 `settings/llm.rs`）：出参
//! `LlmSettingsView` / `ProviderView`，入参 `ProviderInput` / `LlmUpdate`。
//!
//! 前端只吃这四个形状（`settings_get` / `settings_set` / `llm_provider_save`）。
//! 字段增删是**跨端契约变更**：`#[serde(rename_all = "camelCase")]` 出去的名字与
//! `desktop/src/lib/types.ts` 手工对齐，编译器不证（JSON 边界），改这簇要走设置页走查。

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::persist::{ProviderModel, SubagentPin};
use super::policy::NetworkProxy;
use super::synthesize::ProviderModelView;

/// 档位 IPC 投影（无秘钥字段）。
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ProviderView {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub base_url: Option<String>,
    pub api_key_set: bool,
    pub model: String,
    pub window_tokens: Option<u64>,
    pub enabled: bool,
    /// 自定义请求头（非秘钥，编辑器回显用）。
    pub headers: BTreeMap<String, String>,
    /// 该档已配置模型清单（批 A 起为目录合成视图：view.rs 以无目录形态
    /// 铺底，命令层 enrich 挂上 models.dev 数据）。
    pub models: Vec<ProviderModelView>,
}

/// llm 区 IPC 投影：档位表 + 活动档平面速记（模型 chip / 上下文区 /
/// 嵌入回落提示直读平面字段，不各自翻档位表）。
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LlmSettingsView {
    pub active: String,
    pub providers: Vec<ProviderView>,
    pub provider: String,
    pub model: String,
    pub base_url: Option<String>,
    pub api_key_set: bool,
    pub window_tokens: Option<u64>,
    /// 网络代理档（批 C §8.5）。None = system。
    pub network_proxy: Option<NetworkProxy>,
    /// 子代理主模型钉档（批 F）。None = 跟随会话（缺省）。
    pub subagent: Option<SubagentPin>,
}

/// 档位增改入参（`llm_provider_save`）。`id` None = 新建（后端生成）；
/// `api_key` None = 保持已存值（更新时），空串 = 清除；`models` None =
/// 保持已存清单，Some = 整表替换（原 `model` 不在清单时改指首项）。
#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ProviderInput {
    pub id: Option<String>,
    pub label: String,
    pub kind: String,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub model: String,
    pub window_tokens: Option<u64>,
    pub enabled: Option<bool>,
    pub models: Option<Vec<ProviderModel>>,
    /// 自定义请求头：None = 保持已存值，Some = 整表替换（保存校验）。
    pub headers: Option<BTreeMap<String, String>>,
}

/// 部分更新（`settings_set` 入参）。字段 None = 保持不变；base_url /
/// api_key 传空串 = 清除；window_tokens 双层 Option：null = 清除；
/// `active` = 切活动档。平面字段落到目标档（见 `apply_update`）。
/// `subagent`（批 F）：provider_id/model 空串 = 清除钉档（v2.65「空串 =
/// 清除」约定——双层 Option 的 JSON null 与字段缺失不可区分）。
#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct LlmUpdate {
    pub active: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub window_tokens: Option<Option<u64>>,
    pub subagent: Option<SubagentPin>,
}
