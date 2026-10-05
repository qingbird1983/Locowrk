<script setup lang="ts">
/**
 * 设置行（UI shell 文档 §3.32·七 v3.5，用户裁定"左设置名、右设置参数/
 * 按钮"；参考对象 .settings-row 逐值照搬）：行自身 = 一张瓦片卡（wash-1、
 * 圆角 14、内边距 14px 16px），行间 gap 6px 由父级 SettingsGroup 排。
 * 左 = 行名（13px/500）+ 可选说明（hint）+ 动态取值插槽（#detail，12px
 * ink-3——当前值/建议值/警告走这里，不往右列挤）；右 = 控件列（右对齐、
 * 垂直居中、shrink-0、max-w 240、min-h 38 与字段同高）。controlId 给了
 * 就把行名连成 label（点击行名聚焦控件）；wide 放开控件列 240 上限
 * （滑条等宽控件用，参考对象 .settings-font-size 同一逃生口）。
 * #title-extra（v3.7）：行名尾部挂件（警示气泡 HoverTip 等行内附件）。
 */
withDefaults(
  defineProps<{
    title: string;
    hint?: string;
    controlId?: string;
    wide?: boolean;
  }>(),
  { hint: undefined, controlId: undefined, wide: false },
);
</script>

<template>
  <div class="flex items-center justify-between gap-6 rounded-[var(--lw-r-md-plus)] bg-wash-1 px-4 py-3.5">
    <div class="min-w-0 flex-1">
      <component
        :is="controlId ? 'label' : 'div'"
        :for="controlId"
        class="block text-[13px] font-medium leading-tight text-ink-1"
      >
        {{ title }}<slot name="title-extra" />
      </component>
      <p v-if="hint" class="mt-1 text-[11.5px] leading-relaxed text-ink-4">
        {{ hint }}
      </p>
      <slot name="detail" />
    </div>
    <div
      class="flex min-w-0 shrink-0 items-center justify-end gap-2"
      :class="wide ? '' : 'max-w-[240px]'"
      :style="{ minHeight: 'var(--lw-field-h)' }"
    >
      <slot />
    </div>
  </div>
</template>
