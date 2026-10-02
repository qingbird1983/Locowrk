<script setup lang="ts">
/**
 * 警示气泡（UI shell 文档 §3.32·七 v3.7，用户裁定"说明移进气泡，hover
 * 显隐"）：warn 色 ⚠ 图标 + hover 白底气泡（raised + shadow-raised +
 * r-sm、max-w 280、11px ink-2）；纯 CSS hover 显隐，移出即隐。气泡是
 * 浮起件（§6.1 阴影豁免）。默认插槽 = 气泡内容（段落由调用方排）；
 * label 供无障碍名。
 */
import { Icon } from "../../lib/icons";

defineProps<{ label?: string }>();
</script>

<template>
  <span class="group relative ml-1 inline-flex" role="note" :aria-label="label">
    <Icon name="warn" :size="13" class="text-warn" aria-hidden="true" />
    <span
      class="pointer-events-none invisible absolute left-1/2 top-full z-50 mt-1.5 w-max max-w-[280px] -translate-x-1/2 rounded-[var(--lw-r-sm)] border border-line-subtle bg-raised px-3 py-2 text-left text-[11px] leading-relaxed text-ink-2 opacity-0 shadow-raised transition-opacity duration-150 group-hover:visible group-hover:opacity-100"
    >
      <slot />
    </span>
  </span>
</template>
