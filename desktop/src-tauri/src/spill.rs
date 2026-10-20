//! 超预算工具输出的**落盘层**（specs/09 刀② · 方案稿 B 支撑层第一格"spill"）。
//!
//! 一句话：一条工具输出大过「工具结果预算」时，不再只把尾巴砍掉，而是把**全文**
//! 写进 `<数据根>/spill/<会话>/<sha256>.txt`，发给模型的上下文里留
//! `<persisted-output> 路径 + 前段预览 </persisted-output>`。
//!
//! 三件形状上的理由：
//!
//! * **判定单源**：尺寸判定只有发送侧那一处（`llm/wire/payload.rs` 的
//!   `project_tool_output`），本模块只负责"落到哪、写成什么样"。原来那条
//!   `clip_tool_output` 就是同一个判定的纯截断档——它的文案也被收进本模块，
//!   于是一次超预算只会有一个解释者，不会出现"截断层说 24K、落盘层说 20K"。
//! * **日志仍是真相源**：本模块只读 `&str`、只往自己的目录写，会话日志一行不动
//!   （AC-S2-3 第三条 = 落盘前后日志 sha 相同）。
//! * **前缀稳定**：文件名是内容的 sha256（复用 `assets::sha256_hex` 全仓那把唯一
//!   算式），同内容 ⇒ 同路径 ⇒ 占位文本逐字节相同。这正是 M1 前缀缓存纪律要的
//!   "重放两次 hash 相同"，而且是免费拿到的——不需要额外记账。
//!
//! 为什么不落在 `assets/` 里（这是取证时最容易做错的一步）：那里的 GC
//! （`assets/gc.rs:3-5`）判孤儿的办法是"文件名的 sha 有没有出现在会话日志文本里"，
//! 而 spill 文件的 sha **不会**出现在日志里（日志存的是工具输出原文，不是它的 sha）
//! ⇒ 每个 spill 文件天生就是孤儿，宽限期一过就被删，模型回头 `fs_read` 就读空。
//! 语义上也不同：`assets/` 是用户上传的资产，spill 是模型可再生的中间产物。
//!
//! 遗留（登记在 specs/09 刀② 那格）：**spill 目录的清理通路还没有**——按会话分子目录
//! 就是为了让"删会话顺手删它的 spill"成为可能，但那要动删除路径，属于禁区动作，
//! 等它那一刀单独点头。在此之前 spill 只会增长，量级是"每条超预算输出一个文件"。

use std::path::{Path, PathBuf};

/// 数据根下那一格的目录名。**路径算式只有这一处**（判据 SS17b）：宿主、测试、
/// 将来可能的清理通路都从这儿吃，写第二份迟早和这里漂。
const SPILL_DIR: &str = "spill";

/// 落盘句柄（组装点一份，经 `WireCaps` 带进发送侧投影）。
#[derive(Debug, Clone)]
pub struct SpillStore {
    root: PathBuf,
}

impl SpillStore {
    /// 以**数据根**为参（不是 spill 目录本身）：调用方不许自己 `join("spill")`，
    /// 那样目录名就长出了第二个出处。
    pub fn new(data_root: &Path) -> Self {
        Self {
            root: data_root.join(SPILL_DIR),
        }
    }

    /// 落一条超预算输出，返回**给模型读回的路径字符串**（绝对路径：`data_root`
    /// 由组装点给的就是绝对路径）。同内容同会话 = 幂等，已存在不重写
    /// （与 `AssetStore::put` 同一条纪律）。
    ///
    /// 会话 id 先过 `plugin_api::naming::valid_session_id`——它是本仓会话 id 合法性的
    /// **唯一判据**（`persist::session_file` 与 memory-tree 落盘用的都是它，批 D 为
    /// "两侧各写一份校验且已经分叉"立过这条单点）。这一道不是仪式：这个字符串
    /// 直接当目录名用，不校验就等于把路径穿越送到落盘通路上。
    pub fn put(&self, session_id: &str, content: &str) -> Result<String, String> {
        if !plugin_api::naming::valid_session_id(session_id) {
            return Err(format!("非法会话 id，不落盘: {session_id}"));
        }
        let dir = self.root.join(session_id);
        std::fs::create_dir_all(&dir).map_err(|e| format!("spill 目录创建失败: {e}"))?;
        let id = crate::assets::sha256_hex(content.as_bytes());
        let path = dir.join(format!("{id}.txt"));
        if !path.exists() {
            std::fs::write(&path, content).map_err(|e| format!("spill 写入失败: {e}"))?;
        }
        Ok(path.display().to_string())
    }
}

