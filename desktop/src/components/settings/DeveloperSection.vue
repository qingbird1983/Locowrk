<script setup lang="ts">
/**
 * 设置分区：开发者选项（⑫，settings.json `tools` 区 + 暴露调试面板；
 * exposure 文档 §6.3/§9.6）。上半是截断设置（开关 + 每请求最大工具数，
 * 保存即热生效）；下半读最近一次请求的暴露快照——只在打开分区时拉取
 * （§8.8"用户来找"的信息，不推送不轮询），节省 token 按 §3.5 走
 * compactNumber 档位，不显示机器精度原始值。
 */
import { computed, onMounted, reactive, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { pickDirectory, toolExposureDebug } from "../../lib/api";
import { compactNumber } from "../../lib/format";
import { useSettingsStore } from "../../stores/settings";
import type { ExposureDebug } from "../../types";

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
  <section class="space-y-3 rounded-xl border border-line p-4">
    <h3 class="text-xs font-semibold">{{ t("settings.developerTitle") }}</h3>
    <p class="text-[11px] text-ink-4">{{ t("settings.developerHint") }}</p>

    <label class="flex cursor-pointer items-center gap-2 text-xs">
      <input v-model="form.enabled" type="checkbox" class="accent-emerald-500" />
      <span>{{ t("settings.toolsEnabled") }}</span>
    </label>

    <label class="block space-y-1 text-xs">
      <span class="text-ink-3">{{ t("settings.toolsMax") }}</span>
      <input
        v-model.number="form.maxPerRequest"
        type="number"
        min="1"
        max="200"
        class="block w-32 rounded-lg border border-line-strong bg-surface-1 px-2 py-1.5"
      />
      <span class="text-[11px] text-ink-5">{{ t("settings.toolsMaxHint") }}</span>
    </label>

    <!-- S2（清单 §3 约束最小化）：宿主单次工具调用超时可配 -->
    <label class="block space-y-1 text-xs">
      <span class="text-ink-3">{{ t("settings.callTimeout") }}</span>
      <input
        v-model.number="form.callTimeoutSecs"
        type="number"
        min="5"
        max="3600"
        class="block w-32 rounded-lg border border-line-strong bg-surface-1 px-2 py-1.5"
      />
      <span class="text-[11px] text-ink-5">{{ t("settings.callTimeoutHint") }}</span>
    </label>

    <div class="flex items-center gap-2">
      <button
        class="rounded-lg bg-surface-2 px-4 py-1.5 text-xs font-semibold transition hover:bg-surface-3 disabled:cursor-not-allowed disabled:opacity-40"
        :disabled="settings.saving"
        @click="save"
      >
        {{ t("settings.save") }}
      </button>
      <span v-if="settings.savedFlash" class="text-xs text-emerald-400">
        {{ t("settings.saved") }}
      </span>
    </div>

    <!-- ⑱ 工作区目录（fs roots）：增删即存即生效，不走上方"保存" -->
    <div class="space-y-2 border-t border-line pt-3">
      <div class="flex items-center justify-between">
        <span class="text-xs font-semibold text-ink-2">{{ t("settings.fsRootsTitle") }}</span>
        <button
          class="rounded-lg border border-line-strong px-2 py-1 text-[11px] text-ink-3 transition hover:bg-surface-1"
          :disabled="settings.saving"
          @click="addRoot"
        >
          {{ t("settings.fsRootsAdd") }}
        </button>
      </div>
      <p class="text-[11px] text-ink-5">{{ t("settings.fsRootsHint") }}</p>

      <p v-if="!fsRoots.length" class="text-[11px] text-ink-4">
        {{ t("settings.fsRootsEmpty") }}
      </p>
      <ul v-else class="space-y-1">
        <li
          v-for="(root, i) in fsRoots"
          :key="root"
          class="flex items-center justify-between gap-2 rounded-lg bg-surface-1 px-2 py-1.5"
        >
          <span class="break-all text-[11px] text-ink-2">{{ root }}</span>
          <button
            class="shrink-0 rounded px-1.5 text-[11px] text-ink-4 transition hover:bg-surface-2 hover:text-red-400"
            :disabled="settings.saving"
            :title="root"
            @click="removeRoot(i)"
          >
            ✕
          </button>
        </li>
      </ul>
    </div>

    <!-- 暴露调试面板：最近一次请求的暴露分布（只读，手动刷新） -->
    <div class="space-y-2 border-t border-line pt-3">
      <div class="flex items-center justify-between">
        <span class="text-xs font-semibold text-ink-2">{{ t("developer.panelTitle") }}</span>
        <button
          class="rounded-lg border border-line-strong px-2 py-1 text-[11px] text-ink-3 transition hover:bg-surface-1"
          :disabled="loading"
          @click="refresh"
        >
          {{ t("developer.refresh") }}
        </button>
      </div>

      <p v-if="!hasSnapshot(debug)" class="text-[11px] text-ink-4">
        {{ t("developer.empty") }}
      </p>

      <template v-else-if="debug">
        <p v-if="!debug.enabled" class="text-[11px] text-amber-400">
          {{ t("developer.disabled") }}
        </p>
        <p class="text-xs">
          {{ t("developer.exposed", { exposed: debug.exposed, total: debug.total }) }}
        </p>
        <ul class="space-y-1 text-[11px] text-ink-3">
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
        <p v-if="debug.savedTokensEstimate > 0" class="text-[11px] text-emerald-400">
          {{ t("developer.saved", { n: compactNumber(debug.savedTokensEstimate) }) }}
        </p>
      </template>
    </div>
  </section>
</template>
