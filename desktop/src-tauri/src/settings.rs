//! 设置存储（DESIGN.md §13.1 / §13.5）。分区：LLM 配置、⑬ embedding 模型、
//! 语言、快捷键、⑨ 上下文管理、⑪ 记忆注入、⑫ 工具暴露、S1 shell 治理
//! （高危解锁）、⑬ 插件管理（自动停用）。持久化在数据根 `settings.json`；
//! API Key 只落盘不出 IPC（投影只有 `api_key_set` 布尔），真适配器在后端侧读原值。

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use kernel_agent::{CompactConfig, MemoryInjectConfig};
use serde::{Deserialize, Serialize};

mod appearance;
mod chat;
mod context;
mod developer;
mod embedding;
mod llm;
mod memory;
mod multimodal;
mod plugins;
mod shell;
mod thinking;
mod tools;
mod update;
mod view;

pub use appearance::{AppearanceSettings, AppearanceUpdate, AppearanceView};
pub use chat::{ChatSettings, ChatUpdate, ChatView};
pub use context::{ContextSettings, ContextSettingsView, ContextUpdate, STABILITY_ROUNDS_RANGE};
pub use developer::{DeveloperSettings, DeveloperUpdate, DeveloperView};
pub use embedding::{EmbeddingSettings, EmbeddingUpdate, EmbeddingView};
pub use llm::{
    new_profile_id, synthesize_view, synthesize_view_raw, validate_headers, LlmConfig,
    LlmSettings, LlmSettingsView, LlmUpdate, NetworkProxy, ProviderInput, ProviderModel,
    ProviderModelView, ProviderProfile, ProviderView, SubagentPin, PROVIDERS,
};
pub use memory::{MemorySettings, MemorySettingsView, MemoryUpdate, TailStrategy};
pub use multimodal::{MultimodalSettings, MultimodalUpdate, MultimodalView};
pub use plugins::{PluginsSettings, PluginsUpdate, PluginsView};
pub use shell::{ShellSettings, ShellSettingsView, ShellUpdate};
pub use thinking::{
    ThinkingSettings, ThinkingUpdate, ThinkingView, THINKING_LANGUAGE_VALUES, THINKING_NONE,
};
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
    /// 外观区（⑰：明暗主题，类型见 `appearance` 子模块）。字段级缺省：
    /// 旧 settings.json 无此区时取 dark（与历史行为一致）。
    pub appearance: AppearanceSettings,
    /// 全局快捷键（切换主窗口）。字段级缺省：旧 settings.json 无此字段
    /// 时取 SHORTCUT_DEFAULT，而不是空串。
    #[serde(default = "default_shortcut")]
    pub shortcut: String,
    /// llm 档位区（v2.58 档位制：`{active, providers}`，DESIGN §13.1；
    /// 生效配置 = `llm()` 平面投影）。旧平面字段 open 迁移为单档 default。
    pub llm: LlmConfig,
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
    /// 对话行为区（UI shell 文档 §3.2 ChatSection / §2.7 列宽偏好；类型见
    /// `chat` 子模块）。字段级缺省零迁移。
    pub chat: ChatSettings,
    /// 开发者选项区（v2.49，默认隐藏；类型见 `developer` 子模块）。
    pub developer: DeveloperSettings,
    /// 思考语言区（docs/design/NEW/language of thought.md；类型见
    /// `thinking` 子模块）。缺省 follow_ui，旧 settings.json 无此区零迁移。
    pub thinking: ThinkingSettings,
    /// 多模态区（批 D：上传校验上限 + 孤儿资产 GC；类型见 `multimodal`
    /// 子模块）。缺省 2048px/5MB/白名单 png-jpg-webp-gif+pdf/autoGc 开/
    /// 7 天宽限，旧 settings.json 无此区零迁移。
    pub multimodal: MultimodalSettings,
}

fn default_shortcut() -> String {
    SHORTCUT_DEFAULT.into()
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            locale: LOCALE_SYSTEM.into(),
            appearance: AppearanceSettings::default(),
            shortcut: SHORTCUT_DEFAULT.into(),
            llm: LlmConfig::default(),
            embedding: EmbeddingSettings::default(),
            context: ContextSettings::default(),
            memory: MemorySettings::default(),
            tools: ToolsSettings::default(),
            shell: ShellSettings::default(),
            plugins: PluginsSettings::default(),
            chat: ChatSettings::default(),
            developer: DeveloperSettings::default(),
            thinking: ThinkingSettings::default(),
            multimodal: MultimodalSettings::default(),
        }
    }
}

