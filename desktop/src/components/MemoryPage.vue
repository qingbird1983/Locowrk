<script setup lang="ts">
/**
 * 记忆页（⑪ 填充，Context and Memory.md §4.5 / UI shell doc §3.1 列表页
 * 模式）：搜索 / 列表（新→旧）/ 行内编辑 / 删除 / 清空；每条显示来源
 * 徽章（SourceBadge）与相对时间（lib/format.ts）。数据全部经 IPC 命令
 * （memory_list / update / delete / clear_all），本页不写业务规则。
 * ⑬ 增列经历记忆区（语义召回的"聊过什么、做过什么决策"）——同样必须
 * 可见可删（§11.3 信任基础），无编辑语义（自动沉淀的原文，不可改）。
 *
 * 删除 = 行内二次确认（与批量卸载同款"再点一次"模式）；清空 = 底部
 * 二次确认（§11.3：一键清空，除确认外不加任何阻力）。编辑行内展开，
 * 不开全局弹层（§5 弹层规则：弹层是阻塞决策，只留给审批）。
 */
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import ListPage from "./patterns/ListPage.vue";
import SourceBadge from "./SourceBadge.vue";
import HoverTip from "./settings/HoverTip.vue";
import TreeSection from "./memory/TreeSection.vue";
import { relativeTime } from "../lib/format";
import { workspaceDisplayName } from "../lib/session-groups";
import {
  experienceClearAll,
  experienceDelete,
  experienceList,
  memoryClearAll,
  memoryDelete,
  memoryList,
  memoryReconcile,
  memoryRevealFolder,
  memoryUpdate,
} from "../lib/api";
import type { ExperienceRow, MemoryRow } from "../types";

const { t } = useI18n();

const search = ref("");
const entries = ref<MemoryRow[] | null>(null); // null = 加载中
const loadError = ref("");

/** 行内状态：编辑中的键位 / 编辑缓冲 / 删除二次确认中的键位（㉘ 行唯一
 * 身份 = (workspace, key)，项目内同名 key 不再全局唯一）。 */
const editingKey = ref<string | null>(null);
const editValue = ref("");
const confirmingDelete = ref<string | null>(null);

function rowId(row: MemoryRow): string {
  return `${row.workspace}\u0000${row.key}`;
}
const confirmingClear = ref(false);
const saving = ref(false);

const isEmpty = computed(() => (entries.value?.length ?? 0) === 0);
const emptyText = computed(() =>
  search.value ? t("memory.noMatch") : t("memory.empty"),
);

async function load() {
  loadError.value = "";
  try {
    entries.value = await memoryList(search.value.trim() || undefined);
  } catch (e) {
    loadError.value = String(e);
    entries.value = [];
  }
}

// 搜索防抖 300ms；清空搜索词立即刷新
let timer: ReturnType<typeof setTimeout> | undefined;
watch(search, (v, old) => {
  clearTimeout(timer);
  if (!v.trim() && old?.trim()) {
    void load();
    return;
  }
  timer = setTimeout(() => void load(), 300);
});

// F2：挂载先 reconcile（人改 .md 打开本页即生效——文件为准回写索引；
// 文件层关 = 后端零计数空转），再拉列表让事实区直接反映合并结果。
onMounted(async () => {
  try {
    await memoryReconcile();
  } catch {
    // 对账失败不拦页面（列表照常展示索引现状）
  }
  void load();
  void loadExperiences();
});

/** 打开记忆人类层目录（~/.Locowrk/memory/）——人的编辑面入口。 */
function openFolder() {
  void memoryRevealFolder().catch((e) => {
    loadError.value = String(e);
  });
}

function startEdit(row: MemoryRow) {
  editingKey.value = rowId(row);
  editValue.value = row.value;
  confirmingDelete.value = null;
}

function cancelEdit() {
  editingKey.value = null;
  editValue.value = "";
}

