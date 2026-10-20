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
import { llmCacheDiagnose, toolExposureDebug } from "../../lib/api";
import type { CacheBreakReport } from "../../types";
import { compactNumber } from "../../lib/format";
import { useChatStore } from "../../stores/chat";
import { useSettingsStore } from "../../stores/settings";
import { useShellStore } from "../../stores/shell";
import { useToastStore } from "../../stores/toast";
import type { ExposureDebug } from "../../types";
import SettingsGroup from "./SettingsGroup.vue";
import SettingsRow from "./SettingsRow.vue";
import SettingsToggle from "./SettingsToggle.vue";

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

/** 锁回：分区隐藏（危险开关保留原值——重新解锁后仍是原状态，如实告知）。 */
function hide() {
  void settings.saveDeveloper({ unlocked: false });
  toast.push("ok", t("developer.hidden"));
  shell.setDestination("chat");
}

/** M6.1 断裂诊断：面板打开拉一次（当前会话最近一步），不轮询。 */
const diag = ref<CacheBreakReport | null>(null);

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

  </div>
</template>
