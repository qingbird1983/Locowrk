//! 会话旁车元数据（`<id>.meta.json`，自 persist.rs 按单文件上限拆出
//! ——拆分是独立任务、外部路径 `session_store::write_*` 不变）：重命名
//! 标题（v2.49）/ 工作区归属（v2.50）/ 权限模式（㉘·B）/ 模型覆盖
//! （v2.58）。缺文件 = 全缺省；合并写、全字段空删文件。

use std::path::Path;

use serde::{Deserialize, Serialize};

use std::path::PathBuf;

use super::{StoreError, session_file};

/// 会话旁车元数据（`<id>.meta.json`，v2.49 会话运营）：存重命名标题；
/// `workspace` 留给 §2.3 会话↔工作区关联批次；`permission_mode` 留给
/// ㉘·B 权限模式；`organizer` 留给 M4 整理模式切流。缺文件 = 全缺省。
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct SessionMetaFile {
    pub title: Option<String>,
    pub workspace: Option<String>,
    pub permission_mode: Option<String>,
    /// 会话级模型覆盖（v2.58 §3.30）：`model` + `model_provider` 成对
    /// （档位 id + 模型名），None = 跟随设置缺省。
    pub model: Option<String>,
    pub model_provider: Option<String>,
    /// 会话级整理模式（M4 §3.38①）：`"legacy"` | `"tree"`；None = 未显式
    /// 选择，跟随开发者缺省档（desktop `developer.defaultOrganizer`）。
    pub organizer: Option<String>,
}

/// 旁车元数据路径（id 校验同日志文件；`<id>.jsonl` → `<id>.meta.json`，
/// 后缀替换只动 `jsonl`，id 内含 `.` 不受影响）。
pub(super) fn meta_file(dir: &Path, session_id: &str) -> Result<PathBuf, StoreError> {
    Ok(session_file(dir, session_id)?.with_extension("meta.json"))
}

/// 读旁车元数据：缺文件 / 坏 JSON = None（不致命——标题回落自动推导）。
pub fn read_meta(dir: &Path, session_id: &str) -> Option<SessionMetaFile> {
    let path = meta_file(dir, session_id).ok()?;
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

pub(super) fn invalid_id(e: StoreError) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidInput, e.to_string())
}

/// 旁车字段写入（合并语义）：读出既有 → 应用更新 → 全字段空删文件（回落全自动），否则合并写盘。
/// v2.49 的 `write_title` 空标题**删整个文件**，连带抹掉已关联的 workspace（v2.50 起并存）——改为只清字段。
fn write_meta(
    dir: &Path,
    session_id: &str,
    update: impl FnOnce(&mut SessionMetaFile),
) -> std::io::Result<()> {
    let path = meta_file(dir, session_id).map_err(invalid_id)?;
    let mut meta = read_meta(dir, session_id).unwrap_or_default();
    update(&mut meta);
    if meta.title.is_none()
        && meta.workspace.is_none()
        && meta.permission_mode.is_none()
        && meta.model.is_none()
        && meta.model_provider.is_none()
        && meta.organizer.is_none()
    {
        return match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        };
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(
        &path,
        serde_json::to_vec_pretty(&meta).expect("meta serialize"),
    )
}

/// 写重命名标题（v2.49）：trim 后空串 = 只清标题字段（回落自动标题，
/// 保留 workspace 等其余字段）；非空 = 合并写入。
pub fn write_title(dir: &Path, session_id: &str, title: &str) -> std::io::Result<()> {
    let t = title.trim().to_string();
    write_meta(dir, session_id, |m| {
        m.title = (!t.is_empty()).then_some(t.clone());
    })
}

/// 写工作区归属（v2.50 §2.3）：`Some(根目录)` = 关联（trim 后空串视同清除）；`None` = 移出。
/// 合并写入保留标题；全字段空时删旁车文件。
pub fn write_workspace(
    dir: &Path,
    session_id: &str,
    workspace: Option<&str>,
) -> std::io::Result<()> {
    let ws = workspace
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    write_meta(dir, session_id, |m| m.workspace = ws)
}

/// 写会话级权限模式（㉘·B）：`Some(mode)` = 设置（调用方已校验取值）；`None` = 清除（回落设置缺省）。
/// 合并写入保留其余字段。
pub fn write_permission_mode(
    dir: &Path,
    session_id: &str,
    mode: Option<&str>,
) -> std::io::Result<()> {
    let mode = mode
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    write_meta(dir, session_id, |m| m.permission_mode = mode)
}

/// 写会话级模型覆盖（v2.58 §3.30）：`Some((provider_id, model))` = 设置
/// （成对写入——只改模型名不换档路由会产生跨档错配）；`None` = 清除
/// （回落设置缺省）。合并写入保留其余字段。
pub fn write_model_override(
    dir: &Path,
    session_id: &str,
    model: Option<(&str, &str)>,
) -> std::io::Result<()> {
    let pair = model.map(|(p, m)| (p.trim().to_string(), m.trim().to_string()));
    write_meta(dir, session_id, |m| match pair.clone() {
        Some((p, mm)) if !p.is_empty() && !mm.is_empty() => {
            m.model_provider = Some(p);
            m.model = Some(mm);
        }
        _ => {
            m.model_provider = None;
            m.model = None;
        }
    })
}

/// 写会话级整理模式（M4 §3.38①）：`Some("legacy" | "tree")` = 设置（调用方
/// 已校验取值）；`None` = 清除（跟随开发者缺省档）。合并写入保留其余字段。
pub fn write_organizer(dir: &Path, session_id: &str, mode: Option<&str>) -> std::io::Result<()> {
    let mode = mode
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    write_meta(dir, session_id, |m| m.organizer = mode)
}
