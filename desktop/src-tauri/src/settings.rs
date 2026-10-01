//! 设置存储（DESIGN.md §13.1 / §13.5）。分区：LLM 配置、⑬ embedding 模型、
//! 语言、快捷键、⑨ 上下文管理、⑪ 记忆注入、⑫ 工具暴露、S1 shell 治理
//! （高危解锁）、⑬ 插件管理（自动停用）。持久化在数据根 `settings.json`；
//! API Key 只落盘不出 IPC（投影只有 `api_key_set` 布尔），真适配器在后端
//! 侧读原值。

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use kernel_agent::{CompactConfig, MemoryInjectConfig};
use serde::{Deserialize, Serialize};

mod context;
mod embedding;
mod llm;
mod memory;
mod plugins;
mod shell;
mod tools;
mod update;

pub use context::{ContextSettings, ContextSettingsView, ContextUpdate};
pub use embedding::{EmbeddingSettings, EmbeddingUpdate, EmbeddingView};
pub use llm::{LlmSettings, LlmSettingsView, LlmUpdate, PROVIDERS};
pub use memory::{MemorySettings, MemorySettingsView, MemoryUpdate};
pub use plugins::{PluginsSettings, PluginsUpdate, PluginsView};
pub use shell::{ShellSettings, ShellSettingsView, ShellUpdate};
pub use tools::{ToolsSettings, ToolsSettingsView, ToolsUpdate};

/// locale 取值：`system`（跟随系统，缺省）/ `zh-CN` / `en`（§13.5）。
pub const LOCALE_SYSTEM: &str = "system";
const LOCALES: [&str; 3] = [LOCALE_SYSTEM, "zh-CN", "en"];
/// 全局快捷键缺省值（tray.rs 启动注册用；settings.json `shortcut`）。
pub const SHORTCUT_DEFAULT: &str = "Ctrl+Shift+L";
/// 模型窗口未设置时的保守回落：沿用 v2.14 固定 32k 保护（Context 文档
/// §3.2 的窗口 × 百分比语义要等窗口已知才成立）。
pub const FALLBACK_WINDOW_TOKENS: u64 = 32_000;

const FILE_NAME: &str = "settings.json";

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    pub locale: String,
    /// 全局快捷键（切换主窗口）。字段级缺省：旧 settings.json 无此字段
    /// 时取 SHORTCUT_DEFAULT，而不是空串。
    #[serde(default = "default_shortcut")]
    pub shortcut: String,
    pub llm: LlmSettings,
    /// embedding 模型区（⑬ 智能层共享基建；类型见 `embedding` 子模块）。
    pub embedding: EmbeddingSettings,
    /// 上下文管理区（⑨）。字段级缺省：旧 settings.json 无此字段时取默认
    /// （窗口未设置 / 阈值 30% / 分层保留）。
    pub context: ContextSettings,
    /// 记忆注入区（⑪，类型见 `memory` 子模块）。
    pub memory: MemorySettings,
    /// 工具暴露区（⑫，类型见 `tools` 子模块）。
    pub tools: ToolsSettings,
    /// shell 治理区（S1，类型见 `shell` 子模块）。
    pub shell: ShellSettings,
    /// 插件管理区（⑬ 自动停用；类型见 `plugins` 子模块）。
    pub plugins: PluginsSettings,
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
            embedding: EmbeddingSettings::default(),
            context: ContextSettings::default(),
            memory: MemorySettings::default(),
            tools: ToolsSettings::default(),
            shell: ShellSettings::default(),
            plugins: PluginsSettings::default(),
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
    pub embedding: EmbeddingView,
    pub context: ContextSettingsView,
    pub memory: MemorySettingsView,
    pub tools: ToolsSettingsView,
    pub shell: ShellSettingsView,
    pub plugins: PluginsView,
}

pub struct SettingsStore {
    root: PathBuf,
    inner: Mutex<AppSettings>,
}

