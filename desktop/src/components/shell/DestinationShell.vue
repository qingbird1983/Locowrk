<script setup lang="ts">
/**
 * 目的地整页容器（UI shell doc §2.4 / §3.31 页头形态）：非对话目的地在
 * 侧栏右侧铺满，自带 56px 页头（20px 标题 / 600 / -0.02em）+ 拖拽段 +
 * 窗口控制车道预留。页头主按钮与 `…` 溢出菜单由具体目的地按需经插槽
 * 提供（§2.4 页头形态）；内容区滚动。
 *
 * `back`：设置目的地连侧栏一起接管（§3.32），返回通路在 ㉔ 设置导航轨
 * （「‹ 返回应用」）；导航轨落地前由页头左侧的返回钮承担，不留死角。
 */
import { useI18n } from "vue-i18n";
import { useShellStore } from "../../stores/shell";

defineProps<{ title: string; back?: boolean }>();
const { t } = useI18n();
const shell = useShellStore();
</script>

<template>
  <section class="flex min-h-0 min-w-0 flex-1 flex-col bg-page">
    <header
      class="flex h-14 shrink-0 items-center gap-3 px-6"
      style="padding-right: var(--lw-wincontrols)"
    >
      <button
        v-if="back"
        class="back-btn shrink-0"
        type="button"
        @click="shell.setDestination('chat')"
      >
        ‹ {{ t("common.back") }}
      </button>
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

<style scoped>
.back-btn {
  font-size: 12.5px;
  color: var(--lw-ink-3);
  padding: 4px 6px;
  border-radius: var(--lw-r-sm);
}
.back-btn:hover {
  color: var(--lw-ink-1);
  background: var(--lw-wash-2);
}
</style>
