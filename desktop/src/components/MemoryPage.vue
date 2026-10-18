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
import HoverTip from "./settings/HoverTip.vue";
import SettingsSegmented from "./settings/SettingsSegmented.vue";
import TreeBoard from "./memory/TreeBoard.vue";
import TreeSection from "./memory/TreeSection.vue";
import InjectSection from "./memory/InjectSection.vue";
import LayerSection from "./memory/LayerSection.vue";
import ObsoleteSection from "./memory/ObsoleteSection.vue";
import TypeBadge from "./TypeBadge.vue";
import { relativeTimeLabel, relativeTimeSec } from "../lib/format";
import { workspaceDisplayName } from "../lib/session-groups";
import {
  memoryClearAll,
  memoryDelete,
  memoryList,
  memoryReconcile,
  memoryRevealFolder,
  memoryUpdate,
} from "../lib/api";
import type { MemoryRow } from "../types";

const { t } = useI18n();

/**
 * 页签（队列 Q14 第三刀 · MB §6 那句「MemoryPage 改双页签（记忆树默认 / 事实库）」
 * 的兑现）。⚠ 一条需要说明的取舍：稿只点了两个页签的名字，没说**编辑面**归哪一档。
 * MB §3.2 的只读裁定写的是「编辑留在 TreeSection（现状不动，合并再议）」，所以本刀
 * 按「看板 = 只读导航、其余全在事实库档」落地——`TreeSection`（章节编辑/删除）与
 * `InjectSection`（注入解释）一起进「事实库」，一个字节的功能都没改，只换了位置。
 * 要拆成三档（看板 / 目录 / 事实库）只需要动这里一行，等真机走过一遍再定。
 */
const tab = ref<"board" | "facts">("board");
const tabs = [
  { value: "board", label: t("memory.tabBoard") },
  { value: "facts", label: t("memory.tabFacts") },
];

const search = ref("");

/**
 * 层区与回收区管的是**同一批文件**（清一层 / 恢复一条都会改变事实列表），所以两边的动作
 * 之后统一由这里重拉：列表自己拉，另外两块各暴露一个 `reload` 把手（`defineExpose`）。
 * 不用 `:key` 重建整棵子树——那会让开关区闪一下、把未消散的 hover 提示打掉。
 */
const layerSection = ref<InstanceType<typeof LayerSection> | null>(null);
const trashSection = ref<InstanceType<typeof ObsoleteSection> | null>(null);
async function onMemoryChanged() {
  await load();
  layerSection.value?.reload();
  trashSection.value?.reload();
}
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
  return relativeTimeLabel(relativeTimeSec(row.updated_at), (k, p) => t(k, p));
}

</script>

<template>
  <!-- 目的地壳的内容层是定高块，画布必须有确定高度才能量包围盒/算端点 ⇒ 根给
       `h-full`（ListPage 与各区块本来就按 overflow-hidden + 内滚设计，行为不变）。 -->
  <div class="flex h-full min-h-0 flex-1 flex-col overflow-hidden">
    <!-- 页签条（Q14 第三刀）：分段控件的唯一形态 = SettingsSegmented；栏内分区
         用间距不用发丝线（MB §3.1），所以这里不给 border-bottom。 -->
    <div class="flex shrink-0 items-center px-5 pb-2 pt-3">
      <SettingsSegmented
        v-model="tab"
        :options="tabs"
        :label="t('memory.tabLabel')"
      />
    </div>

    <!-- 看板档 = 只读导航画布（左栏 + 四列汇聚，MB §3） -->
    <TreeBoard v-if="tab === 'board'" />

    <!-- 事实库档 = 现役全部内容原样搬进来（事实列表 + 章节编辑 + 注入解释 + 清空） -->
    <template v-else>
    <!-- 层区（刀④b 三枚行为开关 + 刀④c「每层一行」= ADR-0007 §18 那一整句）。版式、写回
         与「删除该层」的二次确认都在组件里，理由写在 memory/LayerSection.vue 的头注。 -->
    <LayerSection ref="layerSection" @changed="onMemoryChanged" />
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
            <!-- 分类（刀④c · ADR-0007 §18 管理页那一列）：与来源同族同形态，值域六个档
                 由后端 `normalize_type` 收，前端只出文案。 -->
            <TypeBadge :type="row.type" />
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

    <!-- 回收区（刀④c）：已删但还没真删的那一批，逐条可恢复；「清空回收区」才是不可逆
         那一次（带"再点一次"）。落在事实列表之后、两块只读区之前——它是这一页删除动作的
         后续，不该沉到页面最底。 -->
    <ObsoleteSection ref="trashSection" @changed="onMemoryChanged" />

    <!-- M3 §3.37① 双区块之二：记忆树（情景）只读区块（TreeSection 自持
         状态；影子运行零写操作，编辑/删除属 M5） -->
    <TreeSection />

    <!-- 队列 Q10（L4）：注入解释只读区块（自持状态；读的是观测表里最近一次
         真算过四信号的回执，一个字节都不写） -->
    <InjectSection />

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
    </template>
  </div>
</template>
