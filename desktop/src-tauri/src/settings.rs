//! 设置存储（DESIGN.md §13.1 设置模块 / §13.5 i18n 持久化）。
//!
//! 最小面板四块：LLM 配置（provider/model/base_url/api_key）、会话存储
//! 位置（只读展示）、语言切换（system/zh-CN/en，缺省跟随系统）、全局
//! 快捷键（阶段八⑤，切换主窗口，缺省 Ctrl+Shift+L）。
//! 持久化在数据根 `settings.json`，与 lockfile.json 同目录；文件在
//! `%USERPROFILE%\.Locowrk`，不属于项目文件，API Key 落盘合规
//! （AGENTS.md 禁的是写进仓库文件）。
//!
//! 纪律：**API Key 只进磁盘，不出 IPC**——前端拿到的投影只有
//! `api_key_set` 布尔；真适配器（LLM.md 方案一）在后端侧读原值。

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri_plugin_global_shortcut::Shortcut;

/// locale 取值：`system`（跟随系统，缺省）/ `zh-CN` / `en`（§13.5）。
pub const LOCALE_SYSTEM: &str = "system";
const LOCALES: [&str; 3] = [LOCALE_SYSTEM, "zh-CN", "en"];
/// LLM.md 方案一：统一适配器按 provider 路由（genai 封装，后续阶段接入）。
const PROVIDERS: [&str; 3] = ["openai", "anthropic", "ollama"];
/// 全局快捷键缺省值（tray.rs 启动注册用；settings.json `shortcut`）。
pub const SHORTCUT_DEFAULT: &str = "Ctrl+Shift+L";

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
}

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

    /// 校验 → 应用到副本 → 落盘成功才提交内存。返回最新投影。
    pub fn update(
        &self,
        locale: Option<String>,
        shortcut: Option<String>,
        llm: Option<LlmUpdate>,
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
        }
    }
}

#[cfg(test)]
mod tests;
