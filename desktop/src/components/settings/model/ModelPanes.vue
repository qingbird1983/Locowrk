<script setup lang="ts">
/**
 * 模型双栏选择（UI shell 文档 §3.30 添加/编辑服务模态，参考 PI-Desktop
 * ModelSelectionPanes）：左「该服务的模型」= 全选框（半态）+ 获取列表 +
 * 搜索 + 复选行（ID mono 可选中复制 + 显示名 + 右缘窗口徽章，取自静态
 * 档案，缺省空白不发明）；右「模型设置 N」= 搜索 + 拖拽行（把手 + ID +
 * 别名 + ctx·max 徽章 + 视觉徽章 + 高级折叠 + ×）+ 高级展开（别名 + 图像
 * 输入钉死 + 只读推理梯子 + 价格行，均带「跟随 models.dev，编辑即钉死」
 * 提示）+ 自定义模型添加行（重 ID 报已添加；添加即查目录就地升级——批 A）。
 * 目录合并：sources 为 null 的行（客户端新建）自动查目录补齐，未命中落定
 * 全未知形态防回环。
 * S4 刀④：上下文窗口 / 最大输出的编辑入口移入设置 → 开发者（普通用户
 * 不填数值）；本弹窗只保留只读徽章。探测/防抖在父组件
 * （ProviderSetupDialog），这里只摆清单与选中态。
 */
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { llmLookupModel, llmTestModel } from "../../../lib/api";
import { Icon } from "../../../lib/icons";
import {
  catalogHitToRow,
  emptyModelRow,
  formatPricingShort,
  hasImageInput,
  markRowUnknown,
} from "../../../lib/model-catalog";
import { formatLimitShort } from "../../../lib/provider-presets";
import { profileWindow } from "../../../lib/model-profiles";
import type { ModelTestResult, ProviderModelView } from "../../../types";
import HoverTip from "../HoverTip.vue";

