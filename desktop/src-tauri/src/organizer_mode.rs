//! 会话整理模式（M4 记忆树切流，树 v2 §11.2 阶段 2 / UI shell §3.38①）：
//! `legacy | tree` 两档 + 决策数据源。
//!
//! 档位是**会话级属性**：持久层 = 会话旁车 `<id>.meta.json` `organizer`
//! 字段（重启保持），内存层 = 本模块缓存 + 写穿——与 permission_mode
//! （㉘·B）同构。**未显式设置过的会话跟随开发者缺省档**
//! （settings `developer.defaultOrganizer`，缺省 legacy，保存即热换）；
//! 老会话旁车无字段 = 零迁移。消费方：
//! ① `organize/assemble.rs` 树组装源（树会话才切流）；
//! ② `tool_source.rs` memory_expand / memory_collapse 目录门控；
//! ③ `commands/sessions.rs` `session_set_organizer` 命令。
//! 切换即时生效（下一次请求按新档组装，事件流零改动——树是投影）。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, RwLock};

use session_store::{read_meta, write_organizer};

/// 两档整理模式（camelCase 序列化与前端 settings/旁车对齐）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OrganizerMode {
    /// 现网组装：全量历史 + ctx-compact 压缩（缺省；影子期语义不变）。
    #[default]
    Legacy,
    /// 树模式：ContextAssembler 组装（L0 目录 + 未整理尾巴），不跑压缩。
    Tree,
}

impl OrganizerMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            OrganizerMode::Legacy => "legacy",
            OrganizerMode::Tree => "tree",
        }
    }

    /// 容错解析：未知值回落 None（旁车手改不致命，跟随缺省档）。
    pub fn parse(s: &str) -> Option<OrganizerMode> {
        match s {
            "legacy" => Some(OrganizerMode::Legacy),
            "tree" => Some(OrganizerMode::Tree),
            _ => None,
        }
    }
}

/// 模式存储：缓存 + 旁车写穿。`mode_of` 未命中缓存时读一次旁车（缺字段
/// / 无会话 = 设置缺省）并缓存；`set` 写穿旁车 + 更新缓存。会话删除后
/// 残留缓存条目无害（id 不复用）。
pub struct OrganizerModeStore {
    sessions_dir: PathBuf,
    /// 未显式设置会话的缺省档（settings `developer.defaultOrganizer`，
    /// 保存即热换）。
    default: RwLock<OrganizerMode>,
    cache: Mutex<HashMap<String, OrganizerMode>>,
}

impl OrganizerModeStore {
    pub fn new(sessions_dir: PathBuf, default: OrganizerMode) -> Self {
        Self {
            sessions_dir,
            default: RwLock::new(default),
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// 设置换入新缺省档（settings_set 保存即生效）。
    pub fn set_default(&self, mode: OrganizerMode) {
        *self.default.write().expect("organizer default poisoned") = mode;
    }

    pub fn default_mode(&self) -> OrganizerMode {
        *self.default.read().expect("organizer default poisoned")
    }

    /// 会话生效档：`None`（无会话）= 缺省档；有会话 = 缓存 → 旁车 → 缺省。
    pub fn mode_of(&self, session_id: Option<&str>) -> OrganizerMode {
        match session_id {
            None => self.default_mode(),
            Some(id) => self.mode_of_session(id),
        }
    }

    fn mode_of_session(&self, session_id: &str) -> OrganizerMode {
        if let Some(m) = self
            .cache
            .lock()
            .expect("organizer cache poisoned")
            .get(session_id)
        {
            return *m;
        }
        // 只缓存**显式设置过**的会话——未设置不缓存，随缺省档热换走
        //（"新会话缺省"语义 = 没选过的会话不做死限定；权限档同款）
        let explicit = read_meta(&self.sessions_dir, session_id)
            .and_then(|meta| meta.organizer)
            .and_then(|s| OrganizerMode::parse(&s));
        match explicit {
            Some(m) => {
                self.cache
                    .lock()
                    .expect("organizer cache poisoned")
                    .insert(session_id.to_string(), m);
                m
            }
            None => self.default_mode(),
        }
    }

    /// 设置会话档（写穿旁车 + 缓存；旁车失败 = Err，缓存不动）。
    pub fn set_session_mode(&self, session_id: &str, mode: OrganizerMode) -> std::io::Result<()> {
        write_organizer(&self.sessions_dir, session_id, Some(mode.as_str()))?;
        self.cache
            .lock()
            .expect("organizer cache poisoned")
            .insert(session_id.to_string(), mode);
        Ok(())
    }

    /// 清除会话显式档（跟随缺省档走；缓存同步移除）。
    pub fn clear_session_mode(&self, session_id: &str) -> std::io::Result<()> {
        write_organizer(&self.sessions_dir, session_id, None)?;
        self.cache
            .lock()
            .expect("organizer cache poisoned")
            .remove(session_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
