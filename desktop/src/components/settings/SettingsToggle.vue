<script setup lang="ts">
/**
 * 设置行开关（UI shell 文档 §3.32·七 v3.5；参考对象 .settings-toggle
 * 逐值照搬）：32×20 全圆角轨道 + 16px 滑钮，on = translateX(12px)。
 * on 轨 = accent / on 钮 = accent-ink（不设独立 token）；过渡走 §6.6
 * fast/ease-standard 基线层（button 全局 transition），滑钮位移单独声明。
 * role="switch"，无中间态；disabled 半透明（:enabled:hover 才加深，
 * 禁用态 hover 不变色）。设置行内布尔项一律用它，复选框退场（§3.32·七）。
 */
const props = defineProps<{
  modelValue: boolean;
  label: string;
  disabled?: boolean;
}>();

const emit = defineEmits<{ "update:modelValue": [value: boolean] }>();

function toggle() {
  if (!props.disabled) emit("update:modelValue", !props.modelValue);
}
</script>

<template>
  <button
    type="button"
    role="switch"
    :aria-checked="modelValue"
    :aria-label="label"
    :disabled="disabled"
    class="relative h-5 w-8 shrink-0 rounded-full"
    :class="
      modelValue
        ? 'bg-accent enabled:hover:bg-accent-hover'
        : 'bg-switch-track enabled:hover:bg-switch-track-hover'
    "
    @click="toggle"
  >
    <span
      class="absolute left-0.5 top-0.5 block h-4 w-4 rounded-full shadow-raised transition-transform duration-150 ease-out"
      :class="modelValue ? 'translate-x-3 bg-accent-ink' : 'bg-switch-knob'"
    />
  </button>
</template>
