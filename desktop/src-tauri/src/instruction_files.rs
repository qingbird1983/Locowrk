//! 指令文件层宿主实现（提示词分层设计 §4.3）：全局文件取 `<数据根>/AGENTS.md`，
//! 项目级取项目根 `AGENTS.md`（回落 `.Locowrk/AGENTS.md`），拼成
//! [`InstructionSource`] 内文，由内核
//! [`kernel_agent::InstructionFilesInject`] 包裹 `<system-reminder>` 后插
//! 消息列头部。
//!
//! 子目录级按需指令**裁掉**（设计稿 §7 记档：ZCode 亦未做；Locowrk 是
//! 项目归属制，无 cwd 语义）。
//!
//! 快照锁字节（设计稿 §3.3）：按 `(session, scope)` 键位**会话首查读
//! 文件并缓存**（None 也缓存——MemoryInject 预取同款），中途人改文件 =
//! 显式变更：当前会话不变，下一会话生效；会话改归属 = 换键 = 重算。
//! 全局文件缓存在 `""` 会话键位下进程级共享（读一次）。

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use kernel_agent::InstructionSource;

use crate::memory::MemoryStore;

/// 单文件字节上限（设计稿 §4.3：100KB，超限截断 + 标记如实，不静默
/// 丢弃）。
const MAX_FILE_BYTES: usize = 100 * 1024;

/// 缓存上限（FIFO 驱逐）：与 Memory-Inject 预取同款保险丝。
const CACHE_CAP: usize = 128;

/// 指令文件源（组装点构造：全局路径 = 数据根 AGENTS.md；开关 = settings
/// `memory.instructionFiles` 闭包）。
pub struct InstructionFiles {
    store: Arc<MemoryStore>,
    global_path: PathBuf,
    enabled: Arc<dyn Fn() -> bool + Send + Sync>,
    cache: Mutex<Cache>,
}

#[derive(Default)]
struct Cache {
    map: HashMap<String, Option<String>>,
    order: VecDeque<String>,
}

impl InstructionFiles {
    pub fn new(
        store: Arc<MemoryStore>,
        global_path: PathBuf,
        enabled: Arc<dyn Fn() -> bool + Send + Sync>,
    ) -> Self {
        Self {
            store,
            global_path,
            enabled,
            cache: Mutex::new(Cache::default()),
        }
    }

    /// 未命中路径的加载体（纯函数语义：读盘 + 拼段，无隐藏状态）。
    fn load_uncached(&self, session_id: Option<&str>) -> Option<String> {
        let mut sections: Vec<String> = Vec::new();
        if let Some(text) = read_capped(&self.global_path) {
            sections.push(section(&self.global_path.display().to_string(), text));
        }
        // 项目级：归属会话的仓库根，先找业界共识文件名 AGENTS.md，回落
        // .Locowrk/AGENTS.md（F2 交接目录——同一仓库两处皆认，先到先得）。
        if let Some(repo) = self.store.repo_root(session_id) {
            let candidates = [
                repo.join("AGENTS.md"),
                repo.join(crate::repo_context::DIR).join("AGENTS.md"),
            ];
            for cand in &candidates {
                if let Some(text) = read_capped(cand) {
                    sections.push(section(&cand.display().to_string(), text));
                    break;
                }
            }
        }
        if sections.is_empty() {
            return None;
        }
        Some(format!(
            "# agentsMd\nCodebase and user instructions are shown below. Be sure \
             to adhere to these instructions. IMPORTANT: These instructions \
             OVERRIDE any default behavior and you MUST follow them exactly as \
             written.\n\n{}",
            sections.join("\n\n")
        ))
    }
}

impl InstructionSource for InstructionFiles {
    fn load(&self, session_id: Option<&str>) -> Option<String> {
        if !(self.enabled)() {
            return None;
        }
        // 缓存键 = (session, scope)：归属变更 = 换键 = 重算（㉘·A·fix
        // 同款键位纪律；全局文件在 ("", scope) 下照常缓存——scope 空 = 键
        // 同，命中进程级一份）。
        let key = format!(
            "{}\u{1}{}",
            session_id.unwrap_or(""),
            self.store.scope_of_session(session_id)
        );
        let mut cache = self.cache.lock().expect("instruction cache poisoned");
        if let Some(hit) = cache.map.get(&key) {
            return hit.clone();
        }
        let loaded = self.load_uncached(session_id);
        if !cache.map.contains_key(&key) {
            cache.order.push_back(key.clone());
            while cache.order.len() > CACHE_CAP {
                if let Some(old) = cache.order.pop_front() {
                    cache.map.remove(&old);
                }
            }
        }
        cache.map.insert(key, loaded.clone());
        loaded
    }
}

/// 单文件段标题（ZCode agentsMd 同款形状）。
fn section(path: &str, text: String) -> String {
    format!("Contents of {path} (workspace instructions):\n\n{text}")
}

/// 读文件（缺文件 / 空文件 = None；超限截断 + 标记）。二进制内容不探测
/// ——UTF-8 lossy 照收（指令文件坏了如实呈现，不静默丢弃，设计稿 §9.3）。
fn read_capped(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    if bytes.is_empty() {
        return None;
    }
    let truncated = bytes.len() > MAX_FILE_BYTES;
    let mut text = String::from_utf8_lossy(&bytes[..bytes.len().min(MAX_FILE_BYTES)]).into_owned();
    if truncated {
        text.push_str("\n\n[File truncated: instruction files are capped at 100KB]");
    }
    Some(text)
}

#[cfg(test)]
mod tests;
