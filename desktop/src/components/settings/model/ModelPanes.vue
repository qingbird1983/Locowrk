<script setup lang="ts">
/**
 * 模型双栏选择（UI shell 文档 §3.30 添加/编辑服务模态，参考 PI-Desktop
 * ModelSelectionPanes）：左「该服务的模型」= 全选框（半态）+ 获取列表 +
 * 搜索 + 复选行（ID mono 可选中复制 + 显示名 + 右缘窗口徽章，取自静态
 * 档案，缺省空白不发明）；右「模型设置 N」= 搜索 + 拖拽行（把手 + ID +
 * 别名 + 窗口徽章 + 高级折叠 + ×）+ 高级展开（别名 + 上下文窗口预设档
 * chips + 数字输入）+ 自定义模型添加行（重 ID 报已添加）。
 * 探测/防抖在父组件（ProviderSetupDialog），这里只摆清单与选中态。
 */
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Icon } from "../../../lib/icons";
import { formatLimitShort, WINDOW_PRESETS } from "../../../lib/provider-presets";
import { profileWindow } from "../../../lib/model-profiles";
import type { ProviderModelView } from "../../../types";

const props = defineProps<{
  /** 探测到的可用模型 id（空 = 未探测/无清单）。 */
  available: string[];
  /** 探测状态：idle 未探测 / loading / ready / error。 */
  probeState: "idle" | "loading" | "ready" | "error";
  /** 已选清单（右栏真源，父组件持久）。 */
  chosen: ProviderModelView[];
  disabled?: boolean;
}>();
const emit = defineEmits<{
  fetch: [];
  /** 整表替换已选（勾选/取消/拖拽/添加/移除统一走这里）。 */
  update: [models: ProviderModelView[]];
}>();

const { t } = useI18n();
const availQuery = ref("");
const chosenQuery = ref("");
const customId = ref("");
const customError = ref("");
const expandedId = ref<string | null>(null);
const dragId = ref<string | null>(null);
const overIndex = ref(-1);

const selectedSet = computed(() => new Set(props.chosen.map((m) => m.id.toLowerCase())));

/** 左栏行 = 探测清单 ∪ 已选手工 id（探测失败/离线时已选仍可见可管理）。 */
const availRows = computed(() => {
  const ids = new Set(props.available.map((i) => i.toLowerCase()));
  const merged = [...props.available];
  for (const m of props.chosen) {
    if (!ids.has(m.id.toLowerCase())) merged.push(m.id);
  }
  const needle = availQuery.value.trim().toLowerCase();
  return needle ? merged.filter((i) => i.toLowerCase().includes(needle)) : merged;
});
const selectedVisible = computed(
  () => availRows.value.filter((i) => selectedSet.value.has(i.toLowerCase())).length,
);
const allVisible = computed(
  () => availRows.value.length > 0 && selectedVisible.value === availRows.value.length,
);
const someVisible = computed(
  () => selectedVisible.value > 0 && selectedVisible.value < availRows.value.length,
);

/** 显示名（静态档案前缀命中；未命中空串不渲染）。 */
const displayName = (id: string): string => {
  const hit = props.chosen.find((m) => m.id.toLowerCase() === id.toLowerCase());
  return hit?.alias?.trim() || "";
};
/** 窗口徽章：已配 contextWindow ?? 静态档案 ?? 空。 */
const windowBadge = (id: string): string => {
  const hit = props.chosen.find((m) => m.id.toLowerCase() === id.toLowerCase());
  return formatLimitShort(hit?.contextWindow ?? profileWindow(id));
};

function toggle(id: string) {
  const key = id.toLowerCase();
  if (selectedSet.value.has(key)) {
    emit("update", props.chosen.filter((m) => m.id.toLowerCase() !== key));
    return;
  }
  emit("update", [...props.chosen, { id, alias: null, contextWindow: null }]);
}

