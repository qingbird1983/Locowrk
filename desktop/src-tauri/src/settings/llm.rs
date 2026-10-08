//! LLM 主模型区类型（阶段八② / S2 窗口 / v2.58 档位制）。持久态 =
//! `LlmConfig { active, providers }`（DESIGN §13.1 模型档位制——多
//! provider 配置管理）；生效配置 = 活动档的平面投影 `LlmSettings`
//! （字段语义与全部既有消费方不变）。旧平面字段由 `settings.rs::open`
//! 迁移为单档 `default`（一次幂等）。校验与应用逻辑在本模块
//! `apply_update`，设置原值 / IPC 投影 / 部分更新三形齐备。

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::llm::catalog::CatalogModel;
use crate::llm::keyring::KEYRING_MARK;

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

/// 档位内一个已配置模型（㉚ 模型配置页）。`context_window` None =
/// 跟随静态档案 / 保守保护；`alias` 仅展示。不含密钥，可进 IPC。
/// 批 A 起：目录能力（窗口/输出/模态等）**不落这里**——settings 只存
/// 用户钉死值，目录合成发生在 IPC 投影（`synthesize_view`）。
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ProviderModel {
    pub id: String,
    pub alias: Option<String>,
    pub context_window: Option<u64>,
    /// 用户钉死的最大输出（token）。None = 跟随目录 / 不发（批 B 起
    /// 钉死值才随请求发送 + 预检钳制，供应商稿 §6）。
    pub max_output: Option<u64>,
    /// 钉死标记（"user"）。只存用户写入的值，目录永不落 settings，
    /// 本字段恒 Some("user") 或 None（显式列出让语义自证）。
    pub max_output_source: Option<String>,
    /// 用户钉死的图像输入（tri-state：None = 随目录 / 纯文本缺省）。
    pub supports_image: Option<bool>,
}

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
    /// 该档已配置模型清单（配置页右栏；`model` 应指其中一项）。只存用户
    /// 钉死面——目录能力数据永不落这里（批 A，供应商稿 §九）。
    pub models: Vec<ProviderModel>,
    /// 自定义请求头（批 C §8.1）：client 默认头之后最后写入者合并；
    /// 保留键保存时拒收（`validate_headers`）。
    pub headers: BTreeMap<String, String>,
    /// Key 存储标记（批 C §8.4）：Some("keyring") = 系统凭据库；None =
    /// 文件存储（存量明文，旧文件零迁移）。
    pub key_storage: Option<String>,
}

impl ProviderProfile {
    /// Key 是否已配置（明文或凭据库两态都算——IPC 的 api_key_set 用）。
    pub fn has_key(&self) -> bool {
        self.api_key.is_some() || self.key_storage.as_deref() == Some(KEYRING_MARK)
    }
}

/// `ProviderProfile::enabled` 的字段级 serde 缺省（旧文件无此字段 = 启用）。
fn default_enabled() -> bool {
    true
}

/// llm 区持久态：活动指针 + 档位表 + 代理档（§8.5，全区一级）。
/// `active` 指向不存在 / 表空 = 未配置（平面投影回落缺省，DemoLlm 语义）。
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct LlmConfig {
    pub active: String,
    pub providers: Vec<ProviderProfile>,
    /// 网络代理（§8.5）。None = system（reqwest 缺省吃环境变量）。
    pub network_proxy: Option<NetworkProxy>,
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
    /// Key 经凭据库双读回落解析（明文优先）。
    pub fn flat(&self) -> LlmSettings {
        match self.active_profile() {
            Some(p) => self.flat_profile(p),
            None => LlmSettings {
                network_proxy: self.network_proxy.clone(),
                ..LlmSettings::default()
            },
        }
    }

    /// 指定档 → 平面投影（会话覆盖路由同吃；Key 凭据库解析 +
    /// headers + 全区代理档随行）。
    pub fn flat_profile(&self, p: &ProviderProfile) -> LlmSettings {
        LlmSettings {
            provider: p.kind.clone(),
            model: p.model.clone(),
            base_url: p.base_url.clone(),
            api_key: crate::llm::keyring::resolve(p),
            window_tokens: p.window_tokens,
            headers: p.headers.clone(),
            network_proxy: self.network_proxy.clone(),
        }
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
                    headers: BTreeMap::new(),
                    key_storage: None,
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
/// 消费方——适配器 / embedding 回落 / 探测——同形零改动）。批 C 起随行
/// 自定义请求头与网络代理档。
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
    /// 自定义请求头（批 C §8.1，最后写入者合并）。
    pub headers: BTreeMap<String, String>,
    /// 网络代理（批 C §8.5）。None = system。
    pub network_proxy: Option<NetworkProxy>,
}

impl Default for LlmSettings {
    fn default() -> Self {
        Self {
            provider: "openai".into(),
            model: String::new(),
            base_url: None,
            api_key: None,
            window_tokens: None,
            headers: BTreeMap::new(),
            network_proxy: None,
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
