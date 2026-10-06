/**
 * 子转录投影（UI shell 文档 §3.16，v2.52；v2.104 对齐）：`subagent_transcript`
 * 的 wire 事件序列 → chat store 同款 `ChatItem[]`——页签经共享核心
 * TranscriptFlow 渲染（主对话整个流程原样复用），投影按**主对话历史重放
 * 同形**补齐：用户消息带事件位与轮起点时刻（v2.101）、思考行整段落项
 * （无时戳 = 首句摘要形态）、轮收束时刻挂最后一条用户消息（第 0 层计时
 * 行读数终点）、悬空 tool_call 如实挂 pending（与重放一致）。只读一次性
 * 投影：数据来自全量重放（整段 chunk，无流式时序）。审批决策记录行
 * （已允许/已拒绝）随主对话 v2.98 同规**全隐藏**（v2.105 目检裁定：页签
 * = 主对话消息流原样，系统条审计形态退役），不投影。
 */
import type { AgentEvent } from "../types";
import type { ChatItem, ToolItem } from "../stores/chat";

export function projectSubTranscript(events: AgentEvent[]): ChatItem[] {
  const items: ChatItem[] = [];
  let id = 1;
  for (const e of events) {
    switch (e.kind) {
      case "user_message":
        items.push({
          id: id++,
          kind: "user",
          text: e.content,
          pos: e.pos,
          ...(e.ts != null ? { ts: e.ts } : {}),
        });
        break;
      case "thinking": {
        // v2.104 对齐主对话重放：整段落项，append-or-create（连续思考
        // 在 §3.7 过程组里本就成组）；无时戳 = 首句摘要形态
        if (!e.text) break;
        const last = items[items.length - 1];
        if (last?.kind === "thinking") last.text += e.text;
        else items.push({ id: id++, kind: "thinking", text: e.text });
        break;
      }
      case "chunk": {
        // 纯工具调用步正文为空（投影不发空 chunk）；连续段并入上一条
        if (!e.delta) break;
        const last = items[items.length - 1];
        if (last?.kind === "assistant") last.text += e.delta;
        else items.push({ id: id++, kind: "assistant", text: e.delta });
        break;
      }
      case "tool_call":
        items.push({
          id: id++,
          kind: "tool",
          callId: e.call_id,
          tool: e.tool,
          args: e.args,
          sideEffect: e.side_effect,
          pluginVersion: e.plugin_version,
          // 与主对话重放同形：结果未落（崩溃截断）如实挂 pending
          pending: true,
          ok: null,
        });
        break;
      case "tool_result": {
        const item = [...items]
          .reverse()
          .find((i): i is ToolItem => i.kind === "tool" && i.callId === e.call_id);
        if (item) {
          item.pending = false;
          item.ok = e.ok;
          item.output = e.output;
          item.error = e.error;
        }
        break;
      }
      case "turn_done": {
        // v2.101 对齐：轮收束时刻挂最后一条用户消息（计时行读数终点；
        // 已记不覆盖，旧日志无时刻不虚构）
        if (e.ts == null) break;
        const lastUser = [...items]
          .reverse()
          .find(
            (i): i is Extract<ChatItem, { kind: "user" }> => i.kind === "user",
          );
        if (lastUser && lastUser.endTs === undefined) lastUser.endTs = e.ts;
        break;
      }
      // 审批决策记录行（已允许/已拒绝）：随主对话 v2.98 同规全隐藏
      // （v2.105 目检裁定），不再以系统条形态保留
      case "approval_resolved":
        break;
      default:
        break; // usage / schedule_fired 等：页签不渲染
    }
  }
  return items;
}
