<script setup lang="ts">
/**
 * 子 Agent 转录页签（UI shell 文档 §3.16，v2.52）：工作面板内的**只读**
 * 转录——数据 = `subagent_transcript` 全量投影（与 session_history 同一
 * project_events，跨重启可回看），渲染复用 MessageRow / ToolRow 同一套
 * 行组件（§3.7–3.9 同一心智模型；近似：平铺过程行，过程组合成组后续批
 * 接入）。底部固定只读说明——只读面必须自证，不然用户以为能插话。
 * v2.102 同逻辑：折叠遮罩/气泡内悬浮钮/右下直角气泡经 MessageRow 自动
 * 同形；遮罩宿主面盖 bg-dock。
 */
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { subagentTranscript } from "../../lib/api";
import { projectSubTranscript } from "../../lib/sub-transcript-items";
import type { ChatItem } from "../../stores/chat";
import MessageRow from "../chat/MessageRow.vue";
import ToolRow from "../chat/ToolRow.vue";
import SystemNotice from "../chat/SystemNotice.vue";

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
      <template v-else>
        <template v-for="item in items" :key="item.id">
          <MessageRow
            v-if="item.kind === 'user' || item.kind === 'assistant'"
            :kind="item.kind"
            :text="item.text"
          />
          <ToolRow v-else-if="item.kind === 'tool'" :item="item" />
          <SystemNotice
            v-else-if="item.kind === 'system'"
            :text="item.key === '__raw__' ? String(item.params?.text ?? '') : t(item.key, item.params ?? {})"
          />
        </template>
      </template>
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
/* 折叠遮罩的宿主面 = 本面板列底（bg-dock），覆盖 MessageRow 缺省
 * --lw-bg——遮罩渐变底部要叠出气泡的真实表面色（v2.102） */
.subtab-scroll {
  --lw-msg-host: var(--lw-bg-dock);
}
</style>
