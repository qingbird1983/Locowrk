<script setup lang="ts">
/**
 * 记忆树（情景）区块（M3 §3.37① + M5 编辑面）：会话切换器 + 章节列表
 * （展开看 L1 摘要 / 看原文）+ 内联编辑标题/摘要 + 两步删除捆 + 本地搜索。
 * 编辑只动投影（树是投影，原文一个字不改；纪律 5：用户编辑打「手动」
 * 标记，自动层此后永不覆盖）；看原文与记录员材料同形同源。
 * 自持状态不牵动事实区；打开拉取不轮询（⑫/§3.36③ 同款）。
 */
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import {
  memoryTreeChapters,
  memoryTreeDeleteChapter,
  memoryTreeEditChapter,
  memoryTreeOriginal,
  memoryTreeSessions,
} from "../../lib/api";
import { relativeTime } from "../../lib/format";
import { workspaceDisplayName } from "../../lib/session-groups";
import { useChatStore } from "../../stores/chat";
import ProjectTree from "./ProjectTree.vue";
import type {
  TreeChapterView,
  TreeChaptersView,
  TreeOriginalItem,
  TreeSummaryView,
} from "../../types";

const { t } = useI18n();
const chat = useChatStore();

const sessions = ref<TreeSummaryView[] | null>(null); // null = 加载中
const selected = ref(""); // `${workspace}\u0000${sessionId}`
const chapters = ref<TreeChaptersView | null>(null);
const loadError = ref("");
const search = ref("");
/** 视图档位（M6.4）：会话（默认，可编辑）| 项目（跨会话联合只读）。 */
const viewMode = ref<"session" | "project">("session");

/** 展开态（本地保持，切换会话重置；§3.28 只增不收回惯例的反面——
 * 换会话是换数据源，重置才诚实）。 */
const expanded = ref<Set<string>>(new Set());
/** 原文缓存：chapterId → 行（或错误文案）；只增不收回。 */
const originals = ref<Map<string, TreeOriginalItem[]>>(new Map());
const originalErrors = ref<Map<string, string>>(new Map());

/** 内联编辑态（M5）：{章节, 字段, 草稿}；null = 未在编辑。 */
const editing = ref<{ id: string; field: "title" | "summary"; value: string } | null>(null);
const editError = ref("");
/** 两步删除确认（§3.6 行菜单同惯例）：记章节 id，再点同钮才真删。 */
const confirmingDelete = ref("");

function keyOf(s: TreeSummaryView): string {
  return `${s.workspace}\u0000${s.sessionId}`;
}

/** 当前选中会话的项目归属（M6.4 项目档数据源键位）。 */
const currentWorkspace = computed(
  () =>
    sessions.value?.find((x) => keyOf(x) === selected.value)?.workspace ?? "",
);

/** 项目档点捆 → 切回会话档并展开该捆（编辑/删除在会话档单点复用）。 */
async function openInSession(sessionId: string, chapterId: string) {
  const target = sessions.value?.find((x) => x.sessionId === sessionId);
  if (target) {
    viewMode.value = "session";
    selected.value = keyOf(target);
    await loadBundles();
  }
  expanded.value = new Set([chapterId]);
  const b = chapters.value?.chapters.find((c) => c.chapterId === chapterId);
  if (b) void loadOriginal(b);
}

/** 会话标题（侧栏同源；树在、会话已删 = 回退 id 前缀）。 */
/** 选择器行：标题 · 项目 · N 章节（未归属跳过项目段，不留空段）。 */
function optionText(s: TreeSummaryView): string {
  const parts = [titleOf(s)];
  const ws = workspaceDisplayName(s.workspace);
  if (ws) parts.push(ws);
  parts.push(t("memory.treeSessionCount", { n: s.chapterCount }));
  return parts.join(" · ");
}

function titleOf(s: TreeSummaryView): string {
  const meta = chat.sessions.find((x) => x.id === s.sessionId);
  return meta?.title ?? s.sessionId.slice(0, 8);
}

