//! 线上协议（OpenAI Chat Completions，LLM.md v3.0）：请求装配 / SSE chunk
//! 解析 / 用量映射，纯函数 + 流式累积器。**唯一线路**——非流式 `complete`
//! 也走这条管道聚合（无第二形状）。
//!
//! 台账第三节 · 第一批第四刀（548 → 本文件只留声明）：四片按线路上的位置沉进 `wire/`，
//! 靠 `pub use X::*` 保持公开面与此前逐字相同——`app_llm.rs` 与 `client.rs` 的
//! `use super::wire::{…}`、`llm/tests/assets.rs` 的 `super::wire::project_tool_assets`
//! 一个字都不改。四片 = `payload`（请求装配 + 能力上限 + Q8 截断）·
//! `media`（发送前门控与工具结果图片投影）· `messages`（中立消息 → 线上消息）·
//! `stream`（流式累积器 + 用量映射）。
//! 两片之间唯一的跨片引用是 `build_payload ⇄ message_value` 与 `message_value ⇄ clip_tool_output`
//! 两条（共 2 处可见性抬升，枚举在 spec 的 edits 里，判据吃"施加后的基线"）。
//! 判定表列的是五簇（预检 / 媒体投影 / 序列化 / 累积器 / usage），落地四片：usage 只有
//! `parse_usage` 一个函数、只被累积器调用，单立一片 = 为行数造结构 ⇒ 并进 `stream`。

mod media;
mod messages;
mod payload;
mod stream;

pub use media::*;
pub use payload::*;
pub use stream::*;
