<script setup lang="ts">
/**
 * 设置分区：上下文管理（settings.json `context` 区，⑨）。v2.137（用户
 * 裁定「上下文根本不压缩了」）：整理阈值（绝对 token，100K–800K 每 100K
 * 一档）——对话上下文达到该 token 数触发记忆树整理；旧「压缩阈值（百分比）
 * / 压缩策略 / 稳定期」随 ctx-compact 桌面链退役。Q8（CB §7.4）加第二根
 * 旋钮 = 工具结果预算（KiB，0 = 不限制）：它挡的是「一条工具输出灌进上下文」
 * 这件事，与阈值管「多久整理一次」是两回事，所以本区不再只有一根旋钮。
 * 滑动时气泡跟随显示当前档位（100K…800K）。保存即生效（下一轮触发判定
 * 读设置，runner UsagePolicy 同源；预算走 llm 组装点 → wire 发送侧）。
 * S2 刀③ 加第三根旋钮 = 微压缩开关（context.microEnabled，缺省开）：它管的不是
 * 「一条多大」（预算）也不是「多久整理」（阈值），而是「很旧的工具输出要不要
 * 先换成一行占位」——同一区各挡一件事。布尔一律开关（§3.32·七）。
 * S2 刀④c 加第四根旋钮 = 深度压缩开关（context.compactEnabled，缺省开）：占用到
 * 有效阈值时把**较旧的一段**交给模型写成一份七节摘要、近期原文保持原样。它是三层
 * 里唯一"一段原文换一份摘要"的一层，所以必须有可关的那一格（可配置原则②）；同区
 * 另四枚数值/文本（目标比、保留额度、摘要模型、自定义指令）后端已可读可写，
 * 界面行按刀序归刀⑥——这一节先把"要不要动"这一格交到人手上。
 * 行版式（§3.32·七 v3.5）：说明文字走行左 #detail。
 * S2 刀⑥ 补齐这一区的**数值档**：目标比、近期原文额度、微压缩保留组数、滑窗保留段数
 * 四枚数字输入（与上面「工具结果预算」同一种 `.lw-field-num` 紧凑档，不新造形态），
 * 每枚**紧跟它所要调的那枚开关**排布——一层一组的读法是"这层动不动 → 这层动多狠"。
 * 两枚后端字段（摘要模型 / 自定义压缩指令）没在这一节出现：前者与「记忆管理器」是
 * 同一类东西（后台角色用哪个模型），按"同类只许一种形态"跟着它住进模型分区；后者要
 * 设置页多行输入框这一**新形态**，按 §4 得先改标准再写码，已登记为待批。
 * S3 刀④b 加第五根数值旋钮 = 输出预留上限（context.outputReserveCapTokens，缺省 21K，
 * 0 = 不为输出让位）：有效阈值里窗口那侧先扣掉它（实际预留还会与模型输出上限取小），
 * 调它就是调"压缩点离窗口多近"。界面上那颗数的出处由用量浮层的扣减链写明。
 */
import { computed, reactive, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useSettingsStore } from "../../stores/settings";
import SettingsGroup from "./SettingsGroup.vue";
import SettingsRow from "./SettingsRow.vue";
import SettingsSaveRow from "./SettingsSaveRow.vue";
import SettingsToggle from "./SettingsToggle.vue";

const { t } = useI18n();
const settings = useSettingsStore();

const MIN = 100_000;
const MAX = 800_000;
const STEP = 100_000;

const form = reactive({
  thresholdTokens: 100_000,
  usageBar: true,
  // Q8：工具结果预算（KiB，0 = 不限制）。原样传后端，前端不换算成字节
  toolResultBudgetKib: 24,
  // S3 刀④b：输出预留上限（token，缺省 = 改前常量 21K，0 = 不为输出让位）。
  // 镜像后端 `OUTPUT_RESERVE_CAP_DEFAULT`，不在这里另算有效阈值。
  outputReserveCapTokens: 21_000,
  // S2 刀③：微压缩开关（缺省开）。前端只传布尔，判据全在后端
  microEnabled: true,
  // S2 刀④c：深度压缩开关（缺省开）。同上——这一层的判据在 compact 侧
  compactEnabled: true,
  // S2 刀⑥：四枚数值档。缺省照后端 `ContextSettings::default` 那一份（目标比 50 /
  // 近期原文 15000 / 微压缩保留 5 组 / 滑窗保留 1 段）——**照生产抄，不照想象编**：
  // 这里写错一个数，投影未到之前界面就会替用户"预填"一个后端不认的缺省。
  compactTargetPct: 50,
  compactKeepTokens: 15_000,
  microKeepGroups: 5,
  slideKeepRuns: 1,
});

watch(
  () => settings.view,
  (v) => {
    if (!v) return;
    form.thresholdTokens = v.context.thresholdTokens;
    form.usageBar = v.context.usageBar;
    form.toolResultBudgetKib = v.context.toolResultBudgetKib;
    form.outputReserveCapTokens = v.context.outputReserveCapTokens;
    form.microEnabled = v.context.microEnabled;
    form.compactEnabled = v.context.compactEnabled;
    form.compactTargetPct = v.context.compactTargetPct;
    form.compactKeepTokens = v.context.compactKeepTokens;
    form.microKeepGroups = v.context.microKeepGroups;
    form.slideKeepRuns = v.context.slideKeepRuns;
  },
  { immediate: true },
);

