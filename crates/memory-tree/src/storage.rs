//! 树投影存储（路线表 M2：**(workspace, session)** 键位，对齐 ㉘·A）：
//! `<数据根>/memory-tree/<scope>/<session>.json`——scope = 工作区路径的
//! 可读编码（非安全字符换 `_` + fnv1a 短哈希防碰撞；未归属 = `_root`）。
//! **投影可丢**：坏文件按不存在处理（整树重建兜底，§1.3），绝不 panic。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::tree::MemoryTree;

const DIR_NAME: &str = "memory-tree";
const SESSION_ID_MAX: usize = 64;

/// 会话 id 白名单（与 session_log 落盘命名同源收紧）：防路径穿越。
fn valid_session_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= SESSION_ID_MAX
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// fnv-1a 64 位（无依赖、跨运行稳定）。
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// 工作区 → 目录名：可读前缀 + 短哈希（`F:\a\b` → `F__a_b-1a2b3c4d`）；
/// 未归属（空串）= `_root`。
pub fn scope_dir(workspace: &str) -> String {
    if workspace.is_empty() {
        return "_root".into();
    }
    let safe: String = workspace
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = safe.trim_matches('_');
    let head: String = trimmed.chars().take(40).collect();
    let hash = format!("{:08x}", (fnv1a(workspace.as_bytes()) >> 32) as u32);
    format!("{head}-{hash}")
}

/// 扫描恢复项：在途任务（崩溃重投 §5.3）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PendingHit {
    pub workspace: String,
    pub session_id: String,
}

/// 树投影清单项（M3 只读呈现：记忆页会话切换器数据源）。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TreeSummary {
    pub workspace: String,
    pub session_id: String,
    pub chapter_count: usize,
    pub chaptered_until: usize,
    pub pending: bool,
    pub paused_auto: bool,
    /// 投影文件 mtime（毫秒）；取不到 = 0（排序垫底，不阻塞列表）。
    pub updated_ms: i64,
}

/// 树投影存储句柄（`root` = 数据根）。
#[derive(Debug, Clone)]
pub struct TreeStore {
    root: PathBuf,
}

impl TreeStore {
    pub fn new(data_root: &Path) -> Self {
        Self {
            root: data_root.join(DIR_NAME),
        }
    }

    fn path_for(&self, workspace: &str, session_id: &str) -> Result<PathBuf, String> {
        if !valid_session_id(session_id) {
            return Err(format!("非法会话 id：{session_id:?}"));
        }
        Ok(self
            .root
            .join(scope_dir(workspace))
            .join(format!("{session_id}.json")))
    }

    /// 读投影：缺文件 / 坏 JSON = None（树是投影，丢了就重建）。
    pub fn load(&self, workspace: &str, session_id: &str) -> Option<MemoryTree> {
        let path = self.path_for(workspace, session_id).ok()?;
        let bytes = std::fs::read(path).ok()?;
        match serde_json::from_slice::<MemoryTree>(&bytes) {
            Ok(t) => Some(t),
            Err(e) => {
                tracing::warn!("memory-tree 投影损坏（忽略并重建）：{e}");
                None
            }
        }
    }

    /// 原子写（tmp + rename）：崩在半途不留坏文件。
    pub fn save(&self, tree: &MemoryTree) -> Result<PathBuf, String> {
        let path = self.path_for(&tree.workspace, &tree.session_id)?;
        let dir = path.parent().ok_or_else(|| "树投影路径异常".to_string())?;
        std::fs::create_dir_all(dir).map_err(|e| format!("memory-tree 目录创建失败: {e}"))?;
        let bytes = serde_json::to_vec_pretty(tree).map_err(|e| e.to_string())?;
        let tmp = dir.join(format!(".{}.tmp", tree.session_id));
        std::fs::write(&tmp, bytes).map_err(|e| format!("树投影写入失败: {e}"))?;
        std::fs::rename(&tmp, &path).map_err(|e| format!("树投影落盘失败: {e}"))?;
        Ok(path)
    }

    /// 删投影（"删投影重来，零损失"的结构性回退）。返回是否确有文件。
    pub fn delete(&self, workspace: &str, session_id: &str) -> Result<bool, String> {
        let path = self.path_for(workspace, session_id)?;
        Ok(std::fs::remove_file(path).is_ok())
    }

