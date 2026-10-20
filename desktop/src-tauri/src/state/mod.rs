//! 共享状态组装（DESIGN.md §12.3）：Context + PluginRegistry + Kernel
//! （含审批门）+ 会话表 + PluginService（阶段七）。
//!
//! 启动顺序固定（§11.2 对账式恢复 / §12.3 备注）：
//! 1. `startup_reconcile`——清 tombstone、lockfile 与磁盘对账、恢复
//!    enabled 插件。必须在加载任何插件之前调用（此时无文件锁）
//! 2. 开发态装载（`plugins/`，内核开发用，不经 lockfile）
//! 3. 安装态已由第 1 步恢复完毕（`~/.Locowrk`，lockfile 管理）

mod app_state;
mod background;
mod boot;
mod kernel_stack;
mod llm_stack;
mod memory_stack;
mod plugins;
mod tools;
mod waterfall;

pub use app_state::AppState;

use std::sync::Arc;

use tauri::AppHandle;

pub fn init_state(app: AppHandle) -> AppState {
    // 启动基座簇（§12.3）：Context / 注册表 / 事件出口 / 桌面通知 vtable 钩子 /
    // 数据根 / 调度库 / 统计库 / embedding / 会话表与启动回放 / SettingsStore（含
    // 降级）/ fsRoots 与记忆文件化 env 注入。四条顺序契约写在 state/boot.rs 头注。
    let boot::Stack {
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
    } = boot::build(boot::Init { app: app.clone() });

    // LLM 栈簇（§7.2 + LLM.md v3.0）：停止标记 / 重试通知 / 模型覆盖 / 诊断账本 /
    // 能力目录 / 资产存储 / AppLlm 组装，末尾把 LlmService 与 LlmStreamSink 注册
    // 进 Context。字段名 = 原变量名（diagnoser 解构回 llm_diagnoser），下游语句逐字不动。
    let llm_stack::Stack {
        stop_flags,
        model_overrides,
        diagnoser: llm_diagnoser,
        catalog,
        assets,
        app_llm,
        llm,
        refusals,
    } = llm_stack::build(llm_stack::Init {
        app: app.clone(),
        ctx: Arc::clone(&ctx),
        settings: Arc::clone(&settings),
        sessions_dir: sessions_dir.clone(),
        root: root.clone(),
    });

    // 记忆与工作区簇（⑪ + ㉘ + F1 + M4）：fsRoots/开关供给闭包（保存即生效，不是
    // 启动快照）→ 会话上下文解析器（含 plugin-host 全局注入）→ MemoryStore →
    // 权限模式存储 → 树存储与组装器（TreeContextSource 注册）。
    let memory_stack::Stack {
        session_ctx,
        workspace_of,
        memory_files,
        files_on,
        memory,
        permission_modes,
        tree_store,
        receipt_sink,
        recall_log,
    } = memory_stack::build(memory_stack::Init {
        ctx: Arc::clone(&ctx),
        settings: Arc::clone(&settings),
        sessions_dir: sessions_dir.clone(),
        root: root.clone(),
    });

    // 主层压缩的熔断表（S2 刀⑤a）：两个使用方——step 边界的 `Compactor`（写它）与
    // REQUEST 环上的滑窗保护网（读它）——必须看同一张表，所以它在两者之前构造、各注
    // 一份 `Arc`。为什么不许各 new 一张 = `compact::Failures` 头注。
    let compact_failures = Arc::new(crate::compact::Failures::default());

    // waterfall 监听器簇（提示词分层设计 §6）：**注册序 = 执行序 = 前缀缓存红线**，
    // 序本体与三道交叉验证防线（prompt_layering.rs / check-middleware-wiring.mjs /
    // DESIGN §7.1）写在 state/waterfall.rs 头注——要动顺序先读它。
    let waterfall::Stack {
        exposure_cfg,
        exposure,
        memory_cfg,
        fragments,
    } = waterfall::build(waterfall::Init {
        ctx: Arc::clone(&ctx),
        settings: Arc::clone(&settings),
        usage: Arc::clone(&usage),
        embed_port: embed_port.clone(),
        memory: Arc::clone(&memory),
        root: root.clone(),
    });

    // 中间件链 + 审批 + 内核簇（§7.1 / 纪律 6 / ⑮）：洋葱序与「sink 先于 approval」
    // 的先后写在 state/kernel_stack.rs 头注。
    let kernel_stack::Stack {
        timeout_cfg,
        permission_rules,
        approval,
        ask_hub,
        gate,
        kernel,
    } = kernel_stack::build(kernel_stack::Init {
        ctx: Arc::clone(&ctx),
        sink: Arc::clone(&sink),
        settings: Arc::clone(&settings),
        permission_modes: Arc::clone(&permission_modes),
        session_ctx: Arc::clone(&session_ctx),
        root: root.clone(),
    });

    // 工具目录簇（§7.2 + §3.38③④）：宿主内置工具构造 + with_builtin 挂目录 +
    // search_catalog/main_source 两棵树 + ToolSource 注册。语句自本函数原样搬进
    // state/tools.rs（批 F-3a），本调用点就是它们原来的位置；返回的 console 账本
    // 是 AppState 唯一要接的句柄，其余都在簇内消费掉了。
    let browser_console = tools::build_and_provide(tools::Init {
        app: app.clone(),
        ctx: Arc::clone(&ctx),
        registry: Arc::clone(&registry),
        settings: Arc::clone(&settings),
        sessions_dir: sessions_dir.clone(),
        root: root.clone(),
        usage: Arc::clone(&usage),
        embed_port: embed_port.clone(),
        llm: Arc::clone(&llm),
        app_llm: Arc::clone(&app_llm),
        catalog: Arc::clone(&catalog),
        assets: Arc::clone(&assets),
        kernel: Arc::clone(&kernel),
        gate: Arc::clone(&gate),
        ask_hub: Arc::clone(&ask_hub),
        schedules: schedules.clone(),
        permission_modes: Arc::clone(&permission_modes),
        tree_store: tree_store.clone(),
        workspace_of: Arc::clone(&workspace_of),
        receipt_sink,
    });

    // 插件编排簇（§11.2 + §12.3）：服务开启（含数据根降级）→ 启动对账 → 开发态装载 →
    // 状态广播 → 自动停用巡检。语句自本函数原样搬进 state/plugins.rs，本调用点就是
    // 它们原来的位置——启动顺序在编排面上仍然可读，逐字段显名传参不动位置。
    let plugins = plugins::open_and_load(plugins::Init {
        app: app.clone(),
        ctx: Arc::clone(&ctx),
        registry: Arc::clone(&registry),
        settings: Arc::clone(&settings),
        usage: Arc::clone(&usage),
        root: root.clone(),
    });

    // 编排器与后台任务簇（M2/M3 §3.37 + ⑭ 调度引擎 + F1 §6 首启导出 + 批 D §6.3 GC）：
    // 整理事件闭包 / Organizer / spawn_engine / 在途重投 / 资产巡检。
    // 「emit 只落本模块」的原因（tao 不能进桌面测试二进制）写在 state/background.rs 头注。
    let background::Stack {
        organizer,
        compactor,
    } = background::build(background::Init {
        app: app.clone(),
        settings: Arc::clone(&settings),
        sessions_dir: sessions_dir.clone(),
        root: root.clone(),
        workspace_of: Arc::clone(&workspace_of),
        model_overrides: Arc::clone(&model_overrides),
        catalog: Arc::clone(&catalog),
        fragments: Arc::clone(&fragments),
        kernel: Arc::clone(&kernel),
        sink: Arc::clone(&sink),
        usage: Arc::clone(&usage),
        sessions: Arc::clone(&sessions),
        last_chat: Arc::clone(&last_chat),
        stop_flags: Arc::clone(&stop_flags),
        schedules: schedules.clone(),
        memory: Arc::clone(&memory),
        tree_store: tree_store.clone(),
        memory_files: Arc::clone(&memory_files),
        files_on: Arc::clone(&files_on),
        assets: Arc::clone(&assets),
        // S2 刀⑧：拒收读数表——发送侧（llm_stack 里那台 AppLlm）写，压缩侧读，
        // 两边都在组装点拿到同一份 `Arc` ⇒ 与下面那枚熔断表同一条共用件纪律。
        compact_refusals: Arc::clone(&refusals),
        // 最后一枚用 move：waterfall 那一头已经拿走一份，这张表从此由两个使用方共有。
        compact_failures,
    });

    AppState {
        app,
        kernel,
        plugins,
        approval,
        permission_modes,
        permission_rules,
        model_overrides,
        fragments,
        sink,
        sessions,
        sessions_dir,
        sub_transcripts_dir: root.join("sub-transcripts"),
        last_chat,
        settings,
        llm_diagnoser,
        memory_cfg,
        memory,
        recall_log,
        memory_files,
        memory_files_on: files_on,
        usage,
        exposure_cfg,
        exposure,
        timeout_cfg,
        embed,
        notifications,
        schedules,
        organizer,
        compactor,
        tree_store,
        stop_flags,
        ask: ask_hub,
        catalog,
        assets,
        browser: browser_console,
    }
}
