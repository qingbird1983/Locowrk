//! ⑭ 定时调度（ROADMAP ⑭ / plugins.md P2 tool-schedule）：宿主内置
//! 工具面 + 触发引擎。调度器/存储/触发都是宿主关切（插件无后台常驻
//! 能力，tokio time 已在依赖）——同 search_tools 归宿主先例，不经插件
//! 管道。持久化 = 宿主自有 schedule.sqlite3（store.rs）；触发语义
//! （目标解析 / 忙顺延 / 启动补跑）见 engine.rs 模块注释；工具面与
//! 分级见 tools.rs。
//!
//! 降级：库打不开 = 调度整体缺席（工具面不注册、引擎不起），聊天
//! 不受影响——与 usage / experiences 同纪律。装配两步走：`open_db`
//! 在工具目录构造前（内置工具面要持有库），`spawn` 在 kernel 就绪后。

mod engine;
pub(crate) mod store;
mod tools;

#[cfg(test)]
mod tests;

pub(crate) use engine::{spawn, EngineDeps};
pub(crate) use store::ScheduleDb;
pub(crate) use tools::{ScheduleCreateTool, ScheduleDeleteTool, ScheduleListTool};

/// 开调度库（幂等建表）。None = 打不开（降级：工具面与引擎都不装）。
pub(crate) fn open_db(root: &std::path::Path) -> Option<Arc<ScheduleDb>> {
    match ScheduleDb::open(&root.join("schedule.sqlite3")) {
        Ok(db) => Some(Arc::new(db)),
        Err(e) => {
            tracing::warn!("schedule db unavailable: {e}；定时任务降级为缺席");
            None
        }
    }
}

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use session_store::Session;

use crate::events::EventSink;
use crate::settings::SettingsStore;
use crate::usage::UsageRecorder;

/// 起引擎后台任务（组装点在 kernel / settings 就绪后调用一次；db 打开
/// 失败的降级态不调——工具面同样缺席，不出现"能建不能触发"的半态）。
#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_engine(
    db: Arc<ScheduleDb>,
    kernel: Arc<kernel_agent::Kernel>,
    sink: Arc<dyn EventSink>,
    usage: Arc<UsageRecorder>,
    settings: Arc<SettingsStore>,
    model_overrides: Arc<crate::model_override::ModelOverrideStore>,
    catalog: Arc<crate::llm::catalog::CatalogStore>,
    sessions: Arc<std::sync::Mutex<HashMap<String, Arc<tokio::sync::Mutex<Session>>>>>,
    sessions_dir: std::path::PathBuf,
    last_chat: Arc<RwLock<Option<String>>>,
    app: tauri::AppHandle,
    organizer: Arc<crate::organize::Organizer>,
    tree_assembler: Arc<crate::organize::TreeAssembler>,
    stop_flags: Arc<crate::stop::StopFlags>,
) {
    spawn(EngineDeps {
        db,
        kernel,
        sink,
        usage,
        settings,
        model_overrides,
        catalog,
        sessions,
        sessions_dir,
        last_chat,
        app,
        organizer,
        tree_assembler,
        stop_flags,
    });
}
