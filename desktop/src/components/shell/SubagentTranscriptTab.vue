<script setup lang="ts">
/**
 * 子 Agent 转录页签（UI shell 文档 §3.16）：工作面板内的**只读**转录——
 * 数据 = `subagent_transcript` 全量投影（与 session_history 同一
 * project_events，跨重启可回看）。渲染 = 主对话消息流共享核心
 * TranscriptFlow（v2.104 用户裁定：子代理工作过程的分层折叠消息流 =
 * 主对话整个流程原样复用——任务容器/动作块相位/复制钮判据全同形）。
 * 只读差异：不传 editableIds（无编辑）、running/stopRequested 不传
 * （快照 = 历史重放同形——计时行恒冻结态「✓ 已工作」，与主对话重放
 * 同形）、审批决策行不投影（v2.105 裁定：与主对话 v2.98 同规全隐藏）、
 * 系统条动作不上接。底部固定只读说明——只读面必须自证，不然用户以为
 * 能插话。
 */
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { subagentTranscript } from "../../lib/api";
import { projectSubTranscript } from "../../lib/sub-transcript-items";
import type { ChatItem } from "../../stores/chat";
import TranscriptFlow from "../chat/TranscriptFlow.vue";

const props = defineProps<{ childId: string }>();
const { t } = useI18n();

const items = ref<ChatItem[]>([]);
const state = ref<"loading" | "ready" | "missing">("loading");

onMounted(async () => {
  try {
    const events = await subagentTranscript(props.childId);
    items.value = projectSubTranscript(events);
    state.value = "ready";
  } catch {
    state.value = "missing";
  }
});

const noticeText = computed(() => t("panel.readonlyNote"));
</script>

<template>
  <div class="subtab flex min-h-0 flex-1 flex-col">
    <div class="subtab-scroll scroll-y min-h-0 flex-1 px-3 py-2">
      <p v-if="state === 'loading'" class="px-1 py-4 text-center text-[length:var(--lw-fs-12)] text-ink-4">
        {{ t("panel.loading") }}
      </p>
      <div v-else-if="state === 'missing'" class="px-1 py-4 text-center text-[length:var(--lw-fs-12)] text-ink-4">
        {{ t("panel.missing") }}
      </div>
      <TranscriptFlow v-else :items="items" />
    </div>
    <!-- 只读自证（§3.16）：composer 不渲染，固定说明条 -->
    <div
      class="shrink-0 border-t border-line-subtle px-3 py-2 text-[length:var(--lw-fs-12)] text-ink-4"
    >
      {{ noticeText }}
    </div>
  </div>
</template>

<style scoped>
/* 折叠遮罩的宿主面 = 本面板列底（bg-dock），覆盖 TranscriptFlow 内
 * MessageRow 的缺省 --lw-bg——遮罩渐变底部要叠出气泡的真实表面色 */
.subtab-scroll {
  --lw-msg-host: var(--lw-bg-dock);
}
</style>
