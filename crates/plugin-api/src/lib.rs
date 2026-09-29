//! 冻结的插件 ABI 契约层（DESIGN.md §5）。
//!
//! 纪律：本 crate 一旦对外发布，任何改动都会破坏已编译插件。
//! 只暴露 C ABI + JSON 边界，不暴露 Rust 类型；加字段只能追加到结构体末尾。
//! 计划内容：`HostVtable`、`DescribeFn`/`InitFn`/`CallFn`/`ShutdownFn`/`FreeBufFn`
//! 符号约定、`PluginManifest` / `ToolSpec` / `SideEffect` 元数据契约。

/// 当前插件 ABI 版本。宿主加载插件时必须严格校验，不一致即拒绝加载。
pub const PLUGIN_ABI_VERSION: u32 = 1;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abi_version_is_frozen_at_1() {
        assert_eq!(PLUGIN_ABI_VERSION, 1);
    }
}
