//! 命令行解析与运行时启动。手写解析即可（子命令只有 demo）。

use std::process::ExitCode;

pub fn run() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("demo") => {
            let plugins_root = args
                .iter()
                .position(|a| a == "--plugins-root")
                .and_then(|i| args.get(i + 1))
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| std::path::PathBuf::from("plugins"));
            tracing_subscriber::fmt()
                .with_target(true)
                .with_max_level(tracing::Level::INFO)
                .init();
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("tokio runtime");
            match runtime.block_on(crate::demo::run(&plugins_root)) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("demo failed: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        other => {
            eprintln!(
                "Locowrk CLI — usage:\n  \
                 locowrk demo [--plugins-root DIR]  # 阶段三验收：加载 1.2.0 → 激活 1.3.0 → 回退 1.2.0"
            );
            match other {
                Some(unknown) => {
                    eprintln!("unknown command: {unknown}");
                    ExitCode::FAILURE
                }
                None => ExitCode::SUCCESS,
            }
        }
    }
}