/// IPC 投影（settings_get / settings_set 返回值）：无任何秘钥字段。
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    pub locale: String,
    /// 外观区（⑰ 明暗主题）。
    pub appearance: AppearanceView,
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
    /// 对话行为区（§3.2 ChatSection）。
    pub chat: ChatView,
    /// 开发者选项区（v2.49，默认隐藏分区）。
    pub developer: DeveloperView,
    /// 思考语言区（docs/design/NEW/language of thought.md）。
    pub thinking: ThinkingView,
    /// 多模态区（批 D：上传校验上限 + 孤儿资产 GC）。
    pub multimodal: MultimodalView,
}

pub struct SettingsStore {
    root: PathBuf,
    inner: Mutex<AppSettings>,
}

impl SettingsStore {
    /// 打开（不存在则用默认值建文件）；目录建不出来算错误。迁移两条
    /// （都幂等，落盘由下一次任意保存完成）：S2 旧 `context.windowTokens`
    /// 搬到 llm 活动档；v2.58 旧平面 llm 区（provider/model/…）包成单档
    /// `default`（档位制，DESIGN §13.1）。
    pub fn open(root: &Path) -> Result<Self, String> {
        std::fs::create_dir_all(root)
            .map_err(|e| format!("设置目录创建失败 {}: {e}", root.display()))?;
        let path = root.join(FILE_NAME);
        let mut settings = match std::fs::read(&path) {
            Ok(bytes) => {
                let mut raw: serde_json::Value = serde_json::from_slice(&bytes)
                    .map_err(|e| format!("settings.json 解析失败: {e}"))?;
                llm::migrate_flat_llm(&mut raw);
                serde_json::from_value::<AppSettings>(raw)
                    .map_err(|e| format!("settings.json 解析失败: {e}"))?
            }
            Err(_) => AppSettings::default(),
        };
        // 批 C §8.4 惰性迁移：明文 API Key → 系统凭据库。有变更**立即
        // 落盘**（安全改进不留在内存等下一次保存）；写失败只 warn（内存
        // 态已是迁移后形态，凭据库里已有 Key）。
        if crate::llm::keyring::migrate(&mut settings.llm) {
            let bytes = serde_json::to_vec_pretty(&settings).expect("settings serialize");
            if let Err(e) = std::fs::write(&path, bytes) {
                tracing::warn!("keyring 迁移后 settings.json 落盘失败: {e}");
            }
        }
        if settings.llm.flat().window_tokens.is_none() && settings.context.window_tokens.is_some() {
            let w = settings.context.window_tokens.take();
            if settings.llm.providers.is_empty() {
                // 旧文件连 llm 区都没有：物化缺省档接住窗口（active 同步点亮）
                settings.llm.providers.push(llm::ProviderProfile {
                    id: "default".into(),
                    label: "openai".into(),
                    kind: "openai".into(),
                    window_tokens: w,
                    enabled: true,
                    ..Default::default()
                });
                settings.llm.active = "default".into();
            } else {
                settings.llm.providers[0].window_tokens = w;
            }
        }
        Ok(Self {
            root: root.to_path_buf(),
            inner: Mutex::new(settings),
        })
    }

    pub fn view(&self) -> SettingsView {
        let s = self.inner.lock().expect("settings poisoned");
        view::project(&s, &self.root)
    }

    /// 后端专用：LLM 生效配置全量（含 api_key 原值）——活动档平面投影，
    /// 只给模型适配器读，不进任何 IPC 投影。
    pub fn llm(&self) -> LlmSettings {
        self.inner.lock().expect("settings poisoned").llm.flat()
    }

    /// 后端专用：llm 档位区全量（v2.58 档位命令读写用；含 api_key 原值，
    /// 不进 IPC 投影）。
    pub fn llm_config(&self) -> LlmConfig {
        self.inner.lock().expect("settings poisoned").llm.clone()
    }

