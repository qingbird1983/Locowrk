// stores/chat/projection.ts —— `agent/event` / 会话日志 → UI 条目的**分发表**（T22 刀二下沉）。
// 只做一件事：把一个 `AgentEvent` 投影成 items 上的读写。它不发请求、不碰后端、
// 不读设置（唯一的例外 = 转交给 notices.ts 的告警，那里才碰 settings/toast）。
// 实时增量与历史重放**共用这一张表**（§10.2「UI 事件是会话日志的投影」的前端半边），
// 所以分支里到处是重放守卫（读 state 上的 replaying 标记）：重放不虚构时序、不吃「默认展开」。
// 台账 T3 实测过这张表的覆盖度（21 个事件变体 ↔ 22 条 case，`check-channels` 盯着），
// 分片之后那张闸照样吃得到——它递归扫 `desktop/src/stores/`，本文件在面内。
import { compactNumber } from "../../lib/format";
import { useAsksStore } from "../asks";
import { nextId } from "./ids";
import { checkCacheAlert, pushNotice } from "./notices";
import { dispatchQueue } from "./send";
import type { ChatState } from "./state";
import type { ChatItem, ToolItem } from "./types";
import type { AgentEvent } from "../../types";

export function applyEvent(st: ChatState, e: AgentEvent) {
  switch (e.kind) {
    case "user_message":
      // ㉕·fix 重放恢复用户气泡（此前投影漏 UserMessage，重启后历史
      // 会话只剩模型回复）。实时轮发送即本地回显，本事件不实时到达。
      // pos（v2.57）：编辑重发的定位键随行保存。ts（v2.101）：轮起点
      // 时刻随重放到达（旧日志无 → 不虚构，计时行诚实省略读数）。
      // v2.58 §3.12：新轮开始——用量聚合重置（重放同序重建）
      st.turnUsage = null;
      st.items.push({
        id: nextId(),
        kind: "user",
        text: e.content,
        pos: e.pos,
        ...(e.ts != null ? { ts: e.ts } : st.replaying ? {} : { ts: Date.now() }),
        ...(e.parts?.length ? { parts: e.parts } : {}),
      });
      break;
    case "thinking":
      // v2.57 §3.9：推理行点亮（实时 delta 与重放整段同形，追加到
      // 末尾思考行或新起一行——连续思考行在 §3.7 过程组里本就成组）
      appendThinking(st, e.text);
      break;
    case "chunk":
      appendAssistant(st, e.delta);
      break;
    case "tool_call":
      settleThinking(st);
      st.items.push({
        id: nextId(),
        kind: "tool",
        callId: e.call_id,
        tool: e.tool,
        args: e.args,
        sideEffect: e.side_effect,
        pluginVersion: e.plugin_version,
        status: "running",
        ok: null,
        ...(st.replaying ? {} : { ts: Date.now() }),
      });
      // ⑮ 派生不再发「已派生子 Agent」告知条（v2.105 目检裁定：两条
      // 状态盒退役，简洁状态收进动作名后——TranscriptFlow L1 极简行）
      break;
    case "tool_result": {
      const item = st.items.find(
        (i): i is ToolItem => i.kind === "tool" && i.callId === e.call_id,
      );
      if (item) {
        item.status = "settled";
        item.ok = e.ok;
        item.output = e.output;
        item.error = e.error;
        if (!st.replaying) item.doneTs = Date.now();
        // ⑮ 收束不再发「子 Agent 收束」告知条（v2.105 目检裁定，同上
        // ——状态与摘要看工具行明细 / 子代理卡 / 转录页签）
      }
      break;
    }
    case "tool_status":
      // 行相位补投（Q12·C1）：审批请求一发出就把那条在途调用点亮。
      // 只认 pending_approval——后端刻意不给其余取值，成败与悬空由
      // tool_result / tool_uncertain 各自负责（不许一个事实两个出处）。
      // 结果已回（乱序/竞态）时**不许**回退成等待态，故加 status 守卫。
      {
        const item = st.items.find(
          (i): i is ToolItem => i.kind === "tool" && i.callId === e.call_id,
        );
        if (item && item.status === "running" && e.status === "pending_approval") {
          item.status = "awaiting_approval";
        }
      }
      break;
    case "turn_header":
      // 轮头（Q12·第二刀 v2.205）：状态与扣过等待的时长挂 user 行——与 files /
      // endTs 同一个容器、同一条「找不到 user 行 = 调度轮 ⇒ 不虚构归属」的规矩。
      // `active_ms` 为 null/缺省时**不写 activeMs**：缺 = "数据层读不出"（旧日志
      // 没有审批两时刻），计时行据此回落到墙钟读数；把它折算成 0 就是谎报
      // "这轮一秒没干活"（与 ±N 的 NULL 档同一条纪律）。
      // 顺序上后端保证 turn_header → file_changes → turn_done，本 case 因此
      // 早于 endTs 落位；两者互不覆盖（一个写状态/时长，一个写收束时刻）。
      {
        const head = [...st.items].reverse().find((i) => i.kind === "user");
        if (head && head.kind === "user") {
          head.turnState = e.state;
          if (e.active_ms != null) head.activeMs = e.active_ms;
        }
      }
      break;
    case "file_changes":
      // 本轮文件改动（Q12·C1）：**不作为独立行**，挂在轮头容器上——
      // 与 endTs 挂 user 行同一做法（TranscriptFlow 按 user 行分轮）。
      // 找不到 user 行 = 调度轮（没有用户消息，计时行本就无从起算，同一
      // 缺口）⇒ 不虚构归属、不造暂存死数据，如实不挂。⚠ 订正（v2.205）：
      // 这里原先写「等 TurnHeader 那一刀的轮级容器再接手」，实落形状 =
      // **轮级容器就是 user 行的 `turnState` / `activeMs` / `files` 三个字段**
      // （上一条 case），没有另造一种独立行——CF §2.1 说的"轮头"是读数容器，
      // 不是新行种（同类只许一种形态：分轮仍以 user 行为界，不并列两套）。
      {
        const head = [...st.items].reverse().find((i) => i.kind === "user");
        if (head && head.kind === "user") {
          head.files = e.files;
        }
      }
      break;
    case "tool_uncertain":
      // G2 崩溃恢复：悬空未裁决——行进「结果未知」态 + 组外决策卡。
      // 只在重放产出（悬空只在崩溃后出现）；找不到宿主行（声明位
      // 缺失的极端形态）卡照出，细节缺失如实渲染。
      {
        const item = st.items.find(
          (i): i is ToolItem => i.kind === "tool" && i.callId === e.call_id,
        );
        if (item) item.uncertain = true;
        st.items.push({
          id: nextId(),
          kind: "uncertain",
          callId: e.call_id,
          tool: e.tool,
          args: e.args,
        });
      }
      break;
    case "uncertain_resolved":
      // G2：裁决落日志——卡收为记录行、行复位（随后的合成 tool_result
      // 同源闭合行明细；找不到行/卡时各自静默，重放幂等）。
      {
        const item = st.items.find(
          (i): i is ToolItem => i.kind === "tool" && i.callId === e.call_id,
        );
        if (item) item.uncertain = false;
        const card = st.items.find(
          (i): i is Extract<ChatItem, { kind: "uncertain" }> =>
            i.kind === "uncertain" && i.callId === e.call_id,
        );
        if (card) card.resolved = { decision: e.decision, note: e.note };
      }
      break;
    case "turn_done":
      st.running = false;
      // steps（v2.101 起可选）：TurnEnd 的重放投影不带，保留 null 如实
      st.steps = e.steps ?? st.steps;
      // v2.101：轮收束时刻挂在最后一条用户消息上（第 0 层计时行重放
      // 读数终点）。调度轮没有用户消息——找不到就跳过，不虚构；
      // endTs 已有（该轮已记过）不覆盖。
      if (e.ts != null) {
        for (let i = st.items.length - 1; i >= 0; i--) {
          const it = st.items[i]!;
          if (it.kind === "user") {
            if (it.endTs === undefined) it.endTs = e.ts;
            break;
          }
        }
      }
      // §3.39②：轮次收束 → 消费打断目标 / 自动派发队首
      void dispatchQueue(st);
      break;
    case "schedule_fired":
      // ⑭ 定时触发：SystemNotice 告知（无动作按钮；触发的助手回复
      // 以 chunk 跟进。窗口不在位时后端另发系统通知兜底）
      pushNotice(st, "notice.scheduleFired", { title: e.title });
      break;
    case "approval_resolved":
      // §3.7 v4.11（裁定①）：决策记录 = 过程组内的平铺行（无卡片底、
      // 无独立气泡；被拒调用的失败工具行由 tool_result 分支自理）。
      // 投影唯一来源（实时增量与历史重放同源，无双条）。
      st.items.push({
        id: nextId(),
        kind: "decision",
        tool: e.tool,
        allowed: e.allowed,
      });
      break;
    case "ask_user":
      // §3.39①：选择卡走 composer 坞（asks store），对话流零气泡。
      // ask 事件不进会话日志（与审批请求同纪律），重放不会出现。
      useAsksStore().push(e);
      break;
    case "ask_resolved":
      // 四路同源摘卡（应答/跳过/超时/停止）
      useAsksStore().resolve(e.request_id);
      break;
    case "llm_retry":
      // LLM.md v3.0 传输重试：清掉本步未完成的流式缓冲（重试轮 delta
      // 从零重建同一气泡），平铺状态行如实告知（不吓人也不装没事）
      resetAssistant(st);
      if (!st.replaying) {
        pushNotice(st, "notice.llmRetried", { attempt: e.attempt });
      }
      break;
    case "turn_capped":
      // 单轮步数到顶（不再硬报错）：如实告知 + 「继续」指引
      pushNotice(st, "notice.turnCapped", { steps: e.steps });
      break;
    case "context_usage":
      st.contextUsage = {
        used: e.used,
        threshold: e.threshold,
        windowTokens: e.window_tokens ?? 0,
        toolsEstTokens: e.tools_est_tokens ?? 0,
      };
      break;
    case "llm_usage": {
      // v2.58 §3.12：按轮累计（speed = 最后一步生成速度）
      const u = st.turnUsage ?? {
        input: 0, output: 0, cacheRead: 0, cacheWrite: 0,
        reasoning: 0, total: 0, speed: null, steps: 0,
      };
      u.input += e.input;
      u.output += e.output;
      u.cacheRead += e.cache_read;
      u.cacheWrite += e.cache_write;
      u.reasoning += e.reasoning;
      u.total += e.total;
      u.steps += 1;
      if (e.duration_ms > 0) {
        u.speed = Math.round((e.output / e.duration_ms) * 1000);
      }
      st.turnUsage = u;
      // M0 观测（KV v2 §7）：会话累计同步累加——与轮级重置解耦，
      // 重放同源重建，供命中率/节省/告警消费
      const su = st.sessionUsage ?? {
        input: 0, output: 0, cacheRead: 0, cacheWrite: 0,
        reasoning: 0, total: 0, steps: 0,
      };
      su.input += e.input;
      su.output += e.output;
      su.cacheRead += e.cache_read;
      su.cacheWrite += e.cache_write;
      su.reasoning += e.reasoning;
      su.total += e.total;
      su.steps += 1;
      st.sessionUsage = su;
      checkCacheAlert(st, su);
      break;
    }
    case "context_compacted": {
      // 压缩系统消息（Context 文档 §3.8.3）：[查看详情][撤销]，
      // 撤销经 context_undo_compact（rewind_to，ChatMessages 接线）
      const id = nextId();
      const saved = Math.max(0, e.from_tokens - e.to_tokens);
      st.items.push({
        id,
        kind: "system",
        key: "notice.contextCompacted",
        params: { saved: compactNumber(saved), strategy: e.strategy },
        actions: ["details", "undo"],
      });
      if (e.summary_preview) st.noticeDetails[id] = e.summary_preview;
      break;
    }
  }
}

