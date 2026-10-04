<script setup lang="ts">
/**
 * 开发者分区 · 记忆树观测面板（M2 §3.36③）：当前会话的树投影读数——
 * 捆数/游标、未整理尾巴、占用锚点（真实上报/字符估算如实标注）、反弹
 * 计数与熔断态、在途任务、最近整理记录。只读：打开拉一次 + 手动刷新
 * （⑫ 暴露面板同款，不推送不轮询）。自 DeveloperSection 拆出（三问
 * ③：面板随 M3/M4 独立演进，与危险开关互不牵动）。
 */
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { memoryTreeView } from "../../lib/api";
import { compactNumber } from "../../lib/format";
import { useChatStore } from "../../stores/chat";
import type { TreeDebug } from "../../types";
import SettingsGroup from "./SettingsGroup.vue";
import SettingsRow from "./SettingsRow.vue";

const { t } = useI18n();
const chat = useChatStore();

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
      <button
        type="button"
        class="lw-btn px-3 py-1.5 text-xs text-ink-2"
        :disabled="loading"
        @click="refresh"
      >
        {{ t("developer.refresh") }}
      </button>
    </SettingsRow>
    <div class="space-y-2 rounded-[14px] bg-wash-1 px-4 py-3.5 text-xs">
      <p v-if="!tree" class="text-[11.5px] text-ink-4">
        {{ t("developer.treeEmpty") }}
      </p>
      <template v-else>
        <p>{{ t("developer.treeBundles", { n: tree.bundleCount, cursor: tree.bundledUntil }) }}</p>
        <p>{{ t("developer.treeTail", { events: tree.tailEvents, tokens: compactNumber(tree.tailTokensEst) }) }}</p>
        <p>
          {{
            t("developer.treeAnchor", {
              tokens: compactNumber(tree.anchorTokens),
              source: tree.anchorSource === "llm_usage" ? t("developer.treeAnchorLlm") : t("developer.treeAnchorEstimate"),
              pct: tree.thresholdPct,
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
          <p class="text-[11.5px] font-medium text-ink-3">{{ t("developer.treeHistory") }}</p>
          <p
            v-for="r in tree.history.slice(-5).reverse()"
            :key="r.ts"
            class="mt-1 text-[11px] leading-relaxed text-ink-4"
          >
            {{
              t("developer.treeHistoryRow", {
                time: relTime(r.ts),
                reason: r.reason,
                mode: r.mode,
                status: r.status,
                bundles: r.bundles,
                input: compactNumber(r.recorderInput),
                output: compactNumber(r.recorderOutput),
                ms: r.durationMs,
              })
            }}
          </p>
        </div>
      </template>
    </div>
  </SettingsGroup>
</template>
