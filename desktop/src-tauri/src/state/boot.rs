//! 组装点·启动基座簇（DESIGN §12.3 + 纪律「数据根绑定一次」）：Context /
//! PluginRegistry / 事件出口 → 桌面通知 vtable 钩子 → 数据根 → 通知账本 →
//! 调度库 → 使用统计 → embedding 共享柄 → 会话表与启动回放 → SettingsStore
//! （含降级）→ embedding 首配 → fsRoots env 注入。
//!
//! 语句自 `state/mod.rs` 的 `init_state` **原样搬出**（批 F-3b·F5 结构性拆分），
//! 一条未重排。四条顺序契约（错了就是静默失效，不是报错）：
//! 1. `register_vtable_hook` 必须先于任何插件 init——tool-notify 通道经这个槽投递；
//! 2. `sessions_dir` 一次算出、多处共用（回放 / 覆盖存储 / 权限模式同键位）；
//! 3. `restore_all` 必须先于调度引擎（引擎是会话表的消费方）；
//! 4. SettingsStore 打不开就落临时目录继续跑（聊天可用性优先于设置完整性），
//!    但 embed 首配与 env 注入必须在 settings 就绪之后。
//!
//! 会话表用 `Arc<Mutex<HashMap<…>>>`：外层 std Mutex 只护 map 存取，绝不跨
//! await 持有（会话锁本身是 tokio Mutex，见 app_state.rs 字段注释）。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

use cordis_core::Context;
use plugin_host::PluginRegistry;
use session_store::Session;
use tauri::{AppHandle, Manager};

use crate::data_root;
use crate::events::{EventSink, TauriSink};
use crate::llm::{EmbedClient, EmbeddingSettingsLlmView, SharedEmbed};
use crate::settings::SettingsStore;
use crate::usage::{UsageDb, UsageRecorder};

pub(crate) struct Init {
    pub app: AppHandle,
}

/// 启动基座句柄：字段名 = 原 `init_state` 局部变量名，调用点按名解构，
/// 因此下游簇与 `AppState` 的字面量语句可以逐字不动。
pub(crate) struct Stack {
    pub ctx: Arc<Context>,
    pub registry: Arc<PluginRegistry>,
    pub sink: Arc<dyn EventSink>,
    pub notifications: Arc<crate::notifications::NotificationStore>,
    /// 数据根（一次绑定、多处共用：设置 / 插件编排 / LLM 配置 / 资产 / 审计都读它）。
    pub root: PathBuf,
    pub schedules: Option<Arc<crate::scheduler::ScheduleDb>>,
    pub usage: Arc<UsageRecorder>,
    pub embed: SharedEmbed,
    pub embed_port: Arc<crate::llm::CooledEmbed>,
    pub sessions: Arc<Mutex<HashMap<String, Arc<tokio::sync::Mutex<Session>>>>>,
    pub sessions_dir: PathBuf,
    pub last_chat: Arc<RwLock<Option<String>>>,
    pub settings: Arc<SettingsStore>,
}

pub(crate) fn build(init: Init) -> Stack {
    let Init { app } = init;

    let ctx = Arc::new(Context::new());
    let registry = Arc::new(PluginRegistry::new());
    let sink: Arc<dyn EventSink> = Arc::new(TauriSink::new(app.clone()));

    // ⑭ 桌面通知注入 vtable 槽：必须先于任何插件 init（tool-notify 通道经此投递）。
    crate::desktop_notify::register_vtable_hook(app.clone());

    // 数据根绑定一次，设置 / 插件编排 / LLM 配置共用（settings.json 同目录）。
    let root = data_root::data_dir();

    // 应用内通知记录（§3.13）：manage 给 TauriSink 入账钩子惰性取用。
    let notifications = Arc::new(crate::notifications::NotificationStore::open(
        root.join("notifications.json"),
    ));
    app.manage(Arc::clone(&notifications));

    // ⑭ 调度库先开（工具目录构造要持有）；引擎等 kernel 就绪后再起。
    let schedules = crate::scheduler::open_db(&root);

    // 使用统计库（⑩③）：usage.sqlite3，5s 批量刷盘；打不开只丢统计；⑫ 的 EWMA 积累与 search_tools 高频回退也从这里读。
    let usage = match UsageDb::open(&root.join("usage.sqlite3")) {
        Ok(db) => UsageRecorder::spawn(db),
        Err(e) => {
            tracing::warn!("usage db unavailable: {e}；使用统计降级为丢弃");
            UsageRecorder::disabled()
        }
    };

    // ⑬ embedding 共享柄：未配置 = None 内值（各处自行降级）；settings_set 后命令层换入即生效。port 全宿主共用（冷却不分家）。
    let embed = SharedEmbed::new();
    let embed_port = embed.port();

    // 会话表（内存）：键 = session id；⑭ 触发目标 / ⑮ 子快照同口径。
    let sessions: Arc<Mutex<HashMap<String, Arc<tokio::sync::Mutex<Session>>>>> =
        Arc::new(Mutex::new(HashMap::new()));
    let sessions_dir = root.join("sessions");
    // ⑲ 启动恢复：回放进内存表——必须在调度引擎（消费方）之前。
    let restored = crate::session_log::restore_all(&sessions_dir, &sessions);
    if restored > 0 {
        tracing::info!(restored, dir = %sessions_dir.display(), "sessions restored");
    }
    let last_chat: Arc<RwLock<Option<String>>> = Arc::new(RwLock::new(None));

    let settings: Arc<SettingsStore> = Arc::new(match SettingsStore::open(&root) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("{e}；设置降级到临时目录");
            let fallback = std::env::temp_dir().join("locowrk-degraded-data");
            SettingsStore::open(&fallback).expect("settings unavailable even in fallback dir")
        }
    });

    // settings 就绪：按 embedding 区灌入首批配置（llm 区 provider/端点兜底）
    {
        let cfg = settings.embedding();
        let l = settings.llm();
        embed.set(EmbedClient::from_settings(
            &cfg,
            &EmbeddingSettingsLlmView {
                provider: l.provider,
                base_url: l.base_url,
            },
        ));
    }

    // ⑱ 工作区目录 env 注入：fsutil 每次调用读 env，settings_set 后重注入即生效免重启
    crate::data_root::apply_fs_roots_env(&settings);
    // S4 刀③c：记忆整线退役，`memory.files` 设置项与记忆文件化 env 注入一并删除
    // （上一个刀次已无读者，本刀连写 env 的那行一起收走）。

    Stack {
        ctx,
        registry,
        sink,
        notifications,
        root,
        schedules,
        usage,
        embed,
        embed_port,
        sessions,
        sessions_dir,
        last_chat,
        settings,
    }
}
