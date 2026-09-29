<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { usePluginStore } from "../stores/plugins";
import PluginCard from "./PluginCard.vue";
import PluginIcon from "./PluginIcon.vue";
import SideEffectBadges from "./SideEffectBadges.vue";
import { pickLocalized } from "../lib/i18n-helpers";
import type { InstalledPlugin } from "../types";

const { t, locale } = useI18n();
const store = usePluginStore();

const tab = ref<"discover" | "installed">("discover");
const search = ref("");
/** 发现页每张卡的选装版本（缺省 latest）。 */
const picked = ref(new Map<string, string>());

onMounted(() => {
  void store.init();
});

const filteredMarket = computed(() => {
  const q = search.value.trim().toLowerCase();
  if (!q) return store.marketplace;
  return store.marketplace.filter(
    (m) =>
      m.id.toLowerCase().includes(q) ||
      pickLocalized(m.display.description, locale.value, "")
        .toLowerCase()
        .includes(q),
  );
});

/**
 * 已安装列表按 origin 分两大类（packaging.md §2.4）：官方内置在前、
 * 第三方在后。分类只影响展示分组，不是信任边界。
 */
const originGroups = computed(() => {
  const official = store.installed.filter((p) => p.display.origin === "official");
  const community = store.installed.filter((p) => p.display.origin !== "official");
  return [
    { key: "official", list: official },
    { key: "community", list: community },
  ].filter((g) => g.list.length > 0);
});

function displayName(p: { id: string; display: { display_name?: Record<string, string> } }): string {
  return pickLocalized(p.display.display_name, locale.value, p.id);
}

function latestOf(id: string): string | undefined {
  const item = store.marketplace.find((m) => m.id === id);
  return item?.versions[item.versions.length - 1];
}

/** 只前向比较：本地版本 < 市场 latest ⇒ 可更新（§13.3）。 */
function updateAvailable(p: InstalledPlugin): boolean {
  const latest = latestOf(p.id);
  return latest !== undefined && cmpVer(latest, p.version) > 0;
}

function cmpVer(a: string, b: string): number {
  const pa = a.split(".").map(Number);
  const pb = b.split(".").map(Number);
  for (let i = 0; i < 3; i++) {
    const d = (pa[i] || 0) - (pb[i] || 0);
    if (d) return d;
  }
  return 0;
}

/** 可回退的版本：本进程加载过、且不是当前生效版本的。 */
function rollbackTargets(p: InstalledPlugin): string[] {
  return p.loaded_versions.filter((v) => v !== p.active_version && v !== p.version);
}

function pickVersion(id: string, versions: string[]): string {
  if (!picked.value.has(id)) {
    picked.value.set(id, versions[versions.length - 1]);
  }
  return picked.value.get(id)!;
}
</script>

<template>
  <div class="flex min-h-0 flex-1 flex-col overflow-hidden">
    <!-- 页签 + 搜索 -->
    <div
      class="flex items-center gap-1 border-b border-neutral-800 px-5 py-2"
    >
      <button
        v-for="key in (['discover', 'installed'] as const)"
        :key="key"
        class="rounded-lg px-3 py-1.5 text-xs font-medium transition"
        :class="
          tab === key
            ? 'bg-neutral-700 text-neutral-100'
            : 'text-neutral-500 hover:text-neutral-300'
        "
        @click="tab = key"
      >
        {{ t(`market.${key}`) }}
      </button>
      <input
        v-if="tab === 'discover'"
        v-model="search"
        :placeholder="t('market.search')"
        class="ml-auto w-56 rounded-lg border border-neutral-700 bg-neutral-800 px-3 py-1.5 text-xs text-neutral-200 placeholder-neutral-600 outline-none focus:border-neutral-500"
      />
    </div>

    <!-- 错误条 -->
    <p
      v-if="store.error"
      class="border-b border-red-900/50 bg-red-950/50 px-5 py-1.5 text-xs text-red-300"
    >
      {{ store.error }}
    </p>

    <div class="min-h-0 flex-1 overflow-y-auto px-5 py-4">
      <!-- 发现页：卡片网格（packaging.md §6.3 PluginCard） -->
      <template v-if="tab === 'discover'">
        <p
          v-if="filteredMarket.length === 0"
          class="text-sm text-neutral-600"
        >
          {{ t("market.emptyDiscover") }}
        </p>
        <div class="grid grid-cols-1 gap-3 sm:grid-cols-2 xl:grid-cols-3">
          <PluginCard
            v-for="item in filteredMarket"
            :key="item.id"
            :item="item"
            :latest="latestOf(item.id)"
          >
            <template #actions>
              <select
                :value="pickVersion(item.id, item.versions)"
                class="rounded border border-neutral-700 bg-neutral-900 px-2 py-1 text-xs text-neutral-300 outline-none"
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
                class="ml-auto rounded bg-emerald-900/50 px-2 py-1 text-[10px] text-emerald-300"
              >
                {{ t("market.installedBadge") }}
              </span>
              <button
                v-else
                class="ml-auto rounded-lg bg-indigo-600 px-3 py-1.5 text-xs font-semibold text-white transition hover:bg-indigo-500 disabled:cursor-not-allowed disabled:opacity-40"
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
      </template>

      <!-- 已安装页：按 origin 分两大类（官方内置 / 第三方），行内操作齐全 -->
      <template v-else>
        <p v-if="store.installed.length === 0" class="text-sm text-neutral-600">
          {{ t("market.emptyInstalled") }}
        </p>
        <section
          v-for="group in originGroups"
          :key="group.key"
          class="mb-5"
        >
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
                  {{ displayName(p) }}
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
                v-if="updateAvailable(p)"
                class="rounded-full bg-indigo-900/50 px-2 py-0.5 text-[10px] text-indigo-300"
              >
                {{ t("market.updateAvailable", { version: latestOf(p.id) }) }}
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
                  v-if="updateAvailable(p)"
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
      </template>
    </div>
  </div>
</template>