async function saveEdit() {
  if (!editingKey.value || saving.value) return;
  saving.value = true;
  try {
    const row = entries.value?.find((r) => rowId(r) === editingKey.value);
    if (!row) return;
    await memoryUpdate(row.workspace, row.key, editValue.value);
    cancelEdit();
    await load();
  } catch (e) {
    loadError.value = String(e);
  } finally {
    saving.value = false;
  }
}

/** 删除：第一次点进入确认态，再点执行；点其他处/重选行会复位。 */
async function remove(row: MemoryRow) {
  const id = rowId(row);
  if (confirmingDelete.value !== id) {
    confirmingDelete.value = id;
    return;
  }
  confirmingDelete.value = null;
  try {
    await memoryDelete(row.workspace, row.key);
    await load();
  } catch (e) {
    loadError.value = String(e);
  }
}

async function clearAll() {
  if (!confirmingClear.value) {
    confirmingClear.value = true;
    return;
  }
  confirmingClear.value = false;
  try {
    await memoryClearAll();
    await load();
  } catch (e) {
    loadError.value = String(e);
  }
}

function timeText(row: MemoryRow): string {
  const r = relativeTime(row.updated_at);
  return r.n === undefined ? t(`memory.time.${r.key}`) : t(`memory.time.${r.key}`, { n: r.n });
}

// ---- 经历记忆区（⑬）：独立于事实记忆的列表；只删不改 ----

const experiences = ref<ExperienceRow[] | null>(null);
const confirmingExpDelete = ref<number | null>(null);
const confirmingExpClear = ref(false);

async function loadExperiences() {
  try {
    experiences.value = await experienceList();
  } catch {
    experiences.value = []; // 库缺席/不可用 = 空区块，不阻塞事实记忆页
  }
}

function expTimeText(row: ExperienceRow): string {
  const r = relativeTime(row.createdAt);
  return r.n === undefined ? t(`memory.time.${r.key}`) : t(`memory.time.${r.key}`, { n: r.n });
}

async function removeExperience(row: ExperienceRow) {
  if (confirmingExpDelete.value !== row.id) {
    confirmingExpDelete.value = row.id;
    return;
  }
  confirmingExpDelete.value = null;
  try {
    await experienceDelete(row.id);
    await loadExperiences();
  } catch (e) {
    loadError.value = String(e);
  }
}

async function clearExperiences() {
  if (!confirmingExpClear.value) {
    confirmingExpClear.value = true;
    return;
  }
  confirmingExpClear.value = false;
  try {
    await experienceClearAll();
    await loadExperiences();
  } catch (e) {
    loadError.value = String(e);
  }
}
</script>

