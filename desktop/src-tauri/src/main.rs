//! 可执行入口：只调用库入口，不写逻辑（AGENTS.md 文件规模纪律）。

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    locowrk_desktop_lib::run()
}