const filtered = computed(() => {
  const q = search.value.trim().toLowerCase();
  const list = chapters.value?.chapters ?? [];
  if (!q) return list;
  return list.filter(
    (b) =>
      b.title.toLowerCase().includes(q) || b.summary.toLowerCase().includes(q),
  );
});

async function loadSessions() {
  loadError.value = "";
  try {
    sessions.value = await memoryTreeSessions();
    const list = sessions.value;
    if (list.length === 0) {
      chapters.value = null;
      return;
    }
    // 缺省选中 = 当前会话（有树时），否则列表首项（§3.37①）
    const cur = list.find((s) => s.sessionId === chat.sessionId);
    selected.value = keyOf(cur ?? list[0]);
  } catch (e) {
    loadError.value = String(e);
    sessions.value = [];
  }
}

async function loadBundles() {
  const s = sessions.value?.find((x) => keyOf(x) === selected.value);
  if (!s) return;
  loadError.value = "";
  editing.value = null;
  editError.value = "";
  confirmingDelete.value = "";
  expanded.value = new Set();
  originals.value = new Map();
  originalErrors.value = new Map();
  try {
    chapters.value = await memoryTreeChapters(s.sessionId, s.workspace);
  } catch (e) {
    loadError.value = String(e);
    chapters.value = null;
  }
}

watch(selected, loadBundles);

onMounted(loadSessions);

function toggleExpand(b: TreeChapterView) {
  const next = new Set(expanded.value);
  if (next.has(b.chapterId)) {
    next.delete(b.chapterId);
  } else {
    next.add(b.chapterId);
    void loadOriginal(b);
  }
  expanded.value = next;
}

async function loadOriginal(b: TreeChapterView) {
  if (originals.value.has(b.chapterId) || originalErrors.value.has(b.chapterId))
    return;
  const s = sessions.value?.find((x) => keyOf(x) === selected.value);
  if (!s) return;
  try {
    const rows = await memoryTreeOriginal(s.sessionId, s.workspace, b.chapterId);
    originals.value.set(b.chapterId, rows);
  } catch (e) {
    originalErrors.value.set(b.chapterId, String(e));
  }
}

// ---- 编辑面（M5）：内联编辑 + 两步删除；只动投影，成功后就地更新 ----

function startEdit(b: TreeChapterView, field: "title" | "summary") {
  editError.value = "";
  confirmingDelete.value = "";
  editing.value = {
    id: b.chapterId,
    field,
    value: field === "title" ? b.title : b.summary,
  };
}

function cancelEdit() {
  editing.value = null;
  editError.value = "";
}

async function saveEdit() {
  const e = editing.value;
  const s = sessions.value?.find((x) => keyOf(x) === selected.value);
  if (!e || !s) return;
  const patch =
    e.field === "title" ? { title: e.value } : { summary: e.value };
  try {
    await memoryTreeEditChapter(s.sessionId, s.workspace, e.id, patch);
    const b = chapters.value?.chapters.find((c) => c.chapterId === e.id);
    if (b) {
      if (e.field === "title") {
        b.title = e.value.trim();
        b.titleEditedByUser = true;
      } else {
        b.summary = e.value.trim();
        b.summaryEditedByUser = true;
      }
    }
    editing.value = null;
    editError.value = "";
  } catch (err) {
    editError.value = String(err);
  }
}

async function removeChapter(b: TreeChapterView) {
  const s = sessions.value?.find((x) => keyOf(x) === selected.value);
  if (!s) return;
  if (confirmingDelete.value !== b.chapterId) {
    confirmingDelete.value = b.chapterId;
    return;
  }
  confirmingDelete.value = "";
  try {
    await memoryTreeDeleteChapter(s.sessionId, s.workspace, b.chapterId);
    const list = chapters.value;
    if (list)
      list.chapters = list.chapters.filter((c) => c.chapterId !== b.chapterId);
  } catch (err) {
    editError.value = String(err);
  }
}

