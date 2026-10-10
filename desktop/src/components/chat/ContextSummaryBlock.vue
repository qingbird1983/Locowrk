<script setup lang="ts">
/**
 * 整理段落总结块（v2.141，Organize Summary Design v1）：断层缝合处的
 * 轮级总结——此前工作段的浓缩叙事，落日志的历史消息面（实时与重放同形）。
 * 缺省折叠只露首行预览，点开看全文（总结可到数百字，整段平铺会淹没转录）。
 * 淡色左对齐块——它是内容不是 UI 陈述，比 SystemNotice 宽、不起气泡。
 */
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";

const props = defineProps<{ text: string }>();
const { t } = useI18n();

const open = ref(false);
const lines = computed(() =>
  props.text.split("\n").filter((l) => l.trim().length > 0),
);
const preview = computed(() => lines.value[0] ?? "");
</script>

<template>
  <div class="ctx-summary w-full rounded-[var(--lw-r-md)] border border-line bg-wash-1 px-4 py-2">
    <button
      class="flex w-full items-center gap-2 text-left"
      :title="t('memory.summaryHint')"
      @click="open = !open"
    >
      <span class="shrink-0 text-[length:var(--lw-fs-11)] font-medium text-ink-3">
        {{ t("memory.summaryBlock") }}
      </span>
      <span
        v-if="!open"
        class="min-w-0 flex-1 truncate text-[length:var(--lw-fs-11)] text-ink-5"
      >
        {{ preview }}
      </span>
      <span class="ml-auto shrink-0 text-[length:var(--lw-fs-10)] text-ink-5">
        {{ open ? t("memory.summaryCollapse") : t("memory.summaryExpand") }}
      </span>
    </button>
    <div
      v-if="open"
      class="mt-2 space-y-1.5 border-t border-line pt-2"
    >
      <p
        v-for="(l, i) in lines"
        :key="i"
        class="text-xs leading-relaxed text-ink-2"
      >
        {{ l }}
      </p>
    </div>
  </div>
</template>
