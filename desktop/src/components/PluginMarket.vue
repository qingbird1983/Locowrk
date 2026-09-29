<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { usePluginStore } from "../stores/plugins";
import type { InstalledPlugin } from "../types";

const { t } = useI18n();
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
      m.id.toLowerCase().includes(q) || m.description.toLowerCase().includes(q),
  );
});

/** 已安装页按分类分组（§13.3）：分类取 id 的第一段（tool-* → tool）。 */
const groupedInstalled = computed(() => {
  const groups = new Map<string, InstalledPlugin[]>();
  for (const p of store.installed) {
    const cat = p.id.split("-")[0] || p.id;
    if (!groups.has(cat)) groups.set(cat, []);
    groups.get(cat)!.push(p);
  }
  return [...groups.entries()].sort(([a], [b]) => a.localeCompare(b));
});

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
      <!-- 发现页：卡片网格 -->
      <template v-if="tab === 'discover'">
        <p
          v-if="filteredMarket.length === 0"
          class="text-sm text-neutral-600"
        >
          {{ t("market.emptyDiscover") }}
        </p>
        <div class="grid grid-cols-1 gap-3 sm:grid-cols-2 xl:grid-cols-3">
          <div
            v-for="item in filteredMarket"
            :key="item.id"
            class="flex flex-col gap-2 rounded-xl border border-neutral-800 bg-neutral-800/60 p-4"
          >
            <div class="flex items-start justify-between gap-2">
              <h3 class="font-mono text-sm font-semibold text-neutral-100">
                {{ item.id }}
              </h3>
              <span
                class="rounded-full bg-neutral-900 px-2 py-0.5 text-[10px] text-neutral-400"
              >
                {{ latestOf(item.id) }}
              </span>
            </div>
            <p class="min-h-8 text-xs leading-relaxed text-neutral-400">
              {{ item.description }}
            </p>
            <div class="mt-auto flex items-center gap-2">
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
            </div>
          </div>
        </div>
      </template>

      <!-- 已安装页：按分类分组，每行操作齐全（§13.3） -->
      <template v-else>
        <p v-if="store.installed.length === 0" class="text-sm text-neutral-600">
          {{ t("market.emptyInstalled") }}
        </p>
        <section v-for="[cat, list] in groupedInstalled" :key="cat" class="mb-5">
          <h3
            class="mb-2 text-[10px] font-semibold uppercase tracking-wider text-neutral-500"
          >
            {{ cat }}
          </h3>
          <div class="overflow-hidden rounded-xl border border-neutral-800">
            <div
              v-for="p in list"
              :key="p.id"
              class="flex flex-wrap items-center gap-x-4 gap-y-2 border-b border-neutral-800 bg-neutral-800/40 px-4 py-3 last:border-b-0"
            >
              <div class="min-w-40">
                <p class="font-mono text-sm font-semibold text-neutral-100">
                  {{ p.id }}
                </p>
                <p class="text-[11px] text-neutral-500">
                  {{ t("market.versionLabel") }} {{ p.version }}
                </p>
              </div>
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

              <div class="ml-auto flex items-center gap-2">
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
