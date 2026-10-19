//! 组装点·waterfall 监听器簇（提示词分层设计《Context and Memory.md》§6 +
//! exposure 文档 §7）：`agent/request` 决策环上八个监听器的**注册序本体**。
//!
//! 语句自 `state/mod.rs` 的 `init_state` **原样搬出**（批 F-3b·F5 结构性拆分），
//! **注册序就是执行序，一条不许重排**——这是前缀缓存红线（§5 / v2.139）：
//! ToolExposure → BaseSystemInject（静态身份层 = system 字节前缀）→
//! MemoryInject → ThinkingLanguage → TreeDirectoryInject（append-only 殿后）→
//! InstructionFilesInject → FragmentInject → MicroCompact（S2 刀③）。
//! 静态段排在目录之前一旦反过来，每整理一次就把思考语言句的缓存断一次；
//! 目录之后若再挂任何**静态段**，目录增长会连带破前缀。
//! 第八环为什么可以在红线之后：它不往 system 里写任何东西，改的是 messages 里
//! 若干条**旧工具结果的正文**——前缀缓存里那段静态字节它一个字都不碰；而消息列
//! 每轮本来就在增长，缓存断在"最旧的那批结果"上正是这一层的用途（参照实现
//! ZCode 同形：microcompact 就在组装末位）。
//!
//! 三道防线交叉验证（改这里之前先读它们）：
//! · `crates/kernel-agent/tests/prompt_layering.rs` 锁「静态段前缀 + 记忆居中 +
//!   目录殿后 + 目录增长前缀逐字节不变」；
//! · `scripts/check-middleware-wiring.mjs` 抓「有定义、无生产构造点」；
//! · DESIGN §7.1 注册序正文。
//! 三个热换配置（exposure_cfg / memory_cfg + settings 供给闭包）用 `RwLock`
//! 注入：`settings_set` 成功即换入，不等重启。

use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use cordis_core::Context;
use kernel_agent::{
    points, MemoryInject, MemoryInjectConfig, Retriever, ToolExposure, ToolExposureConfig,
};

use crate::llm::CooledEmbed;
use crate::memory::MemoryStore;
use crate::settings::SettingsStore;
use crate::usage::{UsageEwma, UsageRecorder};

pub(crate) struct Init {
    pub ctx: Arc<Context>,
    pub settings: Arc<SettingsStore>,
    pub usage: Arc<UsageRecorder>,
    pub embed_port: Arc<CooledEmbed>,
    pub memory: Arc<MemoryStore>,
    pub root: PathBuf,
    /// S2 刀③：微压缩那一环要的三份读数（主层阈值 = 滑杆 ∩ 窗口 − 输出预留）。
    /// `catalog` / `model_overrides` 由 `llm_stack` 交出，`sessions_dir` 由 `boot`
    /// 交出（冷启播种 idle 时 stat 会话日志）。
    pub catalog: Arc<crate::llm::catalog::CatalogStore>,
    pub model_overrides: Arc<crate::model_override::ModelOverrideStore>,
    pub sessions_dir: PathBuf,
}

/// 三件要进 `AppState`（命令层热换）与后续簇（碎片置位）的句柄。
pub(crate) struct Stack {
    pub exposure_cfg: Arc<RwLock<ToolExposureConfig>>,
    pub exposure: Arc<ToolExposure>,
    pub memory_cfg: Arc<RwLock<MemoryInjectConfig>>,
    pub fragments: Arc<crate::fragments::FragmentPending>,
}

