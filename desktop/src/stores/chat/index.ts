// 聊天状态（DESIGN.md §10.2：store 管状态，组件只渲染）。
// UI 事件是会话日志的投影（后端 runner.rs），本 store 只消费不生产。
/**
 * T22（2026-10-18）把这个 1148 行的单体拆成了同目录的功能片：
 *   types.ts / state.ts / ids.ts / getters.ts —— 形状、状态面、发号、三个派生读数
 *   projection.ts —— `AgentEvent` → items 的分发表（唯一投影，实时与重放共用）
 *   notices.ts —— 系统通知条四个生产口 + M0 命中率告警
 *   session.ts —— 会话生命周期与列表层 · config.ts —— 会话级配置写穿
 *   attachments.ts —— 附件预览条 · send.ts —— 发送 / 斜杠命令 / 停止 / 排队
 * 本文件只剩**装配 + 薄转发**，公开面（`useChatStore` 的 39 个 action、3 个 getter、
 * 21 个 state 字段、7 个导出类型）一条没减 ⇒ 41 处消费点一行没改。
 *
 * 为什么是「片函数收 `st: ChatState`、action 里转发」而不是「actions 直接拆到各片用 `this`」：
 * 后者要给每片写 `this: ChatStore`，而 ChatStore 的类型又来自本文件的 defineStore
 * ⇒ 类型自引用环（TS 7022 / 2502 一族，vue-tsc 会直接看不见成员）。收 ChatState 只依赖
 * state.ts 那一份声明，片与片之间变成**显式函数调用**（projection → send → session），
 * 谁调谁一眼看得见，不再靠 `this` 这层隐式全通道。
 *
 * 参数一律叫 `st` 不叫 `s`：原实现内部的 `.find((s) => …)` 用的是 `s`，同名会形成遮蔽。
 * 转发处的 `chatSend.send(this, …)` 读起来像递归，其实是命名空间导入的那个函数——
 * 所以每片都用 `import * as chatXxx`，歧义在名字里就断掉。
 */
import * as chatAttachments from "./attachments";
import * as chatConfig from "./config";
import * as chatNotices from "./notices";
import * as chatProjection from "./projection";
import * as chatSend from "./send";
import * as chatSession from "./session";
import { chatGetters } from "./getters";
import { initialChatState } from "./state";
import { defineStore } from "pinia";
import type { AgentEvent } from "../../types";

// 类型面在 ./types.ts；这里把它按**原 chat.ts 的公开面**原样转发。
// 外部真 import 过的只有四条（实测 grep）：ChatItem / SystemItem / ToolItem / AttachmentItem。
// S4 刀②/③c：`SummaryItem` 与 `context_summary` 通道整链退役（整理线不再存在）。
export type {
  AttachmentItem,
  ChatItem,
  SessionUsage,
  SystemItem,
  ToolItem,
  TurnUsage,
} from "./types";

export const useChatStore = defineStore("chat", {
  state: initialChatState,
  getters: chatGetters,
  actions: {
    // ---- 会话生命周期与列表层（./session.ts）----
    init() {
      return chatSession.init(this);
    },
    refreshSessions() {
      return chatSession.refreshSessions(this);
    },
    switchSession(id: string) {
      return chatSession.switchSession(this, id);
    },
    renameSession(id: string, title: string) {
      return chatSession.renameSession(this, id, title);
    },
    setPendingWorkspace(ws: string | null) {
      chatSession.setPendingWorkspace(this, ws);
    },
    associateSession(id: string, workspace: string | null) {
      return chatSession.associateSession(this, id, workspace);
    },
    deleteSession(id: string) {
      return chatSession.deleteSession(this, id);
    },
    exportSession(id: string) {
      return chatSession.exportSession(this, id);
    },
    newSession(workspace?: string | null) {
      chatSession.newSession(this, workspace);
    },
    applySessionTitle(id: string, title: string) {
      chatSession.applySessionTitle(this, id, title);
    },
    // ---- 会话级配置写穿（./config.ts）----
    setModel(providerId: string, model: string) {
      return chatConfig.setModel(this, providerId, model);
    },
    clearModelOverride() {
      return chatConfig.clearModelOverride(this);
    },
    setThinking(level: string | null) {
      return chatConfig.setThinking(this, level);
    },
    setPermissionMode(mode: "plan" | "ask" | "acceptEdits" | "auto") {
      return chatConfig.setPermissionMode(this, mode);
    },
    resolveUncertain(
      callId: string,
      decision: "completed" | "not_executed" | "inspected",
      note = "",
    ) {
      return chatConfig.resolveUncertain(this, callId, decision, note);
    },
    // ---- 事件投影（./projection.ts，实时与重放同一张表）----
    onEvent(e: AgentEvent) {
      chatProjection.applyEvent(this, e);
    },
    appendAssistant(delta: string) {
      chatProjection.appendAssistant(this, delta);
    },
    resetAssistant() {
      chatProjection.resetAssistant(this);
    },
    settleThinking() {
      chatProjection.settleThinking(this);
    },
    appendThinking(text: string) {
      chatProjection.appendThinking(this, text);
    },
    // ---- 通知条与观测告警（./notices.ts）----
    checkCacheAlert(su: Parameters<typeof chatNotices.checkCacheAlert>[1]) {
      chatNotices.checkCacheAlert(this, su);
    },
    pushNotice(
      key: string,
      params?: Record<string, unknown>,
      actions?: string[],
    ) {
      chatNotices.pushNotice(this, key, params, actions);
    },
    pushRawSystem(text: string) {
      chatNotices.pushRawSystem(this, text);
    },
    // ---- 附件预览条（./attachments.ts）----
    addAttachmentFile(file: File) {
      return chatAttachments.addAttachmentFile(this, file);
    },
    addAttachmentPath(path: string) {
      return chatAttachments.addAttachmentPath(this, path);
    },
    removeAttachment(key: number) {
      chatAttachments.removeAttachment(this, key);
    },
    dropImageAttachments() {
      chatAttachments.dropImageAttachments(this);
    },
    settleAttachments() {
      return chatAttachments.settleAttachments(this);
    },
    // ---- 发送 / 斜杠命令 / 停止 / 排队（./send.ts）----
    send(text: string, parts?: Parameters<typeof chatSend.send>[2]) {
      return chatSend.send(this, text, parts);
    },
    submit(text: string) {
      return chatSend.submit(this, text);
    },
    stop() {
      return chatSend.stop(this);
    },
    removeQueued(id: number) {
      chatSend.removeQueued(this, id);
    },
    clearQueue() {
      chatSend.clearQueue(this);
    },
    sendQueuedNow(id: number) {
      return chatSend.sendQueuedNow(this, id);
    },
    resumeDrain() {
      chatSend.resumeDrain(this);
    },
    dispatchQueue() {
      return chatSend.dispatchQueue(this);
    },
    editResend(pos: number | null, text: string) {
      return chatSend.editResend(this, pos, text);
    },
  },
});
