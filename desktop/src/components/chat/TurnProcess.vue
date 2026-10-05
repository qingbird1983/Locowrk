<script setup lang="ts">
/**
 * 轮次过程组（UI shell doc §3.7）：一轮助手的"过程"整体折叠成一行，只留
 * 结论在外面。头行 = sparkle + 状态词 + · 时长 + · N 个工具（有失败加
 * ⓘ N 个问题）+ chevron（只换字形，不做旋转过渡）；hover 填充向列外溢
 * 3px；展开体左缩进 + 1px 引导线，内含思考行 / 工具行 / 子 Agent 卡——
 * 系统通知、助手正文、错误行是组的兄弟节点，不进组。
 *
 * 折叠是本地 UI 状态：不入 store、不持久化；默认一律收起（含运行中，
 * 用户 2026-10-01 裁定）；defaultExpanded 只在挂载时生效一次（「默认展开
 * 工具执行过程」开关不回流已渲染的组）。时长只在实时轮次显示（历史重放
 * 无时间戳，不虚构时序）。展开/收起动效 = §6.6 ④（grid-rows 滑动 +
 * 内容 40ms 延后，结束后摘 overflow）。
 */
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Icon } from "../../lib/icons";

const props = defineProps<{
  running: boolean;
  /** v4.11 §3.39④：本轮被用户停止（收束后头行「已停止」，重放恒否）。 */
  stopped?: boolean;
  toolCount: number;
  failures: number;
  /** 组内最早的过程项时刻；undefined = 历史重放（不显示时长）。 */
  startTs?: number;
  defaultExpanded?: boolean;
}>();

const { t } = useI18n();
const expanded = ref(props.defaultExpanded ?? false);
/** 展开动画结束后才摘 overflow（否则聚焦环/引导线被裁）。 */
const settled = ref(false);

function onToggle() {
  if (expanded.value) settled.value = false; // 收起即刻收回 overflow
  expanded.value = !expanded.value;
}
function onTransitionEnd(e: TransitionEvent) {
  if (e.propertyName !== "grid-template-rows") return;
  if (expanded.value) settled.value = true;
}

// 运行中的头行时长每秒走字（运行结束停在收束时刻）
const now = ref(Date.now());
const finishedAt = ref<number | null>(null);
let timer: number | null = null;
watch(
  () => props.running,
  (r, prev) => {
    if (prev && !r && finishedAt.value === null) finishedAt.value = Date.now();
    if (r && props.startTs !== undefined && timer === null) {
      now.value = Date.now();
      timer = window.setInterval(() => (now.value = Date.now()), 1000);
    } else if (!r && timer !== null) {
      window.clearInterval(timer);
      timer = null;
    }
  },
  { immediate: true },
);
onBeforeUnmount(() => {
  if (timer !== null) window.clearInterval(timer);
});

const durationText = computed(() => {
  if (props.startTs === undefined) return "";
  const end = finishedAt.value ?? (props.running ? now.value : null);
  if (end === null) return "";
  const sec = Math.max(0, Math.round((end - props.startTs) / 1000));
  if (sec < 60) return `${sec}s`;
  if (sec < 3600) return `${Math.floor(sec / 60)}m ${sec % 60}s`;
  return `${Math.floor(sec / 3600)}h ${Math.floor((sec % 3600) / 60)}m`;
});

const metaText = computed(() => {
  const parts = [
    durationText.value,
    t("turn.toolCount", { n: props.toolCount }),
  ].filter(Boolean);
  return `· ${parts.join(" · ")}`;
});
</script>

<template>
  <div class="turn">
    <button
      class="cv-head"
      type="button"
      :aria-expanded="expanded"
      @click="onToggle"
    >
      <Icon name="sparkle" :size="14" class="shrink-0 text-ink-3" />
      <span class="cv-head__label">
        {{ running ? t("turn.running") : stopped ? t("turn.stopped") : t("turn.done") }}
      </span>
      <span class="cv-head__meta">{{ metaText }}</span>
      <span
        v-if="failures > 0"
        class="cv-head__meta inline-flex items-center gap-1"
      >
        <Icon name="info" :size="12" />
        {{ t("turn.problemCount", { n: failures }) }}
      </span>
      <Icon
        :name="expanded ? 'chevronDown' : 'chevronRight'"
        :size="12"
        class="cv-head__chev"
      />
    </button>
    <!-- §6.6 ④：grid-rows 0fr→1fr 滑动；内容 opacity 展开延后 40ms -->
    <div
      class="turn__grid"
      :class="{ 'is-open': expanded, 'is-settled': settled }"
      @transitionend="onTransitionEnd"
    >
      <div class="turn__body"><slot /></div>
    </div>
  </div>
</template>

<style scoped>
.turn {
  padding: var(--lw-gap-4) 0 var(--lw-gap-10);
}
.turn__grid {
  display: grid;
  /* v2.74·fix（目检实抓，CDP 实测 body 1693 vs 容器 560）：隐式 auto 列轨
   * 按内容 max-content 撑轨越界——列宽必须显式钉死容器；min-width:0 兜
   * settled 态 overflow:visible 的自动最小尺寸。 */
  grid-template-columns: minmax(0, 1fr);
  grid-template-rows: 0fr;
  transition: grid-template-rows var(--dur-fast) var(--ease-in);
}
.turn__grid.is-open {
  grid-template-rows: 1fr;
  transition: grid-template-rows var(--dur-base) var(--ease-slide);
}
.turn__grid.is-settled .turn__body {
  overflow: visible;
}
.turn__body {
  min-width: 0;
  min-height: 0;
  overflow: hidden;
  opacity: 0;
  /* v4.11 用户裁定（照 ZCode 过程流基准）：展开体与头行同一左缘——
   * 不缩进、无引导线，扁平直排 */
  display: flex;
  flex-direction: column;
  gap: 1px; /* stylelint-disable-line declaration-property-value-disallowed-list -- 光学修正(孤值1,§二⑬) */
  margin-top: var(--lw-gap-2);
  transition: opacity var(--dur-fast) var(--ease-in);
}
.turn__grid.is-open .turn__body {
  opacity: 1;
  transition: opacity var(--dur-base) var(--ease-out) 40ms;
}
/* 嵌套折叠纪律（§6.6 ④）：一次只动被操作的层——内部子单元各自只在被
 * 直接点击时才动画（v-show / 各组件自己的 0fr），此处不加二次动画。 */
</style>
