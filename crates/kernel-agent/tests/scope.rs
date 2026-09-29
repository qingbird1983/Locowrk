//! 阶段四验收：Scope 隔离（§3.4 / §14）——两 Agent 工具视图隔离，
//! scoped 层遮蔽 global、未遮蔽的服务回退 global。

use std::sync::Arc;

use cordis_core::{Context, ScopeKey};
use kernel_agent::{LlmService, ToolSource};
use plugin_api::SideEffect;
use serde_json::json;
use session_store::SessionEvent;

mod common;

use common::{MockSource, MockTool, SeqLlm, kernel, user_session};

#[tokio::test]
async fn scoped_tool_views_are_isolated() {
    let ctx = Arc::new(Context::new());
    // 全局视图：plugin-echo（模拟 FFI 插件工具）
    let global_echo = Arc::new(MockTool::new(
        "echo",
        SideEffect::Pure,
        json!("global-echo"),
    ));
    ctx.provide_dyn::<dyn ToolSource>(Arc::new(MockSource::new([Arc::clone(&global_echo)])));
    let global_llm = Arc::new(SeqLlm::new(vec![SeqLlm::call_tool(1, "echo")]));
    ctx.provide_dyn::<dyn LlmService>(Arc::clone(&global_llm) as Arc<dyn LlmService>);

    // 作用域 B 视图：同名工具指向另一个实现（进程内 stub）
    let scope_b = ScopeKey::new();
    let stub_echo = Arc::new(MockTool::new(
        "echo",
        SideEffect::Pure,
        json!("scoped-echo"),
    ));
    ctx.provide_dyn_scoped::<dyn ToolSource>(
        &scope_b,
        Arc::new(MockSource::new([Arc::clone(&stub_echo)])),
    );
    let scoped_llm = Arc::new(SeqLlm::new(vec![SeqLlm::call_tool(1, "echo")]));
    ctx.provide_dyn_scoped::<dyn LlmService>(
        &scope_b,
        Arc::clone(&scoped_llm) as Arc<dyn LlmService>,
    );

    // 全局 Agent：解析到全局 echo
    let mut s1 = user_session();
    kernel(&ctx).step(&mut s1).await.unwrap();
    assert_eq!(global_echo.calls(), 1);
    assert_eq!(stub_echo.calls(), 0);
    assert!(matches!(
        s1.events().last(),
        Some(SessionEvent::ToolResult { output, .. }) if output == &json!("global-echo")
    ));

    // 作用域 Agent：scoped 层遮蔽全局，解析到 stub
    let mut s2 = user_session();
    kernel(&ctx).scoped(scope_b).step(&mut s2).await.unwrap();
    assert_eq!(stub_echo.calls(), 1, "作用域 B 用自己的工具视图");
    assert_eq!(
        global_llm.requests.lock().unwrap().len(),
        1,
        "B 也用自己的 LLM"
    );
    assert!(matches!(
        s2.events().last(),
        Some(SessionEvent::ToolResult { output, .. }) if output == &json!("scoped-echo")
    ));
}
