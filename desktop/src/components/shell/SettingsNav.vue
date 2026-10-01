<script setup lang="ts">
/**
 * 设置导航轨（UI shell doc §3.31 / §3.32）：数据驱动扁平索引
 * （lib/settings-index.ts 唯一真源）按 group 分组、组内按索引顺序出项。
 * 顶部 46px 带 = 拖拽区 + 圆角搜索框（§3.32：设置页自带 46px 带，头 =
 * 导航轨顶部）；搜分区名 + 分区内行标题（索引预声明的 searchKeys），无
 * 结果一行「没有匹配的设置」，组标签随命中变空而隐藏；底部固定
 * 「‹ 返回应用」。
 */
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useShellStore } from "../../stores/shell";
import {
  SETTINGS_GROUPS,
  sectionSearchText,
  sectionsByGroup,
  type SettingsSectionId,
} from "../../lib/settings-index";
import { Icon } from "../../lib/icons";

const props = defineProps<{ current: SettingsSectionId }>();
const emit = defineEmits<{ select: [id: SettingsSectionId] }>();
const { t } = useI18n();
const shell = useShellStore();

const search = ref("");

/** 搜索过滤后的组（组内全不命中 → 整组隐藏）。空 query = 全量。 */
const visibleGroups = computed(() => {
  const q = search.value.trim().toLowerCase();
  return SETTINGS_GROUPS.map((g) => ({
    group: g,
    sections: sectionsByGroup(g.id).filter(
      (s) =>
        !q ||
        sectionSearchText(s, t).includes(q) ||
        t(s.labelKey).toLowerCase().includes(q),
    ),
  })).filter((g) => g.sections.length > 0);
});

function onSelect(id: SettingsSectionId) {
  emit("select", id);
}
</script>

<template>
  <nav class="sidenav relative flex h-full flex-col bg-side" aria-label="Settings">
    <!-- 46px 带左段：拖拽区 + 搜索框（§3.32 头 = 导航轨顶部） -->
    <header class="flex h-[var(--lw-toolbar-h)] shrink-0 items-center gap-2 px-3">
      <div class="h-full w-1.5 shrink-0" data-tauri-drag-region />
      <div class="sidenav-search min-w-0 flex-1">
        <Icon name="search" :size="14" class="text-ink-4" />
        <input
          v-model="search"
          type="text"
          :placeholder="t('settings.navSearch')"
          :aria-label="t('settings.navSearch')"
          class="min-w-0 flex-1 bg-transparent text-xs text-ink-1 outline-none placeholder:text-ink-4"
        />
      </div>
      <div class="h-full w-1.5 shrink-0" data-tauri-drag-region />
    </header>

    <!-- 分组列表：组标签 11.5px ink-4；项 28px 行圆角 8，选中 wash-3 -->
    <div class="scroll-y min-h-0 flex-1 px-3 pb-2">
      <template v-for="{ group, sections } in visibleGroups" :key="group.id">
        <div class="sidenav-group-label">{{ t(group.labelKey) }}</div>
        <button
          v-for="s in sections"
          :key="s.id"
          class="sidenav-item"
          type="button"
          :aria-current="props.current === s.id ? 'true' : undefined"
          :class="{ 'is-active': props.current === s.id }"
          @click="onSelect(s.id)"
        >
          <Icon :name="s.icon" :size="14" class="text-ink-3" />
          <span class="truncate">{{ t(s.labelKey) }}</span>
        </button>
      </template>
      <p
        v-if="visibleGroups.length === 0"
        class="px-2 py-3 text-xs text-ink-4"
      >
        {{ t("settings.navEmpty") }}
      </p>
    </div>

    <!-- 底部固定：返回应用（§3.31） -->
    <footer class="shrink-0 p-2">
      <button
        class="sidenav-item text-ink-3 hover:text-ink-1"
        type="button"
        @click="shell.setDestination('chat')"
      >
        ‹ <span>{{ t("common.back") }}</span>
      </button>
    </footer>
  </nav>
</template>

<style scoped>
.sidenav {
  width: var(--lw-sidenav-w);
  border-right: 1px solid var(--lw-line);
}
.sidenav-search {
  display: flex;
  align-items: center;
  gap: 6px;
  height: var(--lw-ctl);
  padding: 0 10px;
  border-radius: var(--lw-r-full);
  background: var(--lw-wash-2);
}
.sidenav-group-label {
  padding: 14px 12px 4px;
  font-size: 11.5px;
  color: var(--lw-ink-4);
}
.sidenav-item {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  height: 28px;
  padding: 0 8px 0 12px;
  border-radius: var(--lw-r-xs);
  font-size: 13px;
  color: var(--lw-ink-1);
  text-align: left;
  transition: background var(--dur-fast) var(--ease-out),
    color var(--dur-fast) var(--ease-out);
}
.sidenav-item:hover {
  background: var(--lw-wash-2);
}
.sidenav-item.is-active {
  background: var(--lw-wash-3);
}
</style>
