<script setup lang="ts">
/**
 * 插件图标（packaging.md §6.3）：data URL 直接进 <img>；加载失败
 * onerror 隐藏图片露出首字母兜底（§7 Fallback 矩阵，任何图标问题
 * 都不崩卡片）。
 */
import { computed, ref } from "vue";

const props = defineProps<{
  icon?: string | null;
  id: string;
  /** Tailwind 尺寸类，缺省 40px。 */
  size?: string;
}>();

const broken = ref(false);
const sizeClass = computed(() => props.size ?? "h-10 w-10");

/** data URL 的 img 不该再失败，但防空串/坏串：空值或加载失败都走兜底。 */
const iconSrc = computed(() =>
  props.icon && !broken.value ? props.icon : undefined,
);
const fallback = computed(() => props.id[0]?.toUpperCase() ?? "?");
</script>

<template>
  <div
    class="grid shrink-0 place-items-center rounded-lg bg-wash-2 text-sm font-semibold text-ink-3"
    :class="sizeClass"
  >
    <img
      v-if="iconSrc"
      :src="iconSrc"
      :alt="id"
      class="h-full w-full object-contain p-0.5"
      @error="broken = true"
    />
    <span v-else>{{ fallback }}</span>
  </div>
</template>
