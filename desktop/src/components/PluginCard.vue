<script setup lang="ts">
/**
 * 市场卡片（UI shell 文档 §3.29，v2.53 对齐规范形态）：卡 = wash-1 底、
 * 圆角 14、14px 16px；卡内自上而下 = 34px 图标块（圆角 8 + 图标/首字母）
 * + 标题 14px/500 + 元信息行 11px ink-4（作者 · 版本）+ 描述 12px ink-2
 * 截 3 行 + 副作用 chips 行（真实语义复用 §3.4 徽章）+ 底部行（左
 * license/origin，右 = 选版本 + 安装 primary / ✓ 已安装 ok 文字——动作区
 * 由父组件经 actions 插槽注入）。本地市场无安装量/更新时间数据源，不发明。
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
  if (props.latest) parts.push(`v${props.latest}`);
  return parts.join(" · ");
});
const licenseText = computed(
  () => props.item.display.license ?? props.item.display.origin,
);
</script>

<template>
  <div class="market-card">
    <div class="flex items-center gap-3">
      <div class="market-card__icon">
        <PluginIcon :icon="item.display.icon" :id="item.id" size="h-full w-full" />
      </div>
      <div class="min-w-0 flex-1">
        <h3 class="truncate text-[14px] font-medium text-ink-1" :title="item.id">
          {{ name }}
        </h3>
        <p v-if="meta" class="truncate text-[11px] text-ink-4">{{ meta }}</p>
      </div>
    </div>
    <p
      v-if="desc"
      class="line-clamp-3 min-h-9 text-xs leading-relaxed text-ink-2"
    >
      {{ desc }}
    </p>
    <div
      v-if="item.display.side_effects.length > 0"
      class="flex flex-wrap items-center gap-1.5"
    >
      <SideEffectBadges :kinds="item.display.side_effects" />
    </div>
    <div class="mt-auto flex items-center gap-2 pt-1">
      <span
        class="truncate rounded-[var(--lw-r-3xs)] px-1.5 py-0.5 text-[10px] text-ink-4"
        :title="licenseText"
      >
        {{ licenseText }}
      </span>
      <div class="ml-auto flex min-w-0 items-center gap-2">
        <slot name="actions" />
      </div>
    </div>
  </div>
</template>

<style scoped>
.market-card {
  display: flex;
  min-height: 100%;
  flex-direction: column;
  gap: 8px;
  border-radius: var(--lw-r-lg);
  background: var(--lw-wash-1);
  padding: 14px 16px;
}
.market-card__icon {
  display: flex;
  width: 34px;
  height: 34px;
  flex: 0 0 auto;
  align-items: center;
  justify-content: center;
  border-radius: var(--lw-r-md);
  background: var(--lw-wash-2);
  overflow: hidden;
}
</style>
