<script setup lang="ts">
/**
 * 开发者分区 · 记忆树观测面板（M2 §3.36③）：当前会话的树投影读数——
 * 章节数/游标、未整理尾巴、占用锚点（真实上报/字符估算如实标注）、反弹
 * 计数与熔断态、在途任务、最近整理记录。只读：打开拉一次 + 手动刷新
 * （⑫ 暴露面板同款，不推送不轮询）。自 DeveloperSection 拆出（三问
 * ③：面板随 M3/M4 独立演进，与危险开关互不牵动）。
 */
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { memoryTreeView, organizeNow } from "../../lib/api";
import { compactNumber } from "../../lib/format";
import { useChatStore } from "../../stores/chat";
import { useToastStore } from "../../stores/toast";
import type { TreeDebug } from "../../types";
import SettingsGroup from "./SettingsGroup.vue";
import SettingsRow from "./SettingsRow.vue";

const { t } = useI18n();
const chat = useChatStore();
const toast = useToastStore();

const tree = ref<TreeDebug | null>(null);
const loading = ref(false);

async function refresh() {
  loading.value = true;
  try {
    tree.value = await memoryTreeView(chat.sessionId);
  } catch {
    tree.value = null;
  } finally {
    loading.value = false;
  }
}
onMounted(() => void refresh());

/** 手动整理（S2 刀⑦ 从主对话流的 `/整理` 搬进本面板）。它同时是**自动整理熔断之后
 * 唯一的复位口**（`memory-tree` 里 `paused_auto` 只由这条链清），所以命令改语义时
 * 不能把它一起删掉。发起即返回——整理本体后台跑，收束那条线住在会话区，本区因此
 * 只报"已发起"，不冒充进度；紧接着刷一次读数，是为了让上面那行熔断态**当场**翻回来
 * （点了按钮却看不见任何变化 = 用户只能靠重开面板确认）。 */
const organizing = ref(false);
async function organize() {
  organizing.value = true;
  try {
    await organizeNow(chat.sessionId);
    toast.push("ok", t("developer.organizeStarted"));
    await refresh();
  } catch (e) {
    toast.push("danger", String(e));
  } finally {
    organizing.value = false;
  }
}

/** 整理阈值显示（绝对 token → 100K–800K 档位文本）。 */
function thresholdText(tokens: number): string {
  return `${Math.round(tokens / 1000)}K`;
}

function relTime(ts: number): string {
  const diff = Date.now() - ts;
  const mins = Math.floor(diff / 60_000);
  if (mins < 1) return "<1m";
  if (mins < 60) return `${mins}m`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) return `${hours}h`;
  return `${Math.floor(hours / 24)}d`;
}
</script>

<template>
  <SettingsGroup :title="t('developer.organizeTitle')" :hint="t('developer.organizeHint')">
    <!-- 观测面板：只读，打开拉取 + 手动刷新（⑫ 同款不轮询） -->
    <SettingsRow :title="t('developer.treePanelTitle')">
      <div class="flex items-center gap-2">
        <button
          type="button"
          class="lw-btn px-3 py-1.5 text-xs text-ink-2"
          :disabled="loading"
          @click="refresh"
        >
          {{ t("developer.refresh") }}
        </button>
        <!-- 手动整理 + 熔断复位（刀⑦ 从 `/整理` 搬来，同一种 lw-btn 形态） -->
        <button
          type="button"
          class="lw-btn px-3 py-1.5 text-xs text-ink-2"
          :disabled="organizing"
          @click="organize"
        >
          {{ t("developer.organizeNow") }}
        </button>
      </div>
    </SettingsRow>
    <div class="space-y-2 rounded-[var(--lw-r-md-plus)] bg-wash-1 px-4 py-3.5 text-xs">
      <p v-if="!tree" class="text-[length:var(--lw-fs-12)] text-ink-4">
        {{ t("developer.treeEmpty") }}
      </p>
      <template v-else>
        <p>{{ t("developer.treeChapters", { n: tree.chapterCount, cursor: tree.chapteredUntil }) }}</p>
        <p>{{ t("developer.treeTail", { events: tree.tailEvents, tokens: compactNumber(tree.tailTokensEst) }) }}</p>
        <p>
          {{
            t("developer.treeAnchor", {
              tokens: compactNumber(tree.anchorTokens),
              source: tree.anchorSource === "llm_usage" ? t("developer.treeAnchorLlm") : t("developer.treeAnchorEstimate"),
              threshold: thresholdText(tree.thresholdTokens),
              pacing: tree.pacing === "batch" ? t("developer.treePacingBatch") : t("developer.treePacingIncremental"),
            })
          }}
        </p>
        <p :class="tree.pausedAuto ? 'text-warn' : 'text-ink-3'">
          {{
            tree.pausedAuto
              ? t("developer.treeRebound", { n: tree.reboundCount })
              : t("developer.treeReboundClean", { n: tree.reboundCount })
          }}
        </p>
        <p v-if="tree.pending" class="text-ink-3">
          {{ t("developer.treePending", { from: tree.pending.from, to: tree.pending.to, attempts: tree.pending.attempts }) }}
        </p>
        <div v-if="tree.history.length" class="pt-1">
          <p class="text-[length:var(--lw-fs-12)] font-medium text-ink-3">{{ t("developer.treeHistory") }}</p>
          <p
            v-for="r in tree.history.slice(-5).reverse()"
            :key="r.ts"
            class="mt-1 text-[length:var(--lw-fs-11)] leading-relaxed text-ink-4"
          >
            {{
              t("developer.treeHistoryRow", {
                time: relTime(r.ts),
                reason: r.reason,
                mode: r.mode === "recorder" ? t("developer.modeMemoryManager") : t("developer.modeRule"),
                status: r.status,
                chapters: r.chapters,
                input: compactNumber(r.manager_input),
                output: compactNumber(r.manager_output),
                ms: r.durationMs,
              })
            }}
          </p>
        </div>
      </template>
    </div>
  </SettingsGroup>
</template>
