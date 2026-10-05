<script setup lang="ts">
/**
 * 记忆树（情景）区块（M3，UI shell §3.37①）：只读呈现树投影——会话
 * 切换器（memory_tree_sessions 扫描，最近更新降序）+ 捆列表（展开看
 * L1 摘要 / 看原文）+ 本地搜索。零写操作（编辑/删除属 M5）；看原文与
 * 记录员材料同形同源（memory_tree_original，build_items 同一函数）。
 * 自持状态不牵动事实区；打开拉取不轮询（⑫/§3.36③ 同款）。
 */
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { memoryTreeBundles, memoryTreeOriginal, memoryTreeSessions } from "../../lib/api";
import { relativeTime } from "../../lib/format";
import { workspaceDisplayName } from "../../lib/session-groups";
import { useChatStore } from "../../stores/chat";
import type {
  TreeBundleView,
  TreeBundlesView,
  TreeOriginalItem,
  TreeSummaryView,
} from "../../types";

const { t } = useI18n();
const chat = useChatStore();

const sessions = ref<TreeSummaryView[] | null>(null); // null = 加载中
const selected = ref(""); // `${workspace}\u0000${sessionId}`
const bundles = ref<TreeBundlesView | null>(null);
const loadError = ref("");
const search = ref("");

/** 展开态（本地保持，切换会话重置；§3.28 只增不收回惯例的反面——
 * 换会话是换数据源，重置才诚实）。 */
const expanded = ref<Set<string>>(new Set());
/** 原文缓存：bundleId → 行（或错误文案）；只增不收回。 */
const originals = ref<Map<string, TreeOriginalItem[]>>(new Map());
const originalErrors = ref<Map<string, string>>(new Map());

function keyOf(s: TreeSummaryView): string {
  return `${s.workspace}\u0000${s.sessionId}`;
}

/** 会话标题（侧栏同源；树在、会话已删 = 回退 id 前缀）。 */
/** 选择器行：标题 · 项目 · N 捆（未归属跳过项目段，不留空段）。 */
function optionText(s: TreeSummaryView): string {
  const parts = [titleOf(s)];
  const ws = workspaceDisplayName(s.workspace);
  if (ws) parts.push(ws);
  parts.push(t("memory.treeSessionCount", { n: s.bundleCount }));
  return parts.join(" · ");
}

function titleOf(s: TreeSummaryView): string {
  const meta = chat.sessions.find((x) => x.id === s.sessionId);
  return meta?.title ?? s.sessionId.slice(0, 8);
}

const filtered = computed(() => {
  const q = search.value.trim().toLowerCase();
  const list = bundles.value?.bundles ?? [];
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
      bundles.value = null;
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
  expanded.value = new Set();
  originals.value = new Map();
  originalErrors.value = new Map();
  try {
    bundles.value = await memoryTreeBundles(s.sessionId, s.workspace);
  } catch (e) {
    loadError.value = String(e);
    bundles.value = null;
  }
}

watch(selected, loadBundles);

onMounted(loadSessions);

function toggleExpand(b: TreeBundleView) {
  const next = new Set(expanded.value);
  if (next.has(b.bundleId)) {
    next.delete(b.bundleId);
  } else {
    next.add(b.bundleId);
    void loadOriginal(b);
  }
  expanded.value = next;
}

async function loadOriginal(b: TreeBundleView) {
  if (originals.value.has(b.bundleId) || originalErrors.value.has(b.bundleId))
    return;
  const s = sessions.value?.find((x) => keyOf(x) === selected.value);
  if (!s) return;
  try {
    const rows = await memoryTreeOriginal(s.sessionId, s.workspace, b.bundleId);
    originals.value.set(b.bundleId, rows);
  } catch (e) {
    originalErrors.value.set(b.bundleId, String(e));
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
      ? "memory.sourceRecorder"
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
      <span class="text-[length:var(--lw-fs-10)] text-ink-5">{{ t("memory.treeReadOnly") }}</span>
      <span
        v-if="sessions && sessions.length > 0"
        class="text-[length:var(--lw-fs-10)] text-ink-5"
      >
        {{ sessions.length }}
      </span>
      <button
        class="ml-auto rounded-[var(--lw-r-xs)] px-2 py-1 text-[length:var(--lw-fs-11)] text-ink-4 transition hover:bg-wash-2 hover:text-ink-1"
        @click="loadSessions"
      >
        {{ t("common.refresh") }}
      </button>
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

        <!-- 捆列表（只读：展开摘要 / 看原文） -->
        <p
          v-if="filtered.length === 0"
          class="mt-2 text-[length:var(--lw-fs-11)] text-ink-5"
        >
          {{ t("memory.noMatch") }}
        </p>
        <ul v-else class="mt-2 space-y-2">
          <li
            v-for="b in filtered"
            :key="b.bundleId"
            class="rounded-[var(--lw-r-md)] bg-wash-1 px-4 py-3"
          >
            <!-- 头行：折叠 + L0 标题 + 来源 + 时间 -->
            <button
              class="flex w-full items-center gap-2 text-left"
              @click="toggleExpand(b)"
            >
              <span class="w-3 shrink-0 font-mono text-[length:var(--lw-fs-10)] text-ink-5">
                {{ expanded.has(b.bundleId) ? "▾" : "▸" }}
              </span>
              <span class="min-w-0 flex-1 truncate text-xs font-medium text-ink-1">
                {{ b.title }}
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

            <!-- 展开体：L1 摘要全文 + 来源徽章 + 原文 -->
            <template v-if="expanded.has(b.bundleId)">
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
              </div>

              <!-- 原文切片（与记录员材料同形；只读行渲染） -->
              <div v-if="originals.has(b.bundleId)" class="mt-2 space-y-1 pl-5">
                <p
                  v-for="row in originalRows(b.bundleId)"
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
                v-else-if="originalErrors.has(b.bundleId)"
                class="mt-2 pl-5 text-[length:var(--lw-fs-11)] text-danger"
              >
                {{
                  t("memory.originalFailed", {
                    reason: originalErrors.get(b.bundleId),
                  })
                }}
              </p>
              <p v-else class="mt-2 pl-5 text-[length:var(--lw-fs-11)] text-ink-5">
                {{ t("memory.originalLoading") }}
              </p>
            </template>
          </li>
        </ul>
      </template>
  </div>
</template>