impl SettingsStore {
    /// 打开（不存在则用默认值建文件）；目录建不出来算错误。S2 迁移：
    /// 旧 `context.windowTokens` 搬到 `llm` 区（信息归属修正——窗口是
    /// 模型属性）。落盘由下一次任意保存完成，重启重做迁移幂等。
    pub fn open(root: &Path) -> Result<Self, String> {
        std::fs::create_dir_all(root)
            .map_err(|e| format!("设置目录创建失败 {}: {e}", root.display()))?;
        let path = root.join(FILE_NAME);
        let mut settings = match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice::<AppSettings>(&bytes)
                .map_err(|e| format!("settings.json 解析失败: {e}"))?,
            Err(_) => AppSettings::default(),
        };
        if settings.llm.window_tokens.is_none() && settings.context.window_tokens.is_some() {
            settings.llm.window_tokens = settings.context.window_tokens.take();
        }
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

    /// 后端专用：生效窗口（S2 llm 区优先；context 旧字段兜底——正常路径
    /// open() 迁移后恒为 None）。None = 未设置。
    pub fn effective_window(&self) -> Option<u64> {
        let s = self.inner.lock().expect("settings poisoned");
        s.llm.window_tokens.or(s.context.window_tokens)
    }

    /// 后端专用：生效阈值绝对值（runner UsagePolicy / ctx-compact 判定
    /// 同源）。窗口未知回落 FALLBACK_WINDOW_TOKENS 保守保护。
    pub fn effective_threshold_tokens(&self) -> u64 {
        let s = self.inner.lock().expect("settings poisoned");
        let window = s
            .llm
            .window_tokens
            .or(s.context.window_tokens)
            .unwrap_or(FALLBACK_WINDOW_TOKENS);
        s.context.effective_threshold(window)
    }

    /// 后端专用：生效压缩配置（ctx-compact 共享锁的内容物）。
    pub fn compact_config(&self) -> CompactConfig {
        let c = self.context();
        CompactConfig {
            threshold_tokens: self.effective_threshold_tokens(),
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

    /// 后端专用：记忆区全量（⑬ extract 钩子读 auto_extract 用）。
    pub fn memory(&self) -> MemorySettings {
        self.inner.lock().expect("settings poisoned").memory.clone()
    }

    /// 后端专用：embedding 区全量（⑬ SharedEmbed 构建用）。
    pub fn embedding(&self) -> EmbeddingSettings {
        self.inner
            .lock()
            .expect("settings poisoned")
            .embedding
            .clone()
    }

    /// 后端专用：插件管理区全量（⑬ 自动停用天数）。
    pub fn plugins(&self) -> PluginsSettings {
        self.inner
            .lock()
            .expect("settings poisoned")
            .plugins
            .clone()
    }

    fn project(&self, s: &AppSettings) -> SettingsView {
        // S2：窗口以 llm 区为准（context 旧字段只在迁移前的旧文件里出现）
        let window = s.llm.window_tokens.or(s.context.window_tokens);
        SettingsView {
            locale: s.locale.clone(),
            shortcut: s.shortcut.clone(),
            data_dir: self.root.display().to_string(),
            llm: LlmSettingsView {
                provider: s.llm.provider.clone(),
                model: s.llm.model.clone(),
                base_url: s.llm.base_url.clone(),
                api_key_set: s.llm.api_key.is_some(),
                window_tokens: s.llm.window_tokens,
            },
            embedding: EmbeddingView {
                base_url: s.embedding.base_url.clone(),
                model: s.embedding.model.clone(),
                api_key_set: s.embedding.api_key.is_some(),
            },
            context: ContextSettingsView {
                threshold_pct: s.context.threshold_pct,
                strategy: s.context.strategy,
                threshold_tokens: s
                    .context
                    .effective_threshold(window.unwrap_or(FALLBACK_WINDOW_TOKENS)),
                suggested_pct: s.context.suggested_pct(window),
            },
            memory: MemorySettingsView {
                enabled: s.memory.enabled,
                top_k: s.memory.top_k,
                auto_extract: s.memory.auto_extract,
            },
            tools: ToolsSettingsView {
                enabled: s.tools.enabled,
                max_per_request: s.tools.max_per_request,
                call_timeout_secs: s.tools.call_timeout_secs,
            },
            shell: ShellSettingsView {
                unlock_risky: s.shell.unlock_risky,
            },
            plugins: PluginsView {
                auto_disable_idle_days: s.plugins.auto_disable_idle_days,
            },
        }
    }
}

#[cfg(test)]
mod tests;
