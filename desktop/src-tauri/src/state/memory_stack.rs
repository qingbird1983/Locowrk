//! 组装点·记忆与工作区簇（DESIGN ⑪ + ㉘ 会话→项目解析 + F1 文件化 §3 + M4 §3.38②）：
//! fsRoots 供给闭包 → 会话上下文解析器（含插件侧全局注入）→ 工作区视图 →
//! 记忆人类层与总开关 → `MemoryStore` → 会话级权限模式 → 树存储与组装器
//! （`TreeContextSource` 注册进内核服务表）。
//!
//! 语句自 `state/mod.rs` 的 `init_state` **原样搬出**（批 F-3b·F5 结构性拆分），
//! 一条未重排。两条必须留住的因果：
//! 1. `set_session_context_resolver` 是**进程级注入**（plugin-host 只认全局槽），
//!    必须早于任何插件调用——插件的 `CallCtx.workspace/fs_roots` 由它产出；
//! 2. `fs_roots_of` / `files_on` / `repo_on` 都是**每次调用重读设置**的闭包，
//!    不是启动快照：㉘·C 围栏让路与 F1 总开关都靠它做到「保存即生效」，
//!    换成 `Vec<String>`/`bool` 快照就是一处静默的Behaviour 漂移。

use std::path::PathBuf;
use std::sync::Arc;

use cordis_core::Context;

use crate::memory::MemoryStore;
use crate::settings::SettingsStore;

pub(crate) struct Init {
    pub ctx: Arc<Context>,
    pub settings: Arc<SettingsStore>,
    pub sessions_dir: PathBuf,
    pub root: PathBuf,
}

/// 下游簇与 `AppState` 要接的句柄（字段名 = 原变量名）。
pub(crate) struct Stack {
    pub session_ctx: crate::workspace::SessionContextResolver,
    pub workspace_of: crate::workspace::WorkspaceResolver,
    pub memory_files: Arc<crate::memory_files::MemoryFiles>,
    pub files_on: Arc<dyn Fn() -> bool + Send + Sync>,
    pub memory: Arc<MemoryStore>,
    pub permission_modes: Arc<crate::permission_mode::PermissionModeStore>,
    pub tree_store: memory_tree::TreeStore,
    pub tree_assembler: Arc<crate::organize::TreeAssembler>,
}

pub(crate) fn build(init: Init) -> Stack {
    let Init {
        ctx,
        settings,
        sessions_dir,
        root,
    } = init;

    // 记忆库（⑪）：Memory-Inject 检索源 + 管理页存储。㉘ 会话→项目解析器一次构造多处消费：插件调用期上下文（workspace + fs_roots 围栏）/注入键位/经历键位。㉘·C 围栏让路：fs_roots = 全局声明根（fsRoots 供给闭包读当前设置，保存即生效）。
    let fs_roots_of = {
        let s = Arc::clone(&settings);
        Arc::new(move || s.fs_roots()) as Arc<dyn Fn() -> Vec<String> + Send + Sync>
    };
    let session_ctx =
        crate::workspace::session_context_resolver(sessions_dir.clone(), root.clone(), fs_roots_of);
    plugin_host::set_session_context_resolver(Arc::clone(&session_ctx));
    let workspace_of = crate::workspace::workspace_view(Arc::clone(&session_ctx));
    // F1 人类层（文件化设计 §3）：md 句柄 + 总开关闭包——MemoryStore 四条
    // 宿主写路径（提取 write / 管理页 update·delete·clear）成功后同步 md。
    let memory_files = Arc::new(crate::memory_files::MemoryFiles::new(&root));
    let files_on = {
        let s = Arc::clone(&settings);
        Arc::new(move || s.memory().files) as Arc<dyn Fn() -> bool + Send + Sync>
    };
    // F2 交接注入（repo_context.rs）：归属会话注入 <repo>/.Locowrk/ 交接段
    let repo_on = {
        let s = Arc::clone(&settings);
        Arc::new(move || s.memory().repo_context) as Arc<dyn Fn() -> bool + Send + Sync>
    };
    let memory = Arc::new(
        MemoryStore::new(&root, Arc::clone(&workspace_of))
            .with_files(Arc::clone(&memory_files), Arc::clone(&files_on))
            .with_repo_context(Arc::clone(&repo_on)),
    );
    let permission_modes = Arc::new(crate::permission_mode::PermissionModeStore::new(
        sessions_dir.clone(),
        settings.default_permission_mode(),
    )); // ㉘·B 会话级权限模式（旁车持久 + 缓存；决策表/目录围栏数据源）
        // M4 树会话组装器（§3.38②）：TreeContextSource 注册进内核服务表——
        // v2.137 起一切会话皆树（legacy 整理面退役）= L0 目录 + 未整理尾巴。
        // M5 起存储句柄收敛为 AppState 单例（便宜克隆），组装点共用。
    let tree_store = memory_tree::TreeStore::new(&root);
    let tree_assembler = Arc::new(crate::organize::TreeAssembler::new(
        Arc::clone(&settings),
        Arc::clone(&workspace_of),
        tree_store.clone(),
        sessions_dir.clone(),
    ));
    ctx.provide_dyn::<dyn kernel_agent::TreeContextSource>(Arc::clone(&tree_assembler) as Arc<_>);

    Stack {
        session_ctx,
        workspace_of,
        memory_files,
        files_on,
        memory,
        permission_modes,
        tree_store,
        tree_assembler,
    }
}