    /// 启动扫描：所有 scope 下的投影里带在途任务的 (workspace, session)。
    /// scope 目录是 workspace 的有损编码，真名从投影内读回。
    pub fn scan_pending(&self) -> Vec<PendingHit> {
        let mut hits = Vec::new();
        for (tree, session_id) in self.scan_all() {
            if tree.pending.is_some() {
                hits.push(PendingHit {
                    workspace: tree.workspace,
                    session_id,
                });
            }
        }
        hits
    }

    /// 全量扫描（M3 只读呈现）：树清单按更新时间降序（mtime；同刻按
    /// session_id 稳定序）。坏投影跳过不阻塞列表——投影可丢的总原则。
    pub fn scan_trees(&self) -> Vec<TreeSummary> {
        let mut out: Vec<(TreeSummary, String)> = Vec::new();
        for (tree, session_id) in self.scan_all() {
            out.push((
                TreeSummary {
                    chapter_count: tree.chapters.len(),
                    chaptered_until: tree.chaptered_until,
                    pending: tree.pending.is_some(),
                    paused_auto: tree.paused_auto,
                    workspace: tree.workspace,
                    session_id: session_id.clone(),
                    updated_ms: 0,
                },
                session_id,
            ));
        }
        // mtime 在第二遍补：scan_all 逐文件读过，路径在这里已不可得——
        // 直接重取（清单页低频打开，IO 成本可忽略）。
        for (summary, session_id) in out.iter_mut() {
            let path = self
                .root
                .join(scope_dir(&summary.workspace))
                .join(format!("{session_id}.json"));
            summary.updated_ms = mtime_ms(&path);
        }
        let mut list: Vec<TreeSummary> = out.into_iter().map(|(s, _)| s).collect();
        list.sort_by(|a, b| {
            b.updated_ms
                .cmp(&a.updated_ms)
                .then_with(|| a.session_id.cmp(&b.session_id))
        });
        list
    }

    /// 共享扫描骨架：逐 scope 逐文件解析，产出 (树, 会话 id)。
    fn scan_all(&self) -> Vec<(MemoryTree, String)> {
        let mut out = Vec::new();
        for scope in self.list_scopes() {
            for (tree, id) in self.scan_scope(&scope) {
                out.push((tree, id));
            }
        }
        out
    }

    /// 单工作区扫描（F1 tree.md 同步/导出用）：产出 (树, 会话 id, 投影
    /// mtime 毫秒)——mtime 给人改保护（tree.md 新于全部投影 = 跳过重建）
    /// 与稳定排序。
    pub fn scan_workspace(&self, workspace: &str) -> Vec<(MemoryTree, String, i64)> {
        let scope = self.root.join(scope_dir(workspace));
        let mut out: Vec<(MemoryTree, String, i64)> = self
            .scan_scope(&scope)
            .into_iter()
            .map(|(t, id)| {
                let path = scope.join(format!("{id}.json"));
                (t, id, mtime_ms(&path))
            })
            .collect();
        out.sort_by(|a, b| a.1.cmp(&b.1));
        out
    }

    /// scope 目录清单（scan_all 骨架提取）。
    fn list_scopes(&self) -> Vec<std::path::PathBuf> {
        let mut out = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&self.root) {
            for scope in entries.flatten() {
                if scope.path().is_dir() {
                    out.push(scope.path());
                }
            }
        }
        out
    }

    /// 单 scope 解析：产出 (树, 会话 id)。坏投影跳过不阻塞扫描。
    fn scan_scope(&self, scope: &Path) -> Vec<(MemoryTree, String)> {
        let mut out = Vec::new();
        let Ok(files) = std::fs::read_dir(scope) else {
            return out;
        };
        for f in files.flatten() {
            let name = f.file_name();
            let Some(id) = name.to_str().and_then(|s| s.strip_suffix(".json")) else {
                continue;
            };
            let Ok(bytes) = std::fs::read(f.path()) else {
                continue;
            };
            let Ok(tree) = serde_json::from_slice::<MemoryTree>(&bytes) else {
                continue; // 坏投影不阻塞扫描
            };
            out.push((tree, id.to_string()));
        }
        out
    }
}

