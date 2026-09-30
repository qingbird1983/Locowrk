//! 设置存储（DESIGN.md §13.1 / §13.5）。分区：LLM 配置、语言、快捷键、
//! ⑨ 上下文管理、⑪ 记忆注入、⑫ 工具暴露、S1 shell 治理（高危解锁）。
//! 持久化在数据根 `settings.json`；API Key 只落盘不出 IPC（投影只有
//! `api_key_set` 布尔），真适配器在后端侧读原值。

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use kernel_agent::{CompactConfig, MemoryInjectConfig};
use serde::{Deserialize, Serialize};
use tauri_plugin_global_shortcut::Shortcut;

mod context;
mod memory;
mod shell;
mod tools;

pub use context::{ContextSettings, ContextSettingsView, ContextUpdate};
pub use memory::{MemorySettings, MemorySettingsView, MemoryUpdate};
pub use shell::{ShellSettings, ShellSettingsView, ShellUpdate};
pub use tools::{ToolsSettings, ToolsSettingsView, ToolsUpdate};

/// locale 取值：`system`（跟随系统，缺省）/ `zh-CN` / `en`（§13.5）。
pub const LOCALE_SYSTEM: &str = "system";
const LOCALES: [&str; 3] = [LOCALE_SYSTEM, "zh-CN", "en"];
/// LLM.md 方案一：统一适配器按 provider 路由（genai 封装，后续阶段接入）。
const PROVIDERS: [&str; 3] = ["openai", "anthropic", "ollama"];
/// 全局快捷键缺省值（tray.rs 启动注册用；settings.json `shortcut`）。
pub const SHORTCUT_DEFAULT: &str = "Ctrl+Shift+L";
/// 模型窗口未设置时的保守回落：沿用 v2.14 固定 32k 保护（Context 文档
/// §3.2 的窗口 × 百分比语义要等窗口已知才成立）。
pub const FALLBACK_WINDOW_TOKENS: u64 = 32_000;

const FILE_NAME: &str = "settings.json";

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
}

impl Default for LlmSettings {
    fn default() -> Self {
        Self {
            provider: "openai".into(),
            model: String::new(),
            base_url: None,
            api_key: None,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    pub locale: String,
    /// 全局快捷键（切换主窗口）。字段级缺省：旧 settings.json 无此字段
    /// 时取 SHORTCUT_DEFAULT，而不是空串。
    #[serde(default = "default_shortcut")]
    pub shortcut: String,
    pub llm: LlmSettings,
    /// 上下文管理区（⑨）。字段级缺省：旧 settings.json 无此字段时取默认
    /// （窗口未设置 / 阈值 30% / 分层保留）。
    pub context: ContextSettings,
    /// 记忆注入区（⑪，类型见 `memory` 子模块）。
    pub memory: MemorySettings,
    /// 工具暴露区（⑫，类型见 `tools` 子模块）。
    pub tools: ToolsSettings,
    /// shell 治理区（S1，类型见 `shell` 子模块）。
    pub shell: ShellSettings,
}

fn default_shortcut() -> String {
    SHORTCUT_DEFAULT.into()
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            locale: LOCALE_SYSTEM.into(),
            shortcut: SHORTCUT_DEFAULT.into(),
            llm: LlmSettings::default(),
            context: ContextSettings::default(),
            memory: MemorySettings::default(),
            tools: ToolsSettings::default(),
            shell: ShellSettings::default(),
        }
    }
}

/// IPC 投影（settings_get / settings_set 返回值）：无任何秘钥字段。
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    pub locale: String,
    /// 全局快捷键（切换主窗口），如 "Ctrl+Shift+L"。
    pub shortcut: String,
    pub data_dir: String,
    pub llm: LlmSettingsView,
    pub context: ContextSettingsView,
    pub memory: MemorySettingsView,
    pub tools: ToolsSettingsView,
    pub shell: ShellSettingsView,
}

/// 上下文管理区部分更新见 `context` 子模块；此文件只保留通用类型。

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LlmSettingsView {
    pub provider: String,
    pub model: String,
    pub base_url: Option<String>,
    pub api_key_set: bool,
}

/// 部分更新（settings_set 入参）。字段 None = 保持不变；
/// base_url / api_key 传空串 = 清除（回落默认端点 / 删除已存 Key）。
#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct LlmUpdate {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
}

pub struct SettingsStore {
    root: PathBuf,
    inner: Mutex<AppSettings>,
}

