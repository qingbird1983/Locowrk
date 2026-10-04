//! 会话 → 项目上下文解析（㉘，项目级隔离的宿主侧"单一真源"）。
//!
//! 权威数据 = 会话旁车 `<id>.meta.json` 的 `workspace` 字段（§2.3 关联，
//! v2.50 落地）。desktop 消费同一份解析：
//! ① `plugin_host::set_session_context_resolver`——FFI 调用期上下文
//!    （workspace + `fs_roots` 调用期围栏根；㉘·C 起围栏 = 全局声明根、
//!    让路给权限档），memory-sqlite / tool-fs / tool-shell(路径类) /
//!    tool-git / tool-edit 等插件经 vtable `session_context` 反向查询；
//! ② `workspace_view`（workspace-only 投影）——`MemoryStore` /
//!    `ExperienceIndex` 召回键位过滤 + `ApprovalHub` 环境行数据源。
//!
//! 未归属 / 旁车缺失 / id 非法 → workspace `None`、fs_roots 空表（消费方
//! 统一映射为未归属键位 `""` / env 全局围栏回落，绝不 panic、绝不把别的
//! 项目数据漏进来）。子 Agent 会话没有旁车（⑮ 黑盒），解析为未归属——
//! 已知边界：子内记忆落在未归属键位，不随父项目走（诚实且安全）。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use plugin_api::ffi::CallContext;
use session_store::read_meta;

/// session_id → 调用期上下文（workspace + 围栏根；㉘·B 形态）。
pub type SessionContextResolver = Arc<dyn Fn(&str) -> CallContext + Send + Sync>;

/// workspace-only 视图（memory / experiences / 审批环境行消费）。
pub type WorkspaceResolver = Arc<dyn Fn(&str) -> Option<String> + Send + Sync>;

/// 路径规范化：canonicalize 优先（解析符号链接与 `..`，含 Windows verbatim
/// 前缀），失败（不存在/被删）回落原样——插件侧会再 canonicalize 并对失败
/// 根剔除，宁严勿松。
fn canon_or_raw(p: &Path) -> String {
    p.canonicalize()
        .unwrap_or_else(|_| p.to_path_buf())
        .to_string_lossy()
        .into_owned()
}

/// 组装点构造：绑死会话目录、数据根与设置侧 fsRoots 供给（保存即生效，
/// 读当前值）。**㉘·C 围栏让路**（用户裁定"权限选择一等公民，其它阻挡
/// 全部让路"）：**归属会话**下发 `fs_roots = [数据根] ∪ settings.fsRoots`
/// （全局声明根，所有权限档位一致——不再按归属收窄为 `[数据根, 项目
/// 工作区]`；工作区归属只决定记忆键位/环境行/侧栏，不构成权限墙）；
/// **未归属会话不下发**（空表 = 插件回落 `LOCOWRK_FS_ROOTS` 同一集合，
/// headless 同）。出声明根仍由插件 fail-closed 拒绝（文案自描述，v2.66
/// 先例）+ 越界门兜底 shell 命令。
pub fn session_context_resolver(
    sessions_dir: PathBuf,
    data_root: PathBuf,
    fs_roots_of: std::sync::Arc<dyn Fn() -> Vec<String> + Send + Sync>,
) -> SessionContextResolver {
    Arc::new(move |session_id: &str| {
        let workspace = read_meta(&sessions_dir, session_id).and_then(|m| m.workspace);
        let fs_roots: Vec<String> = match &workspace {
            Some(ws) if !ws.is_empty() => {
                let mut roots = vec![canon_or_raw(&data_root)];
                roots.extend((fs_roots_of)().iter().map(|r| canon_or_raw(Path::new(r))));
                roots
            }
            _ => Vec::new(),
        };
        CallContext {
            session_id: Some(session_id.to_string()),
            workspace,
            fs_roots,
        }
    })
}

/// workspace-only 投影：同一份旁车真源的窄视图（不给围栏根）。
pub fn workspace_view(resolver: SessionContextResolver) -> WorkspaceResolver {
    Arc::new(move |session_id: &str| resolver(session_id).workspace)
}

/// 键位归一：消费方统一入口——`None`（无会话/未归属）与 `Some("")` 都
/// 落到未归属键位。sqlite 侧 `workspace` 列 NOT NULL DEFAULT ''。
pub fn scope_key(workspace: Option<&str>) -> String {
    workspace
        .filter(|w| !w.is_empty())
        .unwrap_or("")
        .to_string()
}

#[cfg(test)]
mod tests;
