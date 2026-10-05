<script setup lang="ts">
/**
 * 副作用徽章（UI shell doc §3.4 副作用族配色）：纯 = 中性（状态色稀缺，
 * 不给绿）/ 可补偿 = warn / 不可逆 = danger；语义徽章 = 该色 16% 底 +
 * 该色作字。kinds 已由后端去重并按固定序给出。
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
      return "bg-wash-3 text-ink-2";
    case "compensable":
      return "bg-warn/16 text-warn";
    case "irreversible":
      return "bg-danger/16 text-danger";
    default:
      return "bg-wash-3 text-ink-2";
  }
}
</script>

<template>
  <span
    v-for="k in kinds"
    :key="k"
    class="rounded-full px-2 py-0.5 text-[length:var(--lw-fs-10)] font-medium"
    :class="color(k)"
  >
    {{ label(k) }}
  </span>
</template>
