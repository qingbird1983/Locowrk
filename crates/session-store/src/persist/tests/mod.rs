//! persist 模块测试：往返 / undo 截断重写 + 备份留痕 / 坏行容忍 / 列表。
//! 批 F-23 目录化（原 `tests.rs` 293 行按面分三簇，**用例本体逐字未改**）：共享夹具
//! 留本文件，子簇 `use super::*` 取用——
//! · `roundtrip` id 校验 / 追加-恢复往返 / undo 截断重写与备份 / 坏行容忍
//! · `listing`   列表次序 / 旁车标题·工作区·退役键 / 删除会话
//! · `search`    正文搜索 + 批 F-23（审计 M-24d）单遍 IO 重构的等价证明
//!
//! 为什么这一刀落在性能批里：M-24d 的等价证明要把 HEAD 的 `list_sessions` 与
//! `search_sessions` 两份原文抄成参考实现（约 130 行），293 行的 `tests.rs` 加不下
//! 400 线 ⇒ 按 F-8/F-16 的既有形状目录化（超限件 22 持平，不靠抬基线过关）。

mod listing;
mod roundtrip;
mod search;

use std::path::PathBuf;

use super::*;
use crate::event::SessionEvent;

fn tmp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("locowrk-persist-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn user_msg(content: &str) -> SessionEvent {
    SessionEvent::UserMessage {
        content: content.into(),
        parts: Vec::new(),
    }
}
