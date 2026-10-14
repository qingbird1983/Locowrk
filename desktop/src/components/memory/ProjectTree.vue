<script setup lang="ts">
/**
 * 记忆树 · 项目联合视图（M6.4，树 v2 §11.4）：当前项目**跨会话**全部捆
 * 平铺（最近活动会话优先，会话名标注；memory_tree_project 同源数据，
 * 也是 memory_search_titles 工具的候选池）。只读——点捆跳回会话档展开
 * （编辑/删除在会话档，操作逻辑单点复用）；搜索词由父级传入同吃。
 * v2.141（Organize Summary Design v1 §3.5）：带上跨会话轮级总结链——
 * 时间正序的总结块即项目进展叙事锚点（来龙去脉按时间读下来）。
 */
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { memoryTreeProject } from "../../lib/api";
import TreeBadge from "./TreeBadge.vue";
import { relativeTimeLabel, relativeTimeMs } from "../../lib/format";
import type { ProjectChapterView, ProjectTreeView, RunSummaryView } from "../../types";

const { t } = useI18n();

const props = defineProps<{
  workspace: string;
  search: string;
}>();

const emit = defineEmits<{
  /** 点捆 → 跳会话档（父级切档 + 选中会话 + 展开该捆）。 */
  open: [sessionId: string, chapterId: string];
}>();

const view = ref<ProjectTreeView | null>(null);
const loadError = ref("");

const rows = computed<ProjectChapterView[]>(() => view.value?.chapters ?? []);

const filtered = computed(() => {
  const q = props.search.trim().toLowerCase();
  const list = rows.value;
  if (!q) return list;
  return list.filter(
    (b) =>
      b.title.toLowerCase().includes(q) ||
      b.summary.toLowerCase().includes(q) ||
      b.sessionTitle.toLowerCase().includes(q),
  );
});

/** 跨会话总结链（后端已按时间正序）；搜索词命中正文/会话名才显。 */
const filteredSummaries = computed<RunSummaryView[]>(() => {
  const list = view.value?.summaries ?? [];
  const q = props.search.trim().toLowerCase();
  if (!q) return list;
  return list.filter(
    (s) =>
      s.text.toLowerCase().includes(q) ||
      s.sessionTitle.toLowerCase().includes(q),
  );
});

onMounted(async () => {
  try {
    view.value = await memoryTreeProject(props.workspace);
  } catch (e) {
    loadError.value = String(e);
    view.value = { chapters: [], summaries: [] };
  }
});

function summarySessionLabel(s: RunSummaryView): string {
  return s.sessionTitle || s.sessionId.slice(0, 8);
}

function sessionLabel(b: ProjectChapterView): string {
  return b.sessionTitle || b.sessionId.slice(0, 8);
}

/** 章节/摘要是**毫秒**（`memory-tree` 的 `created_at: now_ms`）——旧代码把毫秒
 * 喂给按秒算的 `relativeTime`，diff 变负 ⇒ 恒命中 `minutes < 1`，这里此前**永远
 * 显示「刚刚」**（v2.168 批 F-18 修，登记为 F6 顺带发现②）。 */
function timeText(ms: number): string {
  return relativeTimeLabel(relativeTimeMs(ms), (k, p) => t(k, p));
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
    v-else-if="view !== null && rows.length === 0"
    class="mt-2 text-[length:var(--lw-fs-11)] text-ink-5"
  >
    {{ t("memory.projectEmpty") }}
  </p>
  <template v-else>
    <!-- 跨会话总结链（v2.141）：项目进展叙事锚点，时间正序读下来 -->
    <div v-if="filteredSummaries.length > 0" class="mt-2 space-y-1.5">
      <p class="text-[length:var(--lw-fs-11)] font-medium text-ink-4">
        {{ t("memory.projectSummaries") }}
      </p>
      <details
        v-for="(s, i) in filteredSummaries"
        :key="`proj-sum-${i}`"
        class="rounded-[var(--lw-r-md)] border border-line bg-wash-1 px-4 py-2"
      >
        <summary class="flex cursor-pointer items-center gap-2 text-[length:var(--lw-fs-11)] text-ink-3">
          <span>{{ t("memory.summaryBlock") }}</span>
          <span class="min-w-0 truncate text-ink-5">{{ summarySessionLabel(s) }}</span>
          <span class="ml-auto shrink-0 text-ink-5">{{ timeText(s.createdAt) }}</span>
        </summary>
        <p class="mt-1.5 whitespace-pre-wrap text-xs leading-relaxed text-ink-2">
          {{ s.text }}
        </p>
      </details>
    </div>
    <p
      v-if="filtered.length === 0"
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
          <TreeBadge
            v-if="b.titleEditedByUser || b.summaryEditedByUser"
            :text="t('memory.treeManual')"
          />
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
</template>
