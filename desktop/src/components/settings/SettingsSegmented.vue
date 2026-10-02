<script setup lang="ts">
/**
 * 分段控件（UI shell 文档 §3.32·七 v3.6 两色控件体系，用户裁定）：容器 =
 * 白底台面（raised + shadow-raised）圆角 full + p-1；选中片 = --lw-bg-side
 * （左轨的灰）+ ink-1，未选透明底 ink-4 hover ink-2。v-model 绑 string；
 * options[].label 由调用方 t() 后传入（文案一律走 i18n，本组件不直接碰表）。
 * 全局同型控件（主题三档 / 用量读数二档）一律用它。
 */
defineProps<{
  modelValue: string;
  options: ReadonlyArray<{ value: string; label: string }>;
  /** 无障碍名（radiogroup 的 aria-label，调用方 t() 后传入）。 */
  label: string;
}>();
defineEmits<{ "update:modelValue": [value: string] }>();
</script>

<template>
  <div
    class="flex w-fit gap-1 rounded-full bg-raised p-1 shadow-raised"
    role="radiogroup"
    :aria-label="label"
  >
    <button
      v-for="o in options"
      :key="o.value"
      type="button"
      role="radio"
      :aria-checked="modelValue === o.value"
      class="rounded-full px-3 py-1 text-xs font-medium"
      :class="
        modelValue === o.value
          ? 'bg-side text-ink-1'
          : 'text-ink-4 hover:text-ink-2'
      "
      @click="$emit('update:modelValue', o.value)"
    >
      {{ o.label }}
    </button>
  </div>
</template>