function toggleVisible(select: boolean) {
  if (!select) {
    const keys = new Set(availRows.value.map((i) => i.toLowerCase()));
    emit("update", props.chosen.filter((m) => !keys.has(m.id.toLowerCase())));
    return;
  }
  const additions = availRows.value
    .filter((i) => !selectedSet.value.has(i.toLowerCase()))
    .map((id) => ({ id, alias: null, contextWindow: null }));
  emit("update", [...props.chosen, ...additions]);
}

const chosenRows = computed(() => {
  const needle = chosenQuery.value.trim().toLowerCase();
  if (!needle) return props.chosen;
  return props.chosen.filter(
    (m) =>
      m.id.toLowerCase().includes(needle) ||
      (m.alias ?? "").toLowerCase().includes(needle),
  );
});

function updateChosen(id: string, patch: Partial<ProviderModelView>) {
  emit(
    "update",
    props.chosen.map((m) => (m.id === id ? { ...m, ...patch } : m)),
  );
}

function removeChosen(id: string) {
  emit("update", props.chosen.filter((m) => m.id !== id));
  if (expandedId.value === id) expandedId.value = null;
}

/** 拖拽排序（HTML5 dnd）：dragover 记落点，drop 重排。 */
function onDrop(targetId: string) {
  const from = props.chosen.findIndex((m) => m.id === dragId.value);
  const to = props.chosen.findIndex((m) => m.id === targetId);
  dragId.value = null;
  overIndex.value = -1;
  if (from === -1 || to === -1 || from === to) return;
  const next = [...props.chosen];
  const [moved] = next.splice(from, 1);
  next.splice(to, 0, moved);
  emit("update", next);
}

/** 手工 id 落右栏即刻展开（PI 同款）；重 id 报已添加。 */
function addCustom() {
  const id = customId.value.trim();
  if (!id) return;
  if (props.chosen.some((m) => m.id.toLowerCase() === id.toLowerCase())) {
    customError.value = t("settings.modelAlreadyAdded");
    return;
  }
  const known = props.available.find((i) => i.toLowerCase() === id.toLowerCase());
  const win = profileWindow(known ?? id);
  emit("update", [
    ...props.chosen,
    { id: known ?? id, alias: null, contextWindow: win },
  ]);
  expandedId.value = known ?? id;
  customId.value = "";
  customError.value = "";
}

// 左栏随探测换血时收起可能已失效的展开行
watch(() => props.available, () => (expandedId.value = null));
</script>

