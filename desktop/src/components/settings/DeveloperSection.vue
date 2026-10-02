<script setup lang="ts">
/**
 * 设置分区：开发者选项（⑫，settings.json `tools` 区 + 暴露调试面板；
 * exposure 文档 §6.3/§9.6）。上半是截断设置（开关 + 每请求最大工具数，
 * 保存即热生效）；下半读最近一次请求的暴露快照——只在打开分区时拉取
 * （§8.8"用户来找"的信息，不推送不轮询），节省 token 按 §3.5 走
 * compactNumber 档位，不显示机器精度原始值。
 * 行版式（§3.32·七 v3.5）：三块组（行组 / 工作区目录 / 快照面板）；
 * 目录列表与快照是天然满宽的块，按宽内容例外整块呈现。
 */
import { computed, onMounted, reactive, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { pickDirectory, toolExposureDebug } from "../../lib/api";
import { compactNumber } from "../../lib/format";
import { useSettingsStore } from "../../stores/settings";
import type { ExposureDebug } from "../../types";
import SettingsGroup from "./SettingsGroup.vue";
import SettingsRow from "./SettingsRow.vue";
import SettingsSaveRow from "./SettingsSaveRow.vue";
import SettingsToggle from "./SettingsToggle.vue";

const { t } = useI18n();
const settings = useSettingsStore();

const form = reactive({
  enabled: true,
  maxPerRequest: 20,
  callTimeoutSecs: 30,
});

watch(
  () => settings.view,
  (v) => {
    if (!v) return;
    form.enabled = v.tools.enabled;
    form.maxPerRequest = v.tools.maxPerRequest;
    form.callTimeoutSecs = v.tools.callTimeoutSecs;
  },
  { immediate: true },
);

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

function save() {
  void settings.saveTools({
    enabled: form.enabled,
    maxPerRequest: form.maxPerRequest,
    callTimeoutSecs: form.callTimeoutSecs,
  });
}

// ---- ⑱ 工作区目录（fs roots）：增删即存即生效（后端重注入 env，
// fsutil 每次工具调用读取），不参与上方表单的"保存"按钮。 ----
const fsRoots = computed(() => settings.view?.tools.fsRoots ?? []);

async function addRoot() {
  const dir = await pickDirectory();
  if (!dir || fsRoots.value.includes(dir)) return;
  await settings.saveTools({ fsRoots: [...fsRoots.value, dir] });
}

function removeRoot(idx: number) {
  void settings.saveTools({ fsRoots: fsRoots.value.filter((_, i) => i !== idx) });
}

/** 快照有效 = 有过至少一次请求（ts 非 0）。 */
function hasSnapshot(d: ExposureDebug | null): d is ExposureDebug {
  return d !== null && d.ts > 0;
}
</script>

<template>
  <div class="space-y-6">
    <div class="space-y-3">
      <SettingsGroup :hint="t('settings.developerHint')">
        <SettingsRow :title="t('settings.toolsEnabled')">
          <SettingsToggle v-model="form.enabled" :label="t('settings.toolsEnabled')" />
        </SettingsRow>

        <SettingsRow
          :title="t('settings.toolsMax')"
          :hint="t('settings.toolsMaxHint')"
          control-id="tools-max"
        >
          <input
            id="tools-max"
            v-model.number="form.maxPerRequest"
            type="number"
            min="1"
            max="200"
            class="w-[140px] rounded-lg border border-line-strong bg-surface-1 px-2 py-1.5 text-xs"
          />
        </SettingsRow>

        <!-- S2（清单 §3 约束最小化）：宿主单次工具调用超时可配 -->
        <SettingsRow
          :title="t('settings.callTimeout')"
          :hint="t('settings.callTimeoutHint')"
          control-id="tools-call-timeout"
        >
          <input
            id="tools-call-timeout"
            v-model.number="form.callTimeoutSecs"
            type="number"
            min="5"
            max="3600"
            class="w-[140px] rounded-lg border border-line-strong bg-surface-1 px-2 py-1.5 text-xs"
          />
        </SettingsRow>
      </SettingsGroup>

      <SettingsSaveRow @save="save" />
    </div>

    <!-- ⑱ 工作区目录（fs roots）：增删即存即生效，不走上方"保存" -->
    <SettingsGroup
      :title="t('settings.fsRootsTitle')"
      :hint="t('settings.fsRootsHint')"
    >
      <template #action>
        <button
          type="button"
          class="rounded-lg border border-line-strong px-2 py-1 text-[11px] text-ink-3 transition hover:bg-surface-1"
          :disabled="settings.saving"
          @click="addRoot"
        >
          {{ t("settings.fsRootsAdd") }}
        </button>
      </template>
      <p v-if="!fsRoots.length" class="px-1 py-1 text-[11.5px] text-ink-4">
        {{ t("settings.fsRootsEmpty") }}
      </p>
      <div
        v-for="(root, i) in fsRoots"
        :key="root"
        class="flex items-center justify-between gap-2 rounded-[14px] bg-wash-1 px-4 py-2.5"
      >
        <span class="min-w-0 break-all text-[11.5px] text-ink-2" :title="root">{{ root }}</span>
        <button
          type="button"
          class="shrink-0 rounded px-1.5 text-[11px] text-ink-4 transition hover:bg-wash-2 hover:text-danger"
          :disabled="settings.saving"
          :aria-label="`${t('settings.fsRootsRemove')} ${root}`"
          @click="removeRoot(i)"
        >
          ✕
        </button>
      </div>
    </SettingsGroup>

    <!-- 暴露调试面板：最近一次请求的暴露分布（只读，手动刷新）——
         混合内容按 §3.32·七 整块一卡 -->
    <SettingsGroup :title="t('developer.panelTitle')">
      <template #action>
        <button
          type="button"
          class="rounded-lg border border-line-strong px-2 py-1 text-[11px] text-ink-3 transition hover:bg-surface-1"
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
            <!-- ⑬ 语义检索信号入选（embedding 未配置恒空，不渲染该行） -->
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
