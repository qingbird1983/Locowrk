//! 子会话转录落盘（UI shell 文档 §3.16，v2.52）：`SubTranscriptSink` 的
//! 桌面实现——`run_child` 收束时把子**自身段**写到
//! `<数据根>/sub-transcripts/<child_id>.jsonl`（一行一事件，同 sessions/
//! 镜像格式）。不进内存会话表：`restore_all` 只扫 `sessions/`，子转录
//! 不会变成侧栏里的独立会话；读取走 `subagent_transcript` 命令（load +
//! 与 `session_history` 同一 `project_events` 全量投影）。

use std::path::PathBuf;

use kernel_agent::SubTranscriptSink;
use session_store::{append_event, session_file, Session};

pub struct SubTranscriptStore {
    dir: PathBuf,
}

impl SubTranscriptStore {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }
}

impl SubTranscriptSink for SubTranscriptStore {
    fn close(&self, child_session_id: &str, own_from: usize, child: &Session) {
        // id 走与 sessions/ 同一白名单校验（防穿越）；fork 早退路径
        // child_session_id 为空 → 非法 id，如实跳过不落盘。
        let Ok(path) = session_file(&self.dir, child_session_id) else {
            tracing::warn!("sub transcript 非法 id，跳过: {child_session_id}");
            return;
        };
        // 防御：同 id 重落先清旧（gate 计数保证进程内 id 唯一，正常不触发）
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                tracing::warn!("sub transcript 旧文件清理失败: {e}");
                return;
            }
        }
        for event in &child.events()[own_from..] {
            if let Err(e) = append_event(&path, event) {
                tracing::warn!(child = %child_session_id, "sub transcript 写入失败: {e}");
                return;
            }
        }
    }
}

#[cfg(test)]
#[path = "sub_transcript/tests.rs"]
mod tests;
