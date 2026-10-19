//! 设置存储（DESIGN.md §13.1 / §13.5）。分区：LLM 配置、⑬ embedding 模型、
//! 语言、快捷键、⑨ 上下文管理、⑪ 记忆注入、⑫ 工具暴露、
//! ⑬ 插件管理（自动停用）。持久化在数据根 `settings.json`；
//! API Key 只落盘不出 IPC（投影只有 `api_key_set` 布尔），真适配器在后端侧读原值。

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use kernel_agent::{LearnWeights, MemoryInjectConfig};
use serde::{Deserialize, Serialize};

mod appearance;
mod chat;
// `pub(crate)` 而不是 `pub use` 抬那两个名字：换算处与缺省常量要被 `llm/tests` 与
// `settings/tests` 两簇按同一条路径读（缺省档由常量算出，不在测试里手抄 24576）。
// 走 re-export 的话非 test 目标里没人用它们 = `unused imports`，而门禁是
// `clippy --all-targets -- -D warnings` ⇒ 本批第一次 desk_clippy 就红在这里（判据自纠④）。
pub(crate) mod context;
mod developer;
mod embedding;
mod llm;
mod memory;
mod multimodal;
mod plugins;
mod thinking;
mod tools;
mod update;
mod view;

pub use appearance::{AppearanceSettings, AppearanceUpdate, AppearanceView};
pub use chat::{ChatSettings, ChatUpdate, ChatView};
pub use context::{ContextSettings, ContextSettingsView, ContextUpdate};
pub use developer::{DeveloperSettings, DeveloperUpdate, DeveloperView};
pub use embedding::{EmbeddingSettings, EmbeddingUpdate, EmbeddingView};
pub use llm::{
    new_profile_id, session_capacity, session_window, synthesize_view, synthesize_view_raw,
    validate_headers, LlmConfig, LlmSettings, LlmSettingsView, LlmUpdate, NetworkProxy,
    ProviderInput, ProviderModel, ProviderModelView, ProviderProfile, ProviderView, SubagentPin,
    PROVIDERS,
};
pub use memory::{MemorySettings, MemorySettingsView, MemoryUpdate, TailStrategy};
pub use multimodal::{MultimodalSettings, MultimodalUpdate, MultimodalView};
pub use plugins::{PluginsSettings, PluginsUpdate, PluginsView};
pub use thinking::{ThinkingSettings, ThinkingUpdate, ThinkingView, THINKING_NONE};
pub use tools::{ToolsSettings, ToolsSettingsView, ToolsUpdate};

