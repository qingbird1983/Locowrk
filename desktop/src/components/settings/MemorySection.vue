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
  organizePacing: "batch",
  organizeChunkTokens: 50_000,
  tailStrategy: "boundary",
  tailWindowTurns: 5,
  collapseEnabled: false,
  collapseThresholdPct: 80,
  files: true,
  repoContext: true,
  // S1 刀④b 两枚行为开关（缺省开 = 与加开关之前唯一的形状一致，真源是后端那两个常量）
  globalLayer: true,
  projectLayer: true,
  instructionFiles: true,
  instructionGlobalKib: 100,
  instructionProjectKib: 100,
  factCards: true,
  factCardMax: 3,
  // Q9 学习档（缺省开；四档千分比，缺省 400/250/200/150 = 后端那份真源，
  // 这里只作 form 初值，载入后立即被 settings.view 覆盖）
  learnEnabled: true,
  relevanceWeight: 400,
  strengthWeight: 250,
  freshnessWeight: 200,
  recencyWeight: 150,
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
    form.organizePacing = v.memory.organizePacing;
    form.organizeChunkTokens = v.memory.organizeChunkTokens;
    form.tailStrategy = v.memory.tailStrategy;
    form.tailWindowTurns = v.memory.tailWindowTurns;
    form.collapseEnabled = v.memory.collapseEnabled;
    form.collapseThresholdPct = v.memory.collapseThresholdPct;
    form.files = v.memory.files;
    form.repoContext = v.memory.repoContext;
    form.globalLayer = v.memory.globalLayer;
    form.projectLayer = v.memory.projectLayer;
    form.instructionFiles = v.memory.instructionFiles;
    form.instructionGlobalKib = v.memory.instructionGlobalKib;
    form.instructionProjectKib = v.memory.instructionProjectKib;
    form.factCards = v.memory.factCards;
    form.factCardMax = v.memory.factCardMax;
    form.learnEnabled = v.memory.learnEnabled;
    form.relevanceWeight = v.memory.relevanceWeight;
    form.strengthWeight = v.memory.strengthWeight;
    form.freshnessWeight = v.memory.freshnessWeight;
    form.recencyWeight = v.memory.recencyWeight;
  },
  { immediate: true },
);

