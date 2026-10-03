//! LLM 主模型区类型（阶段八② / S2 窗口 / v2.58 档位制）。持久态 =
//! `LlmConfig { active, providers }`（DESIGN §13.1 模型档位制——多
//! provider 配置管理）；生效配置 = 活动档的平面投影 `LlmSettings`
//! （字段语义与全部既有消费方不变）。旧平面字段由 `settings.rs::open`
//! 迁移为单档 `default`（一次幂等）。校验与应用逻辑在本模块
//! `apply_update`，设置原值 / IPC 投影 / 部分更新三形齐备。

use serde::{Deserialize, Serialize};

/// LLM.md 方案一：统一适配器按 provider 路由（genai 封装）。
pub const PROVIDERS: [&str; 3] = ["openai", "anthropic", "ollama"];

/// 档位内一个已配置模型（㉚ 模型配置页）。`context_window` None =
/// 跟随静态档案 / 保守保护；`alias` 仅展示。不含密钥，可进 IPC。
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ProviderModel {
    pub id: String,
    pub alias: Option<String>,
    pub context_window: Option<u64>,
}

/// 单个 provider 档位。`api_key` 仅落盘不出 IPC；`kind` = 适配器路由。
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ProviderProfile {
    pub id: String,
    pub label: String,
    pub kind: String,
    /// None = 用 provider 默认端点。
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    /// 空串 = 未配置。
    pub model: String,
    /// None = 未设置（探测 / 静态档案 / 手填），回落保守保护。
    pub window_tokens: Option<u64>,
    /// ㉚ 停用档：选择器隐藏、`active` 回落跳过（不删配置）。缺省 true：
    /// v2.58 落盘的旧档位无此字段，布尔字段缺省落 false 会把存量档全变
    /// 停用（30c 走查实抓），故字段级缺省点亮。
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// 该档已配置模型清单（配置页右栏；`model` 应指其中一项）。
    pub models: Vec<ProviderModel>,
}

/// `ProviderProfile::enabled` 的字段级 serde 缺省（旧文件无此字段 = 启用）。
fn default_enabled() -> bool {
    true
}

/// llm 区持久态：活动指针 + 档位表。`active` 指向不存在 / 表空 =
/// 未配置（平面投影回落缺省，DemoLlm 语义）。
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct LlmConfig {
    pub active: String,
    pub providers: Vec<ProviderProfile>,
}

impl LlmConfig {
    /// 活动档：活动指针（须未停用）→ 首个未停用档 → 首档（全停用兜底，
    /// 避免零可用配置丢平面投影）。
    pub fn active_profile(&self) -> Option<&ProviderProfile> {
        self.providers
            .iter()
            .find(|p| p.id == self.active && p.enabled)
            .or_else(|| self.providers.iter().find(|p| p.enabled))
            .or_else(|| self.providers.first())
    }

    pub fn profile(&self, id: &str) -> Option<&ProviderProfile> {
        self.providers.iter().find(|p| p.id == id)
    }

    /// 生效配置（平面投影）：活动档 → `LlmSettings`；无档 = 缺省。
    pub fn flat(&self) -> LlmSettings {
        self.active_profile()
            .map(|p| LlmSettings {
                provider: p.kind.clone(),
                model: p.model.clone(),
                base_url: p.base_url.clone(),
                api_key: p.api_key.clone(),
                window_tokens: p.window_tokens,
            })
            .unwrap_or_default()
    }