impl SettingsStore {
    /// 打开（不存在则用默认值建文件）；目录建不出来算错误。
    pub fn open(root: &Path) -> Result<Self, String> {
        std::fs::create_dir_all(root)
            .map_err(|e| format!("设置目录创建失败 {}: {e}", root.display()))?;
        let path = root.join(FILE_NAME);
        let settings = match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice::<AppSettings>(&bytes)
                .map_err(|e| format!("settings.json 解析失败: {e}"))?,
            Err(_) => AppSettings::default(),
        };
        Ok(Self {
            root: root.to_path_buf(),
            inner: Mutex::new(settings),
        })
    }

    pub fn view(&self) -> SettingsView {
        let s = self.inner.lock().expect("settings poisoned");
        self.project(&s)
    }

    /// 后端专用：LLM 配置全量（含 api_key 原值）——只给模型适配器读，
    /// 不进任何 IPC 投影。
    pub fn llm(&self) -> LlmSettings {
        self.inner.lock().expect("settings poisoned").llm.clone()
    }

    /// 后端专用：全局快捷键（tray.rs 启动注册用）。
    pub fn shortcut(&self) -> String {
        self.inner
            .lock()
            .expect("settings poisoned")
            .shortcut
            .clone()
    }

    /// 后端专用：语言偏好（tray.rs 菜单文案用）。
    pub fn locale(&self) -> String {
        self.inner.lock().expect("settings poisoned").locale.clone()
    }

    /// 后端专用：上下文管理设置快照（runner 进度条 / compact 配置换入用）。
    pub fn context(&self) -> ContextSettings {
        self.inner
            .lock()
            .expect("settings poisoned")
            .context
            .clone()
    }

    /// 后端专用：生效压缩配置（ctx-compact 共享锁的内容物）。
    pub fn compact_config(&self) -> CompactConfig {
        let c = self.context();
        CompactConfig {
            threshold_tokens: c.effective_threshold(),
            strategy: c.strategy,
        }
    }

    /// 后端专用：记忆注入活配置（MemoryInject 共享锁的内容物，⑪）。
    pub fn memory_inject_config(&self) -> MemoryInjectConfig {
        let m = self.inner.lock().expect("settings poisoned").memory.clone();
        MemoryInjectConfig {
            enabled: m.enabled,
            top_k: m.top_k as usize,
        }
    }

    /// 校验 → 应用到副本 → 落盘成功才提交内存。返回最新投影。
    // 参数与 settings_set 同形：settings.json 各分区一一对应。
    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &self,
        locale: Option<String>,
        shortcut: Option<String>,
        llm: Option<LlmUpdate>,
        context: Option<ContextUpdate>,
        memory: Option<MemoryUpdate>,
        tools: Option<ToolsUpdate>,
        shell: Option<ShellUpdate>,
    ) -> Result<SettingsView, String> {
        let mut next = self.inner.lock().expect("settings poisoned").clone();
        if let Some(loc) = locale {
            if !LOCALES.contains(&loc.as_str()) {
                return Err(format!("不支持的语言: {loc}（可选 system / zh-CN / en）"));
            }
            next.locale = loc;
        }
        if let Some(sc) = shortcut {
            // 校验用全局快捷键解析器本身（与 tray.rs 注册同一套语义）；
            // 空串 / 无法识别都拒绝。注册在命令层做（需要 AppHandle）。
            let sc = sc.trim().to_string();
            if sc.is_empty() {
                return Err("快捷键不能为空（示例 Ctrl+Shift+L）".into());
            }
            if sc.parse::<Shortcut>().is_err() {
                return Err(format!("无法识别的快捷键: {sc}（示例 Ctrl+Shift+L）"));
            }
            next.shortcut = sc;
        }
        if let Some(u) = llm {
            if let Some(p) = &u.provider {
                if !PROVIDERS.contains(&p.as_str()) {
                    return Err(format!(
                        "不支持的 provider: {p}（可选 openai / anthropic / ollama）"
                    ));
                }
                next.llm.provider = p.clone();
            }
            if let Some(m) = u.model {
                next.llm.model = m;
            }
            if let Some(b) = u.base_url {
                next.llm.base_url = (!b.is_empty()).then_some(b);
            }
            if let Some(k) = u.api_key {
                next.llm.api_key = (!k.is_empty()).then_some(k);
            }
        }
        if let Some(u) = context {
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
                    other => next.context.window_tokens = other,
                }
            }
            if let Some(pct) = u.threshold_pct {
                if !(10..=90).contains(&pct) {
                    return Err(format!("压缩阈值 {pct}% 超出范围（10–90，10% 一级）"));
                }
                next.context.threshold_pct = pct;
            }
            if let Some(s) = u.strategy {
                next.context.strategy = s;
            }
        }
        if let Some(u) = memory {
            if let Some(e) = u.enabled {
                next.memory.enabled = e;
            }
            if let Some(k) = u.top_k {
                if !(1..=20).contains(&k) {
                    return Err(format!("记忆注入条数 {k} 超出范围（1–20）"));
                }
                next.memory.top_k = k;
            }
        }
        if let Some(u) = tools {
            if let Some(e) = u.enabled {
                next.tools.enabled = e;
            }
            if let Some(n) = u.max_per_request {
                if !(1..=200).contains(&n) {
                    return Err(format!("每次请求最大工具数 {n} 超出范围（1–200）"));
                }
                next.tools.max_per_request = n;
            }
        }
        if let Some(u) = shell {
            if let Some(e) = u.unlock_risky {
                next.shell.unlock_risky = e;
            }
        }
        let bytes = serde_json::to_vec_pretty(&next).expect("settings serialize");
        std::fs::write(self.root.join(FILE_NAME), bytes)
            .map_err(|e| format!("settings.json 写入失败: {e}"))?;
        *self.inner.lock().expect("settings poisoned") = next;
        Ok(self.view())
    }

    fn project(&self, s: &AppSettings) -> SettingsView {
        SettingsView {
            locale: s.locale.clone(),
            shortcut: s.shortcut.clone(),
            data_dir: self.root.display().to_string(),
            llm: LlmSettingsView {
                provider: s.llm.provider.clone(),
                model: s.llm.model.clone(),
                base_url: s.llm.base_url.clone(),
                api_key_set: s.llm.api_key.is_some(),
            },
            context: ContextSettingsView {
                window_tokens: s.context.window_tokens,
                threshold_pct: s.context.threshold_pct,
                strategy: s.context.strategy,
                threshold_tokens: s.context.effective_threshold(),
                suggested_pct: s.context.suggested_pct(),
            },
            memory: MemorySettingsView {
                enabled: s.memory.enabled,
                top_k: s.memory.top_k,
            },
            tools: ToolsSettingsView {
                enabled: s.tools.enabled,
                max_per_request: s.tools.max_per_request,
            },
            shell: ShellSettingsView {
                unlock_risky: s.shell.unlock_risky,
            },
        }
    }
}

#[cfg(test)]
mod tests;