export function appendAssistant(st: ChatState, delta: string) {
  settleThinking(st);
  const last = st.items[st.items.length - 1];
  if (last && last.kind === "assistant") {
    last.text += delta;
  } else {
    st.items.push({
      id: nextId(),
      kind: "assistant",
      text: delta,
      ...(st.replaying ? {} : { ts: Date.now() }),
    });
  }
}

/** 流式重置（LLM.md v3.0）：传输重试前清空末尾未完成的流式正文——
 * 重试轮的 delta 从零重建同一气泡。末项不是流式正文时无事可做。 */
export function resetAssistant(st: ChatState) {
  const last = st.items[st.items.length - 1];
  if (last && last.kind === "assistant") {
    last.text = "";
  }
}

/** 思考行收口（§3.9 v4.11）：下一条过程项到达时，给末尾仍在"思考中"
 * 的行补 doneTs（`持续了 N 秒`）。重放条目无 ts，天然空转。 */
export function settleThinking(st: ChatState) {
  if (st.replaying) return;
  const last = st.items[st.items.length - 1];
  if (last && last.kind === "thinking" && last.ts !== undefined && last.doneTs === undefined) {
    last.doneTs = Date.now();
  }
}

/** 思考行追加（v2.57 §3.9）：实时 delta / 重放整段共用 append-or-create
 * ——与 appendAssistant 同构；Thinking 事件在日志里从不相邻（每步后
 * 必有 AssistantMessage/ToolCall），重放不会误并两条。 */
export function appendThinking(st: ChatState, text: string) {
  if (!text) return;
  const last = st.items[st.items.length - 1];
  if (last && last.kind === "thinking") {
    last.text += text;
  } else {
    st.items.push({
      id: nextId(),
      kind: "thinking",
      text,
      ...(st.replaying ? {} : { ts: Date.now() }),
    });
  }
}
