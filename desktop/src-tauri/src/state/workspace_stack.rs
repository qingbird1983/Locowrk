//! 组装点·工作区簇（DESIGN ㉘ 会话→项目解析）：fsRoots 供给闭包 →
//! 会话上下文解析器（含插件侧全局注入）→ 会话级权限模式。
//!
//! S4 刀③c：记忆整线退役后本簇只剩这两件。它们的消费者都在别处（插件
//! 调用期上下文 / 审批决策表与目录围栏），与记忆无关，所以不随记忆删。
//!
//! 两条必须留住的因果：
//! 1. `set_session_context_resolver` 是**进程级注入**（plugin-host 只认全局槽），
//!    必须早于任何插件调用——插件的 `CallCtx.workspace/fs_roots` 由它产出；
//! 2. `fs_roots_of` 是**每次调用重读设置**的闭包，不是启动快照：㉘·C 围栏让路
//!    靠它做到「保存即生效」，换成 `Vec<String>` 快照就是一处静默的行为漂移。

use std::path::PathBuf;
use std::sync::Arc;

use crate::settings::SettingsStore;

pub(crate) struct Init {
    pub settings: Arc<SettingsStore>,
    pub sessions_dir: PathBuf,
    pub root: PathBuf,
}

/// 下游簇与 `AppState` 要接的句柄（字段名 = 原变量名）。
pub(crate) struct Stack {
    pub session_ctx: crate::workspace::SessionContextResolver,
    /// 指令文件层的项目链解析（原样 workspace 串，见 `workspace_only` 文档）。
    pub workspace_of: crate::workspace::WorkspaceResolver,
    pub permission_modes: Arc<crate::permission_mode::PermissionModeStore>,
}

pub(crate) fn build(init: Init) -> Stack {
    let Init {
        settings,
        sessions_dir,
        root,
    } = init;

    // ㉘ 会话→项目解析器：插件调用期上下文（workspace + fs_roots 围栏）的数据源。
    // ㉘·C 围栏让路：fs_roots = 全局声明根（fsRoots 供给闭包读当前设置，保存即生效）。
    let fs_roots_of = {
        let s = Arc::clone(&settings);
        Arc::new(move || s.fs_roots()) as Arc<dyn Fn() -> Vec<String> + Send + Sync>
    };
    let session_ctx =
        crate::workspace::session_context_resolver(sessions_dir.clone(), root, fs_roots_of);
    plugin_host::set_session_context_resolver(Arc::clone(&session_ctx));
    let workspace_of = crate::workspace::workspace_only(Arc::clone(&session_ctx));
    let permission_modes = Arc::new(crate::permission_mode::PermissionModeStore::new(
        sessions_dir,
        settings.default_permission_mode(),
    )); // ㉘·B 会话级权限模式（旁车持久 + 缓存；决策表/目录围栏数据源）

    Stack {
        session_ctx,
        workspace_of,
        permission_modes,
    }
}
