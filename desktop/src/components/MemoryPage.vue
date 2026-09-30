<script setup lang="ts">
/**
 * 记忆页（⑪ 填充，Context and Memory.md §4.5 / UI shell doc §3.1 列表页
 * 模式）：搜索 / 列表（新→旧）/ 行内编辑 / 删除 / 清空；每条显示来源
 * 徽章（SourceBadge）与相对时间（lib/format.ts）。数据全部经 IPC 命令
 * （memory_list / update / delete / clear_all），本页不写业务规则。
 *
 * 删除 = 行内二次确认（与批量卸载同款"再点一次"模式）；清空 = 底部
 * 二次确认（§11.3：一键清空，除确认外不加任何阻力）。编辑行内展开，
 * 不开全局弹层（§5 弹层规则：弹层是阻塞决策，只留给审批）。
 */
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import ListPage from "./patterns/ListPage.vue";
import SourceBadge from "./SourceBadge.vue";
import { relativeTime } from "../lib/format";
import { memoryClearAll, memoryDelete, memoryList, memoryUpdate } from "../lib/api";
import type { MemoryRow } from "../types";

const { t } = useI18n();

const search = ref("");
const entries = ref<MemoryRow[] | null>(null); // null = 加载中
const loadError = ref("");

/** 行内状态：编辑中的键 / 编辑缓冲 / 删除二次确认中的键。 */
const editingKey = ref<string | null>(null);
const editValue = ref("");
const confirmingDelete = ref<string | null>(null);
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

onMounted(load);

function startEdit(row: MemoryRow) {
  editingKey.value = row.key;
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
    await memoryUpdate(editingKey.value, editValue.value);
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
  if (confirmingDelete.value !== row.key) {
    confirmingDelete.value = row.key;
    return;
  }
  confirmingDelete.value = null;
  try {
    await memoryDelete(row.key);
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
</script>

<template>
  <div class="flex min-h-0 flex-1 flex-col overflow-hidden">
    <ListPage
      v-model:search="search"
      :is-empty="isEmpty"
      :empty-text="emptyText"
      :search-placeholder="t('memory.search')"
    >
      <p
        v-if="loadError"
        class="mb-2 rounded-lg border border-red-900/60 bg-red-950/60 px-3 py-2 text-xs text-red-300"
      >
        {{ loadError }}
      </p>

      <ul class="space-y-2">
        <li
          v-for="row in entries"
          :key="row.key"
          class="rounded-xl border border-[var(--lw-border)] bg-[var(--lw-surface-1)]/40 px-4 py-3"
        >
          <!-- 头行：key + 来源徽章 + 时间 + 动作 -->
          <div class="flex items-center gap-2">
            <span class="min-w-0 flex-1 truncate font-mono text-xs text-[var(--lw-text-1)]">
              {{ row.key }}
            </span>
            <SourceBadge :source="row.source" />
            <span class="shrink-0 text-[10px] text-[var(--lw-text-4)]">
              {{ timeText(row) }}
            </span>
            <template v-if="editingKey !== row.key">
              <button
                class="shrink-0 rounded-lg px-2 py-1 text-[11px] text-[var(--lw-text-3)] transition hover:bg-[var(--lw-surface-2)] hover:text-[var(--lw-text-1)]"
                @click="startEdit(row)"
              >
                {{ t("memory.edit") }}
              </button>
              <button
                class="shrink-0 rounded-lg px-2 py-1 text-[11px] transition"
                :class="
                  confirmingDelete === row.key
                    ? 'bg-red-950/80 text-red-300'
                    : 'text-[var(--lw-text-3)] hover:bg-[var(--lw-surface-2)] hover:text-red-300'
                "
                @click="remove(row)"
              >
                {{
                  confirmingDelete === row.key ? t("memory.deleteConfirm") : t("memory.delete")
                }}
              </button>
            </template>
          </div>

          <!-- 编辑态：value 文本域 + 保存/取消 -->
          <template v-if="editingKey === row.key">
            <textarea
              v-model="editValue"
              rows="3"
              class="mt-2 block w-full rounded-lg border border-[var(--lw-border-strong)] bg-[var(--lw-surface-1)] px-2 py-1.5 text-xs text-[var(--lw-text-1)] outline-none focus:border-[var(--lw-text-3)]"
            />
            <div class="mt-2 flex items-center gap-2">
              <button
                class="rounded-lg bg-emerald-800/80 px-3 py-1 text-[11px] font-semibold text-emerald-200 transition hover:bg-emerald-700/80 disabled:cursor-not-allowed disabled:opacity-40"
                :disabled="saving"
                @click="saveEdit"
              >
                {{ t("memory.save") }}
              </button>
              <button
                class="rounded-lg px-3 py-1 text-[11px] text-[var(--lw-text-3)] transition hover:bg-[var(--lw-surface-2)]"
                @click="cancelEdit"
              >
                {{ t("memory.cancel") }}
              </button>
            </div>
          </template>

          <!-- 只读态：value 全文（title 兜底超长）+ 标签 -->
          <template v-else>
            <p class="mt-1 whitespace-pre-wrap break-words text-xs text-[var(--lw-text-2)]">
              {{ row.value }}
            </p>
            <p v-if="row.tags" class="mt-1 text-[10px] text-[var(--lw-text-4)]">
              {{ row.tags }}
            </p>
          </template>
        </li>
      </ul>
    </ListPage>

    <!-- 底部：清空（二次确认）+ 提示行 -->
    <div class="flex items-center gap-2 border-t border-[var(--lw-border)] px-5 py-2">
      <button
        v-if="!isEmpty"
        class="rounded-lg px-3 py-1 text-[11px] transition"
        :class="
          confirmingClear
            ? 'bg-red-950/80 text-red-300'
            : 'text-[var(--lw-text-3)] hover:bg-[var(--lw-surface-2)] hover:text-red-300'
        "
        @click="clearAll"
      >
        {{ confirmingClear ? t("memory.clearConfirm") : t("memory.clearAll") }}
      </button>
      <p class="ml-auto text-[11px] text-[var(--lw-text-4)]">{{ t("memory.hint") }}</p>
    </div>
  </div>
</template>
