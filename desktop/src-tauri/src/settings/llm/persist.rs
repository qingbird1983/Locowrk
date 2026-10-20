//! LLM 区的**持久态与生效投影**簇（审计 F5 拆自原 `settings/llm.rs`）：档位表
//! `LlmConfig`、单档 `ProviderProfile`、模型行 `ProviderModel`、子代理钉档
//! `SubagentPin`、平面投影 `LlmSettings`，以及 `active_profile`/`flat`/`apply_update`
//! 三条读改写路径 + `session_window` 四层链 + 新档 id 生成。
//!
//! 落盘格式的唯一真源在这里：`#[serde(rename_all = "camelCase", default)]` 与字段级
//! 缺省（`default_enabled`）决定旧文件怎么被读出来，**动字段等于动磁盘格式**，
//! 所以这一簇不与只进内存的展示合成簇（`synthesize.rs`）混放。

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::ipc::LlmUpdate;
use super::policy::{NetworkProxy, PROVIDERS};
use crate::llm::keyring::KEYRING_MARK;

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
    /// 可用于子代理（批 F）：用户显式标记才进子代理模型池（钉模型选择
    /// 器与终端错误回退链的共同数据源）；None = 不参与，目录无此数据。
    pub available_for_subagents: Option<bool>,
}

/// 子代理主模型钉档（批 F）：None = 跟随会话（子请求走父会话的会话覆盖
/// → 设置缺省）。`thinking_level` Some = 子请求吃钉档档位（方言 clamp 同
/// 批 B 通道）；模型必须在「可用于子代理」标记池里选（UI 侧约束）。
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct SubagentPin {
    pub provider_id: String,
    pub model: String,
    pub thinking_level: Option<String>,
}

impl SubagentPin {
    /// 保存校验：钉档必须指到存在且启用的档位 + 非空模型名。
    pub fn validate(&self, cfg: &LlmConfig) -> Result<(), String> {
        let p = cfg
            .profile(&self.provider_id)
            .ok_or_else(|| format!("子代理模型钉档的档位不存在: {}", self.provider_id))?;
        if !p.enabled {
            return Err(format!(
                "子代理模型钉档指向已停用档位: {}（请先启用或改钉其它档位）",
                p.label
            ));
        }
        if self.model.trim().is_empty() {
            return Err("子代理模型钉档的模型名不能为空".into());
        }
        Ok(())
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
    /// 子代理主模型钉档（批 F）。None = 跟随会话（缺省）。
    pub subagent: Option<SubagentPin>,
}

/// 会话生效模型的**容量读数**（specs/09 刀① 新增）：`(上下文窗口, 输出上限)`。
///
/// 选取链沿用 v2.137 的四层自动识别（会话模型覆盖 → 模型行钉死 → 目录命中
/// （批 A models.dev 快照/在线刷新）→ 档位字段），不读 keyring（`flat()` 会碰
/// 凭据库，这里只走纯档位字段）；与子代理回退链的窗口口径同形。
///
/// 为什么两个读数要一次解析：它们长在同一行上（同一个生效档位里的同一个模型），
/// 分两次解析就得把「会话覆盖 → 活动档 → 模型名」那条选取链写两遍——而选取链
/// 一旦有两份，"窗口按覆盖档取、输出上限按活动档取"这种错配就是迟早的事。
/// `session_window` 由此退成薄封装（签名与语义不动，既有 `settings/tests/window.rs`
/// 的链用例一字未改仍绿 = 等价性证据）。
///
/// 返回元组而不是新结构体：本仓的组装点只用这两个数，包一层类型要连带把它挤进
/// `settings` 的导出面（未命名的重导出会被 `unused_imports` 判死），形状收益抵不过
/// 门禁成本。调用方一律解构成 `window` / `max_output` 命名，位置写法（`.0`/`.1`）
/// 不许出现在生产代码里。
///
/// ⚠ 两半的**未知语义不同**：窗口全未知 = `None`（调用方按「未设置」呈现，压缩侧
/// 见 `context_budget` 的裁定 A）；输出上限全未知 = `None` ⇒ 压缩侧按缺省预留
/// （`context_budget::output_reserve`）。输出上限的链只有两档（模型行钉死 ?? 目录命中，
/// 档位没有输出上限这颗字段）。
pub fn session_capacity(
    cfg: &LlmConfig,
    session_override: Option<&(String, String)>,
    catalog: Option<&crate::llm::catalog::CatalogStore>,
) -> (Option<u64>, Option<u64>) {
    let profile = match session_override {
        Some((pid, _)) => cfg.profile(pid).or_else(|| cfg.active_profile()),
        None => cfg.active_profile(),
    };
    let Some(profile) = profile else {
        return (None, None);
    };
    // 覆盖带的模型名优先（与原实现同形：覆盖换档也换名，档位解析失败仍回落活动档）。
    let model = session_override
        .map(|(_, m)| m.as_str())
        .unwrap_or_else(|| profile.model.as_str());
    if model.is_empty() {
        return (None, None);
    }
    let row = profile.models.iter().find(|m| m.id == model);
    let hit =
        catalog.and_then(|c| c.lookup_model(&profile.kind, profile.base_url.as_deref(), model));
    (
        // 窗口四层：模型行钉死 ?? 目录 ?? 档位（与改前三段 early return 同序同值）。
        row.and_then(|m| m.context_window)
            .or_else(|| hit.as_ref().and_then(|h| h.context_window))
            .or(profile.window_tokens),
        // 输出上限两档：模型行钉死 ?? 目录。
        // ⚠ 与发送侧的**有意差别**：`llm/app_llm.rs` 的 `RowFacts.max_output` 只取
        // 用户钉死值，目录值不下发（原话「不发明能力：目录里没有的、档位里没钉的，
        // 一律不发」——那是"往请求里写参数"的纪律）。预算这一面读的是"这台模型
        // 物理上能给多少上下文"，目录命中就是可用证据，所以允许落第二档。
        row.and_then(|m| m.max_output)
            .or_else(|| hit.as_ref().and_then(|h| h.max_output)),
    )
}

/// 会话生效上下文窗口（[`session_capacity`] 的窗口那一半；签名与语义与
/// v2.137 起一直是的那份逐字相同）。
///
/// S4 刀③b 起生产面只剩 [`session_capacity`]（唯一调用方 `agent_chat` /
/// 调度引擎都一次取两半），本函数因此只服务测试面的等价性判据——
/// `settings/tests/window.rs` 拿它比对"capacity 的第一半没有第二套算术"。
#[cfg(test)]
pub fn session_window(
    cfg: &LlmConfig,
    session_override: Option<&(String, String)>,
    catalog: Option<&crate::llm::catalog::CatalogStore>,
) -> Option<u64> {
    session_capacity(cfg, session_override, catalog).0
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
        // 子代理钉档（批 F）：空 provider_id = 清除；否则校验指向存在且
        // 启用的档位（模型名空同样拒收）。清除路径不校验（先清后删档的
        // 顺序自由）。
        if let Some(pin) = &u.subagent {
            if pin.provider_id.trim().is_empty() || pin.model.trim().is_empty() {
                self.subagent = None;
            } else {
                pin.validate(self)?;
                self.subagent = Some(pin.clone());
            }
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
