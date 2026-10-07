<script setup lang="ts">
/**
 * 记忆树 · 项目联合视图（M6.4，树 v2 §11.4）：当前项目**跨会话**全部捆
 * 平铺（最近活动会话优先，会话名标注；memory_tree_project 同源数据，
 * 也是 memory_search_titles 工具的候选池）。只读——点捆跳回会话档展开
 * （编辑/删除在会话档，操作逻辑单点复用）；搜索词由父级传入同吃。
 */
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { memoryTreeProject } from "../../lib/api";
import { relativeTime } from "../../lib/format";
import type { ProjectChapterView } from "../../types";

const { t } = useI18n();

const props = defineProps<{
  workspace: string;
  search: string;
}>();

const emit = defineEmits<{
  /** 点捆 → 跳会话档（父级切档 + 选中会话 + 展开该捆）。 */
  open: [sessionId: string, chapterId: string];
}>();

const rows = ref<ProjectChapterView[] | null>(null);
const loadError = ref("");

const filtered = computed(() => {
  const q = props.search.trim().toLowerCase();
  const list = rows.value ?? [];
  if (!q) return list;
  return list.filter(
    (b) =>
      b.title.toLowerCase().includes(q) ||
      b.summary.toLowerCase().includes(q) ||
      b.sessionTitle.toLowerCase().includes(q),
  );
});

onMounted(async () => {
  try {
    rows.value = await memoryTreeProject(props.workspace);
  } catch (e) {
    loadError.value = String(e);
    rows.value = [];
  }
});

function sessionLabel(b: ProjectChapterView): string {
  return b.sessionTitle || b.sessionId.slice(0, 8);
}

function timeText(ms: number): string {
  const r = relativeTime(ms);
  return r.n === undefined
    ? t(`memory.time.${r.key}`)
    : t(`memory.time.${r.key}`, { n: r.n });
}
</script>

<template>
  <p
    v-if="loadError"
    class="mt-2 rounded-[var(--lw-r-xs)] bg-danger/10 px-3 py-2 text-xs text-danger"
  >
    {{ loadError }}
  </p>
  <p
    v-else-if="rows !== null && rows.length === 0"
    class="mt-2 text-[length:var(--lw-fs-11)] text-ink-5"
  >
    {{ t("memory.projectEmpty") }}
  </p>
  <p
    v-else-if="filtered.length === 0"
    class="mt-2 text-[length:var(--lw-fs-11)] text-ink-5"
  >
    {{ t("memory.noMatch") }}
  </p>
  <ul v-else class="mt-2 space-y-1.5">
    <li
      v-for="b in filtered"
      :key="`${b.sessionId}\u0000${b.chapterId}`"
      class="rounded-[var(--lw-r-md)] bg-wash-1 px-4 py-2.5"
    >
      <button
        class="flex w-full items-center gap-2 text-left"
        :title="t('memory.projectOpenHint')"
        @click="emit('open', b.sessionId, b.chapterId)"
      >
        <span class="min-w-0 flex-1 truncate text-xs font-medium text-ink-1">
          {{ b.title }}
        </span>
        <span
          v-if="b.titleEditedByUser || b.summaryEditedByUser"
          class="shrink-0 rounded-full bg-wash-3 px-2 py-0.5 text-[length:var(--lw-fs-10)] text-ink-4"
        >
          {{ t("memory.treeManual") }}
        </span>
        <span
          class="max-w-[40%] shrink-0 truncate text-[length:var(--lw-fs-10)] text-ink-5"
        >
          {{ sessionLabel(b) }}
        </span>
        <span class="shrink-0 text-[length:var(--lw-fs-10)] text-ink-5">
          {{ timeText(b.createdAt) }}
        </span>
      </button>
    </li>
  </ul>
</template>