/// 文件 mtime → Unix 毫秒；取不到 = 0。
fn mtime_ms(path: &Path) -> i64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(ws: &str, sid: &str) -> MemoryTree {
        let mut t = MemoryTree::new(ws, sid);
        t.insert_placeholder(0, 4, 1).unwrap();
        t
    }

    #[test]
    fn scope_dir_is_stable_readable_and_collision_safe() {
        assert_eq!(scope_dir(""), "_root");
        let a = scope_dir(r"F:\Warehouse\AIwork\Locowrk");
        let b = scope_dir(r"F:\Warehouse\AIwork\Locowrk");
        assert_eq!(a, b, "同一工作区两次编码必须一致");
        assert!(a.starts_with("F__Warehouse"));
        assert_ne!(a, scope_dir(r"F:\Warehouse\AIwork\Other"));
    }

    #[test]
    fn save_load_roundtrip_and_atomic_tmp_leftover() {
        let dir = std::env::temp_dir().join(format!("mt-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = TreeStore::new(&dir);
        let t = sample("ws-a", "sess-1");
        store.save(&t).expect("保存成功");
        let loaded = store.load("ws-a", "sess-1").expect("可读回");
        assert_eq!(loaded.pending.as_ref().unwrap().from, 0);
        // 同一工作区再次保存不因 tmp 残留失败
        store.save(&sample("ws-a", "sess-1")).expect("二次保存");
        assert!(store.delete("ws-a", "sess-1").unwrap());
        assert!(store.load("ws-a", "sess-1").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn bad_session_id_rejected_and_bad_json_is_none() {
        let dir = std::env::temp_dir().join(format!("mt-test-bad-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = TreeStore::new(&dir);
        assert!(store.load("ws", "../evil").is_none());
        assert!(store.save(&sample("ws", "a/b")).is_err());
        // 坏 JSON = 无投影
        let scope = store.root.join(scope_dir("ws-x"));
        std::fs::create_dir_all(&scope).unwrap();
        std::fs::write(scope.join("bad.json"), b"{ not json").unwrap();
        assert!(store.load("ws-x", "bad").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn scan_pending_finds_only_pending_trees() {
        let dir = std::env::temp_dir().join(format!("mt-test-scan-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = TreeStore::new(&dir);
        store.save(&sample("ws-a", "sess-1")).unwrap();
        let mut clean = MemoryTree::new("ws-a", "sess-2");
        clean.chaptered_until = 3;
        store.save(&clean).unwrap();
        let hits = store.scan_pending();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].session_id, "sess-1");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn scan_workspace_lists_single_scope_with_mtime() {
        let dir = std::env::temp_dir().join(format!("mt-test-ws-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = TreeStore::new(&dir);
        store.save(&sample("ws-a", "sess-2")).unwrap();
        store.save(&sample("ws-a", "sess-1")).unwrap();
        store.save(&sample("ws-b", "sess-3")).unwrap();
        let rows = store.scan_workspace("ws-a");
        assert_eq!(rows.len(), 2, "只取本工作区");
        assert_eq!(rows[0].1, "sess-1", "会话 id 稳定序");
        assert!(rows.iter().all(|r| r.2 > 0), "mtime 在场");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn scan_trees_lists_all_sorted_by_recency() {
        let dir = std::env::temp_dir().join(format!("mt-test-list-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = TreeStore::new(&dir);
        store.save(&sample("ws-a", "sess-1")).unwrap();
        let mut clean = MemoryTree::new("ws-b", "sess-2");
        clean.chaptered_until = 7;
        store.save(&clean).unwrap();
        let list = store.scan_trees();
        assert_eq!(list.len(), 2, "有在途与无在途的树都进清单");
        assert!(list[0].updated_ms >= list[1].updated_ms, "按更新时间降序");
        let pending = list.iter().find(|s| s.session_id == "sess-1").unwrap();
        let clean = list.iter().find(|s| s.session_id == "sess-2").unwrap();
        assert_eq!(pending.chapter_count, 1, "占位章节计入章节数");
        assert!(pending.pending);
        assert_eq!(clean.chaptered_until, 7);
        assert!(!clean.pending);
        // 坏投影文件跳过
        let scope = store.root.join(scope_dir("ws-a"));
        std::fs::write(scope.join("bad.json"), b"{ not json").unwrap();
        assert_eq!(store.scan_trees().len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