function timeText(ms: number): string {
  const r = relativeTime(ms);
  return r.n === undefined
    ? t(`memory.time.${r.key}`)
    : t(`memory.time.${r.key}`, { n: r.n });
}

function kindLabel(kind: TreeOriginalItem["kind"]): string {
  return t(
    kind === "user"
      ? "memory.kindUser"
      : kind === "assistant"
        ? "memory.kindAssistant"
        : kind === "tool_call"
          ? "memory.kindToolCall"
          : "memory.kindToolResult",
  );
}

function originalRows(id: string): TreeOriginalItem[] {
  return originals.value.get(id) ?? [];
}

function sourceLabel(source: string): string {
  return t(
    source === "recorder"
      ? "memory.sourceMemoryManager"
      : source === "rule"
        ? "memory.sourceRule"
        : "memory.sourcePlaceholder",
  );
}
</script>

<template>
  <div class="border-t border-line px-5 py-3">
    <!-- 区块头：标题 + 只读口径自证 + 刷新 -->
    <div class="flex items-center gap-2">
      <h3 class="text-xs font-semibold text-ink-1">
        {{ t("memory.treeTitle") }}
      </h3>
      <span class="text-[length:var(--lw-fs-10)] text-ink-5">{{ t("memory.treeHint") }}</span>
      <span
        v-if="sessions && sessions.length > 0"
        class="text-[length:var(--lw-fs-10)] text-ink-5"
      >
        {{ sessions.length }}
      </span>
      <div class="ml-auto flex items-center gap-1">
        <!-- M6.4 视图档位：会话 | 项目 -->
        <button
          class="rounded-[var(--lw-r-xs)] px-2 py-1 text-[length:var(--lw-fs-11)] transition"
          :class="viewMode === 'session' ? 'bg-wash-3 text-ink-1' : 'text-ink-4 hover:text-ink-1'"
          @click="viewMode = 'session'"
        >
          {{ t("memory.viewSession") }}
        </button>
        <button
          class="rounded-[var(--lw-r-xs)] px-2 py-1 text-[length:var(--lw-fs-11)] transition"
          :class="viewMode === 'project' ? 'bg-wash-3 text-ink-1' : 'text-ink-4 hover:text-ink-1'"
          @click="viewMode = 'project'"
        >
          {{ t("memory.viewProject") }}
        </button>
        <button
          class="rounded-[var(--lw-r-xs)] px-2 py-1 text-[length:var(--lw-fs-11)] text-ink-4 transition hover:bg-wash-2 hover:text-ink-1"
          @click="loadSessions"
        >
          {{ t("common.refresh") }}
        </button>
      </div>
    </div>

    <p v-if="loadError" class="mt-2 rounded-[var(--lw-r-xs)] bg-danger/10 px-3 py-2 text-xs text-danger">
      {{ loadError }}
    </p>

      <!-- 空态：不发明数据（影子期大多数会话还没有树） -->
      <p
        v-if="sessions !== null && sessions.length === 0"
        class="mt-2 text-[length:var(--lw-fs-11)] text-ink-5"
      >
        {{ t("memory.treeEmpty") }}
      </p>

      <template v-else-if="sessions && sessions.length > 0">
        <!-- 会话切换器 + 本地搜索 -->
        <div class="mt-2 flex items-center gap-2">
          <label class="shrink-0 text-[length:var(--lw-fs-10)] text-ink-5">
            {{ t("memory.treeSessionLabel") }}
          </label>
          <select
            v-model="selected"
            class="min-w-0 flex-1 rounded-[var(--lw-r-xs)] border border-line-strong bg-surface-1 px-2 py-1 text-xs text-ink-1 outline-none"
          >
            <option v-for="s in sessions" :key="keyOf(s)" :value="keyOf(s)">
              {{ optionText(s) }}
            </option>
          </select>
          <input
            v-model="search"
            :placeholder="t('memory.treeSearch')"
            class="w-48 shrink-0 rounded-[var(--lw-r-xs)] border border-line-strong bg-surface-1 px-3 py-1 text-xs text-ink-1 outline-none placeholder:text-ink-5 focus:border-line-strong"
          />
        </div>

        <!-- M6.4 项目档：跨会话联合视图（只读，点捆跳会话档） -->
        <ProjectTree
          v-if="viewMode === 'project'"
          :workspace="currentWorkspace"
          :search="search"
          @open="openInSession"
        />
        <!-- 会话档：章节列表（展开摘要 / 看原文 / 编辑 / 删除） -->
        <template v-else>
        <p
          v-if="filtered.length === 0"
          class="mt-2 text-[length:var(--lw-fs-11)] text-ink-5"
        >
          {{ t("memory.noMatch") }}
        </p>
        <ul v-else class="mt-2 space-y-2">
          <li
            v-for="b in filtered"
            :key="b.chapterId"
            class="rounded-[var(--lw-r-md)] bg-wash-1 px-4 py-3"
          >
            <!-- 头行：折叠 + L0 标题 + 来源 + 时间 -->
            <button
              class="flex w-full items-center gap-2 text-left"
              @click="toggleExpand(b)"
            >
              <span class="w-3 shrink-0 font-mono text-[length:var(--lw-fs-10)] text-ink-5">
                {{ expanded.has(b.chapterId) ? "▾" : "▸" }}
              </span>
              <span class="min-w-0 flex-1 truncate text-xs font-medium text-ink-1">
                {{ b.title }}
              </span>
              <span
                v-if="b.titleEditedByUser"
                class="shrink-0 rounded-full bg-wash-3 px-2 py-0.5 text-[length:var(--lw-fs-10)] text-ink-4"
                :title="t('memory.treeManual')"
              >
                {{ t("memory.treeManual") }}
              </span>
              <span
                v-if="b.pending"
                class="shrink-0 rounded-full bg-wash-3 px-2 py-0.5 text-[length:var(--lw-fs-10)] text-ink-4"
              >
                {{ sourceLabel(b.source) }}
              </span>
              <span class="shrink-0 text-[length:var(--lw-fs-10)] text-ink-5">
                {{ timeText(b.createdAt) }}
              </span>
            </button>

            <!-- 展开体：编辑态 / L1 摘要 + 来源徽章 + 操作行 + 原文 -->
            <template v-if="expanded.has(b.chapterId)">
              <!-- 编辑标题（M5 内联编辑态） -->
              <div
                v-if="editing?.id === b.chapterId && editing.field === 'title'"
                class="mt-1.5 pl-5"
              >
                <input
                  v-model="editing.value"
                  maxlength="40"
                  class="w-full rounded-[var(--lw-r-xs)] border border-line-strong bg-surface-1 px-2 py-1 text-xs text-ink-1 outline-none focus:border-line-strong"
                  @keyup.enter="saveEdit"
                  @keyup.esc="cancelEdit"
                />
                <div class="mt-1 flex items-center gap-2">
                  <button
                    class="rounded-[var(--lw-r-xs)] bg-accent px-2.5 py-0.5 text-[length:var(--lw-fs-11)] text-accent-ink"
                    @click="saveEdit"
                  >
                    {{ t("memory.save") }}
                  </button>
                  <button
                    class="text-[length:var(--lw-fs-11)] text-ink-4 hover:text-ink-1"
                    @click="cancelEdit"
                  >
                    {{ t("memory.cancel") }}
                  </button>
                </div>
              </div>
              <!-- 编辑摘要（M5 内联编辑态） -->
              <div
                v-else-if="
                  editing?.id === b.chapterId && editing.field === 'summary'
                "
                class="mt-1.5 pl-5"
              >
                <textarea
                  v-model="editing.value"
                  rows="5"
                  class="w-full resize-y rounded-[var(--lw-r-xs)] border border-line-strong bg-surface-1 px-2 py-1 text-xs leading-relaxed text-ink-1 outline-none focus:border-line-strong"
                ></textarea>
                <div class="mt-1 flex items-center gap-2">
                  <button
                    class="rounded-[var(--lw-r-xs)] bg-accent px-2.5 py-0.5 text-[length:var(--lw-fs-11)] text-accent-ink"
                    @click="saveEdit"
                  >
                    {{ t("memory.save") }}
                  </button>
                  <button
                    class="text-[length:var(--lw-fs-11)] text-ink-4 hover:text-ink-1"
                    @click="cancelEdit"
                  >
                    {{ t("memory.cancel") }}
                  </button>
                </div>
              </div>
              <!-- 查看态：摘要全文 + 来源/手动徽章 + 操作行 -->
              <template v-else>
                <p class="mt-1.5 whitespace-pre-wrap break-words pl-5 text-xs text-ink-3">
                  {{ b.summary }}
                </p>
                <div class="mt-1.5 flex items-center gap-2 pl-5">
                  <span
                    class="rounded-full px-2 py-0.5 text-[length:var(--lw-fs-10)]"
                    :class="
                      b.source === 'recorder'
                        ? 'bg-wash-3 text-ink-4'
                        : 'bg-warn/16 text-warn'
                    "
                    :title="b.source"
                  >
                    {{ sourceLabel(b.source) }}
                  </span>
                  <span
                    v-if="b.summaryEditedByUser"
                    class="rounded-full bg-wash-3 px-2 py-0.5 text-[length:var(--lw-fs-10)] text-ink-4"
                    :title="t('memory.treeManual')"
                  >
                    {{ t("memory.treeManual") }}
                  </span>
                  <template v-if="!b.pending">
                    <button
                      class="text-[length:var(--lw-fs-11)] text-ink-4 transition hover:text-ink-1"
                      @click.stop="startEdit(b, 'title')"
                    >
                      {{ t("memory.treeEditTitle") }}
                    </button>
                    <button
                      class="text-[length:var(--lw-fs-11)] text-ink-4 transition hover:text-ink-1"
                      @click.stop="startEdit(b, 'summary')"
                    >
                      {{ t("memory.treeEditSummary") }}
                    </button>
                    <button
                      class="text-[length:var(--lw-fs-11)] transition"
                      :class="
                        confirmingDelete === b.chapterId
                          ? 'text-danger'
                          : 'text-ink-4 hover:text-danger'
                      "
                      @click.stop="removeChapter(b)"
                    >
                      {{
                        confirmingDelete === b.chapterId
                          ? t("memory.treeConfirmDelete")
                          : t("memory.treeDelete")
                      }}
                    </button>
                  </template>
                </div>

                <!-- 原文切片（与记忆管理器材料同形；只读行渲染） -->
                <div v-if="originals.has(b.chapterId)" class="mt-2 space-y-1 pl-5">
                  <p
                    v-for="row in originalRows(b.chapterId)"
                    :key="row.index"
                    class="break-words text-[length:var(--lw-fs-11)] leading-relaxed"
                    :class="
                      row.kind === 'user'
                        ? 'text-ink-1'
                        : row.kind === 'assistant'
                          ? 'text-ink-3'
                          : 'font-mono text-[length:var(--lw-fs-10)] text-ink-5'
                    "
                  >
                    <span class="mr-1.5 select-none text-ink-5">
                      [{{ row.index }}] {{ kindLabel(row.kind) }}：
                    </span>{{ row.text }}
                  </p>
                </div>
                <p
                  v-else-if="originalErrors.has(b.chapterId)"
                  class="mt-2 pl-5 text-[length:var(--lw-fs-11)] text-danger"
                >
                  {{
                    t("memory.originalFailed", {
                      reason: originalErrors.get(b.chapterId),
                    })
                  }}
                </p>
                <p v-else class="mt-2 pl-5 text-[length:var(--lw-fs-11)] text-ink-5">
                  {{ t("memory.originalLoading") }}
                </p>
              </template>
              <p
                v-if="editError"
                class="mt-1 pl-5 text-[length:var(--lw-fs-11)] text-danger"
              >
                {{ t("memory.treeEditFailed", { reason: editError }) }}
              </p>
            </template>
          </li>
        </ul>
        </template>
      </template>
  </div>
</template>
