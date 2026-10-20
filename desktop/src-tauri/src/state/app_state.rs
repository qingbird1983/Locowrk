//! `AppState` 共享状态结构体与其实例方法（⑭ 起自 state.rs 按职责拆分
//! ——单文件 400 行上限）：字段即组装清单，方法是与字段配套的便捷入口
//! （会话锁 / 最近会话 / 提取依赖快照 / embedding 热重建）。装配本体
//! `init_state` 在 `state/mod.rs`。

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use kernel_agent::{Kernel, ToolExposure, ToolExposureConfig};
use plugin_service::PluginService;
use session_store::Session;
use tauri::AppHandle;

use crate::approval::ApprovalHub;
use crate::events::EventSink;
use crate::llm::{EmbedClient, EmbeddingSettingsLlmView, SharedEmbed};
use crate::settings::SettingsStore;
use crate::usage::UsageRecorder;

pub struct AppState {
    /// 窗口级命令句柄：自动标题（commands.rs）与调度引擎触发通知都经它
    /// 取窗口/发事件。
    pub app: AppHandle,
    pub kernel: Arc<Kernel>,
    /// 插件管理唯一入口（UI 不得直碰 registry，AGENTS.md 架构边界）。
    pub plugins: Arc<PluginService>,
    /// 审批中心（agent_approve / list_pending_approvals 从这里走）。
    pub approval: Arc<ApprovalHub>,
    /// 会话级权限模式存储（㉘·B）：旁车持久 + 缓存，决策表与目录围栏
    /// 的数据源；session_set_permission_mode 命令写穿。
    pub permission_modes: Arc<crate::permission_mode::PermissionModeStore>,
    /// 项目粘性授权规则库（v2.145）：审批决策表（③deny/⑤allow）与
    /// permission_rules_* 管理命令共读。
    pub permission_rules: Arc<crate::permission_rules::PermissionRuleStore>,
    /// 会话级模型覆盖存储（v2.58 §3.30）：旁车持久 + 缓存；AppLlm 每
    /// 请求按会话解析，session_set_model 命令写穿。
    pub model_overrides: Arc<crate::model_override::ModelOverrideStore>,
    // ⚠ 这里原来放的是 `Arc<crate::organize::TreeAssembler>`（M4 §3.38②）。S3 刀④a
    // 摘掉：它在本层的唯一消费者是 `run_turn` 的 `tree` 参数（进度条的第二把分子），
    // ⓟ 把分子收成 `context_budget::occupancy` 一把之后那条参数连字段一起没了。
    // S4 刀③c：内核侧那个 `TreeContextSource` 也随记忆整线退役，本条注释只剩历史读数。
    // 拒收读数没有加成字段：组装点经 `state.compactor.refusal(..)` 读，表只有
    // `Compactor` 一个宿主侧读口（`UsagePolicy::assemble` 的 `refused_tokens` 由它填）。
    /// 会话态碎片 pending 表（提示词分层设计 §4.4）：压缩完成 /
    /// 权限切 Plan / 轮次到顶几处置位，FragmentInject 轮开场取件拼接。
    pub fragments: Arc<crate::fragments::FragmentPending>,
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
    /// 使用统计写入端（⑩③，usage.sqlite3 宿主自有）。降级模式 = 丢弃。
    pub usage: Arc<UsageRecorder>,
    /// 应用内通知记录（§3.13，v2.51）：notifications.json 自持，命令层
    /// 读改；TauriSink 入账钩子经 app.manage 的克隆惰性取用。
    pub notifications: Arc<crate::notifications::NotificationStore>,
    /// ToolExposure 的活配置（⑫，与下方 timeout_cfg 同一热换模式）。
    pub exposure_cfg: Arc<RwLock<ToolExposureConfig>>,
    /// ToolExposure 本体（⑫）：tool_exposure_debug 命令读最近一次截断快照。
    pub exposure: Arc<ToolExposure>,
    /// 宿主 Timeout 时长（S2）：settings tools 区 callTimeoutSecs 的热换锁，
    /// Timeout::hot 持有——保存即生效，无需重启。
    pub timeout_cfg: Arc<RwLock<Duration>>,
    /// embedding 共享柄（⑬）：settings embedding 区热换入，检索/经历/提取
    /// 三处共用；None 内值 = 未配置（智能层缺席，各处自行降级）。
    pub embed: SharedEmbed,
    /// 前缀断裂诊断账本（M6.1）：AppLlm 每 step 写入（开关
    /// `developer.cacheDiagnostics`），`llm_cache_diagnose` 命令读出。
    pub llm_diagnoser: Arc<crate::llm::diagnose::Diagnoser>,
    /// 调度库（⑭，schedule.sqlite3）：目的地页 schedule_list / schedule_delete
    /// 命令与引擎/工具共用同一连接。打不开 = None（调度缺席已降级）。
    pub schedules: Option<Arc<crate::scheduler::ScheduleDb>>,
    /// 主层深度压缩执行体（S2 刀④c）：runner 在每个 step 边界问它一次「到线了没」，
    /// 到线就把较旧的一段交给摘要模型并落一条 `ContextCompacted`。摘要客户端走主模型
    /// 直呼通路（实例在 `state/background.rs` 构造，理由写在那儿）。
    pub compactor: Arc<crate::compact::deep::Compactor>,
    /// 停止标记注册表（v4.11 §3.39④）：run_turn 进场登记 RAII 守卫、
    /// 收束自动注销；agent_stop 命令置位（流式 delta 点 + 步间边界消费）。
    pub stop_flags: Arc<crate::stop::StopFlags>,
    /// ask_user 选择卡中心（v4.11 §3.39①）：ask_respond / ask_snooze /
    /// list_pending_asks 命令入口；agent_stop 经 skip_session 同步收束。
    pub ask: Arc<crate::ask::AskHub>,
    /// 模型能力目录（批 A，供应商稿 §4）：随包快照惰性加载 + 在线刷新
    /// 只进内存；`llm_catalog_*` 命令与 llm 投影合成（enrich）共用。
    pub catalog: Arc<crate::llm::catalog::CatalogStore>,
    /// 资产存储（批 D，`<数据根>/assets/` 内容寻址）：asset_put 命令写、
    /// wire 编码与缩略图渲染读（LRU）；GC 引用面三扫的根。
    pub assets: Arc<crate::assets::AssetStore>,
    /// 浏览器页签 console 账本（v2.144）：前端桥接回传经
    /// browser_report_errors 写入，browser_console 工具读出（验证环）。
    pub browser: Arc<crate::browser::ConsoleStore>,
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
