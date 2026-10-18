<script setup lang="ts">
/**
 * 会话转录（UI shell doc §8.4 ChatTranscript）：滚动区（overscroll contain /
 * scrollbar-gutter stable / 原生锚定保留 ⇒ 不写 overflow-anchor）+ 列（宽 = --lw-content-max
 * 覆盖值，§2.7 三处同源）+ 底部渐隐 mask。消息流的分组/任务容器/相位渲染
 * 在共享核心 chat/TranscriptFlow.vue（v2.104 提取，子代理转录页签同吃），
 * 本组件只负责主对话的容器面：⑤⑨ 跟随滚动（默认贴底，用户上滚即让位）
 * + 回到底部钮（坞上方）+ §3.34 编辑重发 + 系统条动作接线（⑨ 撤销/详情）。
 *
 * v4.11：审批卡迁坞（Composer 槽位，§3.17），转录里只剩系统条目。
 */
import { computed, nextTick, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { contextUndoCompact } from "../lib/api";
import { useChatStore, type SystemItem } from "../stores/chat";
import { useShellStore } from "../stores/shell";
import { useWorkPanelStore } from "../stores/work-panel";
import { clip } from "../lib/format";
import { resetRowFolds } from "../lib/chat/fold-state";
import { Icon } from "../lib/icons";
import HoverTip from "./settings/HoverTip.vue";
import ConversationWidthHandles from "./chat/ConversationWidthHandles.vue";
import TranscriptFlow from "./chat/TranscriptFlow.vue";

const { t } = useI18n();
const chat = useChatStore();
const workPanel = useWorkPanelStore();

/** §3.10：子 Agent 卡是入口——点「看转录」开工作面板 §3.16 页签。 */
function onOpenTranscript(childId: string, title: string) {
  workPanel.openSubTab(childId, clip(title, 24));
}

/** §3.28：edit_diff 卡「看 diff」开工作面板审阅页签。 */
function onOpenDiff(callId: string, path: string, diff: string) {
  workPanel.openReviewTab(callId, path, diff);
}

const props = defineProps<{
  /** composer 坞总高（px）：列宽热区下沿 + 底部渐隐 mask 的锚。 */
  composerBottom: number;
}>();

/** §3.34 可编辑的用户气泡：重放消息（带事件位）或当前最后一条用户消息
 * （实时发送无事件位的解析口）；运行中全部不可编辑。 */
const editableUserIds = computed(() => {
  const ids = new Set<number>();
  if (chat.running) return ids;
  let lastUserIdx = -1;
  chat.items.forEach((it, i) => {
    if (it.kind === "user") lastUserIdx = i;
  });
  chat.items.forEach((it, i) => {
    if (it.kind !== "user") return;
    if (it.pos !== undefined || i === lastUserIdx) ids.add(it.id);
  });
  return ids;
});

/** §3.34 编辑重发：截断失败（运行中/后端拒绝）以系统条如实呈现。 */
async function onEditResend(pos: number | undefined, text: string) {
  try {
    await chat.editResend(pos ?? null, text);
  } catch (err) {
    chat.pushNotice("notice.editResendFailed", { reason: String(err) });
  }
}

// ---- §3.39③⑤ 跟随滚动 + 回到底部（v4.11 裁定⑨）----
// 默认贴底：任何条目变化（含流式 delta）都滚到底；用户上滚（距底
// > 48px）即让位，回到底部钮在 composer 坞上方伺服。
// 两档阈值（§3.39③ / spec 06 AC-8）：**跟随** 48px 内算「就在底部」，一上滚就让位；
// **按钮** 要等离底 > 120px 才出现——刚滚一点就弹钮会跟「轻微回看」抢手。
const scroller = ref<HTMLElement | null>(null);
const following = ref(true);
const showBackBottom = ref(false);
const BOTTOM_EPSILON = 48;
const BACK_BOTTOM_GAP = 120;
function onScroll() {
  const el = scroller.value;
  if (!el) return;
  const gap = el.scrollHeight - el.scrollTop - el.clientHeight;
  following.value = gap <= BOTTOM_EPSILON;
  showBackBottom.value = gap > BACK_BOTTOM_GAP;
}
async function scrollToBottom(smooth = false) {
  await nextTick();
  const el = scroller.value;
  if (!el) return;
  el.scrollTo({ top: el.scrollHeight, behavior: smooth ? "smooth" : "auto" });
}
function resumeFollow() {
  following.value = true;
  void scrollToBottom(true);
}
// watch(chat.items)（隐式深）：流式 delta 改写末条文本也触发——跟随滚动
// 不只看条目数。运行中即时贴底（流式平滑滚动会脱拍），静止变化平滑。
watch(chat.items, () => {
  if (following.value) void scrollToBottom(!chat.running);
});
watch(
  () => chat.sessionId,
  () => {
    following.value = true;
    // 层 3 展开记忆只在会话内有效（设计稿 §8，台账 AF-12）：换会话从零开始
    resetRowFolds();
    void scrollToBottom();
  },
);
onMounted(() => void scrollToBottom());
// 切回对话目的地（§2.6 display:none 挂起）时视口尺寸曾不可用，回来自动贴底
watch(
  () => useShellStore().destination,
  (d) => {
    if (d === "chat") {
      following.value = true;
      void scrollToBottom();
    }
  },
);

/** 系统消息动作（⑨ 接线）：details = 展开详情；undo = 撤销最近一次压缩；
 * view（M3 §3.37③）= 跳记忆页看整理产物（记忆树区块）。 */
async function onNoticeAction(item: SystemItem, action: string) {
  if (action === "view") {
    useShellStore().setDestination("memory");
  } else if (action === "details") {
    const detail = chat.noticeDetails[item.id];
    if (detail) chat.pushRawSystem(detail);
  } else if (action === "undo") {
    try {
      await contextUndoCompact(chat.sessionId);
      chat.pushNotice("notice.compactUndone");
    } catch (err) {
      chat.pushNotice("notice.compactUndoFailed", { reason: String(err) });
    }
  }
}
</script>

<template>
  <div class="relative min-h-0 flex-1">
    <div ref="scroller" class="scroller scroll-y" @scroll.passive="onScroll">
      <div class="column conv-w">
        <TranscriptFlow
          :items="chat.items"
          :running="chat.running"
          :stop-requested="chat.stopRequested"
          :editable-ids="editableUserIds"
          @edit="onEditResend"
          @open-transcript="onOpenTranscript"
          @open-diff="onOpenDiff"
          @notice-action="onNoticeAction"
        />
      </div>
    </div>

    <!-- §3.39③⑤ 回到底部（v4.11）：用户上滚让位后伺服，坞上方居中 -->
    <HoverTip
      class="!static"
      :text="t('chat.backToLatest')"
      :label="t('chat.backToLatest')"
    >
      <Transition name="lw-pop-up">
        <button
          v-if="showBackBottom"
          class="back-bottom"
          type="button"
          :aria-label="t('chat.backToLatest')"
          :style="{ bottom: `${props.composerBottom + 12}px` }"
          @click="resumeFollow"
        >
          <Icon name="chevronDown" :size="14" />
        </button>
      </Transition>
    </HoverTip>

    <!-- 列宽双柄：容器下沿停在 composer 顶部（§2.7） -->
    <ConversationWidthHandles :bottom="props.composerBottom" />
  </div>
</template>

<style scoped>
.scroller {
  flex: 1 1 auto;
  height: 100%;
  min-height: 0;
  overflow-y: auto;
  overscroll-behavior: contain;
  /* both-edges（v2.60 修复批）：滚动条槽两侧对称预留——转录列与 composer
   * 列同以主区中线居中，正文不再整体左偏（旧 stable 只在右缘留槽，列中心
   * 左移半个槽宽） */
  scrollbar-gutter: stable both-edges;
  /* 原生锚定**保留**（缺省 auto，不写声明 = AF-09 的修法）：旧写 `overflow-anchor: none`
   * 之后，视口上方任何高度变化（shiki 换掉代码块那一下）都会把读者原位推走。同窗口实测
   * （CDP，在列首注入 200px 块）：`none` ⇒ scrollTop 300 不动、探测行视口 top 693→893
   * （被推走 200px）；`auto` ⇒ scrollTop 300→500、探测行 top 恒 693（视角钉住）。
   * 跟随态由下面的 `scrollToBottom` 负责，两者同向不打架。 */
  padding: var(--lw-gap-8) var(--lw-gap-24) 0;
  /* 底部渐隐：内容浮到输入卡下方时淡出（遮罩盖住 composer 预留区）。
   * 渐隐色 = --lw-mask-fade 令牌（§六标准：.vue 禁字面色） */
  -webkit-mask-image: linear-gradient(
    to bottom,
    var(--lw-mask-fade) 0,
    var(--lw-mask-fade) calc(100% - 168px),
    color-mix(in oklab, var(--lw-mask-fade) 25%, transparent) calc(100% - 128px),
    transparent calc(100% - 100px)
  );
  mask-image: linear-gradient(
    to bottom,
    var(--lw-mask-fade) 0,
    var(--lw-mask-fade) calc(100% - 168px),
    color-mix(in oklab, var(--lw-mask-fade) 25%, transparent) calc(100% - 128px),
    transparent calc(100% - 100px)
  );
}
.column {
  width: min(100%, var(--lw-content-max));
  margin-inline: auto;
  padding-bottom: 180px; /* stylelint-disable-line declaration-property-value-disallowed-list -- 光学修正(孤值180,§二⑬) */
}
/* §3.39③ 回到底部钮：32px 圆钮，composer 坞上方居中（bottom 动态 =
 * 坞高 + 12px）；z 高于渐隐 mask */
.back-bottom {
  position: absolute;
  left: 50%;
  transform: translateX(-50%);
  width: 32px;
  height: 32px;
  border-radius: var(--lw-r-full);
  background: var(--lw-bg-elevated);
  box-shadow: var(--lw-shadow-raised);
  color: var(--lw-ink-2);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  z-index: 13;
  transition: background var(--dur-fast) var(--ease-standard),
    color var(--dur-fast) var(--ease-standard);
}
.back-bottom:hover {
  background: var(--lw-wash-2);
  color: var(--lw-ink-1);
}
</style>
