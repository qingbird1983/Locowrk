//! 文件视图命令（specs/08 批 1）：薄适配层——判定与读取在 [`crate::file_view`]，
//! 本文件只做参数直通与 blocking 池转派（文件 IO 不进异步运行时线程）。

use crate::file_view::{read_text_file, FileTextDto};

/// 按路径读文本给右栏「文件」页签：只读 / 有大小上限（缺省 256 KiB）/ 拒二进制。
#[tauri::command]
pub async fn file_read_text(path: String, max_bytes: Option<u64>) -> Result<FileTextDto, String> {
    tokio::task::spawn_blocking(move || read_text_file(&path, max_bytes))
        .await
        .map_err(|e| format!("读取任务失败: {e}"))?
}
