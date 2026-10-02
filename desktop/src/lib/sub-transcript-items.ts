/**
 * 子转录投影（UI shell 文档 §3.16，v2.52）：`subagent_transcript` 的 wire
 * 事件序列 → chat store 同款 `ChatItem[]`——页签复用 MessageRow / ToolRow
 * **同一套行组件**（§3.7–3.9 同一心智模型）。只读一次性投影：数据来自
 * 全量重放（整段 chunk，无流式时序）。近似如实记录：平铺过程行，不做
 * §3.7 过程组合成组（ChatTranscript 的 walk 逻辑，后续批次接入）。
 */
import type { AgentEvent } from "../types";
import type { ChatItem, ToolItem } from "../stores/chat";

export function projectSubTranscript(events: AgentEvent[]): ChatItem[] {
  const items: ChatItem[] = [];
  let id = 1;
  for (const e of events) {
    switch (e.kind) {
      case "user_message":
        items.push({ id: id++, kind: "user", text: e.content });
        break;
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
          pending: false,
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
      // 审批在父侧决策，子转录里只是记录（记录条形态照旧）
      case "approval_resolved":
        items.push({
          id: id++,
          kind: "system",
          key: e.allowed ? "notice.approvalAllowed" : "notice.approvalDenied",
          params: { action: e.tool },
        });
        break;
      default:
        break; // turn_done / usage / schedule_fired 等：页签不渲染
    }
  }
  return items;
}