/** 气泡文本与说明行的档位文本（100K…800K）。 */
const thresholdText = computed(() => `${Math.round(form.thresholdTokens / 1000)}K`);

// 气泡跟随：滑条拖动/悬停时显示，left 按值域百分比折算（thumb 半径用
// 固定补偿即可，800K 值域下误差 ≤ 半档；两端向内钳 10% 防出框）。
const dragging = ref(false);
const bubbleLeft = computed(() => {
  const pct = (form.thresholdTokens - MIN) / (MAX - MIN);
  return `${10 + pct * 80}%`;
});

function save() {
  void settings.saveContext({
    thresholdTokens: Math.round(form.thresholdTokens),
    usageBar: form.usageBar,
    // 输入框被清空时 v-model.number 给 NaN → JSON 序列化成 null → 后端按
    // None 处理（保持不变，不是存 0）。0 只能来自用户明确填 0 = 不限制。
    toolResultBudgetKib: Math.round(form.toolResultBudgetKib),
    // S3 刀④b：与上面那枚同一条纪律——原样发后端的 token 数，前端不折算。
    outputReserveCapTokens: Math.round(form.outputReserveCapTokens),
    microEnabled: form.microEnabled,
    compactEnabled: form.compactEnabled,
    // 刀⑥ 四枚数值档，仍然守同一条纪律：**只发界面上真渲染过的键**。摘要模型与自定义
    // 指令两枚依旧不发（前者住进模型分区、后者等新形态批准），发过去等于替用户把他
    // 没看过的值写一遍——`ContextUpdate` 里它们是 optional，不传即不动。
    compactTargetPct: Math.round(form.compactTargetPct),
    compactKeepTokens: Math.round(form.compactKeepTokens),
    microKeepGroups: Math.round(form.microKeepGroups),
    slideKeepRuns: Math.round(form.slideKeepRuns),
  });
}
</script>

