<script setup lang="ts">
/**
 * 已安装页（壳批次拆分 + 列表页模式改造）：搜索 + origin 筛选 chips +
 * 分组行内操作。多选批量栏（⑩ 批量卸载等）与闲置徽章（IdleBadge）随
 * ⑩ plugin_list_with_usage 接入。
 */
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { usePluginStore } from "../../stores/plugins";
import {
  displayName,
  latestOf,
  rollbackTargets,
  updateAvailable,
} from "../../lib/plugin-helpers";
import ListPage from "../patterns/ListPage.vue";
import PluginIcon from "../PluginIcon.vue";
import SideEffectBadges from "../SideEffectBadges.vue";
import type { InstalledPlugin } from "../../types";

const { t, locale } = useI18n();
const store = usePluginStore();

const search = ref("");
/** 空串 = 全部（含在 chips 首位）。 */
const originFilter = ref("");

const filters = computed(() => [
  { key: "", label: t("market.filterAll") },
  { key: "official", label: t("market.origin_official") },
  { key: "community", label: t("market.origin_community") },
]);

/** 搜索 + 筛选后的已安装列表。 */
const filtered = computed(() => {
  const q = search.value.trim().toLowerCase();
  return store.installed.filter((p) => {
    if (originFilter.value) {
      const isOfficial = p.display.origin === "official";
      if (originFilter.value === "official" && !isOfficial) return false;
      if (originFilter.value === "community" && isOfficial) return false;
    }
    if (!q) return true;
    return (
      p.id.toLowerCase().includes(q) ||
      displayName(p, locale.value).toLowerCase().includes(q)
    );
  });
});

/**
 * 按 origin 分两大类（packaging.md §2.4）：官方内置在前、第三方在后。
 * 分类只影响展示分组，不是信任边界。
 */
const originGroups = computed(() => {
  const official = filtered.value.filter((p) => p.display.origin === "official");
  const community = filtered.value.filter((p) => p.display.origin !== "official");
  return [
    { key: "official", list: official },
    { key: "community", list: community },
  ].filter((g) => g.list.length > 0);
});

const isEmpty = computed(() => filtered.value.length === 0);
const emptyText = computed(() =>
  store.installed.length === 0
    ? t("market.emptyInstalled")
    : t("market.noMatch"),
);

function updateAvailableOf(p: InstalledPlugin): boolean {
  return updateAvailable(store.marketplace, p);
}
</script>

<template>
  <ListPage
    v-model:search="search"
    v-model:filter="originFilter"
    :search-placeholder="t('market.search')"
    :filters="filters"
    :is-empty="isEmpty"
    :empty-text="emptyText"
  >
    <section v-for="group in originGroups" :key="group.key" class="mb-5">
      <h3
        class="mb-2 text-[10px] font-semibold uppercase tracking-wider text-neutral-500"
      >
        {{ t(`market.origin_${group.key}`) }}
      </h3>
      <div class="overflow-hidden rounded-xl border border-neutral-800">
        <div
          v-for="p in group.list"
          :key="p.id"
          class="flex flex-wrap items-center gap-x-4 gap-y-2 border-b border-neutral-800 bg-neutral-800/40 px-4 py-3 last:border-b-0"
        >
          <PluginIcon :icon="p.display.icon" :id="p.id" size="h-9 w-9" />
          <div class="min-w-40">
            <p class="text-sm font-semibold text-neutral-100">
              {{ displayName(p, locale) }}
              <span class="ml-1 font-mono text-[10px] font-normal text-neutral-600">{{
                p.id
              }}</span>
            </p>
            <p class="text-[11px] text-neutral-500">
              {{ t("market.versionLabel") }} {{ p.version }}
            </p>
          </div>
          <SideEffectBadges :kinds="p.display.side_effects" />
          <!-- 状态徽标：运行中 / 已禁用 -->
          <span
            class="rounded-full px-2 py-0.5 text-[10px]"
            :class="
              p.active_version
                ? 'bg-emerald-900/50 text-emerald-300'
                : p.enabled
                  ? 'bg-amber-900/50 text-amber-300'
                  : 'bg-neutral-700/60 text-neutral-400'
            "
          >
            {{
              p.active_version
                ? t("market.stateActive")
                : p.enabled
                  ? t("market.statePending")
                  : t("market.stateDisabled")
            }}
          </span>
          <span
            v-if="updateAvailableOf(p)"
            class="rounded-full bg-indigo-900/50 px-2 py-0.5 text-[10px] text-indigo-300"
          >
            {{ t("market.updateAvailable", { version: latestOf(store.marketplace, p.id) }) }}
          </span>

          <!-- 内置插件（开发态直载）：只读展示，不走 lockfile 管理流 -->
          <span
            v-if="p.source === 'builtin'"
            class="ml-auto rounded-full bg-neutral-900 px-2 py-0.5 text-[10px] text-neutral-400"
          >
            {{ t("market.builtinBadge") }}
          </span>
          <div v-else class="ml-auto flex items-center gap-2">
            <button
              class="rounded-lg border border-neutral-600 px-3 py-1.5 text-xs text-neutral-300 transition hover:bg-neutral-700 disabled:cursor-not-allowed disabled:opacity-40"
              :disabled="store.isBusy(p.id)"
              @click="store.setEnabled(p.id, !p.enabled)"
            >
              {{ p.enabled ? t("market.disable") : t("market.enable") }}
            </button>
            <button
              v-if="updateAvailableOf(p)"
              class="rounded-lg bg-indigo-600 px-3 py-1.5 text-xs font-semibold text-white transition hover:bg-indigo-500 disabled:cursor-not-allowed disabled:opacity-40"
              :disabled="store.isBusy(p.id)"
              @click="store.update(p.id)"
            >
              {{ t("market.update") }}
            </button>
            <template v-if="rollbackTargets(p).length > 0">
              <select
                class="rounded border border-neutral-700 bg-neutral-900 px-2 py-1.5 text-xs text-neutral-300 outline-none"
                @change="
                  store.rollback(
                    p.id,
                    ($event.target as HTMLSelectElement).value,
                  )
                "
              >
                <option value="" disabled selected>
                  {{ t("market.rollback") }}
                </option>
                <option
                  v-for="v in rollbackTargets(p)"
                  :key="v"
                  :value="v"
                >
                  v{{ v }}
                </option>
              </select>
            </template>
            <button
              class="rounded-lg border border-red-800/70 px-3 py-1.5 text-xs text-red-300 transition hover:bg-red-900/40 disabled:cursor-not-allowed disabled:opacity-40"
              :disabled="store.isBusy(p.id)"
              @click="store.uninstall(p.id)"
            >
              {{ t("market.uninstall") }}
            </button>
          </div>
        </div>
      </div>
    </section>
  </ListPage>
</template>
