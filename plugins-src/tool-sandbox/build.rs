//! 构建脚本：`LOCOWRK_PLUGIN_VERSION` 决定编译进 manifest 的插件版本。
//! 未设置时回落到 "0.0.0-dev"（只适合 `cargo check`，加载会被
//! registry 的版本一致性校验拒绝）。

fn main() {
    println!("cargo:rerun-if-env-changed=LOCOWRK_PLUGIN_VERSION");
}
