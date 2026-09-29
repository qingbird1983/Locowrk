//! CLI 可执行入口：组装全部 crate（DESIGN.md §8）。
//!
//! 阶段三职责：把 plugin-host 的注册表适配成 kernel-agent 的工具目录
//! 服务注册进 Context，并以脚本化 LLM 跑通"C ABI 插件热插拔"验收叙事。
//! 阶段七补：profile.toml 加载、PluginService 安装态目录与会话持久化。

mod cli;
mod demo;
mod echo_llm;
mod tool_source;

fn main() {
    cli::run();
}
