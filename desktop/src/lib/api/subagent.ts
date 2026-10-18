/** **子 Agent 转录**（v2.52 §3.16）：一条命令，投影映射与会话历史同源，文件缺席就报错。
 * 只有 5 行也单立一片：把它塞进通知或定时任何一边，那一片的名字就开始撒谎。 */

import { invoke } from "@tauri-apps/api/core";
import type { AgentEvent } from "../../types";
/** 子 Agent 转录（v2.52 §3.16）：sub-transcripts/<childId> 全量投影，
 * 与 session_history 同一映射；文件缺席 = reject（页签如实报不可用）。 */
export function subagentTranscript(childId: string): Promise<AgentEvent[]> {
  return invoke("subagent_transcript", { childId });
}
