//! 文件视图读取（specs/08 批 1）：右栏「文件」页签要按路径看磁盘上的文本。
//! 只做三条判据——**只读**、**大小上限**、**二进制拒绝**；不做路径白名单
//! （理由在 `specs/08-artifacts-and-file-tabs/spec.md`：工具本来就按调用读写
//! 任意路径，界面加白名单只会造出「工具读得到、界面看不了」的新裂缝）。
//!
//! 上限缺省 256 KiB（设计稿 §7.2）：超了只回前缀并把 `truncated` 置真——界面
//! 显示「已截断，共 N」。大文件**不整读进内存**（`take` 到上限 + 嗅探窗口）。

use std::io::Read;

use serde::Serialize;

/// 缺省读取上限（字节）：设计稿 §7.2 的 256 KiB。
pub(crate) const DEFAULT_MAX_BYTES: u64 = 256 * 1024;

/// 二进制嗅探窗口（字节）：只扫最前面这些，不整读。
const SNIFF_BYTES: usize = 8 * 1024;

/// 送给前端的文件文本快照。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FileTextDto {
    /// 回显调用方给的路径（原样，不归一）。
    pub path: String,
    /// 文件真实字节数（不是读回来的长度）。
    pub bytes: u64,
    /// 文本内容；`binary` 为真时是空串。
    pub text: String,
    /// 文件比上限大：`text` 只是前缀。
    pub truncated: bool,
    /// 判为二进制（前 8 KiB 里有 NUL）。
    pub binary: bool,
}

/// 按路径读文本（纯逻辑；命令层只做参数直通）。
pub(crate) fn read_text_file(path: &str, max_bytes: Option<u64>) -> Result<FileTextDto, String> {
    let raw = path.trim();
    if raw.is_empty() {
        return Err("路径为空".into());
    }
    let p = std::path::Path::new(raw);
    let meta = std::fs::metadata(p).map_err(|e| format!("读不到这个文件: {e}"))?;
    if !meta.is_file() {
        return Err("这不是一个文件".into());
    }
    let total = meta.len();
    let cap = max_bytes.unwrap_or(DEFAULT_MAX_BYTES).max(1);
    let want = cap.max(SNIFF_BYTES as u64);
    let mut buf = Vec::new();
    std::fs::File::open(p)
        .and_then(|f| f.take(want).read_to_end(&mut buf))
        .map_err(|e| format!("读取失败: {e}"))?;
    if buf[..buf.len().min(SNIFF_BYTES)].contains(&0) {
        return Ok(FileTextDto {
            path: raw.to_string(),
            bytes: total,
            text: String::new(),
            truncated: false,
            binary: true,
        });
    }
    Ok(FileTextDto {
        path: raw.to_string(),
        bytes: total,
        text: String::from_utf8_lossy(&buf[..buf.len().min(cap as usize)]).into_owned(),
        truncated: total > cap,
        binary: false,
    })
}

#[cfg(test)]
mod tests;
