// ask_user 选择卡状态（v4.11 §3.39①）：订阅 agent://ask-user 推送（chat
// store 全量订阅同通道转发，此处不再单开 listen），启动时补拉未决请求，
// respond/skip 回填并本地摘除。卡片渲染在 composer 坞（AskCard.vue）。
import { defineStore } from "pinia";
import { askRespond, listPendingAsks } from "../lib/api";
import type { AskUserEvent } from "../types";

export const useAsksStore = defineStore("asks", {
  state: () => ({
    pending: [] as AskUserEvent[],
  }),
  getters: {
    /** 与审批卡同族：头行排队数 = 除本卡之外的队列长度。 */
    queueCount: (state) => Math.max(0, state.pending.length - 1),
  },
  actions: {
    /** 启动补拉（订阅由 chat store 的全量通道转发，无独立 listen）。 */
    async init() {
      const events = await listPendingAsks();
      this.pending = events.filter(
        (e): e is AskUserEvent => e.kind === "ask_user",
      );
    },
    /** 实时请求入列（chat store ask_user 分支转发；按 request_id 去重）。 */
    push(event: AskUserEvent) {
      if (!this.pending.some((p) => p.request_id === event.request_id)) {
        this.pending.push(event);
      }
    },
    /** 四路同源摘卡信号（应答/跳过/超时/停止），chat store 转发。 */
    resolve(requestId: string) {
      this.pending = this.pending.filter((p) => p.request_id !== requestId);
    },
    /** 回填选择（labels + 自由补充）；命令报错抛给调用方（卡内联报错）。
     * 本地摘除置后：失败时卡保持可重试。 */
    async answer(requestId: string, labels: string[], note: string) {
      await askRespond(requestId, { kind: "answer", labels, note });
      this.resolve(requestId);
    },
    /** 跳过（卡上「跳过」键 / Esc 链的 skip 档）。 */
    async skip(requestId: string) {
      await askRespond(requestId, { kind: "skip" });
      this.resolve(requestId);
    },
  },
});
