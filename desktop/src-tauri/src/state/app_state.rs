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
    /// 会话级权限模式存储（㉘·B）：旁车持久 + 缓存，决策表与目录围栏
    /// 的数据源；session_set_permission_mode 命令写穿。
    pub permission_modes: Arc<crate::permission_mode::PermissionModeStore>,
    /// 会话级模型覆盖存储（v2.58 §3.30）：旁车持久 + 缓存；AppLlm 每
    /// 请求按会话解析，session_set_model 命令写穿。
    pub model_overrides: Arc<crate::model_override::ModelOverrideStore>,
    /// 会话级整理模式存储（M4 §3.38①）：旁车持久 + 缓存；未显式设置
    /// 会话跟随开发者缺省档，session_set_organizer 命令写穿。
    pub organizer_modes: Arc<crate::organizer_mode::OrganizerModeStore>,
    /// 树会话组装器（M4 §3.38②）：TreeContextSource 实现（同时注册进
    /// 内核服务表）；run_turn 用量口径按组装后估算消费。
    pub tree_assembler: Arc<crate::organize::TreeAssembler>,
    /// UI 事件出口（§12.2）：runner 与审批请求共用一个 sink。
    pub sink: Arc<dyn EventSink>,
    /// session_id → 会话锁。会话在锁内串行聊天（tokio Mutex 可跨 await）；
    /// 外层 std Mutex 只护 map 存取，绝不跨 await 持有。Arc 化（⑭）：
    /// 调度引擎持有同一张表（触发目标解析 / 会话锁借用）。
    pub sessions: Arc<Mutex<HashMap<String, Arc<tokio::sync::Mutex<Session>>>>>,
    /// 会话落盘目录（⑲，`<数据根>/sessions`）：runner 增量镜像 / undo
    /// 重写 / session_list 与启动恢复共用。
    pub sessions_dir: std::path::PathBuf,
    /// 子会话转录目录（§3.16，`<数据根>/sub-transcripts`）：SubagentTool
    /// 收束经 SubTranscriptSink 写入，subagent_transcript 命令读出重放。
    pub sub_transcripts_dir: std::path::PathBuf,
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
    /// 记忆人类层句柄（F1/F2）：reconcile 与「打开文件夹」命令的数据源。
    pub memory_files: Arc<crate::memory_files::MemoryFiles>,
    /// 文件化总开关闭包（同 state 组装的 files_on；命令层判定空转）。
    pub memory_files_on: Arc<dyn Fn() -> bool + Send + Sync>,
    /// 使用统计写入端（⑩③，usage.sqlite3 宿主自有）。降级模式 = 丢弃。
    pub usage: Arc<UsageRecorder>,
    /// 应用内通知记录（§3.13，v2.51）：notifications.json 自持，命令层
    /// 读改；TauriSink 入账钩子经 app.manage 的克隆惰性取用。
    pub notifications: Arc<crate::notifications::NotificationStore>,
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
    /// 调度库（⑭，schedule.sqlite3）：目的地页 schedule_list / schedule_delete
    /// 命令与引擎/工具共用同一连接。打不开 = None（调度缺席已降级）。
    pub schedules: Option<Arc<crate::scheduler::ScheduleDb>>,
    /// 记忆树影子运行编排器（M2）：轮次收束钩子 / `/整理` / 启动重投 /
    /// 开发者观测的数据源；只写 memory-tree/ 投影，不改请求组装。
    pub organizer: Arc<crate::organize::Organizer>,
}

impl AppState {
    /// ⑭：agent_chat 每轮记最近聊过的会话——调度触发的目标解析在调度行
    /// 绑定的旧会话条目（重启后）不存在时，落到这里指的"活会话"。
    pub fn note_chat(&self, session_id: &str) {
        if let Ok(mut g) = self.last_chat.write() {
            *g = Some(session_id.to_owned());
        }
    }

    /// v2.49 会话删除后剔除"最近聊过"指针——否则调度兜底会向已删会话
    /// 注入触发轮（session_lock 惰性重建空会话 = 僵尸复活）。
    pub fn forget_chat(&self, session_id: &str) {
        if let Ok(mut g) = self.last_chat.write() {
            if g.as_deref() == Some(session_id) {
                *g = None;
            }
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
