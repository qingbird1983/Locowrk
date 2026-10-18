<script setup lang="ts">
/**
 * 思考行（UI shell doc §3.9，与工具行同吃 ProcessRow 行骨架）：bulb 字形 +
 * 类型「思考」+ 元信息（`思考 · 12 秒`）。
 * **收起态不念思考正文**——正文只在展开体里（用户 2026-10-18 按 ZCode
 * 参考图裁定：收起行是「思考 · 持续了几秒」+ 展开三角，三角 hover 才显形）。
 * 首句摘要仍留着，只当整行的原生 title（悬停可见，不占版面）。
 * 流式中 = 元信息「思考中…」+ 状态位脉动点 + 强制展开；历史重放无时间戳
 * 不虚构，元信息段整个不渲染。
 * 点击展开 = 灰底块内的完整文本（12.5px / 1.55，`--lw-bg-inset` + `--lw-r-sm`，
 * 220px 框内滚动；v2.97 的框约束不变，灰底块取代缩进竖线见设计稿 §7 /
 * AC-7）；流式强制展开 + 自动跟随滚动、收口即折叠（v2.99）不变——只改
 * 展示层，折叠语义零改动。
 */
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import ProcessRow from "./ProcessRow.vue";
import { formatDuration } from "../../lib/format";

const props = defineProps<{
  text: string;
  /** 实时到达时刻（Date.now()）；历史重放无此项 → 回落首句摘要。 */
  ts?: number;
  /** 下一条过程项到达时刻（chat store 收口）；缺 = 还在思考中。 */
  doneTs?: number;
}>();
const { t } = useI18n();

/** 流式中 = 当前动作（L3）：强制展开，收口即折叠。 */
const streaming = computed(
  () => props.ts !== undefined && props.doneTs === undefined,
);
const manual = ref(false);
const expanded = computed(() => streaming.value || manual.value);
function onToggle() {
  manual.value = !expanded.value;
}

/** 摘要态描述：剥 markdown 记号取首句（单行省略交给栅格描述列）。 */
const summary = computed(() => {
  const flat = props.text
    .replace(/[*`_#>]/g, "")
    .split(/\n+/)
    .map((s) => s.trim())
    .filter(Boolean);
  return flat[0] ?? "";
});

/** 元信息段（行壳自带给「·」分隔）：流式中 = 「思考中…」；实时已收口 = 时长；
 *  历史重放无时间戳不虚构 = 空（此时元信息段整个不渲染）。 */
const metaText = computed(() => {
  if (streaming.value) return t("tool.thinkingNow");
  if (props.ts === undefined) return "";
  const sec = Math.max(1, Math.round(((props.doneTs ?? Date.now()) - props.ts) / 1000));
  return formatDuration(sec * 1000);
});

/** 首句摘要只在**重放**时占摘要段：那条路径的落盘投影（`Thinking` 事件）
 *  不带时间戳，算不出时长，总得让行上有字；实时轮一律用时长，不露思考正文
 *  （用户 2026-10-18 按 ZCode 参考图裁定）。 */
const showSummary = computed(() => props.ts === undefined && summary.value !== "");

// 流式跟随：delta 追加后贴底，用户上滚（距底 > 32px）即让位不再拽
const bodyEl = ref<HTMLElement | null>(null);
watch(
  () => props.text,
  () => {
    const el = bodyEl.value;
    if (!streaming.value || !el) return;
    const nearBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 32;
    if (nearBottom) el.scrollTop = el.scrollHeight;
  },
);
</script>

<template>
  <div class="thinking">
    <ProcessRow icon="bulb" :type="t('tool.type.thinking')" :expanded="expanded" :foldable="!streaming" :title="summary" @toggle="onToggle">
      <template v-if="showSummary" #desc>{{ summary }}</template>
      <template v-if="metaText" #meta>{{ metaText }}</template>
      <template v-if="streaming" #status>
        <span class="dot lw-pulse" />
      </template>
    </ProcessRow>
    <!-- 展开体：缩进 + 左竖线（§3.9 v4.13，无卡片底）；v-show 保留 DOM，
         流式自动跟随（上方 watcher） -->
    <div v-show="expanded" ref="bodyEl" class="thinking__body">{{ text }}</div>
  </div>
</template>

<style scoped>
/* v4.14：层 2 行 ↔ 层 3 展开体 = 8px，父容器 flex+gap（margin 撤） */
.thinking {
  display: flex;
  flex-direction: column;
  gap: var(--lw-gap-8);
  min-width: 0;
}
/* 运行脉动点 = 层 2 状态字形，跟随次要色（全局 .dot 底 ink-4） */
.thinking :deep(.dot) {
  background: var(--lw-ink-2);
}
.thinking__body {
  /* 设计稿 §7 + AC-7：思考展开体 = 灰底块（井类 = --lw-bg-inset + r-sm），
     不用缩进竖线——竖线留给工具明细的子行 */
  padding: var(--lw-gap-8) var(--lw-gap-12);
  border-radius: var(--lw-r-sm);
  background: var(--lw-bg-inset);
  font-size: var(--lw-fs-12half);
  line-height: 1.55;
  color: var(--lw-ink-2);
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  /* 框约束（v2.97 目检裁定：思考内容可以很长，展开体封顶框内滚动，
   * 不整段铺开）；220px 与明细文本块同族同值 */
  max-height: 220px;
  overflow-y: auto;
}
</style>
