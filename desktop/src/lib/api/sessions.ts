/** **会话的投影与档位**（⑲ / v2.49 §3.6 / v2.50 §2.3 / ㉘·B §3.33 / v2.58 §3.30 / 批 B §5.2）：
 * 列表、全量历史投影、崩溃恢复裁决、重命名、与工作区关联、权限模式、模型覆盖、推理档位。
 *
 * 中间那条"在文件管理器里打开项目目录"寄居在本片：它校验的是与关联会话同一份根清单，
 * 原文也把它排在关联与档位之间 ⇒ 保序搬运不许重排（要挪它是语义改动，另刀另证）。
 * 会话的删除/搜索/导出/截断在 session-io 片（模型面隔在两段中间，合不成一片）。 */

import { invoke } from "@tauri-apps/api/core";
import type { AgentEvent, SessionMeta } from "../../types";
/** 会话列表（⑲）：sessions/ 目录 mtime 降序；空会话无文件不出现。 */
export function sessionList(): Promise<SessionMeta[]> {
  return invoke("session_list");
}

/** 会话全量投影（⑲）：整个日志按 runner 同一映射转 UI 事件序列，
 * 前端重放恢复；历史不走 agent:// 事件通道。 */
export function sessionHistory(sessionId: string): Promise<AgentEvent[]> {
  return invoke("session_history", { sessionId });
}

/** 崩溃恢复裁决（G2）：对 uncertain 悬空调用记录用户的事实认定
 * （completed / not_executed / inspected + 检查记录），后端落日志并
 * 回推 uncertain_resolved + 合成 tool_result 收束卡与工具行。 */
export function sessionResolveUncertain(
  sessionId: string,
  callId: string,
  decision: "completed" | "not_executed" | "inspected",
  note?: string,
): Promise<void> {
  return invoke("session_resolve_uncertain", {
    sessionId,
    callId,
    decision,
    note: note ?? "",
  });
}

/** 会话重命名（v2.49 §3.6）：旁车 meta.json 标题；空串 = 回落自动标题。 */
export function sessionRename(sessionId: string, title: string): Promise<void> {
  return invoke("session_rename", { sessionId, title });
}

/** 会话↔工作区关联（v2.50 §2.3）：workspace = fsRoots 根目录，null =
 * 移出；后端校验必须是其一根。 */
export function sessionAssociate(
  sessionId: string,
  workspace: string | null,
): Promise<void> {
  return invoke("session_associate", { sessionId, workspace });
}

/** 在文件管理器中打开项目目录（§2.3 v3.9 项目菜单）：后端校验 fsRoots。 */
export function revealInExplorer(path: string): Promise<void> {
  return invoke("reveal_in_explorer", { path });
}

/** 会话级权限模式（㉘·B §3.33）：写旁车 + 模式存储缓存，即时生效；
 * 四档取值后端校验（plan / ask / acceptEdits / auto）。 */
export function sessionSetPermissionMode(
  sessionId: string,
  mode: "plan" | "ask" | "acceptEdits" | "auto",
): Promise<void> {
  return invoke("session_set_permission_mode", { sessionId, mode });
}

/** 会话级模型覆盖（v2.58 §3.30）：成对写旁车 + 缓存，下一轮即生效；
 * model 空串 = 清除覆盖（回落设置缺省）。 */
export function sessionSetModel(
  sessionId: string,
  providerId: string,
  model: string,
): Promise<void> {
  return invoke("session_set_model", { sessionId, providerId, model });
}

/** 会话级推理档位（批 B §5.2）：写旁车 thinking_level + 缓存，下一请求即
 * 生效；level 空串 = 清除（模型走自家缺省档）。语义档由后端校验，档位
 * 在当前模型梯子里没有写法时由解析链 clamp（就近降档/剥除）。 */
export function sessionSetThinking(sessionId: string, level: string): Promise<void> {
  return invoke("session_set_thinking", { sessionId, level });
}
