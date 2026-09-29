// 审批中心状态（§13.1 审批中心的前端半边）：订阅 approval-request 推送，
// 启动时补拉未决请求，decide 回填并本地移除。卡片本体由会话日志投影
// （chat store 的 tool 卡）呈现，这里只管弹窗队列。
import { defineStore } from "pinia";
import { approve, listPendingApprovals, subscribeApprovals } from "../lib/api";
import type { ApprovalRequest } from "../types";

export const useApprovalsStore = defineStore("approvals", {
  state: () => ({
    pending: [] as ApprovalRequest[],
  }),
  actions: {
    async init() {
      // 先补拉再订阅：两者之间到达的请求由订阅回调去重兜底
      this.pending = await listPendingApprovals();
      await subscribeApprovals((req) => {
        if (!this.pending.some((p) => p.request_id === req.request_id)) {
          this.pending.push(req);
        }
      });
    },
    async decide(req: ApprovalRequest, allowed: boolean) {
      await approve(req.request_id, allowed);
      this.pending = this.pending.filter(
        (p) => p.request_id !== req.request_id,
      );
    },
  },
});
