//! 阶段三验收叙事（DESIGN.md §14）：加载 0.1.0 → 激活 0.2.0 →
//! 回退 0.1.0，全程不重启。经 Kernel 主循环驱动 FFI 工具，
//! 会话事件先写日志再执行（纪律 5）。
//!
//! 运行前置：`bash scripts/build_plugins.sh`（把双版本产物暂存到
//! plugins/tool-echo/{0.1.0,0.2.0}/）。

use std::path::Path;
use std::sync::Arc;

use cordis_core::Context;
use kernel_agent::{Kernel, StepOutcome, ToolSource};
use plugin_host::{PluginRegistry, RegistryToolSource};
use semver::Version;
use session_store::{Session, SessionEvent};

use crate::echo_llm::ScriptedLlm;
use crate::tool_source::KernelToolSource;

const PLUGIN_ID: &str = "tool-echo";
const V12: &str = "0.1.0";
const V13: &str = "0.2.0";

/// 依赖方向总览（§8）：本函数是全仓库唯一同时触碰 kernel-agent 与
/// plugin-host 的地方（组装点），其余代码各守各的边界。
pub async fn run(plugins_root: &Path) -> Result<(), String> {
    let ctx = Arc::new(Context::new());
    let registry = Arc::new(PluginRegistry::new());

    // 工具目录与 LLM 都不特殊，只是经 trait 对象键注册的服务（§7.2）
    ctx.provide_dyn::<dyn ToolSource>(Arc::new(KernelToolSource::new(RegistryToolSource::new(
        Arc::clone(&registry),
    ))));
    ctx.provide_dyn::<dyn kernel_agent::LlmService>(Arc::new(ScriptedLlm::new()));

    let kernel = Kernel::new(Arc::clone(&ctx), vec![]);
    let mut session = Session::new();
    session.append(SessionEvent::UserMessage {
        content: "你好，请演示 echo 工具。".into(),
    });

    let v12: Version = V12.parse().unwrap();
    let v13: Version = V13.parse().unwrap();

    // ---- 1) 加载 0.1.0 并激活 ----
    load_and_activate(&registry, PLUGIN_ID, &v12, plugins_root)?;
    banner("phase A: active = 0.1.0");
    step(&kernel, &mut session).await?;

    // ---- 2) 加载 0.2.0 并激活（多版本并存，切换 = 换指针，微秒级）----
    load_and_activate(&registry, PLUGIN_ID, &v13, plugins_root)?;
    banner("phase B: active = 0.2.0（升级，旧版本保留）");
    step(&kernel, &mut session).await?;

    // ---- 3) 回退 0.1.0（不重启，reverse 应立刻不可用）----
    let rolled = registry
        .rollback(PLUGIN_ID)
        .map_err(|e| format!("rollback: {e}"))?;
    assert_eq!(rolled, v12, "rollback must return to 0.1.0");
    banner("phase C: rollback -> 0.1.0（不重启）");
    step(&kernel, &mut session).await?;

    // ---- 收尾：注册表状态 + 会话事件全量打印 ----
    println!("\nregistry states:");
    for s in registry.list_states() {
        println!(
            "  {} active={:?} loaded={:?}",
            s.id,
            s.active.as_ref().map(Version::to_string),
            s.loaded.iter().map(Version::to_string).collect::<Vec<_>>()
        );
    }
    print_session(&session);
    Ok(())
}

/// 打印会话事件全量日志（demo 与 demo4 共用）。
pub(crate) fn print_session(session: &Session) {
    println!("\nsession log ({} events):", session.len());
    for (seq, e) in session.events().iter().enumerate() {
        println!(
            "  [{seq}] {}",
            serde_json::to_string(e).expect("session event serializes")
        );
    }
}

async fn step(kernel: &Kernel, session: &mut Session) -> Result<(), String> {
    match kernel.step(session).await {
        Ok(StepOutcome::Continue { .. }) => Ok(()),
        Err(e) => Err(format!("kernel step: {e}")),
    }
}

pub(crate) fn load_and_activate(
    registry: &PluginRegistry,
    id: &str,
    ver: &Version,
    plugins_root: &Path,
) -> Result<(), String> {
    let path = plugins_root.join(id).join(ver.to_string()).join(dll_name());
    if !path.exists() {
        return Err(format!(
            "plugin artifact missing: {}\nrun `bash scripts/build_plugins.sh` first",
            path.display()
        ));
    }
    registry
        .load(id, ver, &path)
        .and_then(|_| registry.activate(id, ver))
        .map_err(|e| format!("load/activate {id}@{ver}: {e}"))
}

pub(crate) fn dll_name() -> &'static str {
    if cfg!(windows) {
        "tool_echo.dll"
    } else if cfg!(target_os = "macos") {
        "libtool_echo.dylib"
    } else {
        "libtool_echo.so"
    }
}

fn banner(title: &str) {
    println!("\n===== {title} =====");
}
