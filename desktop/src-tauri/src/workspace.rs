//! 会话 → 项目上下文解析（㉘，项目级隔离的宿主侧"单一真源"）。
//!
//! 权威数据 = 会话旁车 `<id>.meta.json` 的 `workspace` 字段（§2.3 关联，
//! v2.50 落地）。desktop 消费同一份解析：
//! ① `plugin_host::set_session_context_resolver`——FFI 调用期上下文
//!    （workspace + `fs_roots` 调用期围栏根；㉘·C 起围栏 = 全局声明根、
//!    让路给权限档），memory-sqlite / tool-fs / tool-shell(路径类) /
//!    tool-git / tool-edit 等插件经 vtable `session_context` 反向查询；
//! ② `workspace_view`（workspace-only 投影）——`MemoryStore` /
//!    `ApprovalHub` 环境行数据源。
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

/// workspace-only 视图（memory 注入键位 / 审批环境行消费）。
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
///
/// ⚠ **Q7c（v2.197）起这里就是记忆键位的唯一收口点**：视图产出的串既进 sqlite 主键
/// （`memories.workspace`），也进**磁盘目录名**（`memory-tree/<scope_dir(ws)>` 与
/// `memory/projects/<project_dir_name(ws)>`），而旁车里存的是用户与模型写下的**原样串**。
/// 原先只有部分消费方各自过 `scope_key`（`memory.rs` 的注入键位、`organize/facts.rs`
/// 的事实卡落库），树投影与 tree.md 那两条通路漏在外面 ⇒ 同一个项目在磁盘上落成**两个
/// 目录**、各自被一条通路读写（2026-10-14 本机实测，见《Key Unification and Legacy
/// Scope Merge v1》§1）。在源头归一 ⇒ 下游所有 `workspace_of` 调用点一次同形，
/// 不需要各补一遍（历批「每一处调用点补一个参数」那种改动面，这里刻意不复现）。
///
/// **语义保持**：未归属仍是 `None`（消费方 `?` 与 `unwrap_or_default` 两种写法都不变），
/// `Some("")` 归一成 `""`，与 `scope_key` 的既有契约同形；`fs_roots` 那半在
/// `session_context_resolver` 里下发**原样 canonicalize 串**，本函数不碰（围栏面见
/// `wskey` 头注「不动文件围栏的根」）。
pub fn workspace_view(resolver: SessionContextResolver) -> WorkspaceResolver {
    Arc::new(move |session_id: &str| {
        resolver(session_id)
            .workspace
            .as_deref()
            .map(|ws| scope_key(Some(ws)))
    })
}

/// 键位归一：消费方统一入口——`None`（无会话/未归属）与 `Some("")` 都
/// 落到未归属键位。sqlite 侧 `workspace` 列 NOT NULL DEFAULT ''。
///
/// **Q2（MR 稿 §6）起这里不再"原样返回"**：委托 [`plugin_api::wskey::normalize`]
/// 做平台收尾（Windows 折叠大小写 + 统一分隔符 + 解析 `..`），因为记忆主键是
/// `(workspace, key)`，同一个项目的两种写法会落成**两个主键**（事实被劈成几份，
/// 谁也查不全谁的）。宿主与插件 `WsCtx` 调的是同一个函数 ⇒ 两侧串形恒等；
/// 归一只作用于**记忆键位**，围栏根 `fs_roots` 的下发串不变（见 `wskey` 头注）。
pub fn scope_key(workspace: Option<&str>) -> String {
    workspace
        .map(plugin_api::wskey::normalize)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