/// locale 取值：`system`（跟随系统，缺省）/ `zh-CN` / `en`（§13.5）。
pub const LOCALE_SYSTEM: &str = "system";
const LOCALES: [&str; 3] = [LOCALE_SYSTEM, "zh-CN", "en"];
/// 全局快捷键缺省值（tray.rs 启动注册用；settings.json `shortcut`）。
pub const SHORTCUT_DEFAULT: &str = "Ctrl+Shift+L";

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
    /// 上下文管理区（⑨）。字段级缺省：旧 settings.json 无此区时取默认
    /// （窗口未设置 / 整理阈值 100K / 进度条开；v2.137 前的旧键幻影）。
    pub context: ContextSettings,
    /// 记忆注入区（⑪，类型见 `memory` 子模块）。
    pub memory: MemorySettings,
    /// 工具暴露区（⑫，类型见 `tools` 子模块）。
    pub tools: ToolsSettings,
    /// 插件管理区（⑬ 自动停用；类型见 `plugins` 子模块）。
    pub plugins: PluginsSettings,
    /// 对话行为区（UI shell 文档 §3.2 ChatSection / §2.7 列宽偏好；类型见
    /// `chat` 子模块）。字段级缺省零迁移。
    pub chat: ChatSettings,
    /// 开发者选项区（v2.49，默认隐藏；类型见 `developer` 子模块）。
    pub developer: DeveloperSettings,
    /// 思考语言区（docs/design/archive/language of thought.md；类型见
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
    pub plugins: PluginsView,
    /// 对话行为区（§3.2 ChatSection）。
    pub chat: ChatView,
    /// 开发者选项区（v2.49，默认隐藏分区）。
    pub developer: DeveloperView,
    /// 思考语言区（docs/design/archive/language of thought.md）。
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

    /// 后端专用（批 F-24b / 审计 M-18c）：一次取锁、把 `&LlmConfig` **按引用**交给闭包，
    /// 从而免掉「为读一个字段而全量克隆含各档明文 Key 的档位区」。返回闭包的产物。
    ///
    /// ⚠ **闭包内绝不能再调本 Store 的任何其它读面**：`inner` 是非重入的
    /// `std::sync::Mutex`，重入 = 自死锁（要别的区就先在闭包外取好）。
    /// ⚠ 交付的是引用 ⇒ 不把 `LlmConfig` 本身传出闭包，也就没有副本可泄露。
    pub fn with_llm_config<R>(&self, f: impl FnOnce(&LlmConfig) -> R) -> R {
        let s = self.inner.lock().expect("settings poisoned");
        f(&s.llm)
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

    /// 后端专用：整理阈值（绝对 token，v2.137——「上下文达到多少 K 就
    /// 整理」；UsagePolicy / 记忆树批量触发同源，不再折算窗口百分比）。
    pub fn organize_threshold_tokens(&self) -> u64 {
        self.inner
            .lock()
            .expect("settings poisoned")
            .context
            .threshold_tokens
    }

    /// 后端专用：微压缩开关（S2 刀③，缺省开）。监听器每请求读一次，
    /// 保存即生效——它是"要不要动"的门，不是数值，锁死到会话内反而违背
    /// 可配置原则「改了就看见」。
    pub fn context_micro_enabled(&self) -> bool {
        self.inner
            .lock()
            .expect("settings poisoned")
            .context
            .micro_enabled
    }

    /// 后端专用：深度压缩一组的活配置（S2 刀④c，缺省 = 刀④ 之前的硬编码常量）。
    /// **一次取锁取齐五枚**，理由与空串读成 `None` 的口径都在
    /// [`context::ContextSettings::compact_config`]，这里只是加锁的那一层薄壳。
    pub fn compact_config(&self) -> context::CompactConfig {
        self.inner
            .lock()
            .expect("settings poisoned")
            .context
            .compact_config()
    }

    /// 后端专用：工具结果预算（队列 Q8 · CB §7.4）——**字节**数，`None` = 不
    /// 限制。KiB→字节的换算只在 `settings/context.rs` 那一份里，wire / 组装点
    /// 都读这里，不再各自乘 1024（§11「同一读数只许一个出处」）。
    pub fn tool_result_budget_bytes(&self) -> Option<usize> {
        let kib = self
            .inner
            .lock()
            .expect("settings poisoned")
            .context
            .tool_result_budget_kib;
        context::tool_result_budget_bytes(kib)
    }

    /// 后端专用：记忆注入活配置（MemoryInject 共享锁的内容物，⑪）。
    ///
    /// 队列 Q9 起带上学习档：这里是 `MemorySettings`（持久化面）到
    /// `MemoryInjectConfig`（生效面）**唯一的换算点**——四个权重原样搬、不做
    /// 归一也不设"和为 1000"的门槛（理由见 `settings/memory.rs::LEARN_WEIGHT_MAX`）。
    ///
    /// ⚠ **改权重的生效时机 = 下一个会话，当前会话不动**（不是疏漏，是选型）：
    /// 内核的会话预取缓存只认 `(session_id, scope_id)` + `top_k`，块字节在会话内
    /// 钉死是 M1 纪律（KV v2：逐轮换字节 = 前缀命中归零）。会话中途把 relevance
    /// 从 400 拖到 500 若立刻重算，代价是打断这一轮对话的缓存前缀，而它买不到
    /// 任何正确性——记忆块的**归属**变了必须重算（那是另一份库的内容），**偏好**
    /// 变了不必。这个形状**已有承重**：`PrefetchEntry` 的字段集只有 `block`/`top_k`，
    /// 而内核用例 `memory_inject/tests/learn.rs::cjk_query_produces_terms_and_hits_prefetch_once`
    /// 断言同会话跑两轮 `seen.len() == 1` —— 将来有人"顺手"把权重塞进失效判据，第二轮
    /// 就会重查，那条用例当场见红。真机走查要按这个口径判读，否则会误判成 bug。
    pub fn memory_inject_config(&self) -> MemoryInjectConfig {
        let m = self.inner.lock().expect("settings poisoned").memory.clone();
        MemoryInjectConfig {
            enabled: m.enabled,
            top_k: m.top_k as usize,
            learn: LearnWeights {
                enabled: m.learn_enabled,
                relevance: m.relevance_weight,
                strength: m.strength_weight,
                freshness: m.freshness_weight,
                recency: m.recency_weight,
            },
        }
    }

    /// 后端专用：记忆区全量（树工具门控 / 文件化开关等读这里）。
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
}

#[cfg(test)]
mod tests;
