//! 共享状态组装（DESIGN.md §12.3）：Context + PluginRegistry + Kernel
//! （含审批门）+ 会话表 + PluginService（阶段七）。
//!
//! 启动顺序固定（§11.2 对账式恢复 / §12.3 备注）：
//! 1. `startup_reconcile`——清 tombstone、lockfile 与磁盘对账、恢复
//!    enabled 插件。必须在加载任何插件之前调用（此时无文件锁）
//! 2. 开发态装载（`plugins/`，内核开发用，不经 lockfile）
//! 3. 安装态已由第 1 步恢复完毕（`~/.Locowrk`，lockfile 管理）

mod app_state;

pub use app_state::AppState;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use cordis_core::Context;
use kernel_agent::{
    points, Audit, CircuitBreaker, CompactConfig, CtxCompact, JsonlAuditSink, Kernel, LlmService,
    MemoryInject, MemoryInjectConfig, Middleware, Retriever, SubagentGate, SubagentTool, Timeout,
    ToolExposure, ToolExposureConfig, ToolSource, Tracing,
};
use plugin_host::{PluginRegistry, RegistryToolSource};
use plugin_service::{LocalDirMarket, PluginService};
use session_store::Session;
use tauri::{AppHandle, Manager};

use crate::approval::ApprovalHub;
use crate::data_root;
use crate::demo_tools::SendEmailTool;
use crate::events::{forward_plugin_states, EventSink, TauriSink};
use crate::experiences::{ExperienceDb, ExperienceIndex};
use crate::llm::{AppLlm, EmbedClient, EmbeddingSettingsLlmView, SharedEmbed};
use crate::memory::MemoryStore;
use crate::plugins_root;
use crate::search_tools::SearchToolsTool;
use crate::settings::SettingsStore;
use crate::tool_source::KernelToolSource;
use crate::usage::{UsageDb, UsageEwma, UsageRecorder};

