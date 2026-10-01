<script setup lang="ts">
/**
 * 目的地整页容器（UI shell doc §2.4 / §3.31 页头形态）：非对话目的地在
 * 侧栏右侧铺满，自带 56px 页头（20px 标题 / 600 / -0.02em）+ 拖拽段 +
 * 窗口控制车道预留。页头主按钮与 `…` 溢出菜单由具体目的地按需经插槽
 * 提供（§2.4 页头形态）；内容区滚动。
 *
 * 设置目的地不走本容器——它连侧栏一起接管（§3.32），壳见 SettingsPage
 * （导航轨底部「‹ 返回应用」承担返回通路）。
 */
defineProps<{ title: string }>();
</script>

<template>
  <section class="flex min-h-0 min-w-0 flex-1 flex-col bg-page">
    <header
      class="flex h-14 shrink-0 items-center gap-3 px-6"
      style="padding-right: var(--lw-wincontrols)"
    >
      <h1
        class="shrink-0 text-[20px] font-semibold tracking-[-0.02em] text-ink-1"
      >
        {{ title }}
      </h1>
      <slot name="actions" />
      <div class="h-full min-w-0 flex-1" data-tauri-drag-region />
    </header>
    <div class="scroll-y min-h-0 flex-1">
      <slot />
    </div>
  </section>
</template>

