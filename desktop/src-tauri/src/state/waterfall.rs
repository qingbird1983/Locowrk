//! 组装点·waterfall 监听器簇（提示词分层设计《Context and Memory.md》§6 +
//! exposure 文档 §7）：`agent/request` 决策环上五个监听器的**注册序本体**。
//!
//! 语句自 `state/mod.rs` 的 `init_state` **原样搬出**（批 F-3b·F5 结构性拆分），
//! **注册序就是执行序，一条不许重排**——这是前缀缓存红线（§5 / v2.139）：
//! ToolExposure → BaseSystemInject（静态身份层 = system 字节前缀）→
//! ThinkingLanguage → InstructionFilesInject → FragmentInject。
//! 静态段排在动态段之前一旦反过来，中途改设置就把静态句的缓存断一次。
//!
//! 压缩不再挂 REQUEST 环（S4 退役面 · specs/11 刀①）：微压缩、滑窗保护网、
//! 树目录注入均已摘除；唯一的压缩是 step 边界的 [`crate::compact::deep::Compactor`]，
//! 由 runner 在 `kernel.step_as` 之前同步调用（模型停着等它，压完回填再继续）。
//! S4 刀③c：记忆注入环（MemoryInject）随记忆整线退役，不再挂在 REQUEST 上。
//!
//! 三道防线交叉验证（改这里之前先读它们）：
//! · `crates/kernel-agent/tests/prompt_layering.rs` 锁「静态段逐字节前缀」；
//! · `scripts/check-middleware-wiring.mjs` 抓「有定义、无生产构造点」；
//! · DESIGN §7.1 注册序正文。
//! 热换配置（exposure_cfg + settings 供给闭包）用 `RwLock` 注入：
//! `settings_set` 成功即换入，不等重启。

use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use cordis_core::Context;
use kernel_agent::{points, Retriever, ToolExposure, ToolExposureConfig};

use crate::llm::CooledEmbed;
use crate::settings::SettingsStore;
use crate::usage::{UsageEwma, UsageRecorder};
use crate::workspace::WorkspaceResolver;

pub(crate) struct Init {
    pub ctx: Arc<Context>,
    pub settings: Arc<SettingsStore>,
    pub usage: Arc<UsageRecorder>,
    pub embed_port: Arc<CooledEmbed>,
    /// 指令文件层的项目链解析（workspace-only，见 `workspace_only`）。
    pub workspace_of: WorkspaceResolver,
    pub root: PathBuf,
}

/// 两件要进 `AppState`（命令层热换）与后续簇（碎片置位）的句柄。
pub(crate) struct Stack {
    pub exposure_cfg: Arc<RwLock<ToolExposureConfig>>,
    pub exposure: Arc<ToolExposure>,
    pub fragments: Arc<crate::fragments::FragmentPending>,
}

pub(crate) fn build(init: Init) -> Stack {
    let Init {
        ctx,
        settings,
        usage,
        embed_port,
        workspace_of,
        root,
    } = init;

    // agent/request 决策环（exposure 文档 §7 / 提示词分层设计 §6，注册次序
    // = 执行次序）：ToolExposure → BaseSystemInject（静态身份层 = system
    // 字节前缀）→ 思考语言 → 指令文件 / 尾部碎片（两者都是消息面，与 system
    // 序无关），配置经 RwLock 热换入。
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

    // 静态身份层（提示词分层设计 §4.1）：include_str! 编译期内嵌六节
    // （引言/身份/工具策略/安全边界/输出纪律/reminder 合法性），跨会话
    // 逐字节同前缀——「用户当前请求 > 背景」总纲治开场仪式。
    ctx.events.on_waterfall(
        points::REQUEST,
        Arc::new(kernel_agent::BaseSystemInject) as Arc<_>,
    );

    // 会话态碎片 pending 表（§4.4）：权限切换/runner 两处置位（organize 的
    // 置位点随记忆整线退役）。
    let fragments = Arc::new(crate::fragments::FragmentPending::default());

    // 思考语言（docs/design/archive/language of thought.md）：排在静态身份层
    // 之后、动态段之前（提示词分层设计 §6）——它是静态句，后面不许再有静态段
    // 被动态内容顶到中间。会话首请求解析并锁死（前缀缓存红线 §5）——中途改
    // 设置当前会话不变，新会话用新配置。
    ctx.events.on_waterfall(
        points::REQUEST,
        Arc::new(crate::thinking_language::ThinkingLanguageInject::new(
            Arc::clone(&settings),
        )),
    );

    // 指令文件层（提示词分层设计 §4.3）：全局 <数据根>/AGENTS.md + 项目层三族候选
    // （AGENTS.md / CLAUDE.md / AGENTS.override.md，族内二选一、族间串联）→
    // 合成首条 user 消息（消息面，会话首查快照锁字节）。恒开；预算写死
    // `INSTRUCTION_CHAIN_KIB`（S4 刀③c：不再依赖记忆库，也不再是可调项）。
    let instruction_source = Arc::new(crate::instruction_files::InstructionFiles::new(
        Arc::clone(&workspace_of),
        root.join("AGENTS.md"),
    ));
    ctx.events.on_waterfall(
        points::REQUEST,
        Arc::new(kernel_agent::InstructionFilesInject::new(
            instruction_source as Arc<dyn kernel_agent::InstructionSource>,
        )),
    );

    // 尾部碎片（§4.4）：pending 表 → 轮开场拼 `<system-reminder>` 在最后
    // 一条 user 消息尾（出现一次即清）——压缩完成/Plan 模式/轮次到顶
    // 不再对模型静默。
    ctx.events.on_waterfall(
        points::REQUEST,
        Arc::new(kernel_agent::FragmentInject::new(
            Arc::clone(&fragments) as Arc<dyn kernel_agent::FragmentSource>
        )),
    );

    Stack {
        exposure_cfg,
        exposure,
        fragments,
    }
}