pub fn init_state(app: AppHandle) -> AppState {
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

    // S1：高危解锁开关插件加载前注入 env（tool-shell init 读一次，改动重启生效）；硬锁类不受影响。
    if settings.shell_unlock_risky() {
        std::env::set_var("LOCOWRK_SHELL_UNLOCK_RISK", "1");
    }
    // ⑱ 工作区目录 env 注入：fsutil 每次调用读 env，settings_set 后重注入即生效免重启
    crate::data_root::apply_fs_roots_env(&settings);
    // F1 记忆文件化总开关（settings_set 热换在 commands.rs）：关 = 插件侧
    // 双写整体跳过（env 缺省未设 = 开，与 memlayout::files_enabled 对齐）。
    if !settings.memory().files {
        std::env::set_var("LOCOWRK_MEMORY_FILES", "0");
    }

    // v2.58 会话级模型覆盖存储（旁车持久 + 缓存；AppLlm 每请求按 LlmRequest.session_id
    // 解析覆盖——档位路由 + 模型名）。模型适配器也只是服务（§7.2，自建
    // OpenAI 兼容客户端 LLM.md v3.0；未配置回落演示替身；⑬ 句柄留 AppState 给隐式提取）。
    // v4.11 停止标记先建（LLM.md v3.0 重试面同柄：重试等待轮询停止 +
    // 重试通知经组装闭包 emit，AppHandle 不进 llm 模块——v2.66 同款）。
    let stop_flags = Arc::new(crate::stop::StopFlags::default());
    let llm_notify = {
        let app = app.clone();
        Arc::new(move |sid: &str, attempt: u32, delay_ms: u64| {
            let event = crate::events::AgentEvent::LlmRetry {
                session_id: sid.to_string(),
                attempt,
                delay_ms,
            };
            if let Err(e) = tauri::Emitter::emit(&app, event.channel(), &event) {
                tracing::warn!("emit {} failed: {e}", event.channel());
            }
        }) as crate::llm::RetryNotify
    };
    let model_overrides = Arc::new(crate::model_override::ModelOverrideStore::new(
        sessions_dir.clone(),
    ));
    let llm: Arc<dyn LlmService> = Arc::new(
        AppLlm::with_overrides(Arc::clone(&settings), Arc::clone(&model_overrides))
            .with_retry_surface(Arc::clone(&stop_flags), llm_notify),
    );
    ctx.provide_dyn::<dyn LlmService>(Arc::clone(&llm));
    // ⑳ 文本 delta 直推（流式步骤整段投影被抑制，§12.2）；v4.11 §3.39④
    // 停止标记注册表同柄——delta 回调点逐段查询，命中即带部分正文收束。
    ctx.provide_dyn::<dyn kernel_agent::LlmStreamSink>(Arc::new(
        crate::events::StreamChunkSink::new(app.clone(), Arc::clone(&stop_flags)),
    ));

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
        // M4 会话级整理模式（树 v2 §11.2 阶段 2）：旁车 `organizer` 持久 +
        // 缓存写穿；未显式设置会话跟随开发者缺省档（settings_set 热换）。
    let organizer_modes = Arc::new(crate::organizer_mode::OrganizerModeStore::new(
        sessions_dir.clone(),
        settings.default_organizer(),
    ));
    // M4 树会话组装器（§3.38②）：TreeContextSource 注册进内核服务表——
    // legacy 会话返回 None（组装零变化），树会话 = L0 目录 + 未整理尾巴。
    let tree_assembler = Arc::new(crate::organize::TreeAssembler::new(
        Arc::clone(&organizer_modes),
        Arc::clone(&settings),
        Arc::clone(&workspace_of),
        memory_tree::TreeStore::new(&root),
        sessions_dir.clone(),
    ));
    ctx.provide_dyn::<dyn kernel_agent::TreeContextSource>(Arc::clone(&tree_assembler) as Arc<_>);

    // 经历记忆库（⑬）：experiences.sqlite3；打不开只丢经历。
    let experiences = match ExperienceDb::open(&root.join("experiences.sqlite3")) {
        Ok(db) => Some(Arc::new(db)),
        Err(e) => {
            tracing::warn!("experiences db unavailable: {e}；经历记忆降级为缺席");
            None
        }
    };

    // agent/request 决策环（exposure 文档 §7，注册次序 = 执行次序）：ToolExposure → Memory-Inject → ctx-compact，配置经 RwLock 热换入。
    let exposure_cfg: Arc<RwLock<ToolExposureConfig>> =
        Arc::new(RwLock::new(settings.exposure_config()));
    let embed_exposure_port: Arc<dyn kernel_agent::EmbedPort> = embed_port.clone();
    let exposure = Arc::new(ToolExposure::new(
        UsageEwma::new(Arc::clone(&usage)),
        Arc::clone(&exposure_cfg),
        Some(Arc::new(Retriever::new(embed_exposure_port))), // ⑬ 语义信号：未配置 = 空表退回两层打分（fail-open）
    ));
    ctx.events
        .on_waterfall(points::REQUEST, Arc::clone(&exposure) as Arc<_>);

    let memory_cfg: Arc<RwLock<MemoryInjectConfig>> =
        Arc::new(RwLock::new(settings.memory_inject_config()));
    ctx.events.on_waterfall(
        points::REQUEST,
        Arc::new(MemoryInject::new(
            crate::memory::memory_source(&memory),
            experiences
                .as_ref()
                .map(|db| {
                    ExperienceIndex::new(
                        Arc::clone(db),
                        Arc::clone(&embed_port),
                        Arc::clone(&workspace_of),
                    )
                })
                .map(|idx| idx as Arc<dyn kernel_agent::ExperienceSource>),
            Arc::clone(&memory_cfg),
        )),
    );

    // M4 树目录注入（§3.38②）：Memory-Inject 之后、ctx-compact 之前——
    // 目录排在记忆块之后（树 v2 §7.1 [2]→[3]）；ctx-compact 见树会话标记
    // 即跳过（树会话不压缩，压缩语义由整理边界承担）。
    ctx.events.on_waterfall(
        points::REQUEST,
        Arc::new(kernel_agent::TreeDirectoryInject) as Arc<_>,
    );

    // ctx-compact（Context and Memory.md §3）：agent/request 决策环上的压缩
    // 监听器——三策略配置驱动（阈值 = 窗口 × 百分比，压到阈值 50%），压缩结果经 payload 带回 kernel 落 ContextCompacted 事件。
    let compact_cfg: Arc<RwLock<CompactConfig>> = Arc::new(RwLock::new(settings.compact_config()));
    ctx.events.on_waterfall(
        points::REQUEST,
        Arc::new(CtxCompact::new(ctx.clone(), Arc::clone(&compact_cfg))),
    );

    // 中间件链组装次序（§7.1）：Tracing → Audit → Timeout → CircuitBreaker。Audit 挂外侧（被拒也留痕）；audit.jsonl 只增不改；S2 超时可配热换入。
    let timeout_cfg: Arc<RwLock<Duration>> = Arc::new(RwLock::new(settings.call_timeout()));
    let mw: Vec<Arc<dyn Middleware>> = vec![
        Arc::new(Tracing),
        Arc::new(Audit::new(Arc::new(JsonlAuditSink::new(
            root.join("audit.jsonl"),
        )))),
        Arc::new(Timeout::hot(Arc::clone(&timeout_cfg))),
        Arc::new(CircuitBreaker::new(5, Duration::from_secs(30))),
    ];
    // 审批门（纪律 9）：守夜超时初值取设置（v2.49 可配置，热换入；0=不限）。㉘·B：接权限模式存储（四档决策表）+ 会话→上下文（环境行/越界门围栏；㉘·C 起传全量 resolver 与插件围栏同源）
    let approval = Arc::new(
        ApprovalHub::with_timeout(
            Arc::clone(&sink),
            Duration::from_secs(settings.approval_timeout_secs()),
        )
        .with_modes(Arc::clone(&permission_modes))
        .with_workspace(Arc::clone(&session_ctx)),
    );
    let resolver: Arc<dyn kernel_agent::ApprovalResolver> = approval.clone();
    // v4.11 §3.39① ask_user 选择卡中心：与审批同款机械件（oneshot + 守夜），
    // 超时初值取设置（0 = 不限；settings_set 热换）。Pure + always：不走
    // 审批门、常驻目录。
    let ask_hub = Arc::new(crate::ask::AskHub::new(Arc::clone(&sink)));
    ask_hub.set_timeout_secs(settings.ask_timeout_secs());
    // ⑮ 子 Agent 快照表：每 step 刷新父转录前缀；subagent_run 执行时 fork
    let gate = Arc::new(SubagentGate::default());
    let kernel = Arc::new(
        Kernel::new(ctx.clone(), mw)
            .with_approval(resolver)
            .with_subagent(Arc::clone(&gate)),
    );

    // 工具目录：始终提供（registry 空 → resolve 报 NotAvailable，错误以失败 ToolResult 闭合，§7.2）。
    // 内置：send_email 演示替身（不可逆 → 审批）；⑭ 调度三件套；⑮ subagent_run（持内核 Arc，组装期有意成环）；
    // search_tools（⑫）持全目录视图（含 hidden）+ usage 高频回退 + ⑬ 语义检索（未配置降级关键词）。
    let send_email: Arc<dyn plugin_api::Tool> = Arc::new(SendEmailTool);
    // §3.16 子转录接收器：子自身段落 sub-transcripts/（§8.2 通路）
    let sub_tx = Arc::new(crate::sub_transcript::SubTranscriptStore::new(
        root.join("sub-transcripts"),
    ));
    let sub_tool: Arc<dyn plugin_api::Tool> = Arc::new(
        SubagentTool::new(Arc::clone(&kernel), Arc::clone(&gate)).with_transcript_sink(sub_tx),
    );
    // v4.11 §3.39① ask_user 内置工具（Pure + always，见 ask_hub 处注释）
    let ask_tool: Arc<dyn plugin_api::Tool> =
        Arc::new(crate::ask::AskUserTool::new(Arc::clone(&ask_hub)));
    let with_host_tools = |src: KernelToolSource| {
        let src = src.with_builtin(Arc::clone(&send_email));
        let src = match &schedules {
            Some(db) => src
                .with_builtin(crate::scheduler::ScheduleCreateTool::new(Arc::clone(db)))
                .with_builtin(crate::scheduler::ScheduleListTool::new(Arc::clone(db)))
                .with_builtin(crate::scheduler::ScheduleDeleteTool::new(Arc::clone(db))),
            None => src,
        };
        src.with_builtin(Arc::clone(&sub_tool))
            .with_builtin(Arc::clone(&ask_tool))
    };

    let search_catalog = Arc::new(with_host_tools(KernelToolSource::new(
        RegistryToolSource::new(Arc::clone(&registry)),
    )));
    let embed_tool_port: Arc<dyn kernel_agent::EmbedPort> = embed_port.clone();
    // M4 树工具（§3.38③④）：expand 树会话常驻（exposure=always）、collapse
    // 开关开启才进目录（§8.3 缺省关）——门控在 KernelToolSource 列表/解析
    // 双侧（legacy 会话不可见不可调）。不进 search_catalog 索引（会话级
    // 工具，人的检索面不含）。
    let tree_gate = Arc::new(crate::tool_source::TreeToolGate::new(
        Arc::clone(&organizer_modes),
        {
            let s = Arc::clone(&settings);
            Arc::new(move || s.memory().collapse_enabled)
        },
    ));
    let main_source = with_host_tools(KernelToolSource::new(RegistryToolSource::new(Arc::clone(
        &registry,
    ))))
    .with_builtin(SearchToolsTool::new(
        search_catalog,
        Arc::clone(&usage),
        Some(embed_tool_port),
    ))
    .with_builtin(crate::organize::MemoryExpandTool::new(
        memory_tree::TreeStore::new(&root),
        Arc::clone(&workspace_of),
        Arc::clone(&organizer_modes),
        sessions_dir.clone(),
    ))
    .with_builtin(crate::organize::MemoryCollapseTool::new(
        memory_tree::TreeStore::new(&root),
        Arc::clone(&workspace_of),
        Arc::clone(&organizer_modes),
    ))
    .with_tree_gate(tree_gate)
    // ㉘·B 计划模式目录围栏（非 Pure 不出目录、resolve 即拒）
    .with_modes(Arc::clone(&permission_modes));
    ctx.provide_dyn::<dyn ToolSource>(Arc::new(main_source));

    // 插件编排（§11.2）：数据根缺省 ~/.Locowrk（LOCOWRK_DATA_DIR可隔离）；打不开时退化到临时目录并明确告警——聊天不受影响。
    // S2：内置插件根一次解析，服务（启停解析工件）与开发态扫描共用。
    let builtin_root = plugins_root::resolve();
    let plugins =
        match open_plugin_service(&root, Arc::clone(&registry), &ctx, builtin_root.clone()) {
            Ok(svc) => Arc::new(svc),
            Err(primary_err) => {
                tracing::error!("plugin service init failed: {primary_err}；降级到临时目录");
                let fallback = std::env::temp_dir().join("locowrk-degraded-data");
                match open_plugin_service(
                    &fallback,
                    Arc::clone(&registry),
                    &ctx,
                    builtin_root.clone(),
                ) {
                    Ok(svc) => Arc::new(svc),
                    Err(e) => panic!("plugin service unavailable even in fallback dir: {e}"),
                }
            }
        };

    // 1. 启动对账（任何插件加载之前）
    if let Err(e) = plugins.startup_reconcile() {
        tracing::warn!("startup reconcile failed: {e}");
    }
    // 2. 开发态装载 plugins/（找不到不致命）。S2：跳过 lockfile 已接管/已停用 id；装载即登记 Builtin 条目（停用落 lockfile，重启不复活）。
    match builtin_root {
        Some(proot) => {
            let skip: std::collections::HashSet<String> =
                plugins.dev_scan_skip_ids().into_iter().collect();
            match plugins_root::load_all(&proot, &registry, &skip) {
                Ok(loaded) if loaded.is_empty() => {
                    tracing::warn!(dir = %proot.display(), "plugins/ 存在但没有加载任何插件")
                }
                Ok(loaded) => {
                    for (id, ver) in &loaded {
                        plugins.register_builtin(id, ver);
                    }
                    tracing::info!(dir = %proot.display(), ?loaded, "plugins loaded")
                }
                Err(e) => tracing::warn!("plugin scan failed: {e}"),
            }
        }
        None => tracing::warn!("plugins/ 目录未找到（可设 LOCOWRK_PLUGINS_DIR）；工具调用将不可用"),
    }
    // 3. 安装态已在对账中恢复；4. 状态广播转发给 UI（§13.4）
    forward_plugin_states(app.clone(), plugins.subscribe());

    // 5. ⑬ 自动停用巡检（exposure 文档 §8.8 例外条款，缺省关；spawn 化不阻塞窗口出现）
    let days = settings.plugins().auto_disable_idle_days;
    if days > 0 {
        let plugins2 = Arc::clone(&plugins);
        let usage2 = Arc::clone(&usage);
        // setup 主线程没有 tokio 上下文，后台化走 tauri 全局运行时（同 events / usage / scheduler-engine 先例）。
        tauri::async_runtime::spawn(async move {
            crate::autodisable::auto_disable_idle(&plugins2, &usage2, days).await;
        });
    }

    // M2 记忆树影子运行编排器：记忆管理器客户端 = 设置路由的兼容客户端直呼；
    // ㉘ 会话→项目解析器同源复用（(workspace, session) 键位对齐 28·A）。
    // 整理事件推送闭包在此组装（M3 §3.37③）：emit 调用只落在本模块——
    // 桌面单测不引用 state，tauri 运行时符号（及其窗口栈 tao）就不会进
    // 测试二进制；无 v6 清单的进程链接 tao 后 comctl32 缺
    // SetWindowSubclass/TaskDialogIndirect，加载即 ENTRYPOINT_NOT_FOUND。
    let organizer_notifier: crate::organize::OrganizeNotifier = {
        let app = app.clone();
        std::sync::Arc::new(move |event, payload| {
            use tauri::Emitter;
            if let Err(e) = app.emit(event, payload) {
                tracing::warn!("{event} 推送失败: {e}");
            }
        })
    };
    let organizer = crate::organize::build_organizer(
        Arc::clone(&settings),
        sessions_dir.clone(),
        Arc::clone(&workspace_of),
        root.clone(),
        Arc::clone(&organizer_modes),
        organizer_notifier,
    );
    // §5.3 崩溃恢复：扫描投影里的在途整理任务重投（会话已删 = 终局放弃）

    // 6. ⑭ 调度引擎（库可用时）：1s tick；错过的 run_at 落在过去，首次 tick 的 due() 天然覆盖启动补跑
    if let Some(db) = &schedules {
        crate::scheduler::spawn_engine(
            Arc::clone(db),
            Arc::clone(&kernel),
            Arc::clone(&sink),
            Arc::clone(&usage),
            Arc::clone(&settings),
            Arc::clone(&sessions),
            sessions_dir.clone(),
            Arc::clone(&last_chat),
            app.clone(),
            Arc::clone(&organizer),
            Arc::clone(&tree_assembler),
            Arc::clone(&stop_flags),
        );
    }
    // 在途整理任务重投（后台，不阻塞窗口出现）
    organizer.recover_pending();

    // F1 首启导出（文件化设计 §6）+ F2 启动 reconcile（先对账再补缺）：
    // 后台线程幂等，总开关关 = 什么都不做。
    let roots_on = {
        let s = Arc::clone(&settings);
        Arc::new(move || s.fs_roots()) as Arc<dyn Fn() -> Vec<String> + Send + Sync>
    };
    crate::memory_files::spawn_initial_export(
        Arc::clone(&memory),
        memory_tree::TreeStore::new(&root),
        (*memory_files).clone(),
        sessions_dir.clone(),
        Arc::clone(&files_on),
        roots_on,
    );

    AppState {
        app,
        kernel,
        plugins,
        approval,
        permission_modes,
        organizer_modes,
        model_overrides,
        tree_assembler,
        sink,
        sessions,
        sessions_dir,
        sub_transcripts_dir: root.join("sub-transcripts"),
        last_chat,
        settings,
        compact_cfg,
        memory_cfg,
        memory,
        memory_files,
        memory_files_on: files_on,
        usage,
        exposure_cfg,
        exposure,
        timeout_cfg,
        embed,
        embed_port,
        experiences,
        llm,
        notifications,
        schedules,
        organizer,
        stop_flags,
        ask: ask_hub,
    }
}

fn open_plugin_service(
    data_dir: &std::path::Path,
    registry: Arc<PluginRegistry>,
    ctx: &Arc<Context>,
    builtin_root: Option<std::path::PathBuf>,
) -> Result<PluginService, String> {
    let market_root =
        data_root::resolve_market_dir().unwrap_or_else(|| data_dir.join("market-absent"));
    tracing::info!(
        data = %data_dir.display(),
        market = %market_root.display(),
        "plugin service opening"
    );
    PluginService::open(
        data_dir.to_path_buf(),
        registry,
        Arc::clone(ctx),
        Box::new(LocalDirMarket::new(market_root)),
        builtin_root,
    )
}
