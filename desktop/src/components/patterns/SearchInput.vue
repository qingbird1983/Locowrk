<script setup lang="ts">
/**
 * 搜索输入（前端标准 §二⑤ 唯一搜索形态）：胶囊高 28 + 左搜索图标 +
 * 无边框输入。**只归形态不归功能**（用户 2026-10-06 澄清）：侧栏搜会话/
 * ⌘K 搜命令/模型菜单过滤模型/市场搜插件，行为各归各，本
 * 组件只提供同一张壳。surface 两档 = 两色控件体系（UI shell §3.32·七）
 * 的语境皮：wash（缺省，浮层与页面内）= wash-1 底；raised（灰轨上）=
 * 浮层底 + 台面影（设置导航轨 v2.44 目检裁定形态）。几何/图标/字号全站
 * 唯一；聚焦无环（§7-12 字段类豁免）。
 */
import { ref } from "vue";
import { Icon } from "../../lib/icons";

const props = withDefaults(
  defineProps<{
    modelValue: string;
    /** 占位文案（调用方 t() 后传入；缺省同时作无障碍名）。 */
    placeholder?: string;
    /** 无障碍名（不传回落 placeholder）。 */
    label?: string;
    surface?: "wash" | "raised";
  }>(),
  { surface: "wash" },
);
const emit = defineEmits<{ "update:modelValue": [value: string] }>();

const input = ref<HTMLInputElement | null>(null);

function onInput(e: Event): void {
  emit("update:modelValue", (e.target as HTMLInputElement).value);
}

/** 使用方打开面板后聚焦输入（⌘K 等）；本组件唯一对外命令。 */
function focus(): void {
  input.value?.focus();
}
defineExpose({ focus });
</script>

<template>
  <label class="lw-search" :class="`lw-search--${props.surface}`">
    <Icon name="search" :size="14" class="lw-search__icon" aria-hidden="true" />
    <input
      ref="input"
      class="lw-search__input"
      type="text"
      :value="props.modelValue"
      :placeholder="props.placeholder"
      :aria-label="props.label ?? props.placeholder"
      spellcheck="false"
      autocomplete="off"
      @input="onInput"
    />
  </label>
</template>

<style scoped>
.lw-search {
  display: flex;
  align-items: center;
  gap: var(--lw-gap-2);
  width: 100%;
  height: var(--lw-ctl);
  padding: 0 var(--lw-gap-10);
  border-radius: var(--lw-r-full);
  background: var(--lw-wash-1);
  cursor: text;
}
.lw-search--raised {
  background: var(--lw-bg-elevated);
  box-shadow: var(--lw-shadow-raised);
}
.lw-search__icon {
  flex: 0 0 auto;
  color: var(--lw-ink-4);
}
.lw-search__input {
  flex: 1;
  min-width: 0;
  border: none;
  outline: none;
  background: transparent;
  font-size: var(--lw-fs-12half);
  color: var(--lw-ink-1);
}
.lw-search__input::placeholder {
  color: var(--lw-ink-4);
}
</style>
