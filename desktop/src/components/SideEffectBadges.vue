<script setup lang="ts">
/**
 * 副作用徽章（packaging.md §6.4 配色）：pure 青绿 / compensable 黄 /
 * irreversible 红。kinds 已由后端去重并按固定序给出。
 */
import { useI18n } from "vue-i18n";

defineProps<{ kinds: string[] }>();

const { t } = useI18n();

function label(kind: string): string {
  // effect.pure / effect.compensable / effect.irreversible（locales 复用）
  return t(`effect.${kind}`);
}

function color(kind: string): string {
  switch (kind) {
    case "pure":
      return "bg-teal-900/40 text-teal-300";
    case "compensable":
      return "bg-yellow-900/40 text-yellow-300";
    case "irreversible":
      return "bg-red-900/40 text-red-300";
    default:
      return "bg-surface-2/60 text-ink-3";
  }
}
</script>

<template>
  <span
    v-for="k in kinds"
    :key="k"
    class="rounded-full px-2 py-0.5 text-[10px] font-medium"
    :class="color(k)"
  >
    {{ label(k) }}
  </span>
</template>
