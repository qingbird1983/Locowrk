//! CLI 可执行入口：组装全部 crate（DESIGN.md §8）。
//!
//! 阶段三职责：把 plugin-host 的注册表适配成 kernel-agent 的工具目录
//! 服务注册进 Context，并以脚本化 LLM 跑通"C ABI 插件热插拔"验收叙事。
//! 阶段四职责：`demo4` 跑通"waterfall 插桩 + Scope 隔离"验收叙事
//! （审批插件改写参数；两 Agent 工具视图隔离）。
//! 阶段七补：会话持久化（session-store）与插件脚手架生成（scaffold）。
//! 注：曾设想过一个 `config/profile.toml`「按 profile 声明加载插件」的入口，
//! 加载器**从未实现**（审计《CODE-AUDIT-v1》H4），文件与设计描述已在 v2.148
//! 批 D 一并移除；开发态装载走 plugin-host 的 dev_scan 扫描 `plugins/`，安装
//! 态归 plugin-service crate + lockfile，本二进制不读任何 profile 文件。

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
