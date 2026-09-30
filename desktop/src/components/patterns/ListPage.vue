<script setup lang="ts">
/**
 * 列表页模式（壳批次，docs/design/UI shell and patterns.md §3.1）：
 * 一切"条目集合"页面（插件已安装页 / 记忆页 / 后续 ⑩ 闲置筛选页）共用
 * 的骨架。只锁结构——搜索框、筛选 chips、批量操作栏插槽、空态；行内容
 * 与动作由使用方以默认插槽提供，本组件不认识任何具体业务类型。
 *
 * 约定：
 * - v-model:search 未绑定时搜索框整个隐藏（如记忆页只有搜索没有 chips）。
 * - filters 是完整的 chip 列表（含"全部"项），选中值经 v-model:filter 回传。
 * - 批量栏只在 batching 为 true 时渲染（⑩ 批量操作接线；批量卸载需
 *   二次确认并列出插件名）。
 */
defineProps<{
  /** 空态文案；isEmpty 为 true 时替代默认插槽显示。 */
  emptyText?: string;
  isEmpty?: boolean;
  /** 搜索框占位文案（如"搜索插件…"），由使用方 i18n。 */
  searchPlaceholder?: string;
  /** 筛选 chips（含"全部"项）；不传或为空则不渲染 chips 行。 */
  filters?: { key: string; label: string }[];
  /** 批量操作栏可见性。 */
  batching?: boolean;
}>();

const search = defineModel<string>("search");
const filter = defineModel<string>("filter");
</script>

<template>
  <div class="flex min-h-0 flex-1 flex-col overflow-hidden">
    <!-- 头部：筛选 chips 在左、搜索框在右（与既有插件页布局一致） -->
    <div
      v-if="search !== undefined || (filters && filters.length > 0)"
      class="flex items-center gap-2 border-b border-[var(--lw-border)] px-5 py-2"
    >
      <div v-if="filters && filters.length > 0" class="flex gap-1">
        <button
          v-for="f in filters"
          :key="f.key"
          class="rounded-full px-3 py-1 text-xs font-medium transition"
          :class="
            filter === f.key
              ? 'bg-[var(--lw-surface-2)] text-[var(--lw-text-1)]'
              : 'text-[var(--lw-text-3)] hover:text-[var(--lw-text-2)]'
          "
          @click="filter = f.key"
        >
          {{ f.label }}
        </button>
      </div>
      <input
        v-if="search !== undefined"
        v-model="search"
        :placeholder="searchPlaceholder"
        class="ml-auto w-56 rounded-lg border border-[var(--lw-border-strong)] bg-[var(--lw-surface-1)] px-3 py-1.5 text-xs text-[var(--lw-text-1)] outline-none placeholder:text-[var(--lw-text-4)] focus:border-[var(--lw-text-3)]"
      />
    </div>

    <!-- 内容区：空态优先 -->
    <div class="min-h-0 flex-1 overflow-y-auto px-5 py-4">
      <p v-if="isEmpty" class="text-sm text-[var(--lw-text-3)]">
        {{ emptyText }}
      </p>
      <slot v-else />
    </div>

    <!-- 批量操作栏：选中非空时固定在底部 -->
    <div
      v-if="batching"
      class="flex items-center gap-2 border-t border-[var(--lw-border)] bg-[var(--lw-surface-1)]/60 px-5 py-2"
    >
      <slot name="batch" />
    </div>
  </div>
</template>
