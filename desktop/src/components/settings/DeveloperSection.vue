<script setup lang="ts">
/**
 * 设置分区：开发者选项（v2.49，UI shell 文档 §3.2 / §七 规则 16②；
 * settings.json `developer` 区）。**默认隐藏分区**——侧栏版本号连点 7 次
 * 解锁（安卓式），这里提供锁回按钮；其余是危险开关（界面上写死的禁止项
 * 的解锁面），缺省全关。⑫ 的暴露调试面板同属开发者信息，自工具与分发
 * 面板迁入。行版式（§3.32·七 v3.5）。
 */
import { onMounted, reactive, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { llmCacheDiagnose, memoryReconcile, toolExposureDebug } from "../../lib/api";
import type { CacheBreakReport, ReconcileSummary } from "../../types";
import { compactNumber } from "../../lib/format";
import { useChatStore } from "../../stores/chat";
import { useSettingsStore } from "../../stores/settings";
import { useShellStore } from "../../stores/shell";
import { useToastStore } from "../../stores/toast";
import type { ExposureDebug } from "../../types";
import SettingsGroup from "./SettingsGroup.vue";
import SettingsRow from "./SettingsRow.vue";
import SettingsToggle from "./SettingsToggle.vue";
import MemoryTreePanel from "./MemoryTreePanel.vue";
import LabPanel from "../lab/LabPanel.vue";

const { t } = useI18n();
const settings = useSettingsStore();
const shell = useShellStore();
const toast = useToastStore();
const chat = useChatStore();

const form = reactive({
  allowDeleteRunningSession: false,
  allowSwitchRunningSession: false,
  cacheHitAlert: false,
  cacheDiagnostics: false,
  cacheHitAlertPercent: 80,
  pinSession: true,
  organizeReboundK: 3,
  organizeReboundM: 3,
  recorderTimeoutSecs: 30,
  recorderMaxRetries: 2,
  ruleChunkTokens: 20_000,
  // Q6（CB §7.2）超时与重试**同调**后的缺省；⚠ 这里的初值只是"后端 view 还没到之前"
  // 的闪现占位，真源永远是 `SettingsStore`（watch immediate 会用 view 覆盖）——
  // 但既然写了数字就必须跟后端一致，否则改缺省改一处就会让首帧说谎（批 Q4 立的
  // 「同一读数多处出现必须一起改」在这里的落点）。
  organizeMaxRerun: 3,
  // Q8b 同一条纪律：初值必须跟后端缺省常量一致（2 / 90），否则首帧说谎
  organizeMaxRelease: 2,
  organizeReleaseWatermarkPct: 90,
  organizeSummaryChapters: 6,
  organizeSummaryAttempts: 2,
});

watch(
  () => settings.view,
  (v) => {
    if (!v) return;
    form.allowDeleteRunningSession = v.developer.allowDeleteRunningSession;
    form.allowSwitchRunningSession = v.developer.allowSwitchRunningSession;
    form.cacheHitAlert = v.developer.cacheHitAlert;
    form.cacheDiagnostics = v.developer.cacheDiagnostics;
    form.cacheHitAlertPercent = v.developer.cacheHitAlertPercent;
    // M1 前缀纪律：JSON 在 tools 区，UI 行同挂本分区（v2.137 稳定期
    // 随 ctx-compact 桌面链退役）
    form.pinSession = v.tools.pinSession;
    // M2 记忆树整理（树 v2 §15 开发者层级）
    form.organizeReboundK = v.developer.organizeReboundK;
    form.organizeReboundM = v.developer.organizeReboundM;
    form.recorderTimeoutSecs = v.developer.recorderTimeoutSecs;
    form.recorderMaxRetries = v.developer.recorderMaxRetries;
    form.ruleChunkTokens = v.developer.ruleChunkTokens;
    form.organizeMaxRerun = v.developer.organizeMaxRerun;
    form.organizeMaxRelease = v.developer.organizeMaxRelease;
    form.organizeReleaseWatermarkPct = v.developer.organizeReleaseWatermarkPct;
    form.organizeSummaryChapters = v.developer.organizeSummaryChapters;
    form.organizeSummaryAttempts = v.developer.organizeSummaryAttempts;
  },
  { immediate: true },
);

function save() {
  void settings.saveDeveloper({
    allowDeleteRunningSession: form.allowDeleteRunningSession,
    allowSwitchRunningSession: form.allowSwitchRunningSession,
    cacheHitAlert: form.cacheHitAlert,
    cacheDiagnostics: form.cacheDiagnostics,
    cacheHitAlertPercent: Math.round(form.cacheHitAlertPercent),
  });
}

/** 工具集会话钉死（KV v2 §4.2）：写 tools 区。 */
function savePin() {
  void settings.saveTools({ pinSession: form.pinSession });
}

/** M2 记忆树整理可调项：全写 developer 区（出界后端拒绝）。 */
function saveOrganize() {
  void settings.saveDeveloper({
    organizeReboundK: Math.round(form.organizeReboundK),
    organizeReboundM: Math.round(form.organizeReboundM),
    recorderTimeoutSecs: Math.round(form.recorderTimeoutSecs),
    recorderMaxRetries: Math.round(form.recorderMaxRetries),
    ruleChunkTokens: Math.round(form.ruleChunkTokens),
    organizeMaxRerun: Math.round(form.organizeMaxRerun),
    organizeMaxRelease: Math.round(form.organizeMaxRelease),
    organizeReleaseWatermarkPct: Math.round(form.organizeReleaseWatermarkPct),
    organizeSummaryChapters: Math.round(form.organizeSummaryChapters),
    organizeSummaryAttempts: Math.round(form.organizeSummaryAttempts),
  });
}


/** 锁回：分区隐藏（危险开关保留原值——重新解锁后仍是原状态，如实告知）。 */
function hide() {
  void settings.saveDeveloper({ unlocked: false });
  toast.push("ok", t("developer.hidden"));
  shell.setDestination("chat");
}

// ---- F2 记忆文件合并（reconcile 手动触发；平时记忆页挂载自动跑） ----
const reconciling = ref(false);
const reconcileResult = ref<ReconcileSummary | null>(null);
/** M6.1 断裂诊断：面板打开拉一次（当前会话最近一步），不轮询。 */
const diag = ref<CacheBreakReport | null>(null);

async function runReconcile() {
  if (reconciling.value) return;
  reconciling.value = true;
  try {
    reconcileResult.value = await memoryReconcile();
    const r = reconcileResult.value;
    toast.push(
      "ok",
      t("developer.reconcileDone", {
        created: r.created,
        updated: r.updated,
        deleted: r.deleted,
        healed: r.healed,
      }),
    );
  } catch (e) {
    toast.push("danger", String(e));
  } finally {
    reconciling.value = false;
  }
}

/** 快照有效 = 有过至少一次请求（ts 非 0）。 */
function hasSnapshot(d: ExposureDebug | null): d is ExposureDebug {
  return d !== null && d.ts > 0;
}

// ---- ⑫ 暴露调试面板（自工具与分发面板迁入，v2.49）：打开分区时拉取 ----
const debug = ref<ExposureDebug | null>(null);
const loading = ref(false);

async function refresh() {
  loading.value = true;
  try {
    debug.value = await toolExposureDebug();
  } catch {
    debug.value = null;
  } finally {
    loading.value = false;
  }
  // M6.1 断裂诊断：开关开才拉（关 = 后端没记，拉了也是 null）
  if (form.cacheDiagnostics && chat.sessionId) {
    try {
      diag.value = await llmCacheDiagnose(chat.sessionId);
    } catch {
      diag.value = null;
    }
  }
}
onMounted(() => void refresh());
</script>

<template>
  <div class="space-y-6">
    <SettingsGroup :hint="t('developer.hint')">
      <SettingsRow :title="t('developer.allowDeleteRunning')" :hint="t('developer.allowDeleteRunningHint')">
        <SettingsToggle
          v-model="form.allowDeleteRunningSession"
          :label="t('developer.allowDeleteRunning')"
          @update:model-value="save"
        />
      </SettingsRow>

      <SettingsRow :title="t('developer.allowSwitchRunning')" :hint="t('developer.allowSwitchRunningHint')">
        <SettingsToggle
          v-model="form.allowSwitchRunningSession"
          :label="t('developer.allowSwitchRunning')"
          @update:model-value="save"
        />
      </SettingsRow>

      <!-- M0 观测（KV v2 §7.4）：命中率告警开关 + 阈值（缺省关） -->
      <SettingsRow :title="t('developer.cacheAlertTitle')" :hint="t('developer.cacheAlertHint')">
        <SettingsToggle
          v-model="form.cacheHitAlert"
          :label="t('developer.cacheAlertTitle')"
          @update:model-value="save"
        />
      </SettingsRow>

      <SettingsRow
        v-if="form.cacheHitAlert"
        :title="t('developer.cacheAlertPercent')"
        control-id="dev-cache-alert-percent"
      >
        <input
          id="dev-cache-alert-percent"
          v-model.number="form.cacheHitAlertPercent"
          type="number"
          min="1"
          max="100"
          class="lw-field lw-field-num px-2 text-xs"
          @change="save"
        />
      </SettingsRow>

      <SettingsRow :title="t('developer.hide')" :hint="t('developer.hideHint')">
        <button
          type="button"
          class="lw-btn px-3 py-1.5 text-xs text-ink-2"
          :disabled="settings.saving"
          @click="hide"
        >
          {{ t("developer.hide") }}
        </button>
      </SettingsRow>
    </SettingsGroup>

    <!-- ⑫ 暴露调试面板：最近一次请求的暴露分布（只读，手动刷新） -->
    <SettingsGroup :title="t('developer.panelTitle')">
      <template #action>
        <button
          type="button"
          class="lw-btn px-3 py-1.5 text-xs text-ink-2"
          :disabled="loading"
          @click="refresh"
        >
          {{ t("developer.refresh") }}
        </button>
      </template>
      <div class="space-y-2 rounded-[var(--lw-r-md-plus)] bg-wash-1 px-4 py-3.5">
        <p v-if="!hasSnapshot(debug)" class="text-[length:var(--lw-fs-12)] text-ink-4">
          {{ t("developer.empty") }}
        </p>

        <template v-else-if="debug">
          <p v-if="!debug.enabled" class="text-[length:var(--lw-fs-12)] text-warn">
            {{ t("developer.disabled") }}
          </p>
          <p class="text-xs">
            {{ t("developer.exposed", { exposed: debug.exposed, total: debug.total }) }}
          </p>
          <ul class="space-y-1 text-[length:var(--lw-fs-12)] text-ink-3">
            <li v-if="debug.always.length">
              {{ t("developer.always", { n: debug.always.length }) }}:
              <span class="text-ink-2">{{ debug.always.join(", ") }}</span>
            </li>
            <li v-if="debug.sessionHot.length">
              {{ t("developer.sessionHot", { n: debug.sessionHot.length }) }}:
              <span class="text-ink-2">{{ debug.sessionHot.join(", ") }}</span>
            </li>
            <li v-if="debug.globalHot.length">
              {{ t("developer.globalHot", { n: debug.globalHot.length }) }}:
              <span class="text-ink-2">{{ debug.globalHot.join(", ") }}</span>
            </li>
            <li v-if="debug.retrieved.length">
              {{ t("developer.retrieved", { n: debug.retrieved.length }) }}:
              <span class="text-ink-2">{{ debug.retrieved.join(", ") }}</span>
            </li>
          </ul>
          <p v-if="debug.savedTokensEstimate > 0" class="text-[length:var(--lw-fs-12)] text-ok">
            {{ t("developer.saved", { n: compactNumber(debug.savedTokensEstimate) }) }}
          </p>
        </template>
      </div>
    </SettingsGroup>

    <!-- M0 观测（KV v2 §7）：前缀缓存读数——会话累计（实时/重放同源聚合）
         + 本轮（turnUsage）。供应商未报告缓存字段 = 行留空态，不虚构。 -->
    <SettingsGroup :title="t('developer.cacheTitle')" :hint="t('developer.cacheHint')">
      <div class="space-y-2 rounded-[var(--lw-r-md-plus)] bg-wash-1 px-4 py-3.5 text-xs">
        <p v-if="!chat.cacheStats" class="text-[length:var(--lw-fs-12)] text-ink-4">
          {{ t("developer.cacheEmpty") }}
        </p>
        <template v-else>
          <p>
            {{ t("developer.cacheHitRate", { pct: chat.cacheStats.hitRate, steps: chat.cacheStats.steps }) }}
          </p>
          <p>
            {{
              t("developer.cacheHitMiss", {
                hit: compactNumber(chat.cacheStats.cacheRead),
                miss: compactNumber(Math.max(0, chat.cacheStats.input - chat.cacheStats.cacheRead)),
                input: compactNumber(chat.cacheStats.input),
              })
            }}
          </p>
        </template>
        <p v-if="chat.turnUsage" class="text-[length:var(--lw-fs-12)] text-ink-3">
          {{
            t("developer.cacheTurn", {
              input: compactNumber(chat.turnUsage.input),
              hit: compactNumber(chat.turnUsage.cacheRead),
              out: compactNumber(chat.turnUsage.output),
            })
          }}
        </p>
        <!-- M6.1 断裂诊断（打开分区拉一次）：最近一步 hash 链比对 -->
        <div
          v-if="form.cacheDiagnostics"
          class="mt-2 border-t border-line pt-2"
        >
          <p class="text-[length:var(--lw-fs-12)] font-medium text-ink-2">
            {{ t("developer.cacheDiagTitle") }}
          </p>
          <p v-if="!diag" class="mt-1 text-[length:var(--lw-fs-12)] text-ink-4">
            {{ t("developer.cacheDiagEmpty") }}
          </p>
          <template v-else>
            <p
              class="mt-1"
              :class="diag.breaks.length ? 'text-warn' : 'text-ink-3'"
            >
              {{
                diag.breaks.length
                  ? t("developer.cacheDiagBroken", { step: diag.step, blocks: diag.breaks.join(", ") })
                  : t("developer.cacheDiagHealthy", { step: diag.step, n: diag.messages })
              }}
            </p>
          </template>
        </div>
      </div>
    </SettingsGroup>

    <!-- M1 前缀纪律（KV v2 §4.2/§6.4/§9 开发者层级）：钉死开关 + 稳定期轮数。
         键 JSON 分属 tools/context 区，UI 行同挂本分区（文档先行 v4.4）。 -->
    <SettingsGroup :title="t('developer.prefixTitle')" :hint="t('developer.prefixHint')">
      <!-- M6.1 断裂诊断开关（KV v2 §7.4，缺省关） -->
      <SettingsRow
        :title="t('developer.cacheDiagnostics')"
        :hint="t('developer.cacheDiagnosticsHint')"
      >
        <SettingsToggle
          v-model="form.cacheDiagnostics"
          :label="t('developer.cacheDiagnostics')"
          @update:model-value="save"
        />
      </SettingsRow>
      <SettingsRow :title="t('developer.pinSession')" :hint="t('developer.pinSessionHint')">
        <SettingsToggle
          v-model="form.pinSession"
          :label="t('developer.pinSession')"
          @update:model-value="savePin"
        />
      </SettingsRow>
    </SettingsGroup>

    <!-- M2 记忆树影子运行（树 v2 §15 开发者层级）：可调项行；观测面板
         拆 MemoryTreePanel（§3.36③，随 M3/M4 独立演进不牵动危险开关） -->
    <SettingsGroup :title="t('developer.filesTitle')" :hint="t('developer.filesHint')">
      <SettingsRow :title="t('developer.reconcileTitle')" :hint="t('developer.reconcileHint')">
        <button
          class="lw-btn rounded-[var(--lw-r-xs)] px-3 py-1 text-[length:var(--lw-fs-11)] font-semibold disabled:cursor-not-allowed disabled:opacity-40"
          :disabled="reconciling"
          @click="runReconcile"
        >
          {{ reconciling ? t("developer.reconcileRunning") : t("developer.reconcileRun") }}
        </button>
      </SettingsRow>
      <p
        v-if="reconcileResult"
        class="px-4 pb-1 text-[length:var(--lw-fs-11)] text-ink-4"
      >
        {{ t("developer.reconcileLast", { ...reconcileResult }) }}
      </p>
    </SettingsGroup>
    <SettingsGroup :title="t('developer.organizeTitle')" :hint="t('developer.organizeHint')">
      <SettingsRow :title="t('developer.reboundK')" :hint="t('developer.reboundKHint')" control-id="dev-organize-k">
        <input
          id="dev-organize-k"
          v-model.number="form.organizeReboundK"
          type="number"
          min="1"
          max="10"
          class="lw-field lw-field-num px-2 text-xs"
          @change="saveOrganize"
        />
      </SettingsRow>
      <SettingsRow :title="t('developer.reboundM')" :hint="t('developer.reboundMHint')" control-id="dev-organize-m">
        <input
          id="dev-organize-m"
          v-model.number="form.organizeReboundM"
          type="number"
          min="1"
          max="10"
          class="lw-field lw-field-num px-2 text-xs"
          @change="saveOrganize"
        />
      </SettingsRow>
      <SettingsRow :title="t('developer.memoryManagerTimeout')" :hint="t('developer.memoryManagerTimeoutHint')" control-id="dev-recorder-timeout">
        <input
          id="dev-recorder-timeout"
          v-model.number="form.recorderTimeoutSecs"
          type="number"
          min="10"
          max="600"
          class="lw-field lw-field-num px-2 text-xs"
          @change="saveOrganize"
        />
      </SettingsRow>
      <SettingsRow :title="t('developer.memoryManagerRetries')" :hint="t('developer.memoryManagerRetriesHint')" control-id="dev-recorder-retries">
        <input
          id="dev-recorder-retries"
          v-model.number="form.recorderMaxRetries"
          type="number"
          min="0"
          max="10"
          class="lw-field lw-field-num px-2 text-xs"
          @change="saveOrganize"
        />
      </SettingsRow>
      <SettingsRow :title="t('developer.ruleChunk')" :hint="t('developer.ruleChunkHint')" control-id="dev-rule-chunk">
        <input
          id="dev-rule-chunk"
          v-model.number="form.ruleChunkTokens"
          type="number"
          min="5000"
          max="50000"
          step="1000"
          class="lw-field lw-field-num px-2 text-xs"
          @change="saveOrganize"
        />
      </SettingsRow>
      <!-- 队列 Q6（CB §12.2）：在途收束后的连续补跑上限，0 = 关掉补跑（只记标记） -->
      <SettingsRow :title="t('developer.organizeMaxRerun')" :hint="t('developer.organizeMaxRerunHint')" control-id="dev-organize-rerun">
        <input
          id="dev-organize-rerun"
          v-model.number="form.organizeMaxRerun"
          type="number"
          min="0"
          max="10"
          class="lw-field lw-field-num px-2 text-xs"
          @change="saveOrganize"
        />
      </SettingsRow>
      <!-- 队列 Q8b（真机「整理跟不上 ⇒ 一步卡死」）：轮内水位泄压。上限 0 = 关掉
           这条通路（关掉后与改前逐字节同形，判据 = release_cap_zero_writes_nothing_at_all） -->
      <SettingsRow
        :title="t('developer.organizeMaxRelease')"
        :hint="t('developer.organizeMaxReleaseHint')"
        control-id="dev-organize-release"
      >
        <input
          id="dev-organize-release"
          v-model.number="form.organizeMaxRelease"
          type="number"
          min="0"
          max="5"
          class="lw-field lw-field-num px-2 text-xs"
          @change="saveOrganize"
        />
      </SettingsRow>
      <SettingsRow
        :title="t('developer.organizeReleaseWatermark')"
        :hint="t('developer.organizeReleaseWatermarkHint')"
        control-id="dev-organize-release-pct"
      >
        <input
          id="dev-organize-release-pct"
          v-model.number="form.organizeReleaseWatermarkPct"
          type="number"
          min="50"
          max="99"
          class="lw-field lw-field-num px-2 text-xs"
          @change="saveOrganize"
        />
      </SettingsRow>
      <!-- 队列 Q8c（泄压留下的机械摘要谁来还）：轮末之后由补写通路换成模型摘要。
           章数 0 = 关掉补写（关掉后连一次读盘都不发生，判据 =
           backfill_off_writes_nothing_at_all）；补写只改摘要正文，目录与游标不动，
           所以它不打断前缀缓存（判据 = backfill_keeps_the_injected_directory_byte_identical） -->
      <SettingsRow
        :title="t('developer.organizeSummaryChapters')"
        :hint="t('developer.organizeSummaryChaptersHint')"
        control-id="dev-organize-summary-chapters"
      >
        <input
          id="dev-organize-summary-chapters"
          v-model.number="form.organizeSummaryChapters"
          type="number"
          min="0"
          max="20"
          class="lw-field lw-field-num px-2 text-xs"
          @change="saveOrganize"
        />
      </SettingsRow>
      <SettingsRow
        :title="t('developer.organizeSummaryAttempts')"
        :hint="t('developer.organizeSummaryAttemptsHint')"
        control-id="dev-organize-summary-attempts"
      >
        <input
          id="dev-organize-summary-attempts"
          v-model.number="form.organizeSummaryAttempts"
          type="number"
          min="0"
          max="5"
          class="lw-field lw-field-num px-2 text-xs"
          @change="saveOrganize"
        />
      </SettingsRow>
      <!-- M5：defaultOrganizer 行退役——键迁用户层 memory.defaultOrganizer
           （缺省 tree），见 MemorySection 缺省整理方式行 -->
    </SettingsGroup>

    <MemoryTreePanel />

    <!-- 【实验室】形态库试挂（beautiful-ui-vue 移植件，目检用，随实验去留） -->
    <LabPanel />
  </div>
</template>
