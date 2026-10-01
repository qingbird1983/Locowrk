//! `AppState` 共享状态结构体与其实例方法（⑭ 起自 state.rs 按职责拆分
//! ——单文件 400 行上限）：字段即组装清单，方法是与字段配套的便捷入口
//! （会话锁 / 最近会话 / 提取依赖快照 / embedding 热重建）。装配本体
//! `init_state` 在 `state/mod.rs`。

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use kernel_agent::{
    CompactConfig, Kernel, LlmService, MemoryInjectConfig, ToolExposure, ToolExposureConfig,
};
use plugin_service::PluginService;
use session_store::Session;
use tauri::AppHandle;

use crate::approval::ApprovalHub;
use crate::events::EventSink;
use crate::experiences::ExperienceDb;
use crate::llm::{EmbedClient, EmbeddingSettingsLlmView, SharedEmbed};
use crate::memory::MemoryStore;
use crate::settings::SettingsStore;
use crate::usage::UsageRecorder;

pub struct AppState {
    /// 阶段八的窗口级命令（对话框、多窗口）从这里取句柄；当前
    /// 事件出口已是共享 sink，字段暂无读取方。
    #[allow(dead_code)]
    pub app: AppHandle,
    pub kernel: Arc<Kernel>,
    /// 插件管理唯一入口（UI 不得直碰 registry，AGENTS.md 架构边界）。
    pub plugins: Arc<PluginService>,
    /// 审批中心（agent_approve / list_pending_approvals 从这里走）。
    pub approval: Arc<ApprovalHub>,
    /// UI 事件出口（§12.2）：runner 与审批请求共用一个 sink。
    pub sink: Arc<dyn EventSink>,
    /// session_id → 会话锁。会话在锁内串行聊天（tokio Mutex 可跨 await）；
    /// 外层 std Mutex 只护 map 存取，绝不跨 await 持有。Arc 化（⑭）：
    /// 调度引擎持有同一张表（触发目标解析 / 会话锁借用）。
    pub sessions: Arc<Mutex<HashMap<String, Arc<tokio::sync::Mutex<Session>>>>>,
    /// 最近聊过的会话（⑭）：agent_chat 每轮记一次。调度触发目标解析用
    /// ——调度行绑定的旧会话条目在重启后不存在，落到"活会话"不落僵尸。
    pub last_chat: Arc<RwLock<Option<String>>>,
    /// 设置存储（§13.1）。Arc 共享给 AppLlm（每请求重读，改动即生效）。
    pub settings: Arc<SettingsStore>,
    /// ctx-compact 的活配置（kernel-agent 侧只认 CompactConfig）。settings_set
    /// 成功后由命令层换入新值——压缩行为随即生效，无需重启。
    pub compact_cfg: Arc<RwLock<CompactConfig>>,
    /// Memory-Inject 的活配置（⑪，同 compact_cfg 的换入模式）。
    pub memory_cfg: Arc<RwLock<MemoryInjectConfig>>,
    /// 记忆库宿主访问（⑪）：Memory-Inject 检索源 + 记忆管理页存储支撑。
    pub memory: Arc<MemoryStore>,
    /// 使用统计写入端（⑩③，usage.sqlite3 宿主自有）。降级模式 = 丢弃。
    pub usage: Arc<UsageRecorder>,
    /// ToolExposure 的活配置（⑫，同 compact_cfg 的换入模式）。
    pub exposure_cfg: Arc<RwLock<ToolExposureConfig>>,
    /// ToolExposure 本体（⑫）：tool_exposure_debug 命令读最近一次截断快照。
    pub exposure: Arc<ToolExposure>,
    /// 宿主 Timeout 时长（S2）：settings tools 区 callTimeoutSecs 的热换锁，
    /// Timeout::hot 持有——保存即生效，无需重启。
    pub timeout_cfg: Arc<RwLock<Duration>>,
    /// embedding 共享柄（⑬）：settings embedding 区热换入，检索/经历/提取
    /// 三处共用；None 内值 = 未配置（智能层缺席，各处自行降级）。
    pub embed: SharedEmbed,
    /// 带故障冷却的 EmbedPort 实现（⑬，全宿主共用一个实例——冷却状态
    /// 不能分散）。
    pub embed_port: Arc<crate::llm::CooledEmbed>,
    /// 经历记忆库（⑬）；打不开 = None（经历不写不注入，事实记忆照常）。
    pub experiences: Option<Arc<ExperienceDb>>,
    /// LLM 服务句柄（⑬ 隐式提取用；与 ctx 里注册的是同一实例）。
    pub llm: Arc<dyn LlmService>,
}

impl AppState {
    /// ⑭：agent_chat 每轮记最近聊过的会话——调度触发的目标解析在调度行
    /// 绑定的旧会话条目（重启后）不存在时，落到这里指的"活会话"。
    pub fn note_chat(&self, session_id: &str) {
        if let Ok(mut g) = self.last_chat.write() {
            *g = Some(session_id.to_owned());
        }
    }

    /// 取（或惰性建）某会话的锁。首聊即用，无需显式建会话。
    pub fn session_lock(&self, id: &str) -> Arc<tokio::sync::Mutex<Session>> {
        self.sessions
            .lock()
            .expect("sessions poisoned")
            .entry(id.to_string())
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(Session::new())))
            .clone()
    }

    /// ⑬ 隐式提取依赖快照（spawn 用，全部 Arc 克隆）。
    pub fn extract_deps(&self) -> crate::extract::ExtractDeps {
        crate::extract::ExtractDeps {
            llm: Arc::clone(&self.llm),
            memory: Arc::clone(&self.memory),
            experiences: self.experiences.clone(),
            embed: Arc::clone(&self.embed_port),
        }
    }

    /// ⑬ settings_set 后重建 embedding 客户端（配置保存即生效）。
    pub fn reload_embed(&self) {
        let cfg = self.settings.embedding();
        let l = self.settings.llm();
        self.embed.set(EmbedClient::from_settings(
            &cfg,
            &EmbeddingSettingsLlmView {
                provider: l.provider,
                base_url: l.base_url,
            },
        ));
    }
}