<template>
  <div class="grid min-h-0 flex-1 grid-cols-2 gap-4">
    <!-- 左栏：该服务的模型 -->
    <div class="flex min-h-0 flex-col rounded-[var(--lw-r-md)] bg-wash-1 p-3">
      <div class="flex items-center gap-2">
        <input
          v-if="availRows.length > 0"
          type="checkbox"
          class="h-3.5 w-3.5 accent-[var(--lw-accent)]"
          :checked="allVisible"
          :indeterminate="someVisible"
          :disabled="disabled"
          :aria-label="t('settings.modelSelectAllVisible')"
          @change="toggleVisible(($event.target as HTMLInputElement).checked)"
        />
        <h4 class="text-[12.5px] font-semibold text-ink-1">{{ t("settings.modelAvailable") }}</h4>
        <button
          type="button"
          class="ml-auto flex items-center gap-1 rounded-[var(--lw-r-xs)] px-2 py-1 text-[11.5px] text-ink-2 transition hover:bg-wash-2 disabled:opacity-50"
          :disabled="disabled || probeState === 'loading'"
          @click="emit('fetch')"
        >
          <Icon name="refresh" :size="13" :class="probeState === 'loading' ? 'animate-none' : ''" />
          {{ probeState === "loading" ? t("settings.modelProbing") : t("settings.modelFetchList") }}
        </button>
      </div>
      <div class="mt-2 flex h-8 items-center gap-2 rounded-[var(--lw-r-xs)] bg-wash-2 px-2.5">
        <Icon name="search" :size="13" class="shrink-0 text-ink-4" />
        <input
          v-model="availQuery"
          class="w-full bg-transparent text-[12.5px] text-ink-1 outline-none placeholder:text-ink-5"
          :placeholder="t('settings.modelSearchId')"
          spellcheck="false"
          autocomplete="off"
        />
      </div>
      <div class="mt-2 min-h-0 flex-1 overflow-y-auto">
        <p v-if="probeState === 'idle'" class="px-1 py-4 text-[12px] leading-relaxed text-ink-4">
          {{ t("settings.modelAvailIdle") }}
        </p>
        <p v-else-if="probeState === 'error' && props.available.length === 0"
           class="px-1 py-4 text-[12px] text-danger">
          {{ t("settings.modelAvailError") }}
        </p>
        <p v-else-if="availRows.length === 0" class="px-1 py-4 text-[12px] text-ink-4">
          {{
            probeState === "loading"
              ? t("settings.modelProbing")
              : t("settings.modelNoMatch")
          }}
        </p>
        <ul v-else class="space-y-0.5">
          <li v-for="id in availRows" :key="id">
            <label
              class="flex cursor-pointer items-center gap-2 rounded-[var(--lw-r-xs)] px-1.5 py-1.5 hover:bg-wash-2"
            >
              <input
                type="checkbox"
                class="h-3.5 w-3.5 accent-[var(--lw-accent)]"
                :checked="selectedSet.has(id.toLowerCase())"
                :disabled="disabled"
                @change="toggle(id)"
              />
              <span class="min-w-0 flex-1 select-text truncate font-mono text-[12px] text-ink-1">{{ id }}</span>
              <span v-if="displayName(id)" class="shrink-0 text-[11px] text-ink-4">{{ displayName(id) }}</span>
              <span v-if="windowBadge(id)" class="shrink-0 text-[10.5px] tabular-nums text-ink-4">
                {{ windowBadge(id) }}
              </span>
            </label>
          </li>
        </ul>
      </div>
    </div>

    <!-- 右栏：模型设置 -->
    <div class="flex min-h-0 flex-col rounded-[var(--lw-r-md)] bg-wash-1 p-3">
      <div class="flex items-center gap-2">
        <h4 class="text-[12.5px] font-semibold text-ink-1">{{ t("settings.modelChosen") }}</h4>
        <span v-if="chosen.length" class="rounded-full bg-wash-2 px-1.5 text-[10.5px] tabular-nums text-ink-3">
          {{ chosen.length }}
        </span>
        <div v-if="chosen.length" class="ml-auto flex h-7 w-36 items-center gap-1.5 rounded-[var(--lw-r-xs)] bg-wash-2 px-2">
          <Icon name="search" :size="12" class="shrink-0 text-ink-4" />
          <input
            v-model="chosenQuery"
            class="w-full bg-transparent text-[11.5px] text-ink-1 outline-none placeholder:text-ink-5"
            :placeholder="t('settings.modelSearchChosen')"
            spellcheck="false"
            autocomplete="off"
          />
        </div>
      </div>
      <div class="mt-2 min-h-0 flex-1 overflow-y-auto">
        <p v-if="chosen.length === 0" class="px-1 py-4 text-[12px] text-ink-4">
          {{ t("settings.modelChosenEmpty") }}
        </p>
        <p v-else-if="chosenRows.length === 0" class="px-1 py-4 text-[12px] text-ink-4">
          {{ t("settings.modelNoMatch") }}
        </p>
        <ul v-else class="space-y-1">
          <li
            v-for="m in chosenRows"
            :key="m.id"
            class="rounded-[var(--lw-r-xs)] bg-wash-2"
            :class="dragId === m.id ? 'opacity-50' : ''"
            @dragover.prevent="overIndex = chosen.indexOf(m)"
            @drop.prevent="onDrop(m.id)"
          >
            <div class="flex items-center gap-1.5 px-1.5 py-1.5">
              <button
                type="button"
                class="cursor-grab text-ink-4 hover:text-ink-2"
                draggable="true"
                :aria-label="t('settings.modelReorder')"
                @dragstart="dragId = m.id"
                @dragend="dragId = null; overIndex = -1"
              >
                <Icon name="grip" :size="13" />
              </button>
              <span class="min-w-0 flex-1 select-text truncate font-mono text-[12px] text-ink-1">{{ m.id }}</span>
              <span v-if="m.alias?.trim()" class="shrink-0 text-[11px] text-ink-4">{{ m.alias }}</span>
              <span class="shrink-0 text-[10.5px] tabular-nums text-ink-4">
                {{ formatLimitShort(m.contextWindow ?? profileWindow(m.id)) }}
              </span>
              <button
                type="button"
                class="rounded-[var(--lw-r-3xs)] px-1.5 py-0.5 text-[11px] text-ink-3 hover:bg-wash-3 hover:text-ink-1"
                :aria-expanded="expandedId === m.id"
                @click="expandedId = expandedId === m.id ? null : m.id"
              >
                {{ t("settings.modelAdvanced") }}
              </button>
              <button
                type="button"
                class="rounded-[var(--lw-r-3xs)] p-0.5 text-ink-4 hover:bg-wash-3 hover:text-danger"
                :aria-label="t('settings.modelRemove')"
                :disabled="disabled"
                @click="removeChosen(m.id)"
              >
                <Icon name="close" :size="12" />
              </button>
            </div>
            <div v-if="expandedId === m.id" class="space-y-2 border-t border-line-subtle px-2.5 py-2">
              <label class="flex items-center gap-2">
                <span class="w-16 shrink-0 text-[11px] text-ink-4">{{ t("settings.modelAlias") }}</span>
                <input
                  :value="m.alias ?? ''"
                  class="lw-field h-7 w-full flex-1 px-2 text-[12px]"
                  spellcheck="false"
                  :placeholder="t('settings.modelAliasPlaceholder')"
                  @input="updateChosen(m.id, { alias: ($event.target as HTMLInputElement).value || null })"
                />
              </label>
              <div class="flex items-start gap-2">
                <span class="w-16 shrink-0 pt-1 text-[11px] text-ink-4">{{ t("settings.modelWindow") }}</span>
                <div class="min-w-0 flex-1">
                  <div class="flex flex-wrap gap-1">
                    <button
                      v-for="p in WINDOW_PRESETS"
                      :key="p.tokens"
                      type="button"
                      class="rounded-full px-2 py-0.5 text-[10.5px] transition"
                      :class="
                        m.contextWindow === p.tokens
                          ? 'bg-accent text-accent-ink'
                          : 'bg-wash-3 text-ink-3 hover:text-ink-1'
                      "
                      :aria-pressed="m.contextWindow === p.tokens"
                      @click="updateChosen(m.id, { contextWindow: p.tokens })"
                    >
                      {{ p.label }}
                    </button>
                  </div>
                  <input
                    type="number"
                    min="1"
                    inputmode="numeric"
                    :value="m.contextWindow ?? ''"
                    class="lw-field lw-field-num mt-1.5"
                    :placeholder="t('settings.modelWindowPlaceholder')"
                    @change="
                      updateChosen(m.id, {
                        contextWindow: Number(($event.target as HTMLInputElement).value) || null,
                      })
                    "
                  />
                </div>
              </div>
            </div>
          </li>
        </ul>
      </div>
      <div class="mt-2 border-t border-line-subtle pt-2">
        <p class="mb-1.5 text-[11px] text-ink-4">{{ t("settings.modelCustom") }}</p>
        <div class="flex gap-1.5">
          <input
            v-model="customId"
            class="lw-field h-8 min-w-0 flex-1 px-2 font-mono text-[12px]"
            :placeholder="t('settings.modelCustomPlaceholder')"
            spellcheck="false"
            autocomplete="off"
            @input="customError = ''"
            @keydown.enter.prevent="addCustom"
          />
          <button
            type="button"
            class="lw-btn flex h-8 shrink-0 items-center gap-1 px-2.5 text-[11.5px] text-ink-1"
            :disabled="disabled"
            @click="addCustom"
          >
            <Icon name="plus" :size="12" />
            {{ t("settings.modelCustomAdd") }}
          </button>
        </div>
        <p v-if="customError" class="mt-1 text-[11px] text-danger">{{ customError }}</p>
      </div>
    </div>
  </div>
</template>
