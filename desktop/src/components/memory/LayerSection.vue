<script setup lang="ts">
/**
 * 层区（S1 刀④b 的三枚行为开关 + 刀④c 的「每层一行」= ADR-0007 §18 那一整句：
 * 「三枚行为开关各关各的 + 每层一行（文件数 / 更新时间 / 打开文件夹 / 删除）」）。
 *
 * 为什么两件事放同一个组件：§18 那句本来就是一件事——"这一层"的身份、它进不进上下文、
 * 它对人的磁盘动作必须在同一处对齐。拆成两块就会漂成"上面关的是全局层、下面清的是另一个
 * 目录"，而那种错位界面上看不出来。
 *
 * 为什么落点在记忆页而不是设置页：设置页那行 `repoContext` 因此撤了——同一枚开关在两页
 * 各摆一次就是两处真相（字段仍随 view 回填、原样保存）。放在事实库档而不是看板档：看板是
 * 只读导航，不该带控件。
 *
 * 为什么**切换即写回**、不给这一区加「保存」行：本页每一个动作——删除、清空、行内编辑
 * ——都是点一下就落盘，它不是表单页；加保存行会让"改了没保存"变成这一页的新状态。
 *
 * ⚠ 层的身份 = 后端给回来的 `hint`，界面只做"原样传回"。「显示哪一层 / 清哪一层 /
 * 开哪个文件夹」三件事因此指的是同一个 `LayerDir`；前端**不算路径**、也不按
 * `MemoryRow.workspace` 自己分组（那一维在人自己新建的文件上只是目录名兜底）。
 *
 * ⚠ 「删除该层」是**软删**：那一层的事实文件全部搬进 `obsolete/`，可以逐条恢复；
 * 真正抹掉只发生在回收区那块点「清空回收区」的那一次（AC-S1-6 与 AC-S1-7 的分界，
 * 所以这句话必须在界面上写出来，不能只写在代码注释里）。
 */
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import HoverTip from "../settings/HoverTip.vue";
import SettingsGroup from "../settings/SettingsGroup.vue";
import SettingsRow from "../settings/SettingsRow.vue";
import SettingsToggle from "../settings/SettingsToggle.vue";
import { useSettingsStore } from "../../stores/settings";
import { memoryClearLayer, memoryLayers, memoryRevealLayer } from "../../lib/api";
import { relativeTimeLabel, relativeTimeSec } from "../../lib/format";
import { workspaceDisplayName } from "../../lib/session-groups";
import type { LayersView } from "../../types";

/** 清层 / 恢复之后叫父页重拉事实列表：那一块的数据源与这里是同一批文件。 */
const emit = defineEmits<{ changed: [] }>();

const { t } = useI18n();
const settings = useSettingsStore();
const layerView = computed(() => settings.view?.memory ?? null);

/**
 * 三枚开关 = 一张表。`save` 各写自己的 patch，不用 `{ [key]: on }` 那种计算键——
 * 它在类型面上退化成索引签名，"这三枚字段都属于 MemoryUpdate"就查不出来了
 * （字段名漂一个，编译不红、静默存一个没人读的键）。
 *
 * `?? true` 是**视图还没加载完**那一瞬的显示兜底，不是第二处缺省定义：后端那三枚缺省
 * 常量（`GLOBAL_LAYER_DEFAULT` / `PROJECT_LAYER_DEFAULT` / `REPO_CONTEXT_DEFAULT`）都是
 * `true`，加载完成后这一行读回的就是真值。
 */
const switches = [
  {
    key: "globalLayer",
    title: t("settings.memoryGlobalLayer"),
    hint: t("settings.memoryGlobalLayerHint"),
    on: () => layerView.value?.globalLayer ?? true,
    save: (on: boolean) => settings.saveMemory({ globalLayer: on }),
  },
  {
    key: "projectLayer",
    title: t("settings.memoryProjectLayer"),
    hint: t("settings.memoryProjectLayerHint"),
    on: () => layerView.value?.projectLayer ?? true,
    save: (on: boolean) => settings.saveMemory({ projectLayer: on }),
  },
  {
    key: "repoContext",
    title: t("settings.memoryRepoContext"),
    hint: t("settings.memoryRepoContextHint"),
    on: () => layerView.value?.repoContext ?? true,
    save: (on: boolean) => settings.saveMemory({ repoContext: on }),
  },
];

const view = ref<LayersView | null>(null);
const loadError = ref("");

async function load() {
  loadError.value = "";
  try {
    view.value = await memoryLayers();
  } catch (e) {
    loadError.value = String(e);
  }
}
onMounted(() => void load());
/** 回收区那边恢复了一条 ⇒ 这一层的条数变了，父页拿这个把手重拉一次。 */
defineExpose({ reload: load });

/**
 * 全局层那一行永远置顶，其余按"最近有更新"倒序。排序属于**呈现**（后端给的是磁盘枚举序），
 * 所以在这里做，不留给后端猜界面要什么。
 */
