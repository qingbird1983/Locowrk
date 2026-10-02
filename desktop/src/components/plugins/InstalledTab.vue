<script setup lang="ts">
/**
 * 已安装页（壳批次拆分 + ⑩ 闲置 UI 接线）：搜索 + origin 筛选 chips +
 * 闲置筛选 chips（exposure 文档 §9.2，文案同用"多久"）+ 行内 IdleBadge
 * （§9.1 只显示一句"多久没用"）+ 多选批量栏（§9.5；批量卸载二次确认 =
 * §五 ConfirmDialog Modal，名单/数量/破坏范围一次说清）。闲置提示只在
 * 用户主动打开本页时出现（§8.8 不弹 toast）。
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
import ConfirmDialog, { type ConfirmItem } from "../shell/ConfirmDialog.vue";
import IdleBadge from "../IdleBadge.vue";
import PluginIcon from "../PluginIcon.vue";
import SideEffectBadges from "../SideEffectBadges.vue";
import type { InstalledPlugin } from "../../types";

const { t, locale } = useI18n();
const store = usePluginStore();

const search = ref("");
/** 空串 = 全部（含在 chips 首位）。 */
const originFilter = ref("");
/** ⑩ 闲置筛选档位（后端 filter_key，嵌套包含）。 */
const usageFilter = ref("");

const originFilters = computed(() => [
  { key: "", label: t("market.filterAll") },
  { key: "official", label: t("market.origin_official") },
  { key: "community", label: t("market.origin_community") },
]);

const usageFilters = computed(() => [
  { key: "", label: t("market.filterAll") },
  { key: "active", label: t("market.stateActive") },
  { key: "disabled", label: t("market.usageDisabled") },
  { key: "gt1m", label: t("market.idleGt1m") },
  { key: "gt3m", label: t("market.idleGt3m") },
  { key: "gt6m", label: t("market.idleGt6m") },
]);

/** 档位嵌套：选 gt1m 也命中 gt3m/gt6m 的行（时长档位语义）。 */
function matchesUsageFilter(p: InstalledPlugin): boolean {
  if (!usageFilter.value) return true;
  const key = store.usageOf(p.id)?.filter_key;
  if (usageFilter.value === "active") return !!p.active_version;
  if (usageFilter.value === "disabled") return !p.enabled;
  if (!key || key === "recent") return false;
  const rank = { gt1m: 1, gt3m: 2, gt6m: 3 } as const;
  return rank[key as keyof typeof rank] >= rank[usageFilter.value as "gt1m" | "gt3m" | "gt6m"];
}

