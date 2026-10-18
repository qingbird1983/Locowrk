/** **会话在磁盘上的进出**（v2.49 / v2.57 §3.34）：删除、正文搜索、导出转录、编辑重发的截断。
 * 四条都以 session 开头、都直接动会话日志这一份磁盘文件，与 sessions 片"读投影与写档位"分家。 */

import { invoke } from "@tauri-apps/api/core";
import type { SessionSearchHit } from "../../types";
/** 会话删除（v2.49）：忙锁守卫在后端（运行中默认拒绝，开发者开关可
 * 强制）。幂等。 */
export function sessionDelete(sessionId: string): Promise<void> {
  return invoke("session_delete", { sessionId });
}

/** 会话正文搜索（v2.49）：大小写不敏感子串，mtime 降序限量。 */
export function sessionSearch(query: string, limit?: number): Promise<SessionSearchHit[]> {
  return invoke("session_search", { query, limit: limit ?? null });
}

/** 会话导出 Markdown 转录（v2.49）：path 由 pickSavePath 选定。返回路径。 */
export function sessionExport(sessionId: string, path: string): Promise<string> {
  return invoke("session_export", { sessionId, path });
}

/** 消息编辑重发的截断步（v2.57 §3.34）：日志截到 pos 之前（None = 最后
 * 一条用户消息）；忙锁守卫在后端（运行中拒绝）。重发由调用方走 chat()。 */
export function sessionTruncateAfter(
  sessionId: string,
  pos: number | null,
): Promise<void> {
  return invoke("session_truncate_after", { sessionId, pos });
}
