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
  isSectionVisible,
  sectionSearchText,
  sectionsByGroup,
  type SettingsSectionId,
} from "../../lib/settings-index";
import { useSettingsStore } from "../../stores/settings";
import { Icon } from "../../lib/icons";

const props = defineProps<{ current: SettingsSectionId }>();
const emit = defineEmits<{ select: [id: SettingsSectionId] }>();
const { t } = useI18n();
const shell = useShellStore();
const settings = useSettingsStore();

const search = ref("");

/** devOnly 分区（开发者选项）仅在解锁后可见（§3.2，v2.49）。 */
const devUnlocked = computed(() => settings.view?.developer.unlocked ?? false);

/** 搜索过滤后的组（组内全不命中 → 整组隐藏）。空 query = 全量。 */
const visibleGroups = computed(() => {
  const q = search.value.trim().toLowerCase();
  return SETTINGS_GROUPS.map((g) => ({
    group: g,
    sections: sectionsByGroup(g.id).filter(
      (s) =>
        isSectionVisible(s, devUnlocked.value) &&
        (!q ||
          sectionSearchText(s, t).includes(q) ||
          t(s.labelKey).toLowerCase().includes(q)),
    ),
  })).filter((g) => g.sections.length > 0);
});

function onSelect(id: SettingsSectionId) {
  emit("select", id);
}
</script>

<template>
  <nav class="sidenav relative flex h-full flex-col bg-side" aria-label="Settings">
    <!-- 46px 带左段 = 纯拖拽区（v3.6：搜索移出带，带只留拖拽职责） -->
    <header class="h-[var(--lw-toolbar-h)] shrink-0">
      <div class="h-full w-full" data-tauri-drag-region />
    </header>

    <!-- 搜索胶囊（v3.6：自占一行在 46px 带之下；白/主底 + 微悬浮阴影） -->
    <div class="px-3 pb-2 pt-1">
      <div class="sidenav-search min-w-0">
        <Icon name="search" :size="14" class="text-ink-4" />
        <input
          v-model="search"
          type="text"
          :placeholder="t('settings.navSearch')"
          :aria-label="t('settings.navSearch')"
          class="min-w-0 flex-1 bg-transparent text-xs text-ink-1 outline-none placeholder:text-ink-4"
        />
      </div>
    </div>

    <!-- 分组列表：组标签 11.5px ink-4；项 30px 行圆角 8，选中 wash-3；
         项间 2px 间隙——高亮瓦片不粘连（v3.6） -->
    <div class="scroll-y min-h-0 flex-1 px-3 pb-2">
      <template v-for="{ group, sections } in visibleGroups" :key="group.id">
        <div class="sidenav-group-label">{{ t(group.labelKey) }}</div>
        <div class="flex flex-col gap-[var(--lw-gap-2)]">
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
        </div>
      </template>
      <p
        v-if="visibleGroups.length === 0"
        class="px-2 py-3 text-xs text-ink-4"
      >
        {{ t("settings.navEmpty") }}
      </p>
    </div>

    <!-- 底部固定：返回应用（§3.31；v3.6 宽度按字符内容收缩） -->
    <footer class="shrink-0 p-2">
      <button
        class="sidenav-item sidenav-item--fit text-ink-3 hover:text-ink-1"
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
  gap: var(--lw-gap-6);
  height: var(--lw-ctl);
  padding: 0 var(--lw-gap-10);
  border-radius: var(--lw-r-full);
  /* v3.6 两色体系：轨灰上的白胶囊 + 微悬浮阴影（§3.32·七） */
  background: var(--lw-bg);
  box-shadow: var(--lw-shadow-raised);
}
.sidenav-group-label {
  padding: var(--lw-gap-14) var(--lw-gap-12) var(--lw-gap-4);
  font-size: var(--lw-fs-12);
  color: var(--lw-ink-4);
}
.sidenav-item {
  display: flex;
  align-items: center;
  gap: var(--lw-gap-8);
  width: 100%;
  height: 30px; /* v3.6：28→30（用户裁定 +2px） */
  padding: 0 var(--lw-gap-8) 0 var(--lw-gap-12);
  border-radius: var(--lw-r-xs);
  font-size: var(--lw-fs-13);
  color: var(--lw-ink-1);
  text-align: left;
  transition: background var(--dur-fast) var(--ease-out),
    color var(--dur-fast) var(--ease-out);
}
/* v3.6：返回应用不做整行瓦片，高亮宽度 = 字符内容宽 */
.sidenav-item--fit {
  width: fit-content;
}
.sidenav-item:hover {
  background: var(--lw-wash-2);
}
.sidenav-item.is-active {
  background: var(--lw-wash-3);
}
</style>
