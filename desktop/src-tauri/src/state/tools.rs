//! 组装点·工具目录簇（DESIGN §7.2 + §3.38③④ + plugins.md「宿主内置」通路）：
//! 宿主内置工具实例的构造 → `with_builtin` 挂进目录 → 树工具与门控/权限围栏
//! 组装 → `ToolSource` 注册进内核服务表。
//!
//! 本簇语句自 `state/mod.rs` 的 `init_state` **原样搬出**（批 F-3a·F5 结构性
//! 拆分），一条未重排。搬出它的理由不是行数，是**改动牵连面**：每加一个宿主
//! 内置工具都要动 `init_state` 本体（v2.131 三件、v2.142 四件、v2.144 两件、
//! v2.135 G5 一件……历批提交说明里全是它），而这段与「启动顺序」其实无关——
//! 它只依赖 `kernel`/`gate` 已就绪。拆出来后 `init_state` 里只剩一个调用点，
//! 加工具改这里，不再牵动组装主函。
//!
//! 三条留在本簇内部的次序契约（顺序错了会静默失效，故写在这里而不是注释在
//! 调用点）：
//! 1. `browser_console` / `browser_emit` 必须先于 `with_host_tools` 闭包就位
//!    ——闭包按名捕获工具变量（v2.144 原注）；
//! 2. `search_catalog` 先于 `main_source`：`search_tools` 吃的是**不含自身**的
//!    全目录视图（否则检索结果里出现检索工具自己）；
//! 3. Q1b 的检索回执采集壳（`with_receipt_sink`）只挂 `main_source`——它是
//!    执行面的观测，`search_catalog` 从不 `resolve`，挂上去既无读数又把"这棵树是什么"
//!    讲错（v2.184 本批）。
//!
//! `pdf_to_images` 缺席 = 随包 pdfium 不在位时不注册（宁缺勿假，注册必然失败
//! 的工具是谎报能力）。

use std::path::PathBuf;
use std::sync::Arc;

use cordis_core::Context;
use kernel_agent::{Kernel, LlmService, SubagentGate, SubagentTool, ToolSource};
use plugin_host::{PluginRegistry, RegistryToolSource};
use tauri::AppHandle;

use crate::demo_tools::SendEmailTool;
use crate::search_tools::SearchToolsTool;
use crate::settings::SettingsStore;
use crate::tool_source::KernelToolSource;
use crate::usage::UsageRecorder;

/// `build_and_provide` 的入参：本簇要消费的启动期句柄（逐字段显名传）。
pub(crate) struct Init {
    pub app: AppHandle,
    pub ctx: Arc<Context>,
    pub registry: Arc<PluginRegistry>,
    pub settings: Arc<SettingsStore>,
    pub root: PathBuf,
    pub usage: Arc<UsageRecorder>,
    /// embedding 冷却柄（`SharedEmbed::port()`）：⑫ 检索的语义信号源。
    pub embed_port: Arc<crate::llm::CooledEmbed>,
    pub llm: Arc<dyn LlmService>,
    /// 全局 LLM 服务同一实例（批 F 子代理模型面工厂要用具体类型）。
    pub app_llm: Arc<crate::llm::AppLlm>,
    pub catalog: Arc<crate::llm::catalog::CatalogStore>,
    pub assets: Arc<crate::assets::AssetStore>,
    pub kernel: Arc<Kernel>,
    pub gate: Arc<SubagentGate>,
    /// ask_user 选择卡中心（v4.11 §3.39①）：与审批同款机械件，工具侧只持句柄。
    pub ask_hub: Arc<crate::ask::AskHub>,
    pub schedules: Option<Arc<crate::scheduler::ScheduleDb>>,
    pub permission_modes: Arc<crate::permission_mode::PermissionModeStore>,
    /// Q1b 检索回执采集件（观测表 + 键位解析器；与注入侧同一个 `Arc<RecallLog>`，
    /// 见 `state/memory_stack.rs`）。
    pub receipt_sink: Arc<crate::recall_receipt::ReceiptSink>,
}

