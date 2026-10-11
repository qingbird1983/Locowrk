//! 组装点·LLM 栈簇（DESIGN §7.2「模型适配器也只是服务」+ LLM.md v3.0 单线协议）：
//! 停止标记 → 重试通知闭包 → 会话级模型覆盖 → 前缀诊断账本 → 能力目录 →
//! 资产存储 → `AppLlm` 组装 → `LlmService` 与 `LlmStreamSink` 注册进 `Context`。
//!
//! 语句自 `state/mod.rs` 的 `init_state` **原样搬出**（批 F-3b·F5 结构性拆分），
//! 一条未重排。次序契约（错位即谎报能力或丢流式）：
//! 1. `stop_flags` 必须先建——LLM.md v3.0 的重试面与 delta 回调共读同一柄
//!    （v4.11 §3.39④：重试等待轮询停止、流式逐段查停止标记）；
//! 2. 重试通知闭包在组装点吃掉 `AppHandle`，`llm` 模块本身不引 Tauri（v2.66 同款
//!    分层：能力模块零 UI 依赖，桌面单测才不会链接 tao）；
//! 3. `catalog` 先于 `AppLlm`（批 D：发送前模态门控要读目录模态数据源）。
//!
//! `AppLlm` 保具体类型句柄（批 F）：子代理模型面工厂与全局服务共用同一实例。

use std::path::PathBuf;
use std::sync::Arc;

use cordis_core::Context;
use kernel_agent::LlmService;
use tauri::{AppHandle, Manager};

use crate::llm::AppLlm;
use crate::settings::SettingsStore;

pub(crate) struct Init {
    pub app: AppHandle,
    pub ctx: Arc<Context>,
    pub settings: Arc<SettingsStore>,
    pub sessions_dir: PathBuf,
    pub root: PathBuf,
}

/// 交给下游簇与 `AppState` 的 LLM 栈句柄（字段名 = 原 `init_state` 里的变量名，
/// 调用点按名解构，后续语句因此可以逐字不动）。
pub(crate) struct Stack {
    pub stop_flags: Arc<crate::stop::StopFlags>,
    pub model_overrides: Arc<crate::model_override::ModelOverrideStore>,
    pub diagnoser: Arc<crate::llm::diagnose::Diagnoser>,
    pub catalog: Arc<crate::llm::catalog::CatalogStore>,
    pub assets: Arc<crate::assets::AssetStore>,
    pub app_llm: Arc<AppLlm>,
    pub llm: Arc<dyn LlmService>,
}

pub(crate) fn build(init: Init) -> Stack {
    let Init {
        app,
        ctx,
        settings,
        sessions_dir,
        root,
    } = init;

    // v2.58 会话级模型覆盖存储（旁车持久 + 缓存；AppLlm 每请求按 LlmRequest.session_id
    // 解析覆盖——档位路由 + 模型名）。模型适配器也只是服务（§7.2，自建
    // OpenAI 兼容客户端 LLM.md v3.0；未配置回落演示替身）。
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
    // M6.1 前缀断裂诊断账本：AppLlm 与命令层共享同一实例。
    let llm_diagnoser = Arc::new(crate::llm::diagnose::Diagnoser::default());
    // 批 A 模型能力目录：随包快照路径解析（打包 = 资源目录；开发态 =
    // crate 根兜底），惰性加载在首次访问（不进启动路径）。批 D 起先于
    // AppLlm 构造（发送前模态门控的目录模态数据源）。
    let catalog_bundled = app
        .path()
        .resolve(
            "resources/models.dev/api.json",
            tauri::path::BaseDirectory::Resource,
        )
        .ok()
        .filter(|p| p.exists())
        .unwrap_or_else(|| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/models.dev/api.json")
        });
    let catalog = Arc::new(crate::llm::catalog::CatalogStore::new(catalog_bundled));
    // 批 D 资产存储：`<数据根>/assets/` 内容寻址（wire 编码字节源 + GC 根）
    let assets = Arc::new(crate::assets::AssetStore::new(root.join("assets")));
    // 批 F：AppLlm 保具体类型句柄——子代理模型面工厂（SubagentLlm 内层）
    // 与全局服务共用同一实例。
    let app_llm = Arc::new(
        AppLlm::with_overrides(Arc::clone(&settings), Arc::clone(&model_overrides))
            .with_retry_surface(Arc::clone(&stop_flags), llm_notify)
            .with_diagnoser(Arc::clone(&llm_diagnoser))
            .with_multimodal(Arc::clone(&assets), Arc::clone(&catalog)),
    );
    let llm: Arc<dyn LlmService> = Arc::clone(&app_llm) as Arc<dyn LlmService>;
    ctx.provide_dyn::<dyn LlmService>(Arc::clone(&llm));
    // ⑳ 文本 delta 直推（流式步骤整段投影被抑制，§12.2）；v4.11 §3.39④
    // 停止标记注册表同柄——delta 回调点逐段查询，命中即带部分正文收束。
    ctx.provide_dyn::<dyn kernel_agent::LlmStreamSink>(Arc::new(
        crate::events::StreamChunkSink::new(app.clone(), Arc::clone(&stop_flags)),
    ));

    Stack {
        stop_flags,
        model_overrides,
        diagnoser: llm_diagnoser,
        catalog,
        assets,
        app_llm,
        llm,
    }
}