pub(crate) fn build(init: Init) -> Stack {
    let Init {
        ctx,
        settings,
        usage,
        embed_port,
        memory,
        root,
        catalog,
        model_overrides,
        sessions_dir,
    } = init;

    // agent/request 决策环（exposure 文档 §7 / 提示词分层设计 §6，注册次序
    // = 执行次序）：ToolExposure → BaseSystemInject（静态身份层 = system
    // 字节前缀）→ Memory-Inject → 思考语言 → 树目录（append-only 殿后）→
    // 指令文件 / 尾部碎片 / 微压缩（三者都是消息面，与 system 序无关），
    // 配置经 RwLock 热换入。
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

    // 会话态碎片 pending 表（§4.4）：organize/权限切换/runner 三处置位。
    let fragments = Arc::new(crate::fragments::FragmentPending::default());

    let memory_cfg: Arc<RwLock<MemoryInjectConfig>> =
        Arc::new(RwLock::new(settings.memory_inject_config()));
    ctx.events.on_waterfall(
        points::REQUEST,
        Arc::new(MemoryInject::new(
            crate::memory::memory_source(&memory),
            Arc::clone(&memory_cfg),
        )),
    );

    // 思考语言（docs/design/archive/language of thought.md）：**排在树目录
    // 之前**（提示词分层设计 §6 重排）——思考语言是静态段，树目录是
    // append-only，动态段必须殿后；旧序（目录之后）每整理一次就把思考
    // 语言句的前缀缓存断一次。会话首请求解析并锁死（前缀缓存红线 §5）
    // ——中途改设置当前会话不变，新会话用新配置。
    ctx.events.on_waterfall(
        points::REQUEST,
        Arc::new(crate::thinking_language::ThinkingLanguageInject::new(
            Arc::clone(&settings),
        )),
    );

    // M4 树目录注入（§3.38②）：system **真尾部**（append-only 殿后，其后
    // 不再有静态段）。v2.137：ctx-compact 桌面链退役（用户裁定「上下文
    // 根本不压缩了」，压缩语义由记忆树整理边界承担；headless 内核侧
    // 中间件保留不动）。
    ctx.events.on_waterfall(
        points::REQUEST,
        Arc::new(kernel_agent::TreeDirectoryInject) as Arc<_>,
    );

    // 指令文件层（提示词分层设计 §4.3）：全局 <数据根>/AGENTS.md + 项目层三族候选
    // （AGENTS.md / CLAUDE.md / AGENTS.override.md，族内二选一、族间串联）→
    // 合成首条 user 消息（消息面，会话首查快照锁字节）。开关 memory.instructionFiles，
    // 两条链各一档预算 memory.instructionGlobalKib / instructionProjectKib（v2.188 · Q4）。
    let instruction_source = Arc::new(crate::instruction_files::InstructionFiles::new(
        Arc::clone(&memory),
        root.join("AGENTS.md"),
        {
            let s = Arc::clone(&settings);
            Arc::new(move || s.memory().instruction_files) as Arc<dyn Fn() -> bool + Send + Sync>
        },
        {
            let s = Arc::clone(&settings);
            Arc::new(move || {
                let m = s.memory();
                (m.instruction_global_kib, m.instruction_project_kib)
            }) as Arc<dyn Fn() -> (u32, u32) + Send + Sync>
        },
    ));
    ctx.events.on_waterfall(
        points::REQUEST,
        Arc::new(kernel_agent::InstructionFilesInject::new(
            instruction_source as Arc<dyn kernel_agent::InstructionSource>,
        )),
    );

    // 尾部碎片（§4.4）：pending 表 → 轮开场拼 `<system-reminder>` 在最后
    // 一条 user 消息尾（出现一次即清）——整理完成/Plan 模式/轮次到顶
    // 不再对模型静默。
    ctx.events.on_waterfall(
        points::REQUEST,
        Arc::new(kernel_agent::FragmentInject::new(
            Arc::clone(&fragments) as Arc<dyn kernel_agent::FragmentSource>
        )),
    );

    // 微压缩（S2 刀③ · specs/09）：把很旧的可再生工具结果清成一行占位，**先于**
    // 主层 LLM 压缩触发。它吃的"主层阈值"必须与整理触发线/进度条分母是同一把尺，
    // 否则减速层就不是"先于主层"而是"任意时候"——所以这里只做取数，算式仍然
    // 只有 `context_budget::effective_threshold` 一个出处（与 chat.rs /
    // organize::budget_threshold / scheduler/engine.rs 三个既有消费点同形）。
    //
    // `window == None` ⇒ 返回 `None` = **没有可信的线**：token 压力通路整条关掉，
    // 只留 idle 那条（2026-10-19 裁定 A 在机械层的同一条姿态——不拿假定的窗口算线）。
    let main_threshold: crate::microcompact::MainThreshold = {
        let settings = Arc::clone(&settings);
        let catalog = Arc::clone(&catalog);
        let overrides = Arc::clone(&model_overrides);
        Arc::new(move |sid: Option<&str>| {
            let ov = sid.and_then(|s| overrides.get(s));
            let (window, max_output) = settings.with_llm_config(|llm| {
                crate::settings::session_capacity(llm, ov.as_ref(), Some(&catalog))
            });
            window.map(|w| {
                crate::context_budget::effective_threshold(
                    settings.organize_threshold_tokens(),
                    Some(w),
                    max_output,
                )
            })
        })
    };
    ctx.events.on_waterfall(
        points::REQUEST,
        Arc::new(crate::microcompact::MicroCompact::new(
            Arc::clone(&settings),
            sessions_dir,
            main_threshold,
        )) as Arc<_>,
    );

    Stack {
        exposure_cfg,
        exposure,
        memory_cfg,
        fragments,
    }
}