<template>
  <SettingsGroup :hint="t('settings.contextHint')">
    <!-- 整理阈值滑条（100K–800K，100K 一档）：滑动时气泡跟随显示当前
         档位；说明走左列 #detail（窗口红线 + 手动 `/压缩` 不受限） -->
    <SettingsRow :title="t('settings.threshold')" wide>
      <template #detail>
        <p class="mt-1 text-xs text-ink-3">
          {{ t("settings.thresholdValue", { n: thresholdText }) }}
        </p>
        <p class="mt-0.5 text-[length:var(--lw-fs-11)] text-ink-4">
          {{ t("settings.thresholdHint") }}
        </p>
      </template>
      <div class="relative pt-7">
        <span
          v-show="dragging"
          class="pointer-events-none absolute top-0 -translate-x-1/2 rounded-[var(--lw-r-sm)] border border-line-subtle bg-elevated px-1.5 py-0.5 text-[length:var(--lw-fs-11)] text-ink-1 shadow-raised"
          :style="{ left: bubbleLeft }"
        >
          {{ thresholdText }}
        </span>
        <input
          v-model.number="form.thresholdTokens"
          type="range"
          :min="MIN"
          :max="MAX"
          :step="STEP"
          class="w-48 accent-[var(--lw-slider-accent)]"
          :aria-label="t('settings.threshold')"
          @pointerdown="dragging = true"
          @pointerup="dragging = false"
          @pointercancel="dragging = false"
          @blur="dragging = false"
        />
      </div>
    </SettingsRow>

    <!-- 输出预留上限（S3 刀④b · AC-S3-8）：有效阈值 = min(滑杆, 窗口 − 预留)，
         这一枚是预留的上限；实际预留还会被模型声明的输出上限削平（取小）。
         数值档与工具结果预算同形（`.lw-field-num`，不换算、不重算阈值）。
         min/max 是后端 `OUTPUT_RESERVE_CAP_RANGE` 的手抄镜像（与下面几枚同一
         条既有处境）：不同步的后果是界面让人填一个后端会拒的值，拒得如实。 -->
    <SettingsRow
      :title="t('settings.outputReserveCap')"
      :hint="t('settings.outputReserveCapHint')"
      control-id="context-output-reserve-cap"
    >
      <input
        id="context-output-reserve-cap"
        v-model.number="form.outputReserveCapTokens"
        type="number"
        min="0"
        max="64000"
        step="1000"
        class="lw-field lw-field-num px-2 text-xs"
      />
    </SettingsRow>

    <!-- Q8 工具结果预算（CB §7.4）：一条工具输出进上下文的上限，0 = 不限制。
         数值档与 Q4 的 instruction*Kib 同形（数字输入 + min/max，不换算字节） -->
    <SettingsRow
      :title="t('settings.toolBudgetKib')"
      :hint="t('settings.toolBudgetKibHint')"
      control-id="context-tool-budget-kib"
    >
      <input
        id="context-tool-budget-kib"
        v-model.number="form.toolResultBudgetKib"
        type="number"
        min="0"
        max="512"
        step="8"
        class="lw-field lw-field-num px-2 text-xs"
      />
    </SettingsRow>

    <!-- 微压缩开关（S2 刀③）：旧工具结果清成一行占位，先于整理触发。
         布尔项一律开关（§3.32·七），说明走 #detail 与进度条那行同形 -->
    <SettingsRow :title="t('settings.microCompact')">
      <template #detail>
        <p class="mt-1 text-xs text-ink-3">{{ t("settings.microCompactHint") }}</p>
      </template>
      <SettingsToggle v-model="form.microEnabled" :label="t('settings.microCompact')" />
    </SettingsRow>

    <!-- 微压缩保留组数（刀⑥）：紧跟它所要调的那枚开关排——一层的读法是"动不动 →
         动多狠"。数值档仍用 `.lw-field-num` 紧凑形态（与工具结果预算同一族）。
         ⚠ min/max 是后端 `MICRO_KEEP_GROUPS_RANGE` 的**手抄镜像**（前端拿不到 Rust
         常量，与上面那枚预算行同一条既有处境）：改后端区间要同步这里，不同步的后果
         是界面让人填一个后端会拒的值——拒得如实（错误消息带范围），不是静默钳制。 -->
    <SettingsRow
      :title="t('settings.microKeepGroups')"
      :hint="t('settings.microKeepGroupsHint')"
      control-id="context-micro-keep-groups"
    >
      <input
        id="context-micro-keep-groups"
        v-model.number="form.microKeepGroups"
        type="number"
        min="1"
        max="50"
        step="1"
        class="lw-field lw-field-num px-2 text-xs"
      />
    </SettingsRow>

    <!-- 深度压缩开关（S2 刀④c）：到线时把较旧的一段换成一份七节摘要。
         与上面那枚的分工要说得出口：微压缩只清"很旧的工具输出"（不动提问与回答），
         这一层才真的把一段原文换成摘要 ⇒ 必须有独立的可关那一格 -->
    <SettingsRow :title="t('settings.deepCompact')">
      <template #detail>
        <p class="mt-1 text-xs text-ink-3">{{ t("settings.deepCompactHint") }}</p>
      </template>
      <SettingsToggle v-model="form.compactEnabled" :label="t('settings.deepCompact')" />
    </SettingsRow>

    <!-- 压缩目标比（刀⑥）：这一成**是我们自己的裁定**（两家参照都没有这颗数），
         所以它必须看得见、改得动——界面说明里不假装照抄谁。 -->
    <SettingsRow
      :title="t('settings.compactTargetPct')"
      :hint="t('settings.compactTargetPctHint')"
      control-id="context-compact-target-pct"
    >
      <input
        id="context-compact-target-pct"
        v-model.number="form.compactTargetPct"
        type="number"
        min="1"
        max="100"
        step="5"
        class="lw-field lw-field-num px-2 text-xs"
      />
    </SettingsRow>

    <!-- 近期原文额度（刀⑥）：与上面那枚是一对——共同决定"压完窗口里剩什么"。
         0 = 全交给摘要（不留一句原话），所以这一档允许 0，不等同于"关掉"。 -->
    <SettingsRow
      :title="t('settings.compactKeepTokens')"
      :hint="t('settings.compactKeepTokensHint')"
      control-id="context-compact-keep-tokens"
    >
      <input
        id="context-compact-keep-tokens"
        v-model.number="form.compactKeepTokens"
        type="number"
        min="0"
        max="400000"
        step="1000"
        class="lw-field lw-field-num px-2 text-xs"
      />
    </SettingsRow>

    <!-- 滑窗保留段数（刀⑥ 销刀⑤ 挂账 ⓕ）：这一层**没有自己的开关**——它只在主层
         连败三次之后自动醒，所以界面上只有"醒过来之后留几段"这一枚可调。
         说明要把"什么时候才轮到它"讲清，否则用户会以为调它就能少压。 -->
    <SettingsRow
      :title="t('settings.slideKeepRuns')"
      :hint="t('settings.slideKeepRunsHint')"
      control-id="context-slide-keep-runs"
    >
      <input
        id="context-slide-keep-runs"
        v-model.number="form.slideKeepRuns"
        type="number"
        min="1"
        max="20"
        step="1"
        class="lw-field lw-field-num px-2 text-xs"
      />
    </SettingsRow>

    <!-- 阈值用量进度条显隐（§3.4 v4.18；v2.137 改名——分母 = 整理阈值；
         布尔项一律开关 §3.32·七） -->
    <SettingsRow :title="t('settings.usageBar')">
      <template #detail>
        <p class="mt-1 text-xs text-ink-3">{{ t("settings.usageBarHint") }}</p>
      </template>
      <SettingsToggle v-model="form.usageBar" :label="t('settings.usageBar')" />
    </SettingsRow>
  </SettingsGroup>

  <SettingsSaveRow class="mt-3" @save="save" />
</template>
