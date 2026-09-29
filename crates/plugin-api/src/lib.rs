//! 冻结的插件 ABI 契约层（DESIGN.md §5）。
//!
//! 纪律：本 crate 一旦对外发布，任何改动都会破坏已编译插件；
//! 加字段只能追加到结构体末尾，`PLUGIN_ABI_VERSION` 变更即 breaking。
//!
//! 阶段一只落地宿主侧类型（`tool` 模块）：它们只活在宿主进程内，
//! 绝不进入 C ABI（§5.2）。阶段三补 FFI 边界：`HostVtable`、五个导出符号
//! 约定（DescribeFn / InitFn / CallFn / ShutdownFn / FreeBufFn）、
//! `PluginManifest` 元数据契约与 `declare_plugin!` 宏。

pub mod tool;

pub use tool::{CallCtx, CallId, SideEffect, Tool, ToolError, ToolSpec};

/// 当前插件 ABI 版本。宿主加载插件时必须严格校验，不一致即拒绝加载；
/// 唯一例外是"加载即失败"（init 从未执行、无代码指针流出）可立即释放句柄（§5.7）。
pub const PLUGIN_ABI_VERSION: u32 = 1;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abi_version_is_frozen_at_1() {
        assert_eq!(PLUGIN_ABI_VERSION, 1);
    }
}
