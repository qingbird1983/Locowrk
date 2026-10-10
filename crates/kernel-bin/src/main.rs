//! CLI 可执行入口：组装全部 crate（DESIGN.md §8）。
//!
//! 阶段三职责：把 plugin-host 的注册表适配成 kernel-agent 的工具目录
//! 服务注册进 Context，并以脚本化 LLM 跑通"C ABI 插件热插拔"验收叙事。
//! 阶段四职责：`demo4` 跑通"waterfall 插桩 + Scope 隔离"验收叙事
//! （审批插件改写参数；两 Agent 工具视图隔离）。
//! 阶段七补：会话持久化（session-store）与插件脚手架生成（scaffold）。
//! 注：`config/profile.toml` **从未有加载器实现**（审计报告 H4），文件与
//! README/DESIGN 的「已接线」描述待批 D 一并清理；插件安装态归
//! plugin-service crate，本二进制只走 plugin-host 注册表。

mod approval_plugin;
mod cli;
mod demo;
mod demo4;
mod demo4_llm;
mod echo_llm;
mod inline_tools;
mod scaffold;
mod tool_source;

fn main() {
    cli::run();
}