/// 组装两棵工具目录（检索视图 + 主目录）并把主目录注册进 `Context`，
/// 返回 `AppState` 需要接的 console 账本句柄。
pub(crate) fn build_and_provide(init: Init) -> Arc<crate::browser::ConsoleStore> {
    let Init {
        app,
        ctx,
        registry,
        settings,
        root,
        usage,
        embed_port,
        llm,
        app_llm,
        catalog,
        assets,
        kernel,
        gate,
        ask_hub,
        schedules,
        permission_modes,
        receipt_sink,
    } = init;

    // 工具目录：始终提供（registry 空 → resolve 报 NotAvailable，错误以失败 ToolResult 闭合，§7.2）。
    // 内置：send_email 演示替身（不可逆 → 审批）；⑭ 调度三件套；⑮ subagent_run（持内核 Arc，组装期有意成环）；
    // search_tools（⑫）持全目录视图（含 hidden）+ usage 高频回退 + ⑬ 语义检索（未配置降级关键词）。
    let send_email: Arc<dyn plugin_api::Tool> = Arc::new(SendEmailTool);
    // §3.16 子转录接收器：子自身段落 sub-transcripts/（§8.2 通路）
    let sub_tx = Arc::new(crate::sub_transcript::SubTranscriptStore::new(
        root.join("sub-transcripts"),
    ));
    // 批 F 子代理模型面工厂：每子 run 造一个 SubagentLlm（钉档 + 可用标
    // 记池的终端错误回退链）+ 报告句柄；链空 = 纯透传零行为变化。
    let child_model_factory: kernel_agent::ChildModelFactory = {
        let llm = Arc::clone(&app_llm);
        let settings = Arc::clone(&settings);
        let catalog = Arc::clone(&catalog);
        Arc::new(move || {
            let report = Arc::new(kernel_agent::ChildModelReport::default());
            let service: Arc<dyn LlmService> = Arc::new(crate::llm::SubagentLlm::new(
                Arc::clone(&llm),
                Arc::clone(&settings),
                Arc::clone(&catalog),
                Arc::clone(&report),
            ));
            kernel_agent::ChildModelHandle { service, report }
        })
    };
    let sub_tool: Arc<dyn plugin_api::Tool> = Arc::new(
        SubagentTool::new(Arc::clone(&kernel), Arc::clone(&gate))
            .with_transcript_sink(Arc::clone(&sub_tx) as Arc<dyn kernel_agent::SubTranscriptSink>)
            .with_child_model_factory(child_model_factory),
    );
    // v4.11 §3.39① ask_user 内置工具（Pure + always，见 ask_hub 处注释）
    let ask_tool: Arc<dyn plugin_api::Tool> =
        Arc::new(crate::ask::AskUserTool::new(Arc::clone(&ask_hub)));
    // 批 E 多模态三件（Pure）：screenshot / read_image 常开；pdf_to_images
    // 仅随包 pdfium 就位时进目录（宁缺勿假——注册必然失败的工具是谎报）。
    // 上限调用期读 settings multimodal 区 = 保存即热换（asset_put 同口径）。
    let media_limits: crate::media::LimitsFn = {
        let s = Arc::clone(&settings);
        Arc::new(move || {
            let m = s.multimodal();
            crate::assets::validate::Limits {
                max_dimension: m.max_dimension,
                max_size_mb: m.max_size_mb,
                image_formats: m.image_formats,
                file_formats: m.file_formats,
            }
        })
    };
    let screenshot_tool =
        crate::media::ScreenshotTool::new(Arc::clone(&assets), Arc::clone(&media_limits));
    let pdfium_lib = crate::media::resolve_pdfium_lib(&app);
    let pdf_tool = pdfium_lib.map(|lib| {
        tracing::info!(lib = %lib.display(), "pdf_to_images 就位");
        crate::media::pdf::PdfToImagesTool::new(Arc::clone(&assets), Arc::clone(&media_limits), lib)
    });
    let read_image_tool = crate::media::ReadImageTool::new(Arc::clone(&assets), media_limits);
    // G5 子代理独立验收（Pure + OnDemand）：读子转录 + 验收 LLM 调用（辅
    // 助降档）。与全局服务同一 AppLlm 实例。
    let verify_tool: Arc<dyn plugin_api::Tool> =
        Arc::new(crate::subagent_verify::SubagentVerifyTool::new(
            Arc::clone(&llm),
            root.join("sub-transcripts"),
        ));
    // 浏览器页签（v2.144）：console 账本 + browser_open 的 UI 推送出口
    //（同一 emit 形态；AppHandle 不进 browser 模块）。须在 with_host_tools
    // 闭包之前就位（闭包按名捕获工具变量）。
    let browser_console = Arc::new(crate::browser::ConsoleStore::new());
    let browser_emit: crate::browser::EmitFn = {
        let app = app.clone();
        std::sync::Arc::new(move |event, payload| {
            use tauri::Emitter;
            if let Err(e) = app.emit(event, payload) {
                tracing::warn!("{event} 推送失败: {e}");
            }
        })
    };
    let browser_open_tool: Arc<dyn plugin_api::Tool> = crate::browser::BrowserOpenTool::new(
        Arc::clone(&settings),
        Arc::clone(&browser_console),
        browser_emit,
    );
    let browser_console_tool: Arc<dyn plugin_api::Tool> =
        crate::browser::BrowserConsoleTool::new(Arc::clone(&browser_console));
    let with_host_tools = |src: KernelToolSource| {
        let src = src
            .with_builtin(Arc::clone(&send_email))
            .with_builtin(screenshot_tool.clone())
            .with_builtin(read_image_tool.clone());
        let src = match &pdf_tool {
            Some(t) => src.with_builtin(t.clone()),
            None => src,
        };
        let src = match &schedules {
            Some(db) => src
                .with_builtin(crate::scheduler::ScheduleCreateTool::new(Arc::clone(db)))
                .with_builtin(crate::scheduler::ScheduleListTool::new(Arc::clone(db)))
                .with_builtin(crate::scheduler::ScheduleDeleteTool::new(Arc::clone(db))),
            None => src,
        };
        src.with_builtin(Arc::clone(&sub_tool))
            .with_builtin(Arc::clone(&ask_tool))
            .with_builtin(Arc::clone(&verify_tool))
            .with_builtin(Arc::clone(&browser_open_tool))
            .with_builtin(Arc::clone(&browser_console_tool))
    };

    let search_catalog = Arc::new(with_host_tools(KernelToolSource::new(
        RegistryToolSource::new(Arc::clone(&registry)),
    )));
    let embed_tool_port: Arc<dyn kernel_agent::EmbedPort> = embed_port.clone();
    let main_source = with_host_tools(KernelToolSource::new(RegistryToolSource::new(Arc::clone(
        &registry,
    ))))
    .with_builtin(SearchToolsTool::new(
        search_catalog,
        Arc::clone(&usage),
        Some(embed_tool_port),
    ))
    // ㉘·B 计划模式目录围栏（非 Pure 不出目录、resolve 即拒）
    .with_modes(Arc::clone(&permission_modes))
    // Q1b（L0·(b)）：检索回执采集壳挂在**执行面**这一棵树上（`search_catalog` 只做
    // `list_all()` 索引，从不 resolve，因此不接）。第 3 条契约同款推理：观测面也只
    // 该覆盖真正会被模型调用的那条通路。
    .with_receipt_sink(receipt_sink);
    ctx.provide_dyn::<dyn ToolSource>(Arc::new(main_source));

    browser_console
}
