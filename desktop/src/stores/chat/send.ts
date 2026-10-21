// stores/chat/send.ts —— 发送、斜杠命令、停止与排队（§3.39，v4.11）。
// 这一族是**唯一会往会话里发东西**的一片：send 起轮、submit 决定直发还是入队、
// stop 打断、dispatchQueue 在轮次收束后消费打断目标与队首。
// 排队是前端态（不入会话日志）：切会话由 session.ts 清空，重放里不会出现。
import { agentStop, chat, compactNow, sessionTruncateAfter } from "../../lib/api";
import { isSlashCommand } from "../../lib/slash-commands";
import { removeAttachment } from "./attachments";
import {
  compactSkip,
  dropCompactRunning,
  pushCompactNotice,
  pushRawSystem,
} from "./notices";
import { refreshSessions } from "./session";
import { nextId } from "./ids";
import type { ChatState } from "./state";
import type { ContentPartDto } from "../../types";

export async function send(st: ChatState, text: string, parts?: ContentPartDto[]) {
  const t = text.trim();
  if (!t && !(parts && parts.length)) return;
  if (st.running) return;
  st.stopRequested = false;
  // §3.39②：手动发送恢复自动派发（停止/出错后的暂停态由用户解）
  st.queueDraining = true;
  st.items.push({
    id: nextId(),
    kind: "user",
    text: t,
    ts: Date.now(),
    ...(parts?.length ? { parts } : {}),
  });
  st.running = true;
  st.steps = null;
  st.turnUsage = null;
  // §3.6 v3.9 项目下新建任务：挂起归属随首轮请求带入（㉘·A·fix 归属
  // 先行）——后端建会话即写旁车，首轮注入/围栏/提取就在正确键位上；
  // 旧时序（轮次收束后才 sessionAssociate）有首查键位缝，已退役。
  const ws = st.pendingWorkspace;
  st.pendingWorkspace = null;
  try {
    await chat(st.sessionId, t, ws, parts);
  } catch (err) {
    // 失败轮次不发 turn_done（后端约定），在这里收尾；§3.39② 队列
    // 转暂停（恢复键由用户按，失败后不打连发）。附件条不清——后端
    // 门控拒发（UnsupportedModality）后换模型重试还要用。
    st.items.push({ id: nextId(), kind: "error", text: String(err) });
    st.queueDraining = false;
    st.running = false;
    void refreshSessions(st);
    void dispatchQueue(st);
    return;
  }
  st.running = false;
  // 发送成功后清空预览条（引用已随消息入会话日志，asset 常驻存储）
  for (const it of [...st.attachments]) removeAttachment(st, it.key);
  // ⑲ 新会话首聊后进列表（列表更新不阻塞输入）
  void refreshSessions(st);
  // §3.39②：轮次收束（成功/失败）→ 消费打断目标 / 自动派发
  void dispatchQueue(st);
}

// ---- §3.39 停止与排队（v4.11）----

/** 统一提交口（composer Enter/发送键）：斜杠命令拦截 → 空闲直发，
 * 运行中入队（§3.39②）。 */
export async function submit(st: ChatState, text: string) {
  const t = text.trim();
  if (!t) return;
  // S2 刀⑦（AC-S2-11）：`/压缩` = **当场跑一次主层深度压缩**，不再触发记忆树那套
  // 章节整理（旧名 `/整理` 到此退役，两家参照的同位按钮都叫 compact）。元操作照旧
  // 不进对话流、不建用户气泡；发丝线形态沿用 v2.143 那一根，只是 tag 换成 compact。
  // 压成了什么**不走返回值**：后端 emit 一条 `agent://context-compacted`，前端那条
  // 断层条（省了多少 / [查看详情] / [撤销]）与自动压缩共用同一个构造——所以这里
  // 压成时只把在途线收走，不留两句同义的话。
  if (isSlashCommand(t, "压缩")) {
    // 摘要正文会流式回来（后端那条 `delta` 相位），手动这一趟**没有** `running`
    // 相位可借 ⇒ 摘要累加器在这儿清一次（自动那一趟在 `projection.ts` 的
    // `running` 相位清）。不清的话这一趟的正文会接在上一趟的尾巴上。
    st.compactSummary = "";
    // 这根线带一个「查看详情」：摘要流式到达时挂在它上面（同自动那一根的形态）。
    pushCompactNotice(st, "notice.compactRunning", undefined, ["details"]);
    try {
      const out = await compactNow(st.sessionId);
      if (out.compacted) dropCompactRunning(st);
      else {
        const skip = compactSkip(out.reason);
        pushCompactNotice(st, skip.key, skip.params);
      }
    } catch (err) {
      // 发起线随失败一并收走（宁缺勿假——不留悬着的「压缩中」线），
      // 错误原文照常出原文系统条。
      dropCompactRunning(st);
      pushRawSystem(st, String(err));
    }
    return;
  }
  if (st.running) {
    st.queue.push({ id: nextId(), text: t });
    return;
  }
  await send(st, t);
}