/** 搜索 + 双排筛选后的已安装列表。 */
const filtered = computed(() => {
  const q = search.value.trim().toLowerCase();
  return store.installed.filter((p) => {
    if (originFilter.value) {
      const isOfficial = p.display.origin === "official";
      if (originFilter.value === "official" && !isOfficial) return false;
      if (originFilter.value === "community" && isOfficial) return false;
    }
    if (!matchesUsageFilter(p)) return false;
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
  store.installed.length === 0 ? t("market.emptyInstalled") : t("market.noMatch"),
);

/** ⑩ 顶部提示条（§9.3）：超 3 个月没用（含半年档）的计数；[去看看]
 * 切到对应筛选。§8.8——只在本页静态出现，不弹 toast 不打断聊天。 */
const staleCount = computed(
  () =>
    store.usage.filter(
      (u) => u.filter_key === "gt3m" || u.filter_key === "gt6m",
    ).length,
);

function updateAvailableOf(p: InstalledPlugin): boolean {
  return updateAvailable(store.marketplace, p);
}

// ---- ⑩⑥ 批量操作（多选 + 批量栏；批量卸载 = §五 ConfirmDialog Modal，
// 名单/数量/破坏范围一次说清——v3.8 走查把批量栏内联两步确认升级成 Modal）----

/** 多选集合（内置插件不进批量——不可卸载；停用/启用走行内按钮）。 */
const selected = ref(new Set<string>());
/** 批量卸载确认弹窗开合（§五 唯一带遮罩 Modal）。 */
const confirmingUninstall = ref(false);
/** 最近一次批量操作的部分失败（§10.2 逐条成败）。 */
const batchError = ref("");

function toggle(id: string, on: boolean) {
  if (on) selected.value.add(id);
  else selected.value.delete(id);
  selected.value = new Set(selected.value);
  confirmingUninstall.value = false;
}

function toggleAll(on: boolean) {
  if (on) {
    store.installed
      .filter((p) => p.source !== "builtin")
      .forEach((p) => selected.value.add(p.id));
  } else {
    selected.value.clear();
  }
  selected.value = new Set(selected.value);
  confirmingUninstall.value = false;
}

const selectedIds = computed(() =>
  store.installed.filter((p) => selected.value.has(p.id)).map((p) => p.id),
);
/** 确认弹窗名单（名字 + id · 版本，mono 元信息）。 */
const confirmItems = computed<ConfirmItem[]>(() =>
  store.installed
    .filter((p) => selected.value.has(p.id))
    .map((p) => ({
      id: p.id,
      name: displayName(p, locale.value),
      meta: `${p.id} · ${p.version}`,
    })),
);

async function runBatch(op: "disable" | "enable" | "uninstall") {
  const ids = selectedIds.value;
  if (ids.length === 0) return;
  if (op !== "uninstall") {
    batchError.value = "";
    const res = await store.batchSetEnabled(ids, op === "enable");
    if (res.failed.length > 0) {
      batchError.value = t("market.batchPartialFail", { n: res.failed.length }) + " " + res.failed.map((f) => f.id).join(", ");
    }
    selected.value = new Set();
    return;
  }
  // 二次确认走 §五 ConfirmDialog（名单/数量/破坏范围一次说清）
  confirmingUninstall.value = true;
}

/** 弹窗里按下「确认卸载」。 */
async function confirmUninstall() {
  confirmingUninstall.value = false;
  const ids = selectedIds.value;
  if (ids.length === 0) return;
  batchError.value = "";
  const res = await store.batchUninstall(ids);
  if (res.failed.length > 0) {
    batchError.value = t("market.batchPartialFail", { n: res.failed.length }) + " " + res.failed.map((f) => f.id).join(", ");
  }
  selected.value = new Set();
}
</script>

<template>
  <ListPage
    v-model:search="search"
    v-model:filter="originFilter"
    v-model:sub-filter="usageFilter"
    :search-placeholder="t('market.search')"
    :filters="originFilters"
    :sub-filters="usageFilters"
    :is-empty="isEmpty"
    :empty-text="emptyText"
    :batching="selected.size > 0"
  >
    <!-- 顶部提示条（§9.3）：只在主动打开本页时可见，点击切换筛选 -->
    <button
      v-if="staleCount > 0 && !usageFilter"
      class="mb-3 flex w-full items-center gap-2 rounded-xl bg-warn/10 px-4 py-2 text-xs text-warn transition hover:bg-warn/16"
      @click="usageFilter = 'gt3m'"
    >
      <span>💡 {{ t("market.hintIdle", { n: staleCount }) }}</span>
      <span class="ml-auto font-medium underline underline-offset-2">
        {{ t("market.hintGo") }}
      </span>
    </button>

    <p
      v-if="batchError"
      class="mb-3 rounded-lg bg-danger/10 px-3 py-2 text-xs text-danger"
    >
      {{ batchError }}
    </p>

    <section v-for="group in originGroups" :key="group.key" class="mb-5">
      <h3
        class="mb-2 text-[10px] font-semibold uppercase tracking-wider text-ink-4"
      >
        {{ t(`market.origin_${group.key}`) }}
      </h3>
      <div class="overflow-hidden rounded-xl border border-line">
        <div
          v-for="p in group.list"
          :key="p.id"
          class="flex flex-wrap items-center gap-x-4 gap-y-2 bg-wash-1 px-4 py-3"
        >
            <!-- 多选框：内置插件不进批量（不可卸载，停用/启用走行内按钮） -->
            <input
              v-if="p.source !== 'builtin'"
              type="checkbox"
              class="h-3.5 w-3.5 accent-[var(--lw-accent)]"
              :checked="selected.has(p.id)"
              :title="t('market.selectAll')"
              @change="toggle(p.id, ($event.target as HTMLInputElement).checked)"
            />
          <PluginIcon :icon="p.display.icon" :id="p.id" size="h-9 w-9" />
          <div class="min-w-40">
            <p class="text-sm font-semibold text-ink-1">
              {{ displayName(p, locale) }}
              <span class="ml-1 font-mono text-[10px] font-normal text-ink-5">{{
                p.id
              }}</span>
            </p>
            <p class="text-[11px] text-ink-4">
              {{ t("market.versionLabel") }} {{ p.version }}
            </p>
          </div>
          <SideEffectBadges :kinds="p.display.side_effects" />
          <!-- 状态徽标：运行中 / 已停用 -->
          <span
            class="rounded-full px-2 py-0.5 text-[10px]"
            :class="
              p.active_version
                ? 'bg-ok/16 text-ok'
                : p.enabled
                  ? 'bg-warn/16 text-warn'
                  : 'bg-wash-3 text-ink-2'
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
          <!-- ⑩ 闲置徽章："多久没用"一句（§9.1 无次数无趋势） -->
          <IdleBadge :idle="store.usageOf(p.id)?.idle ?? null" />
          <span
            v-if="updateAvailableOf(p)"
            class="rounded-full bg-info/16 px-2 py-0.5 text-[10px] text-info"
          >
            {{ t("market.updateAvailable", { version: latestOf(store.marketplace, p.id) }) }}
          </span>

          <!-- 内置插件（S2）：随程序分发不给卸载，但可停用/启用——
               lockfile Builtin 条目持久化，重启不复活停用项 -->
          <div v-if="p.source === 'builtin'" class="ml-auto flex items-center gap-2">
            <span
              class="rounded-full bg-surface-0 px-2 py-0.5 text-[10px] text-ink-3"
            >
              {{ t("market.builtinBadge") }}
            </span>
            <button
              class="rounded-lg border border-line-strong px-3 py-1.5 text-xs text-ink-2 transition hover:bg-surface-2 disabled:cursor-not-allowed disabled:opacity-40"
              :disabled="store.isBusy(p.id)"
              @click="store.setEnabled(p.id, !p.enabled)"
            >
              {{ p.enabled ? t("market.disable") : t("market.enable") }}
            </button>
          </div>
          <div v-else class="ml-auto flex items-center gap-2">
            <button
              class="rounded-lg border border-line-strong px-3 py-1.5 text-xs text-ink-2 transition hover:bg-surface-2 disabled:cursor-not-allowed disabled:opacity-40"
              :disabled="store.isBusy(p.id)"
              @click="store.setEnabled(p.id, !p.enabled)"
            >
              {{ p.enabled ? t("market.disable") : t("market.enable") }}
            </button>
            <button
              v-if="updateAvailableOf(p)"
              class="rounded-lg bg-accent px-3 py-1.5 text-xs font-semibold text-accent-ink transition hover:bg-accent-hover disabled:cursor-not-allowed disabled:opacity-40"
              :disabled="store.isBusy(p.id)"
              @click="store.update(p.id)"
            >
              {{ t("market.update") }}
            </button>
            <template v-if="rollbackTargets(p).length > 0">
              <select
                class="rounded border border-line-strong bg-surface-0 px-2 py-1.5 text-xs text-ink-2 outline-none"
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
              class="rounded-lg text-danger transition hover:bg-danger/10 disabled:cursor-not-allowed disabled:opacity-40"
              :disabled="store.isBusy(p.id)"
              @click="store.uninstall(p.id)"
            >
              {{ t("market.uninstall") }}
            </button>
          </div>
        </div>
      </div>
    </section>

    <!-- 批量操作栏（#batch 插槽）：全选 + 停用/启用/卸载，卸载二次确认列名 -->
    <template #batch>
      <label class="flex items-center gap-1.5 text-xs text-ink-2">
        <input
          type="checkbox"
          class="h-3.5 w-3.5 accent-[var(--lw-accent)]"
          :checked="
            selectedIds.length > 0 &&
            selectedIds.length ===
              store.installed.filter((p) => p.source !== 'builtin').length
          "
          @change="toggleAll(($event.target as HTMLInputElement).checked)"
        />
        {{ t("market.selectAll") }}
      </label>
      <span class="text-xs text-ink-4">
        {{ t("market.selectedCount", { n: selected.size }) }}
      </span>

      <button
        class="rounded-lg border border-line-strong px-3 py-1.5 text-xs text-ink-2 transition hover:bg-surface-2"
        @click="runBatch('disable')"
      >
        {{ t("market.batchDisable") }}
      </button>
      <button
        class="rounded-lg border border-line-strong px-3 py-1.5 text-xs text-ink-2 transition hover:bg-surface-2"
        @click="runBatch('enable')"
      >
        {{ t("market.batchEnable") }}
      </button>
      <button
        class="rounded-lg text-danger transition hover:bg-danger/10"
        @click="runBatch('uninstall')"
      >
        {{ t("market.batchUninstall") }}
      </button>
    </template>
    <!-- 批量卸载二次确认（§五 唯一带遮罩 Modal：名单/数量/破坏范围一次说清） -->
    <ConfirmDialog
      :open="confirmingUninstall"
      :title="t('market.batchUninstallConfirm', { n: selected.size })"
      :sub="t('market.batchUninstallSub')"
      :items="confirmItems"
      :items-label="t('market.batchUninstallList', { n: selected.size })"
      :impact="t('market.batchUninstallImpact', { n: selected.size })"
      :confirm-label="t('market.batchUninstallGo')"
      @confirm="confirmUninstall"
      @cancel="confirmingUninstall = false"
    />
  </ListPage>
</template>
