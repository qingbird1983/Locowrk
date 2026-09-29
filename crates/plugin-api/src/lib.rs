//! 冻结的插件 ABI 契约层（DESIGN.md §5）。
//!
//! 纪律：本 crate 一旦对外发布，任何改动都会破坏已编译插件；
//! 加字段只能追加到结构体末尾，`PLUGIN_ABI_VERSION` 变更即 breaking。
//!
//! 模块：
//! - `ffi`：C ABI 边界——`HostVtable`、五个导出符号约定
//!   （DescribeFn / InitFn / CallFn / ShutdownFn / FreeBufFn）与返回码。
//! - `manifest`：`PluginManifest` 元数据契约（describe 的 JSON 形态）。
//! - `plugin_side`：Rust 插件便利层（`PluginState` + `declare_plugin!`），
//!   编译成 C 符号，本身不进 ABI。
//! - `tool`：宿主侧类型（`Tool` / `CallCtx` 等），只活在宿主进程内，
//!   绝不进入 C ABI（§5.2）。
//! - `fsutil`：文件类工具的安全基座（允许根 canonicalize 校验 + 写前
//!   备份），Rust 便利层，不进 ABI 面（阶段八⑥第二批，安全底线）。
//!
//! 另外把 `serde_json` / `semver` 原样 re-export 给插件 crate 复用，
//! 让插件只依赖本 crate 一个依赖即可（§5.3"Rust 是一等公民"）。

pub mod ffi;
pub mod fsutil;
pub mod manifest;
pub mod plugin_side;
pub mod tool;

pub use ffi::{
    CallFn, DescribeFn, FreeBufFn, HostVtable, InitFn, SYM_CALL, SYM_DESCRIBE, SYM_FREE, SYM_INIT,
    SYM_SHUTDOWN, ShutdownFn,
};
pub use manifest::{
    Author, LocalizedOrString, LocalizedString, ManifestError, PluginKind, PluginManifest,
    PluginOrigin,
};
pub use plugin_side::{PluginState, free_allocated, write_out};
pub use semver;
pub use serde_json;
pub use tool::{CallCtx, CallId, SideEffect, Tool, ToolError, ToolSpec};

/// 当前插件 ABI 版本。宿主加载插件时必须严格校验，不一致即拒绝加载；
/// 唯一例外是"加载即失败"（init 从未执行、无代码指针流出）可立即释放句柄（§5.7）。
pub const PLUGIN_ABI_VERSION: u32 = 1;

#[cfg(test)]
mod tests {
    use super::*;

    /// docs/design/Plug-in packaging.md §2.1 完整示例（manifest 测试共用；
    /// 含工具级 display_name 与 manifest 级 reversible 等被忽略的未知字段）。
    pub const DOC_EXAMPLE: &str = r#"{
      "id": "tool-shout",
      "version": "1.1.0",
      "abi_version": 1,
      "kind": "tool",
      "display_name": { "zh-CN": "大小写转换", "en-US": "Shout", "ja-JP": "シャウト" },
      "description": { "zh-CN": "把文本转换成大写输出", "en-US": "Shouts text back in uppercase" },
      "icon": "assets/icon.svg",
      "author": { "name": "Locowrk Team", "url": "https://github.com/locowrk" },
      "origin": "official",
      "homepage": "https://github.com/locowrk/plugins/tool-shout",
      "license": "MIT",
      "tools": [{
        "name": "shout",
        "display_name": { "zh-CN": "转换大写", "en-US": "Shout" },
        "description": { "zh-CN": "把输入的文本全部转成大写", "en-US": "Convert the input text to uppercase" },
        "schema": { "type": "object", "properties": { "text": { "type": "string" } }, "required": ["text"] },
        "side_effect": "Pure"
      }],
      "reversible": { "kind": "Pure" }
    }"#;

    #[test]
    fn abi_version_is_frozen_at_1() {
        assert_eq!(PLUGIN_ABI_VERSION, 1);
    }
}
