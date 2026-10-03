//! 权限模式（㉘·B，PI-Desktop 借鉴）：会话级四档 + 决策数据源。
//!
//! 档位是**会话级属性**（DESIGN §7.2 决策表）：持久层 = 会话旁车
//! `<id>.meta.json` `permission_mode`（重启保持），内存层 = 本模块的
//! `PermissionModeStore`（缓存 + 写穿）。消费方三处：
//! ① `tool_source.rs` 计划模式目录级围栏（非 Pure 不出目录、resolve 拒）；
//! ② `approval.rs` 决策表（外部路径门 / 模式放行 / 豁免次序）；
//! ③ `commands/session.rs` `session_set_permission_mode` 命令。
//! 设置 `tools.defaultPermissionMode` 只定**新会话缺省**（热换入）。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, RwLock};

use session_store::{read_meta, write_permission_mode};

/// 四档权限模式（camelCase 序列化与前端 settings/旁车对齐）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionMode {
    /// 目录即围栏：非 Pure 工具对模型不可见，调到即如实失败（不弹卡）。
    Plan,
    /// 一切非只读动作逐次审批（缺省；有 undo 的写类也要问）。
    Ask,
    /// 有 undo 的写类（Compensable）自动放行；不可逆动作仍逐次问。
    AcceptEdits,
    /// 全部放行——插件内三类硬底线与外部路径门照常生效（完全访问≠无安全）。
    Auto,
}

impl PermissionMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            PermissionMode::Plan => "plan",
            PermissionMode::Ask => "ask",
            PermissionMode::AcceptEdits => "acceptEdits",
            PermissionMode::Auto => "auto",
        }
    }

    /// 容错解析：未知值回落 `Ask`（旁车手改 / 旧数据不致命）。
    pub fn parse(s: &str) -> Option<PermissionMode> {
        match s {
            "plan" => Some(PermissionMode::Plan),
            "ask" => Some(PermissionMode::Ask),
            "acceptEdits" => Some(PermissionMode::AcceptEdits),
            "auto" => Some(PermissionMode::Auto),
            _ => None,
        }
    }
}

/// 模式存储：缓存 + 旁车写穿。`mode_of` 未命中缓存时读一次旁车（缺字段
/// / 无会话 = 设置缺省）并缓存；`set` 写穿旁车 + 更新缓存。会话删除后
/// 残留的缓存条目无害（id 不复用）。
pub struct PermissionModeStore {
    sessions_dir: PathBuf,
    /// 新会话缺省档（settings `tools.defaultPermissionMode`，保存即热换）。
    default: RwLock<PermissionMode>,
    cache: Mutex<HashMap<String, PermissionMode>>,
}

impl PermissionModeStore {
    pub fn new(sessions_dir: PathBuf, default: PermissionMode) -> Self {
        Self {
            sessions_dir,
            default: RwLock::new(default),
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// 设置换入新会话缺省档（settings_set 保存即生效）。
    pub fn set_default(&self, mode: PermissionMode) {
        *self
            .default
            .write()
            .expect("permission mode default poisoned") = mode;
    }

    pub fn default_mode(&self) -> PermissionMode {
        *self
            .default
            .read()
            .expect("permission mode default poisoned")
    }

    /// 会话生效档：`None`（无会话）= 缺省档；有会话 = 缓存 → 旁车 → 缺省。
    pub fn mode_of(&self, session_id: Option<&str>) -> PermissionMode {
        match session_id {
            None => self.default_mode(),
            Some(id) => self.mode_of_session(id),
        }
    }

    fn mode_of_session(&self, session_id: &str) -> PermissionMode {
        if let Some(m) = self
            .cache
            .lock()
            .expect("permission mode cache poisoned")
            .get(session_id)
        {
            return *m;
        }
        // 旁车一次读：只缓存**显式设置过**的会话——未设置（缺字段 / 旧数据
        // / 手改未知值 / 缺文件）不缓存，随设置缺省走（缺省热换对它们
        // 保持生效；"新会话缺省"语义 = 没选过的会话不做死限定）
        let explicit = read_meta(&self.sessions_dir, session_id)
            .and_then(|meta| meta.permission_mode)
            .and_then(|s| PermissionMode::parse(&s));
        match explicit {
            Some(m) => {
                self.cache
                    .lock()
                    .expect("permission mode cache poisoned")
                    .insert(session_id.to_string(), m);
                m
            }
            None => self.default_mode(),
        }
    }

    /// 设置会话档（写穿旁车 + 缓存；旁车失败 = Err，缓存不动）。
    pub fn set_session_mode(&self, session_id: &str, mode: PermissionMode) -> std::io::Result<()> {
        write_permission_mode(&self.sessions_dir, session_id, Some(mode.as_str()))?;
        self.cache
            .lock()
            .expect("permission mode cache poisoned")
            .insert(session_id.to_string(), mode);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
