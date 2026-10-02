// 审批中心状态（§13.1 审批中心的前端半边）：订阅 approval-request 推送，
// 启动时补拉未决请求，decide 回填并本地移除。卡片本体由会话日志投影
// （chat store 的工具行）呈现；待决卡由转录内联渲染（§3.17 PermissionCard，
// ㉕·对话区① 起 ApprovalModal 退役）。
import { defineStore } from "pinia";
import {
  approve,
  listPendingApprovals,
  subscribeApprovalResolved,
  subscribeApprovals,
} from "../lib/api";
import type { ApprovalRequest } from "../types";

/** 决策粒度（§3.17 三键）：deny=拒绝；once=允许一次；session=允许本次
 * 对话（后端豁免集，本会话同工具不再问）。 */
export type ApprovalDecisionKind = "deny" | "once" | "session";

export const useApprovalsStore = defineStore("approvals", {
  state: () => ({
    pending: [] as ApprovalRequest[],
  }),
  getters: {
    /** 头行排队数（还有 N 个待处理 = 除本卡之外的队列长度）。 */
    queueCount: (state) => Math.max(0, state.pending.length - 1),
  },
  actions: {
    async init() {
      // 先补拉再订阅：两者之间到达的请求由订阅回调去重兜底
      this.pending = await listPendingApprovals();
      await subscribeApprovals((req) => {
        if (!this.pending.some((p) => p.request_id === req.request_id)) {
          this.pending.push(req);
        }
      });
      // 审批结果投影（㉕·对话区①）：超时/他路收束时前端也要摘卡——
      // 决策记录条由 chat store 消费同一事件，这里只管待决队列
      await subscribeApprovalResolved((e) => {
        this.pending = this.pending.filter((p) => p.request_id !== e.call_id);
      });
    },
    /** 三键决策（§3.17）：命令报错时抛给调用方（卡内联报错、恢复可点）。 */
    async decide(req: ApprovalRequest, decision: ApprovalDecisionKind) {
      const allowed = decision !== "deny";
      await approve(
        req.request_id,
        allowed,
        decision === "session" ? "session" : undefined,
      );
      this.pending = this.pending.filter(
        (p) => p.request_id !== req.request_id,
      );
    },
  },
});