function save() {
  void settings.saveMemory({
    enabled: form.enabled,
    topK: form.topK,
    organizePacing: form.organizePacing,
    organizeChunkTokens: Math.round(form.organizeChunkTokens),
    tailStrategy: form.tailStrategy,
    tailWindowTurns: Math.round(form.tailWindowTurns),
    collapseEnabled: form.collapseEnabled,
    collapseThresholdPct: Math.round(form.collapseThresholdPct),
    files: form.files,
    repoContext: form.repoContext,
    globalLayer: form.globalLayer,
    projectLayer: form.projectLayer,
    instructionFiles: form.instructionFiles,
    instructionGlobalKib: Math.round(form.instructionGlobalKib),
    instructionProjectKib: Math.round(form.instructionProjectKib),
    factCards: form.factCards,
    factCardMax: Math.round(form.factCardMax),
    learnEnabled: form.learnEnabled,
    relevanceWeight: Math.round(form.relevanceWeight),
    strengthWeight: Math.round(form.strengthWeight),
    freshnessWeight: Math.round(form.freshnessWeight),
    recencyWeight: Math.round(form.recencyWeight),
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

    <!-- F1 记忆文件化设置项。⚠ 这句是实况，不是旧稿那句话：真源 = md 之后"关 = 回纯
         sqlite"那一档已随 S1 刀②c-1 / 刀③b 消失，这一项现在只决定**启动时对账与首启
         导出那一轮跑不跑**，不决定数据存哪儿（去处 = 刀④c 定改义或退役）。 -->
    <SettingsRow :title="t('settings.memoryFiles')" :hint="t('settings.memoryFilesHint')">
      <SettingsToggle v-model="form.files" :label="t('settings.memoryFiles')" />
    </SettingsRow>
    <!-- 交接层那一枚行为开关**不在这儿显示**：ADR-0007 §18 把三枚行为开关（全局记忆 /
         项目记忆 / 项目交接层）定在同一处，而记忆页（MemoryPage）已经落地那三枚并排。
         同一属性两处开关 = 两处真相，所以这里撤行、`form.repoContext` 仍随 view 回填并
         原样保存（字段没退出数据面，只是不再有两个开关可以拨它）。 -->
    <!-- 指令文件层（提示词分层设计 §4.3）：全局 + 项目三族候选注入消息层头部 -->
    <SettingsRow
      :title="t('settings.memoryInstructionFiles')"
      :hint="t('settings.memoryInstructionFilesHint')"
    >
      <SettingsToggle
        v-model="form.instructionFiles"
        :label="t('settings.memoryInstructionFiles')"
      />
    </SettingsRow>
    <!-- 两条链各一档预算（v2.188 · 队列 Q4）：缺省 100 KiB = 这项可调之前的写死值。
         额度用尽后面的族**不读并在注入里点名**，所以调小是"少注"而不是"静默丢"。 -->
    <SettingsRow
      v-if="form.instructionFiles"
      :title="t('settings.instructionGlobalKib')"
      :hint="t('settings.instructionGlobalKibHint')"
      control-id="memory-instruction-global-kib"
    >
      <input
        id="memory-instruction-global-kib"
        v-model.number="form.instructionGlobalKib"
        type="number"
        min="8"
        max="512"
        class="lw-field lw-field-num px-2 text-xs"
      />
    </SettingsRow>
    <SettingsRow
      v-if="form.instructionFiles"
      :title="t('settings.instructionProjectKib')"
      :hint="t('settings.instructionProjectKibHint')"
      control-id="memory-instruction-project-kib"
    >
      <input
        id="memory-instruction-project-kib"
        v-model.number="form.instructionProjectKib"
        type="number"
        min="8"
        max="512"
        class="lw-field lw-field-num px-2 text-xs"
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
    <!-- Q3 整理事实卡（MR §5 提案 A，缺省开）：整理收束时记录员顺带产出
         ≤N 张事实卡落记忆层；关闭 = 提示词与落库两面都回到不产卡的形状 -->
    <SettingsRow :title="t('settings.factCards')" :hint="t('settings.factCardsHint')">
      <SettingsToggle v-model="form.factCards" :label="t('settings.factCards')" />
    </SettingsRow>
    <SettingsRow
      v-if="form.factCards"
      :title="t('settings.factCardMax')"
      :hint="t('settings.factCardMaxHint')"
      control-id="memory-fact-card-max"
    >
      <input
        id="memory-fact-card-max"
        v-model.number="form.factCardMax"
        type="number"
        min="1"
        max="10"
        class="lw-field lw-field-num px-2 text-xs"
      />
    </SettingsRow>
    <!-- Q9 学习档（ML 稿 §3.3 加权排序 + §3.4 交付即命中）：关掉 = 注入排序逐字节
         回到现役、命中写回一起停（稿 §5.4 的对照档）。四档单位千分比、各自独立，
         **不要求和为 1000**（后端 LEARN_WEIGHT_MAX 的头注给了理由：排序只看相对大小，
         硬约束会逼出"改一个摊平其余三个"的联动机制）。四档全 0 合法 = 回到现役序。
         生效时机：当前会话的注入块已按预取缓存钉死，**改权重下一个会话起效**。 -->
    <SettingsRow :title="t('settings.learnEnabled')" :hint="t('settings.learnEnabledHint')">
      <SettingsToggle v-model="form.learnEnabled" :label="t('settings.learnEnabled')" />
    </SettingsRow>
    <SettingsRow
      v-if="form.learnEnabled"
      :title="t('settings.learnRelevance')"
      :hint="t('settings.learnRelevanceHint')"
      control-id="memory-learn-relevance"
    >
      <input
        id="memory-learn-relevance"
        v-model.number="form.relevanceWeight"
        type="number"
        min="0"
        max="1000"
        class="lw-field lw-field-num px-2 text-xs"
      />
    </SettingsRow>
    <SettingsRow
      v-if="form.learnEnabled"
      :title="t('settings.learnStrength')"
      :hint="t('settings.learnStrengthHint')"
      control-id="memory-learn-strength"
    >
      <input
        id="memory-learn-strength"
        v-model.number="form.strengthWeight"
        type="number"
        min="0"
        max="1000"
        class="lw-field lw-field-num px-2 text-xs"
      />
    </SettingsRow>
    <SettingsRow
      v-if="form.learnEnabled"
      :title="t('settings.learnFreshness')"
      :hint="t('settings.learnFreshnessHint')"
      control-id="memory-learn-freshness"
    >
      <input
        id="memory-learn-freshness"
        v-model.number="form.freshnessWeight"
        type="number"
        min="0"
        max="1000"
        class="lw-field lw-field-num px-2 text-xs"
      />
    </SettingsRow>
    <SettingsRow
      v-if="form.learnEnabled"
      :title="t('settings.learnRecency')"
      :hint="t('settings.learnRecencyHint')"
      control-id="memory-learn-recency"
    >
      <input
        id="memory-learn-recency"
        v-model.number="form.recencyWeight"
        type="number"
        min="0"
        max="1000"
        class="lw-field lw-field-num px-2 text-xs"
      />
    </SettingsRow>
  </SettingsGroup>

  <SettingsSaveRow class="mt-3" @save="save" />
</template>
