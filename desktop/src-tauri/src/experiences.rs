//! 经历记忆库（⑬，Context 文档 §4.3 memory-vector 的宿主侧形态）：
//! "聊过什么、做过什么决策"——按会话沉淀的语义可检索记忆。
//!
//! **为什么不是插件**：与 llm-openai/ollama 宿主侧同理（ROADMAP §四已
//! 有先例）——写入要过 embedding 模型（端点 + 密钥归宿主），插件没有
//! 运行期配置通道；存储/检索本就是宿主关切（Memory-Inject 经
//! `ExperienceSource` 契约消费，内核不碰 SQL）。表结构照 §4.3：content
//! / embedding BLOB / session_id / created_at（f32 小端字节数组）；
//! ㉘ 追加 `workspace` 列（会话归属项目，'' = 未归属）——写入打标、
//! 检索按键位过滤（Context 文档 §4.6 项目隔离）。
//!
//! 模型一致性：检索时维度不符的行直接跳过（换 embedding 模型后旧维度
//! 行自然退役）；同维不同模型的语义漂移是已知近似，文档已记。
//! 同步短事务 + `Mutex<Connection>`，调用方不持锁跨 await（同 usage.rs）。

use std::path::Path;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use kernel_agent::{ExperienceEntry, ExperienceSource};
use rusqlite::Connection;

use crate::llm::CooledEmbed;
use crate::workspace::WorkspaceResolver;
use kernel_agent::EmbedPort;

/// 全库扫描上限：经历库增长是线性的（一回合一条），量级远小于向量库
/// 场景；余弦在 Rust 侧算，2000 行内微秒级。超过只检最近的 2000 条。
const SCAN_LIMIT: i64 = 2000;

/// 一条经历（管理页/注入用，不含向量本体）。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExperienceRow {
    pub id: i64,
    pub content: String,
    pub session_id: String,
    /// 项目归属（㉘）：'' = 未归属；管理页透出，注入不带（ExperienceEntry）。
    pub workspace: String,
    pub created_at: i64,
}

/// f32 向量 ↔ sqlite BLOB（小端字节序）。
pub(crate) fn vec_to_blob(vec: &[f32]) -> Vec<u8> {
    vec.iter().flat_map(|f| f.to_le_bytes()).collect()
}

