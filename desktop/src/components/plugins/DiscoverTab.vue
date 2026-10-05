<script setup lang="ts">
/**
 * 发现页（壳批次拆分）：市场目录卡片网格 + 选装版本 + 安装动作。
 * 搜索经 ListPage 模式承载；卡片本体见 PluginCard（packaging.md §6.3）。
 */
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { usePluginStore } from "../../stores/plugins";
import { displayName, latestOf } from "../../lib/plugin-helpers";
import { pickLocalized } from "../../lib/i18n-helpers";
import ListPage from "../patterns/ListPage.vue";
import PluginCard from "../PluginCard.vue";

const { t, locale } = useI18n();
const store = usePluginStore();

const search = ref("");
/** 每张卡的选装版本（缺省 latest）。 */
const picked = ref(new Map<string, string>());

const filteredMarket = computed(() => {
  const q = search.value.trim().toLowerCase();
  if (!q) return store.marketplace;
  return store.marketplace.filter(
    (m) =>
      m.id.toLowerCase().includes(q) ||
      displayName(m, locale.value).toLowerCase().includes(q) ||
      pickLocalized(m.display.description, locale.value, "")
        .toLowerCase()
        .includes(q),
  );
});

function pickVersion(id: string, versions: string[]): string {
  if (!picked.value.has(id)) {
    picked.value.set(id, versions[versions.length - 1]);
  }
  return picked.value.get(id)!;
}
</script>

<template>
  <ListPage
    v-model:search="search"
    :search-placeholder="t('market.search')"
    :is-empty="filteredMarket.length === 0"
    :empty-text="t('market.emptyDiscover')"
  >
    <!-- §3.29 三列自适应：repeat(auto-fill, minmax(280px, 1fr))，gap 12px -->
    <div class="grid gap-3 [grid-template-columns:repeat(auto-fill,minmax(280px,1fr))]">
      <PluginCard
        v-for="item in filteredMarket"
        :key="item.id"
        :item="item"
        :latest="latestOf(store.marketplace, item.id)"
      >
        <template #actions>
          <select
            :value="pickVersion(item.id, item.versions)"
            class="rounded-[var(--lw-r-3xs)] border border-line-strong bg-surface-0 px-2 py-1 text-xs text-ink-2 outline-none"
            @change="
              picked.set(item.id, ($event.target as HTMLSelectElement).value)
            "
          >
            <option v-for="v in item.versions" :key="v" :value="v">
              v{{ v }}
            </option>
          </select>
          <span
            v-if="store.installed.some((p) => p.id === item.id)"
            class="ml-auto rounded-[var(--lw-r-3xs)] bg-ok/16 px-2 py-1 text-[10px] text-ok"
          >
            {{ t("market.installedBadge") }}
          </span>
          <button
            v-else
            class="ml-auto rounded-[var(--lw-r-xs)] bg-accent px-3 py-1.5 text-xs font-semibold text-accent-ink transition hover:bg-accent-hover disabled:cursor-not-allowed disabled:opacity-40"
            :disabled="store.isBusy(item.id)"
            @click="store.install(item.id, pickVersion(item.id, item.versions))"
          >
            {{
              store.isBusy(item.id)
                ? t("market.installing")
                : t("market.install")
            }}
          </button>
        </template>
      </PluginCard>
    </div>
  </ListPage>
</template>
