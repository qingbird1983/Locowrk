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
import { toolExposureDebug } from "../../lib/api";
import { compactNumber } from "../../lib/format";
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

const form = reactive({
  allowDeleteRunningSession: false,
  allowSwitchRunningSession: false,
});

watch(
  () => settings.view,
  (v) => {
    if (!v) return;
    form.allowDeleteRunningSession = v.developer.allowDeleteRunningSession;
    form.allowSwitchRunningSession = v.developer.allowSwitchRunningSession;
  },
  { immediate: true },
);

function save() {
  void settings.saveDeveloper({
    allowDeleteRunningSession: form.allowDeleteRunningSession,
    allowSwitchRunningSession: form.allowSwitchRunningSession,
  });
}

/** 锁回：分区隐藏（危险开关保留原值——重新解锁后仍是原状态，如实告知）。 */
function hide() {
  void settings.saveDeveloper({ unlocked: false });
  toast.push("ok", t("developer.hidden"));
  shell.setDestination("chat");
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
      <div class="space-y-2 rounded-[14px] bg-wash-1 px-4 py-3.5">
        <p v-if="!hasSnapshot(debug)" class="text-[11.5px] text-ink-4">
          {{ t("developer.empty") }}
        </p>

        <template v-else-if="debug">
          <p v-if="!debug.enabled" class="text-[11.5px] text-warn">
            {{ t("developer.disabled") }}
          </p>
          <p class="text-xs">
            {{ t("developer.exposed", { exposed: debug.exposed, total: debug.total }) }}
          </p>
          <ul class="space-y-1 text-[11.5px] text-ink-3">
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
          <p v-if="debug.savedTokensEstimate > 0" class="text-[11.5px] text-ok">
            {{ t("developer.saved", { n: compactNumber(debug.savedTokensEstimate) }) }}
          </p>
        </template>
      </div>
    </SettingsGroup>
  </div>
</template>
