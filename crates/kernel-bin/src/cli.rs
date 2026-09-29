//! 命令行解析与运行时启动。手写解析即可（子命令 demo / demo4）。

use std::path::PathBuf;
use std::process::ExitCode;

pub fn run() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let subcommand = match args.first().map(String::as_str) {
        Some(sub @ ("demo" | "demo4")) => sub,
        other => {
            eprintln!(
                "Locowrk CLI — usage:\n  \
                 locowrk demo  [--plugins-root DIR]  # 阶段三验收：加载 1.2.0 → 激活 1.3.0 → 回退 1.2.0\n  \
                 locowrk demo4 [--plugins-root DIR]  # 阶段四验收：审批插件改写参数；两 Agent 工具视图隔离"
            );
            return match other {
                Some(unknown) => {
                    eprintln!("unknown command: {unknown}");
                    ExitCode::FAILURE
                }
                None => ExitCode::SUCCESS,
            };
        }
    };

    let plugins_root = args
        .iter()
        .position(|a| a == "--plugins-root")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("plugins"));

    tracing_subscriber::fmt()
        .with_target(true)
        .with_max_level(tracing::Level::INFO)
        .init();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");

    let result = runtime.block_on(async {
        if subcommand == "demo" {
            crate::demo::run(&plugins_root).await
        } else {
            crate::demo4::run(&plugins_root).await
        }
    });

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{subcommand} failed: {e}");
            ExitCode::FAILURE
        }
    }
}
