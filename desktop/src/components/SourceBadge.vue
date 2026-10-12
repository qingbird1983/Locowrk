<script setup lang="ts">
/**
 * 来源徽章（⑪，UI shell doc §3.4 徽章表"来源族"）：记忆条目的来源档位
 * （Context and Memory.md §4.5——用户必须知道每条记忆是谁记的）。后端只
 * 给结构化 source 字符串，前端按档位渲染 i18n 文案（§8.1 纪律）。
 */
import { computed } from "vue";
import { useI18n } from "vue-i18n";

const props = defineProps<{
  /** "explicit" | "extracted" | "organized"（其他值按灰色降级档渲染）。 */
  source: string;
}>();

const { t } = useI18n();

const label = computed(() => {
  if (props.source === "explicit") return t("memory.source_explicit");
  if (props.source === "extracted") return t("memory.source_extracted");
  if (props.source === "organized") return t("memory.source_organized");
  return props.source;
});

/** 配色：explicit 蓝 / extracted 紫 / organized 橙 / 未知灰（与 §3.4 其他族同尺寸）。
 *  organized 用 warn 是 §3.4「来源族」v2.186 新档的定色——整理副产物既不是用户写的
 *  也不是用户要求写的，得比"退役遗留"更显眼，人才挑得出该删哪条。 */
const tone = computed(() => {
  if (props.source === "explicit") return "bg-info/16 text-info";
  if (props.source === "extracted") return "bg-purple/16 text-purple";
  if (props.source === "organized") return "bg-warn/16 text-warn";
  return "bg-wash-3 text-ink-2";
});
</script>

<template>
  <span
    class="inline-flex shrink-0 items-center rounded-full px-2 py-0.5 text-[length:var(--lw-fs-10)] leading-none"
    :class="tone"
  >
    {{ label }}
  </span>
</template>
