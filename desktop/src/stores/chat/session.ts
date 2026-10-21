// stores/chat/session.ts —— 会话生命周期与列表层（T22 刀二搬出）。
// 只管"哪条会话、它的标题/归属/存亡，以及切进去时把日志重放成视图"；
// 会话级**配置写穿**（模型 / 推理档 / 权限档）在 ./config.ts，发送与排队在 ./send.ts。
import {
  pickSavePath,
  sessionAssociate,
  sessionDelete,
  sessionExport,
  sessionHistory,
  sessionList,
  sessionRename,
  subscribeAgentEvents,
  subscribeSessionTitle,
} from "../../lib/api";
import { applyEvent } from "./projection";
import type { ChatState } from "./state";
import type { AgentEvent } from "../../types";

export async function init(st: ChatState) {
  await subscribeAgentEvents((e) => applyEvent(st, e));
  // v2.57 自动标题：旁车落盘后推送（侧栏 + 顶栏即时换名）
  void subscribeSessionTitle((p) => applySessionTitle(st, p.sessionId, p.title));
  // ㉕·对话区①（用户裁定）：启动落首页空态——不再自动接最近会话，
  // 继续哪条由用户从侧栏点开；列表照常拉（侧栏要有内容）。
  await refreshSessions(st);
}

/** 拉会话列表（⑲）：init 与每轮收束后刷新。 */
export async function refreshSessions(st: ChatState) {
  try {
    st.sessions = await sessionList();
  } catch {
    /* 列表拉取失败不打断聊天 */
  }
}

/** 切换会话（⑲）：清视图 → 全量投影重放（恢复气泡/工具卡/系统
 * 通知）。运行中不允许切（会话锁内轮次未收束，投影会半截）。 */
export async function switchSession(st: ChatState, id: string) {
  if (id === st.sessionId || st.running) return;
  st.sessionId = id;
  st.items = [];
  st.steps = null;
  st.contextUsage = null;
  st.compactSummary = "";
  // §3.39②：队列是会话级 UI 态，切会话即清（诚实——预览不跨会话漂移）
  st.queue = [];
  st.preempt = null;
  st.stopRequested = false;
  st.dispatching = false;
  st.queueDraining = true;
  // ㉘·B：档位随会话走（无显式设置 = null，显示取设置缺省）
  st.permissionMode =
    st.sessions.find((s) => s.id === id)?.permissionMode ?? null;
  // v2.58：模型覆盖随会话走（成对字段，null = 跟随设置缺省）
  const meta = st.sessions.find((s) => s.id === id);
  st.modelOverride =
    meta?.model && meta?.modelProvider
      ? { providerId: meta.modelProvider, model: meta.model }
      : null;
  st.thinkingLevel = meta?.thinkingLevel ?? null;
  st.turnUsage = null;
  // M0 观测：会话累计随会话切换重置（重放同源重建），告警标记同理
  st.sessionUsage = null;
  st.cacheAlertFired = false;
  st.pendingWorkspace = null; // 挂起归属只属于触发它的那个新会话
  const events = await sessionHistory(id).catch(() => [] as AgentEvent[]);
  st.replaying = true;
  try {
    for (const e of events) applyEvent(st, e);
  } finally {
    st.replaying = false;
  }
}

/** 重命名会话（v2.49 §3.6）：后端旁车标题（trim 后空 = 回落自动
 * 标题），列表层生效。 */
export async function renameSession(st: ChatState, id: string, title: string) {
  await sessionRename(id, title);
  await refreshSessions(st);
}

/** §3.11 v3.9 工作区 chip 首页语义：直接改挂起归属（首聊自动关联）。 */
export function setPendingWorkspace(st: ChatState, ws: string | null) {
  st.pendingWorkspace = ws;
}

/** 工作区关联（v2.50 §2.3）：workspace = fsRoots 根目录，null =
 * 移出；后端校验必须是其一根，列表层生效。 */
export async function associateSession(
  st: ChatState,
  id: string,
  workspace: string | null,
) {
  await sessionAssociate(id, workspace);
  await refreshSessions(st);
}

/** 删除会话（v2.49）：后端忙锁守卫（运行中默认拒绝，开发者开关可
 * 强制）。删的是当前会话 = 回首页空态（newSession 语义，首条消息
 * 前不产生后端状态）。 */
export async function deleteSession(st: ChatState, id: string) {
  await sessionDelete(id);
  if (id === st.sessionId) newSession(st);
  await refreshSessions(st);
}

/** 导出会话（v2.49）：原生存储对话框 → Markdown 转录；取消 = null。 */
export async function exportSession(
  st: ChatState,
  id: string,
): Promise<string | null> {
  const meta = st.sessions.find((s) => s.id === id);
  const base =
    (meta?.title ?? id)
      .replace(/["\/:*?<>|]/g, "_")
      .trim()
      .slice(0, 40) || id;
  const path = await pickSavePath(`${base}.md`);
  if (!path) return null;
  return sessionExport(id, path);
}

/** 新会话（⑲）：前端生成 id 清空视图；首条消息发出前不创建任何
 * 后端状态（空会话不落盘不进列表，§3.6）。`workspace` = §2.3 v3.9
 * 「在项目下新建任务」：记账挂起，首条消息建会话后自动关联（§3.6）。 */
export function newSession(st: ChatState, workspace?: string | null) {
  if (st.running) return;
  st.sessionId = `desktop-${Date.now()}`;
  st.items = [];
  st.steps = null;
  st.contextUsage = null;
  st.compactSummary = "";
  st.queue = [];
  st.preempt = null;
  st.stopRequested = false;
  st.dispatching = false;
  st.queueDraining = true;
  st.permissionMode = null;
  st.modelOverride = null;
  st.thinkingLevel = null;
  st.turnUsage = null;
  st.sessionUsage = null;
  st.cacheAlertFired = false;
  st.pendingWorkspace = workspace ?? null;
}

/** 自动标题落地（v2.57）：只更新列表层标题（顶栏数据源同源）。 */
export function applySessionTitle(st: ChatState, id: string, title: string) {
  const row = st.sessions.find((s) => s.id === id);
  if (row) row.title = title;
}
