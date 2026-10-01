<script setup lang="ts">
/**
 * 市场卡片（packaging.md §6.3）：图标 + 多语言展示名/描述 + 副作用徽章。
 * 展示名缺失回退 id，描述缺失不渲染描述区（§7 Fallback 矩阵）；
 * 操作区（选版本 / 安装 / 已安装徽标）由父组件经 actions 插槽注入。
 */
import { computed } from "vue";
import { useI18n } from "vue-i18n";

import PluginIcon from "./PluginIcon.vue";
import SideEffectBadges from "./SideEffectBadges.vue";
import { pickLocalized } from "../lib/i18n-helpers";
import type { MarketplaceItem } from "../types";

const props = defineProps<{ item: MarketplaceItem; latest?: string }>();

const { locale } = useI18n();

const name = computed(() =>
  pickLocalized(props.item.display.display_name, locale.value, props.item.id),
);
const desc = computed(() =>
  pickLocalized(props.item.display.description, locale.value, ""),
);
const meta = computed(() => {
  const parts: string[] = [];
  if (props.item.display.author) parts.push(props.item.display.author.name);
  if (props.item.display.license) parts.push(props.item.display.license);
  return parts.join(" · ");
});
</script>

<template>
  <div class="flex flex-col gap-2 rounded-xl bg-wash-1 p-4">
    <div class="flex items-start gap-3">
      <PluginIcon :icon="item.display.icon" :id="item.id" />
      <div class="min-w-0 flex-1">
        <div class="flex items-start justify-between gap-2">
          <h3 class="truncate text-sm font-semibold text-ink-1" :title="item.id">
            {{ name }}
          </h3>
          <span
            v-if="latest"
            class="shrink-0 rounded-full bg-surface-0 px-2 py-0.5 text-[10px] text-ink-3"
          >
            v{{ latest }}
          </span>
        </div>
        <p class="font-mono text-[10px] text-ink-5">{{ item.id }}</p>
      </div>
    </div>
    <p v-if="desc" class="min-h-8 text-xs leading-relaxed text-ink-3">
      {{ desc }}
    </p>
    <div class="flex items-center gap-1.5">
      <SideEffectBadges :kinds="item.display.side_effects" />
      <span v-if="meta" class="ml-auto truncate text-[10px] text-ink-5">{{ meta }}</span>
    </div>
    <div class="mt-auto flex items-center gap-2">
      <slot name="actions" />
    </div>
  </div>
</template>