pub(crate) fn blob_to_vec(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

/// sqlite 存取（同步、短事务）。
pub struct ExperienceDb {
    conn: Mutex<Connection>,
}

impl ExperienceDb {
    /// 开库建表（幂等）。文件损坏/不可写在此报错，由调用方降级。
    /// ㉘ workspace 列：旧库 pragma 探测后 ALTER 补列，DEFAULT '' 覆盖
    /// 既有行（旧经历归入未归属键位）。
    pub fn open(path: &Path) -> Result<Self, String> {
        let conn = Connection::open(path).map_err(|e| e.to_string())?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS experiences (
                 id         INTEGER PRIMARY KEY AUTOINCREMENT,
                 content    TEXT NOT NULL,
                 embedding  BLOB NOT NULL,
                 session_id TEXT NOT NULL,
                 workspace  TEXT NOT NULL DEFAULT '',
                 created_at INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_experiences_created ON experiences(created_at);",
        )
        .map_err(|e| e.to_string())?;
        let has_workspace: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('experiences') WHERE name = 'workspace'",
                [],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        if has_workspace == 0 {
            conn.execute_batch(
                "ALTER TABLE experiences ADD COLUMN workspace TEXT NOT NULL DEFAULT '';",
            )
            .map_err(|e| e.to_string())?;
        }
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// 写入一条经历（调用方已做好内容裁剪与频控；workspace = 会话归属）。
    pub fn insert(
        &self,
        content: &str,
        embedding: &[f32],
        session_id: &str,
        workspace: &str,
    ) -> Result<(), String> {
        let conn = self.conn.lock().expect("experiences db poisoned");
        conn.execute(
            "INSERT INTO experiences (content, embedding, session_id, workspace, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                content,
                vec_to_blob(embedding),
                session_id,
                workspace,
                now_secs()
            ],
        )
        .map(|_| ())
        .map_err(|e| e.to_string())
    }

    /// 语义检索：**指定键位**（㉘ 项目隔离）限 SCAN_LIMIT（新→旧）载入、
    /// 维度匹配行做余弦、取 top-k。向量维度与查询不符的行跳过。
    pub fn search(
        &self,
        query: &[f32],
        top_k: usize,
        workspace: &str,
    ) -> Result<Vec<ExperienceRow>, String> {
        if query.is_empty() || top_k == 0 {
            return Ok(Vec::new());
        }
        let conn = self.conn.lock().expect("experiences db poisoned");
        let mut stmt = conn
            .prepare(
                "SELECT id, content, session_id, created_at, embedding FROM experiences \
                 WHERE workspace = ?1 ORDER BY created_at DESC, id DESC LIMIT ?2",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(rusqlite::params![workspace, SCAN_LIMIT], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, Vec<u8>>(4)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        let mut scored: Vec<(f32, ExperienceRow)> = Vec::new();
        for row in rows {
            let (id, content, session_id, created_at, blob) = row.map_err(|e| e.to_string())?;
            let vec = blob_to_vec(&blob);
            if vec.len() != query.len() {
                continue;
            }
            let sim = cosine(query, &vec);
            if sim > 0.0 {
                scored.push((
                    sim,
                    ExperienceRow {
                        id,
                        content,
                        session_id,
                        workspace: workspace.to_string(),
                        created_at,
                    },
                ));
            }
        }
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        Ok(scored.into_iter().take(top_k).map(|(_, r)| r).collect())
    }

    /// 管理页列表（新→旧，全键位）。
    pub fn list_recent(&self, limit: u32) -> Result<Vec<ExperienceRow>, String> {
        let conn = self.conn.lock().expect("experiences db poisoned");
        let mut stmt = conn
            .prepare(
                "SELECT id, content, session_id, workspace, created_at FROM experiences \
                 ORDER BY created_at DESC, id DESC LIMIT ?1",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([limit], |r| {
                Ok(ExperienceRow {
                    id: r.get(0)?,
                    content: r.get(1)?,
                    session_id: r.get(2)?,
                    workspace: r.get(3)?,
                    created_at: r.get(4)?,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }

    /// 删除一条（管理页）；返回是否真删了。
    pub fn delete(&self, id: i64) -> Result<bool, String> {
        let conn = self.conn.lock().expect("experiences db poisoned");
        let n = conn
            .execute("DELETE FROM experiences WHERE id = ?1", [id])
            .map_err(|e| e.to_string())?;
        Ok(n > 0)
    }

    /// 清空（管理页一键清）；返回删掉的条数。
    pub fn clear_all(&self) -> Result<usize, String> {
        let conn = self.conn.lock().expect("experiences db poisoned");
        conn.execute("DELETE FROM experiences", [])
            .map_err(|e| e.to_string())
    }

    /// 条数（测试用；生产投影走 list_recent）。
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn count(&self) -> Result<u32, String> {
        let conn = self.conn.lock().expect("experiences db poisoned");
        conn.query_row("SELECT COUNT(*) FROM experiences", [], |r| r.get(0))
            .map_err(|e| e.to_string())
    }
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    for (x, y) in a.iter().zip(b) {
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    let denom = na.sqrt() * nb.sqrt();
    if denom == 0.0 || !denom.is_finite() {
        0.0
    } else {
        (dot / denom).clamp(-1.0, 1.0)
    }
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// [`ExperienceSource`] 实现：查询过 embedding（共享冷却柄）→ 库内余弦。
/// embedding 未配置/失败 = 空表降级（实现内部裁决，不向上抛）。
/// ㉘：召回按键位过滤——session_id 经解析器（workspace.rs 单一真源）
/// 得项目根，只见该项目的经历。
pub struct ExperienceIndex {
    db: Arc<ExperienceDb>,
    embed: Arc<CooledEmbed>,
    workspace_of: WorkspaceResolver,
}

impl ExperienceIndex {
    pub fn new(
        db: Arc<ExperienceDb>,
        embed: Arc<CooledEmbed>,
        workspace_of: WorkspaceResolver,
    ) -> Arc<Self> {
        Arc::new(Self {
            db,
            embed,
            workspace_of,
        })
    }
}

#[async_trait]
impl ExperienceSource for ExperienceIndex {
    async fn recall(
        &self,
        session_id: Option<&str>,
        query: &str,
        limit: usize,
    ) -> Vec<ExperienceEntry> {
        let Ok(vecs) = self.embed.embed(&[query.to_string()]).await else {
            return Vec::new();
        };
        let Some(q) = vecs.first() else {
            return Vec::new();
        };
        let workspace =
            crate::workspace::scope_key(session_id.and_then(|s| (self.workspace_of)(s)).as_deref());
        match self.db.search(q, limit, &workspace) {
            Ok(rows) => rows
                .into_iter()
                .map(|r| ExperienceEntry {
                    content: r.content,
                    session_id: r.session_id,
                    created_at: r.created_at,
                })
                .collect(),
            Err(e) => {
                tracing::warn!("experiences search failed: {e}");
                Vec::new()
            }
        }
    }
}

#[cfg(test)]
mod tests;