/** 停止生成（§3.39④）：后端流式截断 + 待决卡 fail-closed；队列转
 * 暂停（draining=false），「已停止」头行由 stopRequested 驱动。 */
export async function stop(st: ChatState) {
  if (!st.running) return;
  st.stopRequested = true;
  st.queueDraining = false;
  try {
    await agentStop(st.sessionId);
  } catch (err) {
    // 命令失败如实回滚（轮次仍在跑，不停留在假停止态）
    st.stopRequested = false;
    if (st.queue.length > 0) st.queueDraining = true;
    pushRawSystem(st, String(err));
  }
}

/** 删除一条排队消息。 */
export function removeQueued(st: ChatState, id: number) {
  st.queue = st.queue.filter((q) => q.id !== id);
}

/** 排队卡头垃圾桶（§3.39② v4.11 参考图）：一键清空全部排队。 */
export function clearQueue(st: ChatState) {
  st.queue = [];
}

/** 「立即发送」= 打断通道（§3.39②）：移出队列 → 停止当前轮 →
 * 收束后立即发送该条（其余保持排队）。空闲时直接发。 */
export async function sendQueuedNow(st: ChatState, id: number) {
  const idx = st.queue.findIndex((q) => q.id === id);
  if (idx < 0) return;
  const [item] = st.queue.splice(idx, 1);
  if (st.running) {
    st.preempt = item.text;
    await stop(st);
  } else {
    await send(st, item.text);
  }
}

/** 恢复自动派发（暂停态的「继续」键）。 */
export function resumeDrain(st: ChatState) {
  st.queueDraining = true;
  void dispatchQueue(st);
}

/** 轮次收束后的派发口：先消费打断目标，再逐条自动派发队首（一条
 * 一轮——send 收束后再进下一轮）。 */
export async function dispatchQueue(st: ChatState) {
  if (st.dispatching) return;
  if (st.preempt && !st.running) {
    st.dispatching = true;
    const text = st.preempt;
    st.preempt = null;
    try {
      await send(st, text);
    } finally {
      st.dispatching = false;
    }
  }
  while (!st.running && st.queueDraining && st.queue.length > 0) {
    st.dispatching = true;
    const head = st.queue.shift()!;
    try {
      await send(st, head.text);
    } finally {
      st.dispatching = false;
    }
  }
}

/** 消息编辑重发（v2.57 §3.34）：后端截断到该消息之前 → 本地从该气泡
 * 起丢条目 → 走既有 send 通道重发（审批/用量/提取零新链路）。
 * `pos = null` = 当前最后一条用户消息（实时发送无事件位的解析口）。
 * 批 D：原消息 parts 随重发（asset 引用复用，零重新上传）。 */
export async function editResend(
  st: ChatState,
  pos: number | null,
  text: string,
) {
  const t = text.trim();
  if (!t || st.running) return;
  await sessionTruncateAfter(st.sessionId, pos);
  let idx = -1;
  if (pos === null) {
    for (let i = st.items.length - 1; i >= 0; i--) {
      if (st.items[i]!.kind === "user") {
        idx = i;
        break;
      }
    }
  } else {
    idx = st.items.findIndex((i) => i.kind === "user" && i.pos === pos);
  }
  let parts: ContentPartDto[] | undefined;
  if (idx >= 0) {
    const it = st.items[idx]!;
    if (it.kind === "user" && it.parts?.length) parts = it.parts;
    st.items.splice(idx);
  }
  await send(st, t, parts);
}
