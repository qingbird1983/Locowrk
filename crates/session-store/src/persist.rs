//! 会话日志落盘（DESIGN.md §4 落盘布局，⑲）：JSONL 一行一个
//! `SessionEvent`，一个文件 = 一个会话。磁盘文件是内存日志的**镜像**——
//! 追加走 [`append_event`]，undo fork 截断走 [`rewrite_log`]（改写前滚动
//! 备份，"永不删除"在磁盘面以备份留痕），恢复 = [`load_session`] 逐行
//! 回放。挂到数据根 `sessions/` 目录由组装点决定（本 crate 只管文件面）。
//!
//! 时点语义：`Session::append` 在游标于末尾时随追加前移、已回退时不动
//! （§4.2）——落盘侧镜像"当前活跃日志"，因此**调用方须在 undo fork 之后
//! 用 rewrite 同步磁盘**，否则恢复会复活已撤销的事件。

use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::StoreError;
use crate::event::SessionEvent;
use crate::session::Session;

/// undo 截断重写前的滚动备份后缀（`<file>.undo.bak`，每次覆盖）。
const UNDO_BACKUP_SUFFIX: &str = "undo.bak";

/// 会话 id 允许字符：ASCII 字母数字与 `.` `_` `-`——前端生成
/// `desktop-<毫秒>`，第三方形态留同族；拒绝路径分隔符防穿越。
fn valid_session_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// 会话 id → JSONL 文件路径（`<dir>/<id>.jsonl`）。id 非法 = [`StoreError::Invalid`]。
pub fn session_file(dir: &Path, session_id: &str) -> Result<PathBuf, StoreError> {
    if !valid_session_id(session_id) {
        return Err(StoreError::Invalid(format!("非法会话 id: {session_id}")));
    }
    Ok(dir.join(format!("{session_id}.jsonl")))
}

/// 追加一行事件（打开-写-关，同 audit.jsonl 模式；聊天量级开销可忽略，
/// 且进程崩溃最多丢最后一行）。目录不存在则创建。
pub fn append_event(path: &Path, event: &SessionEvent) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(
        f,
        "{}",
        serde_json::to_string(event).expect("event serialize")
    )
}

/// 全量重写（undo fork 截断后同步磁盘）：先把原文件滚动备份为
/// `<file>.undo.bak`（每次覆盖，只留最近一次撤销前的完整原文），再写入
/// 截断后的日志。文件不存在时直接写。
pub fn rewrite_log(path: &Path, session: &Session) -> std::io::Result<()> {
    if path.exists() {
        let mut bak = path.as_os_str().to_os_string();
        bak.push(".");
        bak.push(UNDO_BACKUP_SUFFIX);
        std::fs::copy(path, PathBuf::from(bak))?;
    }
    let mut out = String::new();
    for e in session.events() {
        out.push_str(&serde_json::to_string(e).expect("event serialize"));
        out.push('\n');
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, out)
}

/// 恢复：逐行 parse → append（游标随之走到末尾 = 停机时状态）。坏行
/// （崩溃留下的半行 / 手改）跳过计数不致命——前缀事件仍然完整可用。
/// 返回（会话, 跳过行数）。
pub fn load_session(path: &Path) -> std::io::Result<(Session, usize)> {
    use std::io::BufRead;
    let file = std::fs::File::open(path)?;
    let mut session = Session::new();
    let mut skipped = 0usize;
    for line in std::io::BufReader::new(file).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<SessionEvent>(&line) {
            Ok(e) => {
                session.append(e);
            }
            Err(_) => skipped += 1,
        }
    }
    Ok((session, skipped))
}

/// 会话文件元数据（列表用；`updated_at` = 文件 mtime 的 Unix 秒；
/// `title` = 首条用户消息截断（切换器选项文案，None = 还没有用户消息），
/// 截断宽度 [`SESSION_TITLE_CHARS`]）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMeta {
    pub id: String,
    pub updated_at: i64,
    pub events: usize,
    pub title: Option<String>,
}

/// 列表标题截断宽度（字符）。
pub const SESSION_TITLE_CHARS: usize = 40;

/// 列出目录下全部会话（`*.jsonl`，按 mtime 降序）；目录不存在 = 空表。
/// 单遍扫描：事件数 = 可解析行数，顺带抓首条用户消息做标题。
pub fn list_sessions(dir: &Path) -> Vec<SessionMeta> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut metas: Vec<(std::time::SystemTime, SessionMeta)> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "jsonl"))
        .filter_map(|e| {
            let path = e.path();
            let id = path.file_stem()?.to_str()?.to_string();
            if !valid_session_id(&id) {
                return None;
            }
            let meta = e.metadata().ok()?;
            let modified = meta.modified().ok()?;
            let updated_at = modified
                .duration_since(std::time::UNIX_EPOCH)
                .ok()?
                .as_secs() as i64;
            let (events, title) = std::fs::File::open(&path)
                .map(|f| {
                    use std::io::BufRead;
                    let mut events = 0usize;
                    let mut title = None;
                    for line in std::io::BufReader::new(f).lines().map_while(Result::ok) {
                        if line.trim().is_empty() {
                            continue;
                        }
                        let Ok(ev) = serde_json::from_str::<SessionEvent>(&line) else {
                            continue;
                        };
                        events += 1;
                        if title.is_none() {
                            if let SessionEvent::UserMessage { content } = ev {
                                title = Some(content.chars().take(SESSION_TITLE_CHARS).collect());
                            }
                        }
                    }
                    (events, title)
                })
                .unwrap_or((0, None));
            Some((
                modified,
                SessionMeta {
                    id,
                    updated_at,
                    events,
                    title,
                },
            ))
        })
        .collect();
    metas.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.id.cmp(&b.1.id)));
    metas.into_iter().map(|(_, m)| m).collect()
}

#[cfg(test)]
mod tests;
