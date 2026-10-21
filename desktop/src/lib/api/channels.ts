/** **事件通道表与它的订阅入口**（结构闸对账的就是这张表）：一份通道名清单 + 通用订阅、
 * 审批结果订阅、审批请求订阅三个出口。
 *
 * ⚠ 这张表在本仓**只许有一个定义处**，且闸是在前端整根里按锚文本找它的（台账 T23）——
 * 所以本说明书故意不复述表的那一行声明式写法：闸剥注释时不剥块注释的续行，抄进来就会把
 * 定义处判成两个文件。表里每条通道名都有后端常量、映射、stores 开关表三面与它对账，
 * 漏一条不是"少订阅一个事件"，是当场判红。 */

import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { AgentEvent, ApprovalRequest } from "../../types";
const CHANNELS = [
  "agent://chunk",
  "agent://tool-call",
  "agent://tool-result",
  "agent://turn-done",
  "agent://context-usage",
  "agent://context-compacted",
  // ⑭ 定时触发（搭车修复：chat store 有处理分支但本表漏列的死分支）
  "agent://schedule-fired",
  // ㉕·对话区①：审批结果投影（决策记录条 + 摘除待决卡）
  "agent://approval-resolved",
  // v2.57 §3.9：推理 delta 直推（实时与重放同形，chat store append-or-create）
  "agent://thinking",
  // v2.58 §3.12：单步 LLM 用量（实时与重放同形，chat store 按轮聚合）
  "agent://usage",
  // v4.11 §3.39①：ask_user 选择卡（请求 + 四路同源收束，asks store 消费）
  "agent://ask-user",
  "agent://ask-resolved",
  // LLM.md v3.0：传输重试（流式重置）+ 单轮步数到顶告知
  "agent://llm-retry",
  "agent://turn-capped",
  // G2 崩溃恢复：用户裁决的实时回推（收束决策卡 + 闭合工具行）。
  // tool_uncertain 只在会话重放产出（session_history 返回数组），不订阅。
  "agent://uncertain-resolved",
  // 队列 Q12·C1：工具行相位（实时专属，审批请求一发出就点亮等待态）。
  "agent://tool-status",
  // 队列 Q12·C1：本轮文件改动汇总（轮末投影，实时与重放同形）。
  "agent://file-changes",
  // 队列 Q12·第二刀：轮头（收束状态 + 扣过等待的工作时长，轮末投影同形）。
  "agent://turn-header",
  // specs/12 刀②：自动压缩的过程状态（正在压 / 没压成 / 熔断停手 / 压成了）。
  // 实时专属（不进会话日志 ⇒ 重放不产生）。
  "agent://compact-status",
] as const;

export function subscribeAgentEvents(
  onEvent: (e: AgentEvent) => void,
): Promise<UnlistenFn[]> {
  return Promise.all(
    CHANNELS.map((ch) => listen<AgentEvent>(ch, (e) => onEvent(e.payload))),
  );
}

/** 审批结果订阅（㉕·对话区①）：approvals store 专用——待决卡摘除；
 * 决策记录条由 chat store 的全量订阅消费同一通道。 */
export function subscribeApprovalResolved(
  onResolved: (e: Extract<AgentEvent, { kind: "approval_resolved" }>) => void,
): Promise<UnlistenFn[]> {
  return listen<AgentEvent>("agent://approval-resolved", (e) =>
    onResolved(e.payload as Extract<AgentEvent, { kind: "approval_resolved" }>),
  ).then((un) => [un]);
}

export function subscribeApprovals(
  onRequest: (req: ApprovalRequest) => void,
): Promise<UnlistenFn[]> {
  return listen<ApprovalRequest>("agent://approval-request", (e) =>
    onRequest(e.payload),
  ).then((un) => [un]);
}
