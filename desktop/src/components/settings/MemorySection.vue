<script setup lang="ts">
/**
 * 设置分区：记忆注入（settings.json `memory` 区，⑪；Context and
 * Memory.md §6.2）。开关 + 每轮注入条数（1–20，缺省 5）；保存即生效
 * （后端换入 Memory-Inject 活配置，下一轮请求就按新值注入）。
 * 管理入口在顶层"记忆"页（搜索 / 编辑 / 删除 / 清空）。
 * 行版式（§3.32·七 v3.5）：布尔项用 SettingsToggle。
 */
import { reactive, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useSettingsStore } from "../../stores/settings";
import SettingsGroup from "./SettingsGroup.vue";
import SettingsRow from "./SettingsRow.vue";
import SettingsSaveRow from "./SettingsSaveRow.vue";
import SettingsSegmented from "./SettingsSegmented.vue";
import SettingsToggle from "./SettingsToggle.vue";

const { t } = useI18n();
const settings = useSettingsStore();

const form = reactive({
  enabled: true,
  topK: 5,
  autoExtract: false,
  organizePacing: "batch",
  organizeChunkTokens: 50_000,
  tailStrategy: "boundary",
  tailWindowTurns: 5,
  collapseEnabled: false,
  collapseThresholdPct: 80,
  files: true,
  repoContext: true,
});

// M2 整理节奏二档（树 v2 §4.3）：分段控件全局规范（§3.32·七）
const pacingOptions = [
  { value: "batch", label: t("settings.organizePacingBatch") },
  { value: "incremental", label: t("settings.organizePacingIncremental") },
];

// M4 尾巴策略二档（树 v2 §7.2：A 边界结算缺省 / B 固定窗口实验档）
const tailOptions = [
  { value: "boundary", label: t("settings.tailStrategyBoundary") },
  { value: "window", label: t("settings.tailStrategyWindow") },
];

watch(
  () => settings.view,
  (v) => {
    if (!v) return;
    form.enabled = v.memory.enabled;
    form.topK = v.memory.topK;
    form.autoExtract = v.memory.autoExtract;
    form.organizePacing = v.memory.organizePacing;
    form.organizeChunkTokens = v.memory.organizeChunkTokens;
    form.tailStrategy = v.memory.tailStrategy;
    form.tailWindowTurns = v.memory.tailWindowTurns;
    form.collapseEnabled = v.memory.collapseEnabled;
    form.collapseThresholdPct = v.memory.collapseThresholdPct;
    form.files = v.memory.files;
    form.repoContext = v.memory.repoContext;
  },
  { immediate: true },
);

function save() {
  void settings.saveMemory({
    enabled: form.enabled,
    topK: form.topK,
    autoExtract: form.autoExtract,
    organizePacing: form.organizePacing,
    organizeChunkTokens: Math.round(form.organizeChunkTokens),
    tailStrategy: form.tailStrategy,
    tailWindowTurns: Math.round(form.tailWindowTurns),
    collapseEnabled: form.collapseEnabled,
    collapseThresholdPct: Math.round(form.collapseThresholdPct),
    files: form.files,
    repoContext: form.repoContext,
  });
}
</script>

<template>
  <SettingsGroup :hint="t('settings.memoryHint')">
    <SettingsRow :title="t('settings.memoryEnabled')">
      <SettingsToggle v-model="form.enabled" :label="t('settings.memoryEnabled')" />
    </SettingsRow>

    <SettingsRow
      :title="t('settings.memoryTopK')"
      :hint="t('settings.memoryTopKHint')"
      control-id="memory-topk"
    >
      <input
        id="memory-topk"
        v-model.number="form.topK"
        type="number"
        min="1"
        max="20"
        class="lw-field lw-field-num px-2 text-xs"
      />
    </SettingsRow>

    <!-- ⑬ 隐式提取：缺省关（易记垃圾），开启后回合结束异步提事实+经历 -->
    <SettingsRow
      :title="t('settings.memoryAutoExtract')"
      :hint="t('settings.memoryAutoExtractHint')"
    >
      <SettingsToggle v-model="form.autoExtract" :label="t('settings.memoryAutoExtract')" />
    </SettingsRow>

    <!-- F1 记忆文件化（文件化设计 §7）：关 = 回纯 sqlite，md 不再读写 -->
    <SettingsRow :title="t('settings.memoryFiles')" :hint="t('settings.memoryFilesHint')">
      <SettingsToggle v-model="form.files" :label="t('settings.memoryFiles')" />
    </SettingsRow>
    <SettingsRow
      :title="t('settings.memoryRepoContext')"
      :hint="t('settings.memoryRepoContextHint')"
    >
      <SettingsToggle
        v-model="form.repoContext"
        :label="t('settings.memoryRepoContext')"
      />
    </SettingsRow>

    <!-- M2 记忆树整理（树 v2 §4.3/§15）：节奏二档 + 增量块 X（增量档显示） -->
    <SettingsRow
      :title="t('settings.organizePacing')"
      :hint="t('settings.organizePacingHint')"
    >
      <SettingsSegmented
        v-model="form.organizePacing"
        :options="pacingOptions"
        :label="t('settings.organizePacing')"
      />
    </SettingsRow>
    <SettingsRow
      v-if="form.organizePacing === 'incremental'"
      :title="t('settings.organizeChunk')"
      :hint="t('settings.organizeChunkHint')"
      control-id="memory-organize-chunk"
    >
      <input
        id="memory-organize-chunk"
        v-model.number="form.organizeChunkTokens"
        type="number"
        min="10000"
        max="200000"
        step="5000"
        class="lw-field lw-field-num px-2 text-xs"
      />
    </SettingsRow>

    <!-- M4 尾巴策略（树 v2 §7.2，只作用树模式会话）：A 边界结算缺省 /
         B 固定窗口实验档；窗口档显示 N（1–20 缺省 5） -->
    <SettingsRow
      :title="t('settings.tailStrategy')"
      :hint="t('settings.tailStrategyHint')"
    >
      <SettingsSegmented
        v-model="form.tailStrategy"
        :options="tailOptions"
        :label="t('settings.tailStrategy')"
      />
    </SettingsRow>
    <SettingsRow
      v-if="form.tailStrategy === 'window'"
      :title="t('settings.tailWindow')"
      :hint="t('settings.tailWindowHint')"
      control-id="memory-tail-window"
    >
      <input
        id="memory-tail-window"
        v-model.number="form.tailWindowTurns"
        type="number"
        min="1"
        max="20"
        class="lw-field lw-field-num px-2 text-xs"
      />
    </SettingsRow>

    <!-- M4 memory_collapse（§8.3 缺省关——空间急救不是省钱）；开启时
         显示自动收起阈值（50–95% 缺省 80） -->
    <SettingsRow
      :title="t('settings.collapseEnabled')"
      :hint="t('settings.collapseEnabledHint')"
    >
      <SettingsToggle v-model="form.collapseEnabled" :label="t('settings.collapseEnabled')" />
    </SettingsRow>
    <SettingsRow
      v-if="form.collapseEnabled"
      :title="t('settings.collapseThreshold')"
      :hint="t('settings.collapseThresholdHint')"
      control-id="memory-collapse-threshold"
    >
      <input
        id="memory-collapse-threshold"
        v-model.number="form.collapseThresholdPct"
        type="number"
        min="50"
        max="95"
        class="lw-field lw-field-num px-2 text-xs"
      />
    </SettingsRow>
  </SettingsGroup>

  <SettingsSaveRow class="mt-3" @save="save" />
</template>