<template>
  <div class="flex min-h-0 flex-1 flex-col overflow-hidden">
    <!-- M3 §3.37① 双区块之一：事实库区块头（在 ListPage 外，空列表也
         显示——双区块口径一致；现有列表行为零变化）。F2 v4.9：右侧挂
         「打开文件夹」——人的编辑面入口。 -->
    <div class="flex items-center border-b border-line px-5 pb-2 pt-3">
      <h3 class="text-xs font-semibold text-ink-1">
        {{ t("memory.factsTitle") }}
      </h3>
      <HoverTip
        class="ml-auto"
        :text="t('memory.openFolderHint')"
        :label="t('memory.openFolderHint')"
      >
        <button
          class="ml-auto rounded-[var(--lw-r-xs)] px-2 py-1 text-[length:var(--lw-fs-11)] text-ink-4 transition hover:bg-wash-2 hover:text-ink-1"
          @click="openFolder"
        >
          {{ t("memory.openFolder") }}
        </button>
      </HoverTip>
    </div>
    <ListPage
      v-model:search="search"
      :is-empty="isEmpty"
      :empty-text="emptyText"
      :search-placeholder="t('memory.search')"
    >
      <p
        v-if="loadError"
        class="mb-2 rounded-[var(--lw-r-xs)] bg-danger/10 px-3 py-2 text-xs text-danger"
      >
        {{ loadError }}
      </p>

      <ul class="space-y-2">
        <li
          v-for="row in entries"
          :key="rowId(row)"
          class="rounded-[var(--lw-r-md)] bg-wash-1 px-4 py-3"
        >
          <!-- 头行：key + 来源徽章 + 时间 + 动作 -->
          <div class="flex items-center gap-2">
            <span class="min-w-0 flex-1 truncate font-mono text-xs text-ink-1">
              {{ row.key }}
            </span>
            <SourceBadge :source="row.source" />
            <!-- ㉘·A·fix 双层记忆：'' = 全局偏好层，显式标层 -->
            <HoverTip
              v-if="row.workspace"
              class="shrink-0"
              :text="row.workspace"
              :label="row.workspace"
            >
              <span class="shrink-0 text-[length:var(--lw-fs-10)] text-ink-5">
                {{ t("memory.workspace") }} {{ workspaceDisplayName(row.workspace) }}
              </span>
            </HoverTip>
            <span v-else class="shrink-0 text-[length:var(--lw-fs-10)] text-ink-5">
              {{ t("memory.globalTier") }}
            </span>
            <span class="shrink-0 text-[length:var(--lw-fs-10)] text-ink-5">
              {{ timeText(row) }}
            </span>
            <template v-if="editingKey !== rowId(row)">
              <button
                class="shrink-0 rounded-[var(--lw-r-xs)] px-2 py-1 text-[length:var(--lw-fs-11)] text-ink-4 transition hover:bg-wash-2 hover:text-ink-1"
                @click="startEdit(row)"
              >
                {{ t("memory.edit") }}
              </button>
              <button
                class="shrink-0 rounded-[var(--lw-r-xs)] px-2 py-1 text-[length:var(--lw-fs-11)] transition"
                :class="
                  confirmingDelete === rowId(row)
                    ? 'bg-danger/16 text-danger'
                    : 'text-ink-4 hover:bg-wash-2 hover:text-danger'
                "
                @click="remove(row)"
              >
                {{
                  confirmingDelete === rowId(row)
                    ? t("memory.deleteConfirm")
                    : t("memory.delete")
                }}
              </button>
            </template>
          </div>

          <!-- 编辑态：value 文本域 + 保存/取消 -->
          <template v-if="editingKey === rowId(row)">
            <textarea
              v-model="editValue"
              rows="3"
              class="mt-2 block w-full rounded-[var(--lw-r-xs)] bg-wash-2 px-2 py-1.5 text-xs text-ink-1 outline-none"
            />
            <div class="mt-2 flex items-center gap-2">
              <button
                class="rounded-[var(--lw-r-xs)] bg-accent px-3 py-1 text-[length:var(--lw-fs-11)] font-semibold text-accent-ink transition hover:bg-accent-hover disabled:cursor-not-allowed disabled:opacity-40"
                :disabled="saving"
                @click="saveEdit"
              >
                {{ t("memory.save") }}
              </button>
              <button
                class="rounded-[var(--lw-r-xs)] px-3 py-1 text-[length:var(--lw-fs-11)] text-ink-4 transition hover:bg-wash-2"
                @click="cancelEdit"
              >
                {{ t("memory.cancel") }}
              </button>
            </div>
          </template>

          <!-- 只读态：value 全文（title 兜底超长）+ 标签 -->
          <template v-else>
            <p class="mt-1 whitespace-pre-wrap break-words text-xs text-ink-3">
              {{ row.value }}
            </p>
            <p v-if="row.tags" class="mt-1 text-[length:var(--lw-fs-10)] text-ink-5">
              {{ row.tags }}
            </p>
          </template>
        </li>
      </ul>
    </ListPage>

    <!-- M3 §3.37① 双区块之二：记忆树（情景）只读区块（TreeSection 自持
         状态；影子运行零写操作，编辑/删除属 M5） -->
    <TreeSection />

    <!-- ⑬ 经历记忆：语义召回"聊过什么、做过什么决策"；必须可见可删
         （§11.3 信任基础），无编辑语义（自动沉淀原文） -->
    <div class="border-t border-line px-5 py-3">
      <div class="flex items-center gap-2">
        <h3 class="text-xs font-semibold text-ink-1">
          {{ t("memory.experienceTitle") }}
        </h3>
        <span class="text-[length:var(--lw-fs-10)] text-ink-5">
          {{ experiences?.length ?? 0 }}
        </span>
        <button
          v-if="(experiences?.length ?? 0) > 0"
          class="ml-auto rounded-[var(--lw-r-xs)] px-3 py-1 text-[length:var(--lw-fs-11)] transition"
          :class="
            confirmingExpClear
              ? 'bg-danger/16 text-danger'
              : 'text-ink-4 hover:bg-wash-2 hover:text-danger'
          "
          @click="clearExperiences"
        >
          {{ confirmingExpClear ? t("memory.clearConfirm") : t("memory.experienceClear") }}
        </button>
      </div>

      <p v-if="(experiences?.length ?? 0) === 0" class="mt-2 text-[length:var(--lw-fs-11)] text-ink-5">
        {{ t("memory.experienceEmpty") }}
      </p>
      <ul v-else class="mt-2 space-y-2">
        <li
          v-for="row in experiences"
          :key="row.id"
          class="rounded-[var(--lw-r-md)] bg-wash-1 px-4 py-3"
        >
          <div class="flex items-center gap-2">
            <span class="shrink-0 text-[length:var(--lw-fs-10)] text-ink-5">
              {{ expTimeText(row) }}
            </span>
            <span class="truncate font-mono text-[length:var(--lw-fs-10)] text-ink-5">
              {{ t("memory.experienceSession", { id: row.sessionId.slice(0, 8) }) }}
            </span>
            <HoverTip
              v-if="row.workspace"
              class="shrink-0"
              :text="row.workspace"
              :label="row.workspace"
            >
              <span class="shrink-0 text-[length:var(--lw-fs-10)] text-ink-5">
                {{ t("memory.workspace") }} {{ workspaceDisplayName(row.workspace) }}
              </span>
            </HoverTip>
            <button
              class="ml-auto shrink-0 rounded-[var(--lw-r-xs)] px-2 py-1 text-[length:var(--lw-fs-11)] transition"
              :class="
                confirmingExpDelete === row.id
                  ? 'bg-danger/16 text-danger'
                  : 'text-ink-4 hover:bg-wash-2 hover:text-danger'
              "
              @click="removeExperience(row)"
            >
              {{
                confirmingExpDelete === row.id
                  ? t("memory.deleteConfirm")
                  : t("memory.delete")
              }}
            </button>
          </div>
          <p class="mt-1 whitespace-pre-wrap break-words text-xs text-ink-3">
            {{ row.content }}
          </p>
        </li>
      </ul>
    </div>

    <!-- 底部：清空（二次确认）+ 提示行 -->
    <div class="flex items-center gap-2 border-t border-line px-5 py-2">
      <button
        v-if="!isEmpty"
        class="rounded-[var(--lw-r-xs)] px-3 py-1 text-[length:var(--lw-fs-11)] transition"
        :class="
          confirmingClear
            ? 'bg-danger/16 text-danger'
            : 'text-ink-4 hover:bg-wash-2 hover:text-danger'
        "
        @click="clearAll"
      >
        {{ confirmingClear ? t("memory.clearConfirm") : t("memory.clearAll") }}
      </button>
      <p class="ml-auto text-[length:var(--lw-fs-11)] text-ink-5">{{ t("memory.hint") }}</p>
    </div>
  </div>
</template>