    /// 后端专用：llm 区原子改写（v2.58 档位命令：save/delete/切活动档）。
    /// 落盘成功才提交内存（update 同款顺序）；`apply` 返回 Err = 拒绝
    /// 且不污染内存态。返回改写后的全量档位区。
    pub fn update_llm_config(
        &self,
        apply: impl FnOnce(&mut LlmConfig) -> Result<(), String>,
    ) -> Result<LlmConfig, String> {
        let mut next = self.inner.lock().expect("settings poisoned").clone();
        apply(&mut next.llm)?;
        let bytes = serde_json::to_vec_pretty(&next).expect("settings serialize");
        std::fs::write(self.root.join(FILE_NAME), bytes)
            .map_err(|e| format!("settings.json 写入失败: {e}"))?;
        *self.inner.lock().expect("settings poisoned") = next;
        Ok(self.llm_config())
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

    /// 后端专用：思考语言档（thinking_language 注入侧按会话首请求解析用）。
    pub fn thinking_language(&self) -> String {
        self.inner
            .lock()
            .expect("settings poisoned")
            .thinking
            .language
            .clone()
    }

    /// 后端专用：上下文管理设置快照（runner 进度条 / compact 配置换入用）。
    pub fn context(&self) -> ContextSettings {
        self.inner
            .lock()
            .expect("settings poisoned")
            .context
            .clone()
    }

    /// 后端专用：生效窗口（S2 llm 活动档优先；context 旧字段兜底——正常路径 open() 迁移后恒为 None）。None = 未设置。
    pub fn effective_window(&self) -> Option<u64> {
        let s = self.inner.lock().expect("settings poisoned");
        s.llm.flat().window_tokens.or(s.context.window_tokens)
    }

    /// 后端专用：生效阈值绝对值（runner UsagePolicy / ctx-compact 判定
    /// 同源）。窗口未知回落 FALLBACK_WINDOW_TOKENS 保守保护。
    pub fn effective_threshold_tokens(&self) -> u64 {
        let s = self.inner.lock().expect("settings poisoned");
        let window = s
            .llm
            .flat()
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
            stability_rounds: c.stability_rounds,
        }
    }

    /// 后端专用：记忆注入活配置（MemoryInject 共享锁的内容物，⑪）。
    /// M5 起 `experiences` 随行——注入侧每请求读锁，保存即热换（免重建）。
    pub fn memory_inject_config(&self) -> MemoryInjectConfig {
        let m = self.inner.lock().expect("settings poisoned").memory.clone();
        MemoryInjectConfig {
            enabled: m.enabled,
            top_k: m.top_k as usize,
            experiences: m.experiences,
        }
    }

    /// 后端专用：记忆区全量（⑬ extract 钩子读 auto_extract 用）。
    pub fn memory(&self) -> MemorySettings {
        self.inner.lock().expect("settings poisoned").memory.clone()
    }

    /// 后端专用：对话行为区全量（v2.57 自动标题钩子读 auto_title 用）。
    pub fn chat(&self) -> ChatSettings {
        self.inner.lock().expect("settings poisoned").chat.clone()
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

    /// 后端专用：开发者选项区全量（v2.49 危险开关的后端判定用——不信
    /// 前端转述，命令侧直接读设置）。
    pub fn developer(&self) -> DeveloperSettings {
        self.inner
            .lock()
            .expect("settings poisoned")
            .developer
            .clone()
    }

    /// 后端专用：尾巴组装配置（M4 树会话组装源 + 溢出整理触发用）——
    /// 策略（A 边界结算缺省 / B 固定窗口）+ 窗口轮数。
    pub fn tail_config(&self) -> (crate::settings::memory::TailStrategy, u32) {
        let m = self.memory();
        (
            crate::settings::memory::TailStrategy::parse(&m.tail_strategy),
            m.tail_window_turns,
        )
    }

    /// 后端专用：collapse 配置（M4 §8.3 空间急救）——开关 + 自动收起
    /// 占用阈值（%）。
    pub fn collapse_config(&self) -> (bool, u8) {
        let m = self.memory();
        (m.collapse_enabled, m.collapse_threshold_pct)
    }

    /// 后端专用：未显式设置会话的缺省整理模式（M5 阶段 3 起用户层
    /// memory 区，缺省 tree——阶段 2 曾在 developer 区，键已退役）。
    pub fn default_organizer(&self) -> crate::organizer_mode::OrganizerMode {
        crate::organizer_mode::OrganizerMode::parse(&self.memory().default_organizer)
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests;