    /// `settings_set` 的 llm 区部分更新（平面字段映射到活动档；
    /// `active` = 切档，id 不存在即报错不污染内存态）。
    pub fn apply_update(&mut self, u: &LlmUpdate) -> Result<(), String> {
        if let Some(a) = &u.active {
            if self.profile(a).is_none() {
                return Err(format!("模型档位不存在: {a}"));
            }
            self.active = a.clone();
        }
        // 平面字段的目标档：切档指令优先切到的新档，否则当前活动档
        let target = u
            .active
            .as_ref()
            .and_then(|a| self.profile(a).cloned())
            .or_else(|| self.active_profile().cloned());
        let mut p = match target {
            Some(p) => p,
            None => {
                // 空表：平面更新按需物化缺省档（旧单档语义——首次保存
                // 即建档，未保存过 = 未配置不变）
                let kind = u.provider.clone().unwrap_or_else(|| "openai".into());
                ProviderProfile {
                    id: "default".into(),
                    label: kind.clone(),
                    kind,
                    base_url: None,
                    api_key: None,
                    model: String::new(),
                    window_tokens: None,
                    enabled: true,
                    models: Vec::new(),
                }
            }
        };
        if let Some(k) = &u.provider {
            if !PROVIDERS.contains(&k.as_str()) {
                return Err(format!(
                    "不支持的 provider: {k}（可选 openai / anthropic / ollama）"
                ));
            }
            p.kind = k.clone();
        }
        if let Some(m) = &u.model {
            p.model = m.clone();
        }
        if let Some(b) = &u.base_url {
            p.base_url = (!b.is_empty()).then_some(b.clone());
        }
        if let Some(k) = &u.api_key {
            p.api_key = (!k.is_empty()).then_some(k.clone());
        }
        if let Some(w) = u.window_tokens {
            match w {
                Some(n) if n <= 99 => {
                    return Err(format!(
                        "上下文窗口过小: {n} token（单位是 token，如 32768）"
                    ));
                }
                Some(n) if n > 100_000_000 => {
                    return Err(format!("上下文窗口过大: {n} token（上限 1 亿）"));
                }
                other => p.window_tokens = other,
            }
        }
        if let Some(slot) = self.providers.iter_mut().find(|x| x.id == p.id) {
            *slot = p;
            return Ok(());
        }
        // 物化出的缺省档：入表并点亮活动指针
        self.active = p.id.clone();
        self.providers.push(p);
        Ok(())
    }
}

/// 生效配置（v2.58 起为活动档的平面投影；字段语义不变，全部既有
/// 消费方——适配器 / embedding 回落 / 探测——同形零改动）。
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct LlmSettings {
    pub provider: String,
    pub model: String,
    pub base_url: Option<String>,
    /// 仅落盘，永不出 IPC。
    pub api_key: Option<String>,
    /// 模型上下文窗口（token）。None = 未设置（阈值回落保守保护）。
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
    pub models: Vec<ProviderModel>,
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
}

/// 部分更新（`settings_set` 入参）。字段 None = 保持不变；base_url /
/// api_key 传空串 = 清除；window_tokens 双层 Option：null = 清除；
/// `active` = 切活动档。平面字段落到目标档（见 `apply_update`）。
#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct LlmUpdate {
    pub active: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub window_tokens: Option<Option<u64>>,
}

/// 新档 id（后端生成）：时间戳纳秒 + 进程内计数，不引第三方依赖。
pub fn new_profile_id() -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static SEQ: AtomicU32 = AtomicU32::new(1);
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("p{n:x}-{}", SEQ.fetch_add(1, Ordering::Relaxed))
}

/// 旧平面 llm 区 → 单档 `default`（v2.58 档位制，DESIGN §13.1）。幂等：
/// 新格式 llm 区无 `provider` 键，检测即安全。旧值（含 apiKey 原值）
/// 全量保留；label 用 kind 名（用户可在设置里改名）。
pub(super) fn migrate_flat_llm(raw: &mut serde_json::Value) {
    let Some(llm) = raw.get_mut("llm").and_then(|v| v.as_object_mut()) else {
        return;
    };
    if !llm.contains_key("provider") {
        return;
    }
    let mut old = serde_json::Map::new();
    for k in ["provider", "model", "baseUrl", "apiKey", "windowTokens"] {
        if let Some(v) = llm.remove(k) {
            old.insert(k.to_string(), v);
        }
    }
    let kind = old
        .get("provider")
        .and_then(|v| v.as_str())
        .unwrap_or("openai")
        .to_string();
    *llm = serde_json::json!({
        "active": "default",
        "providers": [{
            "id": "default",
            "label": kind,
            "kind": kind,
            "baseUrl": old.get("baseUrl").cloned().unwrap_or(serde_json::Value::Null),
            "apiKey": old.get("apiKey").cloned().unwrap_or(serde_json::Value::Null),
            "model": old.get("model").cloned().unwrap_or(serde_json::json!("")),
            "windowTokens": old.get("windowTokens").cloned().unwrap_or(serde_json::Value::Null),
        }]
    })
    .as_object()
    .cloned()
    .expect("profile object");
}
