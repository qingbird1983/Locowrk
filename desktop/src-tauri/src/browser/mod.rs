//! 工作面板浏览器（UI shell 文档 §2.5 浏览器页签，v2.144）：模型交付可
//! 运行成品的正门——`browser_open` 开页签（lwpreview 协议喂 fsRoots 内
//! 文件 / http(s) 直挂）、`browser_console` 读回页面报错（验证环）。
//! 渲染件 = 前端 iframe（sandbox 围栏 + 自管历史），本模块只管服务端：
//! preview（协议）/ bridge（console 桥注入）/ console（报错账本）/
//! tools（模型面）。

pub mod bridge;
pub mod console;
pub mod preview;
pub mod tools;

pub use console::{ConsoleReport, ConsoleStore};
pub use tools::{BrowserConsoleTool, BrowserOpenTool, EmitFn};
