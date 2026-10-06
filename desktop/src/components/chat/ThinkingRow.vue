<script setup lang="ts">
/**
 * 思考行（UI shell doc §3.9，v4.11 照 ZCode 过程流基准）：sparkle +
 * 「思考」+ 时长（`持续了 N 秒`——实时轮记 ts、下一条过程项到达时收口
 * doneTs；历史重放无时间戳 = 回落首句摘要单行截断，不虚构时长）；点击
 * 展开为完整文本（12.5px / 1.55 / ink-2，wash-1 底圆角 10，220px 框内
 * 滚动，v2.97 裁定）。
 * v2.99 裁定（层层折叠）：当前动作在 3 层展开——流式思考（ts 在、
 * doneTs 未收口）强制展开且框内自动跟随滚动；回复/工具到达收口即折叠
 * （L3→L2），此后回到手动开合。
 */
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Icon } from "../../lib/icons";

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

/** 摘要态：有实时时戳 = 时长；否则剥 markdown 记号取首句。 */
const summary = computed(() => {
  if (props.ts !== undefined) {
    const sec = Math.max(
      1,
      Math.round(((props.doneTs ?? Date.now()) - props.ts) / 1000),
    );
    return t("turn.thinkingFor", { s: sec });
  }
  const flat = props.text
    .replace(/[*`_#>]/g, "")
    .split(/\n+/)
    .map((s) => s.trim())
    .filter(Boolean);
  return flat[0] ?? "";
});

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
    <button
      class="cv-head thinking__header"
      type="button"
      :aria-expanded="expanded"
      @click="onToggle"
    >
      <Icon name="sparkle" :size="14" class="shrink-0 text-ink-3" />
      <span class="cv-head__label">{{ t("turn.thinking") }}</span>
      <span class="thinking__clip">{{ summary }}</span>
      <Icon
        :name="expanded ? 'chevronDown' : 'chevronRight'"
        :size="12"
        class="cv-head__chev"
      />
    </button>
    <!-- 展开体：v-show 保留 DOM（§3.8 折叠铁则），文本 pre-wrap 原样 -->
    <div v-show="expanded" ref="bodyEl" class="thinking__body">{{ text }}</div>
  </div>
</template>

<style scoped>
.thinking__header {
  font-size: var(--lw-fs-12half);
}
.thinking__clip {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--lw-ink-2);
}
.thinking__body {
  margin: var(--lw-gap-4) 0 var(--lw-gap-6);
  border-radius: var(--lw-r-sm);
  background: var(--lw-wash-1);
  padding: var(--lw-gap-10) var(--lw-gap-12);
  font-size: var(--lw-fs-12half);
  line-height: 1.55;
  color: var(--lw-ink-2);
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  /* 框约束（v2.97 目检裁定：思考内容可以很长，展开体封顶框内滚动，
   * 不整段铺开）；220px 与工具明细 pre 同族同值 */
  max-height: 220px;
  overflow-y: auto;
}
</style>