const props = defineProps<{
  /** 探测到的可用模型 id（空 = 未探测/无清单）。 */
  available: string[];
  /** 探测状态：idle 未探测 / loading / ready / error。 */
  probeState: "idle" | "loading" | "ready" | "error";
  /** 已选清单（右栏真源，父组件持久）。 */
  chosen: ProviderModelView[];
  /** 表单 kind + baseUrl（目录匹配的供应商定位输入）。 */
  kind: string;
  baseUrl: string;
  /** 档位 id（逐模型连通测试 §8.3 需已存档位；添加流 = null 不显钮）。 */
  profileId: string | null;
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
/** 在途目录合并（防重复请求 + 合并回环）。 */
const mergingIds = new Set<string>();

// ---- 逐模型连通测试（§8.3）：行头「测试」钮 + 结果徽章（五分类） ----
const testingId = ref<string | null>(null);
const testResults = ref<Record<string, ModelTestResult>>({});
const TEST_CAT_KEYS: Record<string, string> = {
  auth: "settings.testCatAuth",
  model_not_found: "settings.testCatModelNotFound",
  rate_limit: "settings.testCatRateLimit",
  network: "settings.testCatNetwork",
  server: "settings.testCatServer",
  unknown: "settings.testCatUnknown",
};

async function testModel(id: string): Promise<void> {
  if (!props.profileId || testingId.value) return;
  testingId.value = id;
  try {
    testResults.value[id] = await llmTestModel(props.profileId, id);
  } catch (e) {
    // 命令层错误（模型名为空/档位缺失等）按 unknown 如实呈现
    testResults.value[id] = {
      ok: false,
      category: "unknown",
      message: String(e),
      latencyMs: null,
    };
  } finally {
    testingId.value = null;
  }
}

const testBadge = (id: string): { text: string; cls: string; title: string } | null => {
  const r = testResults.value[id];
  if (!r) return null;
  if (r.ok) return { text: `✓ ${r.latencyMs ?? "?"}ms`, cls: "text-ok", title: "" };
  return {
    text: TEST_CAT_KEYS[r.category ?? "unknown"] ? t(TEST_CAT_KEYS[r.category ?? "unknown"]) : t("settings.testCatUnknown"),
    cls: "text-danger",
    title: r.message ?? "",
  };
};

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
/** 右栏行 ctx·max 徽章（目录合成值直读，钉死/目录一视同仁）。 */
const limitBadge = (m: ProviderModelView): string => {
  const parts: string[] = [];
  const ctx = formatLimitShort(m.contextWindow ?? profileWindow(m.id));
  if (ctx) parts.push(ctx);
  const max = formatLimitShort(m.maxOutput);
  if (max) parts.push(max);
  return parts.join(" · ");
};

/** 目录合并：sources 为 null（客户端新建）的行查目录就地升级。 */
async function mergeCatalog(ids: string[]) {
  ids.forEach((id) => mergingIds.add(id.toLowerCase()));
  try {
    const hits = await Promise.all(
      ids.map((id) =>
        llmLookupModel(props.kind, props.baseUrl || null, id).catch(() => null),
      ),
    );
    const byId = new Map(ids.map((id, i) => [id.toLowerCase(), hits[i]]));
    emit(
      "update",
      props.chosen.map((m) => {
        const hit = byId.get(m.id.toLowerCase());
        if (hit === undefined) return m;
        return hit ? catalogHitToRow(m, hit) : markRowUnknown(m);
      }),
    );
  } finally {
    ids.forEach((id) => mergingIds.delete(id.toLowerCase()));
  }
}

watch(
  () => props.chosen,
  (rows) => {
    const need = rows
      .filter((m) => m.sources === null && !mergingIds.has(m.id.toLowerCase()))
      .map((m) => m.id);
    if (need.length > 0) void mergeCatalog(need);
  },
  { immediate: true },
);

function toggle(id: string) {
  const key = id.toLowerCase();
  if (selectedSet.value.has(key)) {
    emit("update", props.chosen.filter((m) => m.id.toLowerCase() !== key));
    return;
  }
  emit("update", [...props.chosen, emptyModelRow(id)]);
}

function toggleVisible(select: boolean) {
  if (!select) {
    const keys = new Set(availRows.value.map((i) => i.toLowerCase()));
    emit("update", props.chosen.filter((m) => !keys.has(m.id.toLowerCase())));
    return;
  }
  const additions = availRows.value
    .filter((i) => !selectedSet.value.has(i.toLowerCase()))
    .map((id) => emptyModelRow(id));
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

/** 钉死编辑：写值的同时把对应来源标成 user（编辑即钉死，供应商稿 §4.1）。 */
function pinEdit(id: string, patch: Partial<ProviderModelView>) {
  const src: Partial<ProviderModelView["sources"]> = {};
  if ("contextWindow" in patch) src.window = "user";
  if ("maxOutput" in patch) src.maxOutput = "user";
  if ("supportsImage" in patch) src.modalities = "user";
  const first = props.chosen.find((m) => m.id === id);
  updateChosen(id, {
    ...patch,
    sources: { ...(first?.sources ?? { window: null, maxOutput: null, modalities: null, reasoning: null }), ...src },
  } as Partial<ProviderModelView>);
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

/** 手工 id 落右栏即刻展开（PI 同款）+ 查目录就地升级（批 A）；重 id 报已添加。 */
function addCustom() {
  const id = customId.value.trim();
  if (!id) return;
  if (props.chosen.some((m) => m.id.toLowerCase() === id.toLowerCase())) {
    customError.value = t("settings.modelAlreadyAdded");
    return;
  }
  const known = props.available.find((i) => i.toLowerCase() === id.toLowerCase());
  const row = emptyModelRow(known ?? id);
  // 静态档案窗口预填（v2.58 行为保留）：只写值、不标来源——sources 保持
  // null 让目录合并接管（走查实抓：标 user 会挡住合并，梯子/价格/模态全
  // 缺席）；目录命中以目录值为准（比静态表准），未命中预填值存盘即钉死。
  const win = profileWindow(known ?? id);
  if (win) row.contextWindow = win;
  emit("update", [...props.chosen, row]);
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
        <h4 class="text-[length:var(--lw-fs-12half)] font-semibold text-ink-1">{{ t("settings.modelAvailable") }}</h4>
        <button
          type="button"
          class="ml-auto flex items-center gap-1 rounded-[var(--lw-r-xs)] px-2 py-1 text-[length:var(--lw-fs-12)] text-ink-2 transition hover:bg-wash-2 disabled:opacity-50"
          :disabled="disabled || probeState === 'loading'"
          @click="emit('fetch')"
        >
          <Icon name="refresh" :size="14" :class="probeState === 'loading' ? 'animate-none' : ''" />
          {{ probeState === "loading" ? t("settings.modelProbing") : t("settings.modelFetchList") }}
        </button>
      </div>
      <div class="mt-2 flex h-8 items-center gap-2 rounded-[var(--lw-r-xs)] bg-wash-2 px-2.5">
        <Icon name="search" :size="14" class="shrink-0 text-ink-4" />
        <input
          v-model="availQuery"
          class="w-full bg-transparent text-[length:var(--lw-fs-12half)] text-ink-1 outline-none placeholder:text-ink-5"
          :placeholder="t('settings.modelSearchId')"
          spellcheck="false"
          autocomplete="off"
        />
      </div>
      <div class="mt-2 min-h-0 flex-1 overflow-y-auto">
        <p v-if="probeState === 'idle'" class="px-1 py-4 text-[length:var(--lw-fs-12)] leading-relaxed text-ink-4">
          {{ t("settings.modelAvailIdle") }}
        </p>
        <p v-else-if="probeState === 'error' && props.available.length === 0"
           class="px-1 py-4 text-[length:var(--lw-fs-12)] text-danger">
          {{ t("settings.modelAvailError") }}
        </p>
        <p v-else-if="availRows.length === 0" class="px-1 py-4 text-[length:var(--lw-fs-12)] text-ink-4">
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
              <span class="min-w-0 flex-1 select-text truncate font-mono text-[length:var(--lw-fs-12)] text-ink-1">{{ id }}</span>
              <span v-if="displayName(id)" class="shrink-0 text-[length:var(--lw-fs-11)] text-ink-4">{{ displayName(id) }}</span>
              <span v-if="windowBadge(id)" class="shrink-0 text-[length:var(--lw-fs-11)] tabular-nums text-ink-4">
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
        <h4 class="text-[length:var(--lw-fs-12half)] font-semibold text-ink-1">{{ t("settings.modelChosen") }}</h4>
        <span v-if="chosen.length" class="rounded-full bg-wash-2 px-1.5 text-[length:var(--lw-fs-11)] tabular-nums text-ink-3">
          {{ chosen.length }}
        </span>
        <div v-if="chosen.length" class="ml-auto flex h-7 w-36 items-center gap-1.5 rounded-[var(--lw-r-xs)] bg-wash-2 px-2">
          <Icon name="search" :size="12" class="shrink-0 text-ink-4" />
          <input
            v-model="chosenQuery"
            class="w-full bg-transparent text-[length:var(--lw-fs-12)] text-ink-1 outline-none placeholder:text-ink-5"
            :placeholder="t('settings.modelSearchChosen')"
            spellcheck="false"
            autocomplete="off"
          />
        </div>
      </div>
      <div class="mt-2 min-h-0 flex-1 overflow-y-auto">
        <p v-if="chosen.length === 0" class="px-1 py-4 text-[length:var(--lw-fs-12)] text-ink-4">
          {{ t("settings.modelChosenEmpty") }}
        </p>
        <p v-else-if="chosenRows.length === 0" class="px-1 py-4 text-[length:var(--lw-fs-12)] text-ink-4">
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
                <Icon name="grip" :size="14" />
              </button>
              <span class="min-w-0 flex-1 select-text truncate font-mono text-[length:var(--lw-fs-12)] text-ink-1">{{ m.id }}</span>
              <span v-if="m.alias?.trim()" class="shrink-0 text-[length:var(--lw-fs-11)] text-ink-4">{{ m.alias }}</span>
              <span v-if="hasImageInput(m)" class="shrink-0 text-ink-4" :title="t('settings.modelVisionBadge')">
                <Icon name="eye" :size="12" />
              </span>
              <span class="shrink-0 text-[length:var(--lw-fs-11)] tabular-nums text-ink-4">
                {{ limitBadge(m) }}
              </span>
              <!-- 逐模型连通测试（§8.3）：徽章常驻最近一次结果，失败 title 带报文 -->
              <span
                v-if="testBadge(m.id)"
                class="shrink-0 text-[length:var(--lw-fs-11)] tabular-nums"
                :class="testBadge(m.id)!.cls"
                :title="testBadge(m.id)!.title"
              >{{ testBadge(m.id)!.text }}</span>
              <button
                v-if="props.profileId"
                type="button"
                class="rounded-[var(--lw-r-3xs)] px-1.5 py-0.5 text-[length:var(--lw-fs-11)] text-ink-3 hover:bg-wash-3 hover:text-ink-1 disabled:opacity-50"
                :disabled="testingId !== null || disabled"
                @click="testModel(m.id)"
              >
                {{ testingId === m.id ? t("settings.modelTestingShort") : t("settings.modelTestBtn") }}
              </button>
              <button
                type="button"
                class="rounded-[var(--lw-r-3xs)] px-1.5 py-0.5 text-[length:var(--lw-fs-11)] text-ink-3 hover:bg-wash-3 hover:text-ink-1"
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
                <span class="w-16 shrink-0 text-[length:var(--lw-fs-11)] text-ink-4">{{ t("settings.modelAlias") }}</span>
                <input
                  :value="m.alias ?? ''"
                  class="lw-field h-7 w-full flex-1 px-2 text-[length:var(--lw-fs-12)]"
                  spellcheck="false"
                  :placeholder="t('settings.modelAliasPlaceholder')"
                  @input="updateChosen(m.id, { alias: ($event.target as HTMLInputElement).value || null })"
                />
              </label>
              <!-- 上下文窗口 / 最大输出：S4 刀④ 数值归类后不在这里编辑
                   （普通用户不填数值）；覆盖入口在设置 → 开发者 → 模型参数。
                   行头 ctx·max 徽章仍只读展示有效值。 -->
              <!-- 图像输入（tri-state 钉死：None 随目录 / 纯文本缺省） -->
              <label class="flex items-center gap-2">
                <span class="flex w-16 shrink-0 items-center gap-0.5 text-[length:var(--lw-fs-11)] text-ink-4">
                  {{ t("settings.modelImageInput") }}
                  <HoverTip
                    v-if="m.sources?.modalities === 'catalog'"
                    class="text-ink-5"
                    :text="t('settings.modelFollowCatalog')"
                    :label="t('settings.modelFollowCatalog')"
                  >
                    <Icon name="info" :size="12" />
                  </HoverTip>
                </span>
                <input
                  type="checkbox"
                  class="h-3.5 w-3.5 accent-[var(--lw-accent)]"
                  :checked="hasImageInput(m)"
                  :disabled="disabled"
                  @change="
                    pinEdit(m.id, {
                      supportsImage: ($event.target as HTMLInputElement).checked,
                    })
                  "
                />
                <span v-if="m.sources?.modalities === null" class="text-[length:var(--lw-fs-11)] text-ink-5">
                  {{ t("settings.modelModalityUnknown") }}
                </span>
              </label>
              <!-- 可用于子代理（批 F）：标记 = 进子代理模型池（钉模型选择器与终端错误回退链的共同数据源） -->
              <label class="flex items-center gap-2">
                <span class="flex w-16 shrink-0 items-center gap-0.5 text-[length:var(--lw-fs-11)] text-ink-4">
                  {{ t("settings.modelSubagentRow") }}
                  <HoverTip
                    class="text-ink-5"
                    :text="t('settings.modelSubagentTip')"
                    :label="t('settings.modelSubagentTip')"
                  >
                    <Icon name="info" :size="12" />
                  </HoverTip>
                </span>
                <input
                  type="checkbox"
                  class="h-3.5 w-3.5 accent-[var(--lw-accent)]"
                  :checked="m.availableForSubagents === true"
                  :disabled="disabled"
                  @change="
                    updateChosen(m.id, {
                      availableForSubagents: ($event.target as HTMLInputElement).checked,
                    })
                  "
                />
              </label>
              <!-- 推理梯子（只读；预设方言接线随批 B） -->
              <div v-if="m.reasoningLadder.length > 0" class="flex items-start gap-2">
                <span class="w-16 shrink-0 pt-1 text-[length:var(--lw-fs-11)] text-ink-4">{{ t("settings.modelLadder") }}</span>
                <div class="flex min-w-0 flex-1 flex-wrap gap-1">
                  <span
                    v-for="lvl in m.reasoningLadder"
                    :key="lvl"
                    class="rounded-full bg-wash-3 px-2 py-0.5 font-mono text-[length:var(--lw-fs-11)] text-ink-3"
                  >{{ lvl }}</span>
                </div>
              </div>
              <!-- 价格（models.dev USD 原值如实，只作参考不进计算） -->
              <div v-if="formatPricingShort(m.pricing)" class="flex items-center gap-2">
                <span class="w-16 shrink-0 text-[length:var(--lw-fs-11)] text-ink-4">{{ t("settings.modelPricing") }}</span>
                <span class="text-[length:var(--lw-fs-11)] tabular-nums text-ink-3">
                  {{ formatPricingShort(m.pricing) }}
                  <span class="text-ink-5">/ {{ t("settings.modelPricingUnit") }}</span>
                </span>
              </div>
            </div>
          </li>
        </ul>
      </div>
      <div class="mt-2 border-t border-line-subtle pt-2">
        <p class="mb-1.5 text-[length:var(--lw-fs-11)] text-ink-4">{{ t("settings.modelCustom") }}</p>
        <div class="flex gap-1.5">
          <input
            v-model="customId"
            class="lw-field h-8 min-w-0 flex-1 px-2 font-mono text-[length:var(--lw-fs-12)]"
            :placeholder="t('settings.modelCustomPlaceholder')"
            spellcheck="false"
            autocomplete="off"
            @input="customError = ''"
            @keydown.enter.prevent="addCustom"
          />
          <button
            type="button"
            class="lw-btn flex h-8 shrink-0 items-center gap-1 px-2.5 text-[length:var(--lw-fs-12)] text-ink-1"
            :disabled="disabled"
            @click="addCustom"
          >
            <Icon name="plus" :size="12" />
            {{ t("settings.modelCustomAdd") }}
          </button>
        </div>
        <p v-if="customError" class="mt-1 text-[length:var(--lw-fs-11)] text-danger">{{ customError }}</p>
      </div>
    </div>
  </div>
</template>
