<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import { useApprovalsStore } from "../stores/approvals";
import { useChatStore } from "../stores/chat";
import type { ApprovalRequest } from "../types";

const { t } = useI18n();
const approvals = useApprovalsStore();
const chat = useChatStore();

// ⑮ 子 Agent 派生的调用（session_id 非当前会话）打标识徽章——
// 冒泡到同一审批门的请求，用户得知道是谁在要权限
function isSubagent(req: ApprovalRequest): boolean {
  return !!req.session_id && req.session_id !== chat.sessionId;
}

// 点击后到后端回填前的短暂窗口内禁用按钮，防双击重复决策
const deciding = ref(new Set<string>());

function pretty(v: unknown): string {
  try {
    return JSON.stringify(v, null, 2);
  } catch {
    return String(v);
  }
}

async function decide(req: ApprovalRequest, allowed: boolean) {
  deciding.value.add(req.request_id);
  try {
    await approvals.decide(req, allowed);
  } finally {
    deciding.value.delete(req.request_id);
  }
}
</script>

<template>
  <!-- 遮罩层：审批未决时阻断下方输入（纪律 9：不可逆调用必须人工决策）；
       入场动效 = §六动效②（遮罩淡入 + 面板轻缩放） -->
  <Transition name="lw-modal">
    <div
      v-if="approvals.pending.length > 0"
      class="fixed inset-0 z-40 flex items-center justify-center px-4 backdrop-blur-sm"
      style="background: var(--lw-scrim)"
    >
      <div
        v-for="req in approvals.pending"
        :key="req.request_id"
        class="lw-modal-panel w-full max-w-md overflow-hidden rounded-xl border border-danger/30 bg-elevated shadow-dialog"
        role="alertdialog"
        aria-modal="true"
      >
        <!-- 头部：风险信号（过渡形态；§3.17 内联决策卡落地后本 Modal 退役） -->
        <div
          class="flex items-center gap-2 border-b border-danger/20 bg-danger/10 px-4 py-3"
        >
          <span class="text-lg text-danger">⚠</span>
          <div>
            <h2 class="text-sm font-semibold text-danger">
              {{ t("approval.title") }}
            </h2>
            <p class="text-[11px] text-danger/80">
              {{ t("approval.subtitle") }}
            </p>
          </div>
          <span
            v-if="isSubagent(req)"
            class="ml-auto rounded bg-purple/16 px-2 py-0.5 text-[10px] font-medium text-purple"
          >
            {{ t("approval.subagentTag") }}
          </span>
          <span
            class="rounded bg-danger/16 px-2 py-0.5 text-[10px] font-medium text-danger"
            :class="{ 'ml-0': isSubagent(req), 'ml-auto': !isSubagent(req) }"
          >
            {{ t(`effect.${req.side_effect}`) }}
          </span>
        </div>

        <!-- 工具与参数 -->
        <div class="space-y-3 px-4 py-3 text-xs">
          <div class="flex items-baseline gap-2">
            <span class="text-ink-4">{{ t("approval.toolLabel") }}</span>
            <code class="font-mono font-semibold text-warn">
              {{ req.tool }}
            </code>
          </div>
          <div>
            <p class="mb-1 text-[10px] uppercase tracking-wider text-ink-4">
              {{ t("approval.argsLabel") }}
            </p>
            <pre
              class="max-h-48 overflow-auto rounded bg-inset p-2 font-mono text-[11px] text-ink-2"
              >{{ pretty(req.args) }}</pre
            >
          </div>
        </div>

        <!-- 决策按钮：主按钮 = accent（§3.17 纪律：风险由染底与胶囊承载，
             按钮不承险）；拒绝 = secondary -->
        <div class="flex justify-end gap-2 border-t border-line px-4 py-3">
          <button
            class="rounded-lg bg-wash-2 px-4 py-1.5 text-xs font-medium text-ink-2 transition hover:bg-wash-3 hover:text-ink-1 disabled:cursor-not-allowed disabled:opacity-40"
            :disabled="deciding.has(req.request_id)"
            @click="decide(req, false)"
          >
            {{ t("approval.deny") }}
          </button>
          <button
            class="rounded-lg bg-accent px-4 py-1.5 text-xs font-semibold text-accent-ink transition hover:bg-accent-hover disabled:cursor-not-allowed disabled:opacity-40"
            :disabled="deciding.has(req.request_id)"
            @click="decide(req, true)"
          >
            {{ t("approval.allow") }}
          </button>
        </div>
      </div>
    </div>
  </Transition>
</template>
