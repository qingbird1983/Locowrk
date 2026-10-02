//! 会话落盘接线（⑲，DESIGN.md §4 落盘布局）：session_store::persist 在
//! 组装点侧的薄封装。磁盘 `<数据根>/sessions/<id>.jsonl` 是内存日志的
//! **镜像**——runner 每步把新增段追加进去，undo fork 截断后整体重写
//! （滚动备份留痕），启动时逐文件回放恢复内存会话表。
//!
//! 纪律对齐：日志只追加不删除（§4.3）在磁盘面的例外只有 undo 重写，
//! 且重写前必有 `.undo.bak` 备份——撤销的物理留痕。

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use session_store::Session;
use session_store::{
    append_event, delete_session, list_sessions, load_session, rewrite_log, search_sessions,
    session_file, write_title, SessionMeta, SessionSearchHit,
};

type SessionsMap = Arc<Mutex<HashMap<String, Arc<tokio::sync::Mutex<Session>>>>>;

/// 启动恢复：逐个会话文件回放进内存表（坏文件/空文件跳过不拦启动）。
/// 返回恢复的会话数。必须在调度引擎等会话消费方启动之前调用。
pub fn restore_all(dir: &Path, sessions: &SessionsMap) -> usize {
    let mut restored = 0usize;
    for meta in list_sessions(dir) {
        let path = match session_file(dir, &meta.id) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(id = %meta.id, "session log 路径非法: {e}");
                continue;
            }
        };
        match load_session(&path) {
            Ok((s, skipped)) => {
                if skipped > 0 {
                    tracing::warn!(id = %meta.id, skipped, "session log 坏行跳过");
                }
                if s.is_empty() {
                    continue;
                }
                sessions
                    .lock()
                    .expect("sessions poisoned")
                    .insert(meta.id, Arc::new(tokio::sync::Mutex::new(s)));
                restored += 1;
            }
            Err(e) => tracing::warn!(id = %meta.id, "session log 读取失败: {e}"),
        }
    }
    restored
}

/// 把日志新增段 `[from..]` 追加进磁盘镜像。失败只告警不阻断聊天——
/// 落盘是持久性保障，不该反过来卡死会话（下次追加按同一 from 语义补齐：
/// 调用方传"投影起点"，与投影共用同一游标）。
pub fn append_slice(dir: &Path, session_id: &str, session: &Session, from: usize) {
    let Ok(path) = session_file(dir, session_id) else {
        tracing::warn!(session_id, "session log 文件名非法，本段不落盘");
        return;
    };
    for e in &session.events()[from..] {
        if let Err(err) = append_event(&path, e) {
            tracing::warn!(session_id, "session log 追加失败: {err}");
            return;
        }
    }
}

/// undo fork 截断后同步磁盘（重写 + 滚动备份）。会话尚无文件 = 无需重写。
pub fn rewrite_after_undo(dir: &Path, session_id: &str, session: &Session) {
    let Ok(path) = session_file(dir, session_id) else {
        return;
    };
    if !path.exists() {
        return;
    }
    if let Err(e) = rewrite_log(&path, session) {
        tracing::warn!(session_id, "session log undo 重写失败: {e}");
    }
}

/// 列表（mtime 降序）：直接透出 SessionMeta（Serialize）。
pub fn list(dir: &Path) -> Vec<SessionMeta> {
    list_sessions(dir)
}

/// 重命名（v2.49 会话运营）：写旁车 meta.json 标题（trim 后空 = 回落
/// 自动标题）。内存会话不动——标题只在列表层生效。
pub fn rename(dir: &Path, session_id: &str, title: &str) -> std::io::Result<()> {
    write_title(dir, session_id, title)
}

/// 删除（v2.49 会话运营）：日志 + 旁车 + undo 备份一并移除（幂等）。
/// 内存条目与调度最近目标的剔除在命令层做（要碰 AppState）。
pub fn delete(dir: &Path, session_id: &str) -> std::io::Result<bool> {
    delete_session(dir, session_id)
}

/// 正文搜索（v2.49 会话搜索）：UserMessage / AssistantMessage 大小写
/// 不敏感子串扫描，mtime 降序限量。
pub fn search(dir: &Path, query: &str, limit: usize) -> Vec<SessionSearchHit> {
    search_sessions(dir, query, limit)
}
