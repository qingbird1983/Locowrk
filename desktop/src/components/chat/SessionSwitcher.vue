<script setup lang="ts">
/**
 * 会话切换器（⑲，UI shell 文档 §3.6）：chat 视图头部的最小功能面——
 * 下拉（session_list mtime 降序，文案 = 首条用户消息截断，无则回退 id）
 * + 新会话按钮。切换 = 全量投影重放（store.switchSession）；会话管理类
 * UI（搜索/删除/改名）归 UI 正式设计阶段，不在此扩。
 */
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useChatStore } from "../../stores/chat";

const { t } = useI18n();
const chat = useChatStore();

const options = computed(() =>
  chat.sessions.map((s) => ({
    id: s.id,
    label: s.title?.trim() || s.id,
  })),
);

const currentLabel = computed(
  () => options.value.find((o) => o.id === chat.sessionId)?.label ?? chat.sessionId,
);

function onSwitch(e: Event) {
  const id = (e.target as HTMLSelectElement).value;
  void chat.switchSession(id);
}
</script>

<template>
  <div class="flex items-center gap-2 border-b border-line px-5 py-1.5">
    <select
      class="max-w-[50%] rounded-lg border border-line-strong bg-surface-1 px-2 py-1 text-xs"
      :value="chat.sessionId"
      :aria-label="t('chat.sessionSwitch')"
      :disabled="chat.running"
      @change="onSwitch"
    >
      <option v-if="!options.some((o) => o.id === chat.sessionId)" :value="chat.sessionId">
        {{ currentLabel }}
      </option>
      <option v-for="o in options" :key="o.id" :value="o.id">{{ o.label }}</option>
    </select>
    <button
      class="rounded-lg border border-line-strong px-2 py-1 text-xs text-ink-3 transition hover:bg-surface-1 disabled:cursor-not-allowed disabled:opacity-40"
      :disabled="chat.running"
      @click="chat.newSession()"
    >
      {{ t("chat.newSession") }}
    </button>
    <span class="truncate text-[11px] text-ink-5">{{ chat.sessionId }}</span>
  </div>
</template>