/// UTF-8 安全切点（从 `clip_tool_output` 原样搬出，**宿主发送侧只此一份**）：
/// 中文/emoji 按字节硬切会当场 panic，切点必须回退到字符边界。
pub(crate) fn char_boundary_cut(content: &str, limit: usize) -> usize {
    let mut cut = limit.min(content.len());
    while cut > 0 && !content.is_char_boundary(cut) {
        cut -= 1;
    }
    cut
}

/// 超预算但**没能落盘**（无句柄 / 写失败 / 无会话身份）时的降级文案：
/// 与改前的截断提示逐字节相同，等价性由 `llm/tests/tool_budget.rs` 那批用例钉着。
/// 降级必须看得见 ⇒ 调用方负责 warn，这里只管文本。
pub(crate) fn clipped(content: &str, cut: usize) -> String {
    let total = content.len();
    format!(
        "{}\n\n【工具结果已按「工具结果预算」截断：原输出 {total} 字节，此处只有前 {cut} 字节。\
         完整内容仍在本会话日志里；需要后面的部分请用同一工具的 offset / max_bytes 参数续取\
         （发送侧上限固定 24 KiB，不随设置变化）。】",
        &content[..cut]
    )
}

/// 落盘成功后的上下文形状（模板**只许这一处**，判据 SS17a）：开标签带路径与原字节数，
/// 标签内是前段预览 + 一句怎么读回全文。
///
/// 为什么让模型走 `fs_read` 而不是"同一工具续取"（改前那句提示教的办法）：
/// `shell` / `fs_grep` 这类输出没有 offset 语义，改前那种提示对它们等于"内容就是没了"；
/// 落盘之后任何超预算输出都变成一份可寻址文件，读回通路只剩 `fs_read` 一条。
/// 数据根本来就在归属会话的 `fs_roots` 围栏内（`workspace.rs:41`），读回不需要新权限。
///
/// ⚠ 文案里**不许出现 `offset`**：刀② 第一版写了"传 offset / max_bytes"，而
/// `fs_read` 的入参只有 `path` 与 `max_bytes`（`tool-fs/src/read.rs` 全量读入后
/// `truncate(max)`，manifest 的 schema 里没有 offset）——那等于教模型传一个不存在的参数，
/// 而且它拿到的仍然只是文件开头。现在的措辞说的是真事：默认 32 KiB、上限 1 MiB
/// （都大于缺省那枚 24 KiB 预算，所以一次读回就能拿到比预览更多的内容），
/// 超过 1 MiB 那一档读不干净、只在日志里，就如实说"超过的部分读不到"。
/// 给 `fs_read` 补 `offset` 是另一刀（要同时改 manifest 与 describe，SS7 那条对账会盯着）。
pub(crate) fn persisted(path: &str, preview: &str, cut: usize, total: usize) -> String {
    format!(
        "<persisted-output path=\"{path}\" total_bytes=\"{total}\" preview_bytes=\"{cut}\">\n\
         {preview}\n\
         【工具结果超过「工具结果预算」，完整输出已落盘在上面 path 指向的文件\
         （会话日志仍保原文，本文件只由发送侧投影产生）。上面只是前 {cut} 字节；\
         要看后面的部分用 fs_read 读该路径——它默认返回前 32 KiB、可传 max_bytes 提到 1 MiB，\
         但没有 offset，比它更靠后的部分读不回来。\
         （发送侧上限固定 24 KiB，不随设置变化）。】\n\
         </persisted-output>"
    )
}

#[cfg(test)]
mod tests;
