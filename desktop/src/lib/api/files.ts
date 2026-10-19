/** **文件视图面**（specs/08 批 1）：按路径读磁盘上的文本，给右栏「文件」页签。
 *
 * 只有读这一条 —— 没有写、没有删除：改文件仍然走工具（specs/08「非目标」明写的边界）。 */

import { invoke } from "@tauri-apps/api/core";
import type { FileTextDto } from "../../types";

/** 按路径读文本（缺省上限 256 KiB，超了只回前缀并置 `truncated`；含 NUL ⇒ `binary`）。 */
export function fileReadText(path: string, maxBytes?: number): Promise<FileTextDto> {
  return invoke("file_read_text", { path, maxBytes });
}