const rows = computed(() => {
  const list = view.value?.rows ?? [];
  return [...list].sort((a, b) => {
    if (!a.hint) return -1;
    if (!b.hint) return 1;
    return b.newest_updated_at - a.newest_updated_at;
  });
});

function layerLabel(hint: string): string {
  return hint ? workspaceDisplayName(hint) : t("memory.globalTier");
}

function updatedLabel(row: { newest_updated_at: number }): string {
  if (!row.newest_updated_at) return t("memory.layerNever");
  return t("memory.layerUpdated", {
    time: relativeTimeLabel(relativeTimeSec(row.newest_updated_at), (k, p) => t(k, p)),
  });
}

function openLayer(hint: string) {
  loadError.value = "";
  memoryRevealLayer(hint).catch((e) => {
    loadError.value = String(e);
  });
}

/** 行内二次确认（与事实列表的删除、批量卸载同款"再点一次"）。 */
const confirmingClear = ref<string | null>(null);
async function clearLayer(hint: string) {
  if (confirmingClear.value !== hint) {
    confirmingClear.value = hint;
    return;
  }
  confirmingClear.value = null;
  try {
    await memoryClearLayer(hint);
    await load();
    emit("changed");
  } catch (e) {
    loadError.value = String(e);
  }
}
</script>

<template>
  <div class="shrink-0 px-5 pb-2 pt-3">
    <SettingsGroup
      :title="t('settings.memoryLayers')"
      :hint="t('settings.memoryLayersHint')"
    >
      <SettingsRow
        v-for="s in switches"
        :key="s.key"
        :title="s.title"
        :hint="s.hint"
      >
        <SettingsToggle
          :model-value="s.on()"
          :label="s.title"
          @update:model-value="s.save"
        />
      </SettingsRow>

      <!-- 每层一行（文件数 / 更新时间 / 打开文件夹 / 删除该层）。限高内滚：本机实测数据根里
           可能有几十个历史项目层，全部铺开会把事实列表挤出一屏（同类清单的形态与
           ListPage 一致：区内滚动、不分页）。 -->
      <p class="mt-1 text-[length:var(--lw-fs-11)] text-ink-5">
        {{ t("memory.layerRowsHint") }}
      </p>
      <p
        v-if="loadError"
        class="mt-1 rounded-[var(--lw-r-xs)] bg-danger/10 px-2 py-1 text-[length:var(--lw-fs-11)] text-danger"
      >
        {{ loadError }}
      </p>
      <ul class="mt-1 max-h-44 space-y-1 overflow-y-auto">
        <li
          v-for="row in rows"
          :key="row.hint || '__global__'"
          class="flex items-center gap-2 rounded-[var(--lw-r-md)] bg-wash-1 px-3 py-2"
        >
          <span class="min-w-0 flex-1 truncate text-xs text-ink-1">
            {{ layerLabel(row.hint) }}
          </span>
          <HoverTip
            v-if="row.unreadable"
            class="shrink-0"
            :text="t('memory.layerUnreadable', { n: row.unreadable })"
            :label="t('memory.layerUnreadable', { n: row.unreadable })"
          >
            <span class="shrink-0 text-[length:var(--lw-fs-10)] text-ink-5">
              {{ t("memory.layerFacts", { n: row.fact_count }) }}
              +
            </span>
          </HoverTip>
          <span
            v-else
            class="shrink-0 text-[length:var(--lw-fs-10)] text-ink-5"
          >
            {{ t("memory.layerFacts", { n: row.fact_count }) }}
          </span>
          <span class="shrink-0 text-[length:var(--lw-fs-10)] text-ink-5">
            {{ updatedLabel(row) }}
          </span>
          <button
            class="shrink-0 rounded-[var(--lw-r-xs)] px-2 py-1 text-[length:var(--lw-fs-11)] text-ink-4 transition hover:bg-wash-2 hover:text-ink-1"
            @click="openLayer(row.hint)"
          >
            {{ t("memory.layerOpen") }}
          </button>
          <button
            class="shrink-0 rounded-[var(--lw-r-xs)] px-2 py-1 text-[length:var(--lw-fs-11)] transition"
            :class="
              confirmingClear === row.hint
                ? 'bg-danger/16 text-danger'
                : 'text-ink-4 hover:bg-wash-2 hover:text-danger'
            "
            @click="clearLayer(row.hint)"
          >
            {{
              confirmingClear === row.hint
                ? t("memory.layerClearConfirm")
                : t("memory.layerClear")
            }}
          </button>
        </li>
      </ul>
      <!-- 两个"没能读出来"的数必须落在界面上：少了它们，"整层读不动"就冒充"这层没记过"。 -->
      <p
        v-if="view && view.blocked_layers"
        class="mt-1 text-[length:var(--lw-fs-11)] text-danger"
      >
        {{ t("memory.layerBlocked", { n: view.blocked_layers }) }}
      </p>
      <p
        v-if="view && view.unreadable_files"
        class="mt-1 text-[length:var(--lw-fs-11)] text-ink-5"
      >
        {{ t("memory.layerUnreadableTotal", { n: view.unreadable_files }) }}
      </p>
    </SettingsGroup>
  </div>
</template>
