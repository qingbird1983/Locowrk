<script setup lang="ts">
/**
 * 工具行（UI shell doc §3.8 v4.13，《Tool Execution Process Display》）：
 * 五列栅格行（ProcessRow 共享列轨）+ 展开明细（缩进竖线子行 = 明细视图
 * ToolDetailView + subagent 标签值行）。状态克制：成功弱化（时长或弱对勾），
 * 失败才高亮（danger 叉 + 红「失败」）；明细恒默认收起（v2.98，v4.20 起
 * 不接设置开关——开关只作用过程组）。subagent_run = §3.10 紧凑行（大卡片 SubagentCard 已退役）：展开为
 * 任务/状态/轮次/工具/摘要标签值行 + 「查看完整对话」入口（不虚构——父侧
 * 拿不到的字段不渲染，⑮ 黑盒边界不变）。
 */
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Icon } from "../../lib/icons";
import ProcessRow from "./ProcessRow.vue";
import ToolDetailView from "./ToolDetailView.vue";
import { toolDiffStats, toolImageAssets } from "../../lib/tool-summary";
import { toolKind, toolDescribe } from "../../lib/tool-detail";
import { formatDuration } from "../../lib/format";
import { assetRead, assetSaveAs } from "../../lib/api";
import { useToastStore } from "../../stores/toast";
import { useViewerStore } from "../../stores/viewer";
import type { AssetRefDto } from "../../types";
import type { ToolItem } from "../../stores/chat";

const props = defineProps<{
  item: ToolItem;
  /** 该调用的审批未决（§3.17）：状态词"等待审批"——还没跑，不是运行中。 */
  awaiting?: boolean;
}>();

/** §3.10 入口（subagent 转录页签）与 §3.28（edit_diff 审阅页签）。 */
const emit = defineEmits<{
  openTranscript: [childId: string, title: string];
  openDiff: [callId: string, path: string, diff: string];
}>();

const { t } = useI18n();

// ---- 工具产物图片（批 E §8.3）：output.asset/assets 的 image/*，行头下
// 预览缩略（点击入查看器）+ 查看大图/保存两钮。设计稿三行形态；pending
// 与无产物不渲染。缩略读 asset_read data URL（模块级缓存防重复读盘）。----
const viewer = useViewerStore();
const toast = useToastStore();
const assets = computed<AssetRefDto[]>(() =>
  props.item.pending ? [] : toolImageAssets(props.item.output),
);
const assetThumbs = ref<Record<string, string>>({});
const thumbCache = new Map<string, string>();
watch(
  assets,
  (list) => {
    for (const a of list) {
      const key = `${a.id}:${a.mime}`;
      const cached = thumbCache.get(key);
      if (cached) {
        assetThumbs.value = { ...assetThumbs.value, [key]: cached };
        continue;
      }
      void assetRead(a.id, a.mime).then((url) => {
        thumbCache.set(key, url);
        assetThumbs.value = { ...assetThumbs.value, [key]: url };
      });
    }
  },
  { immediate: true },
);
function thumbOf(a: AssetRefDto): string {
  return assetThumbs.value[`${a.id}:${a.mime}`] ?? "";
}
function openViewer(a: AssetRefDto) {
  viewer.openAsset(a);
}
async function saveAsset(a: AssetRefDto) {
  const dest = await assetSaveAs(a.id, a.mime, a.name);
  if (dest) toast.push("ok", t("mm.savedAs", { path: dest }));
}
/** 明细恒默认收起（v4.20：设置开关只作用过程组，明细不再接开关）。 */
const expanded = ref(false);
/** 明细懒挂载（v-if+v-show 组合）：多数步骤从不展开，不为它们在挂载期
 * 跑 marked/DOMPurify、建整棵隐藏大 DOM（步数多的轮次流式/重放/展开
 * 都背着这些隐藏子树卡）；首次展开一次性渲染，此后保 DOM 秒开。 */
const everExpanded = ref(false);

const kind = computed(() => toolKind(props.item.tool));
const typeLabel = computed(() => t(`tool.type.${kind.value.type}`));
const desc = computed(() => toolDescribe(props.item.tool, props.item.args));

/** 写/编辑类行描述尾的变更统计（取不到不渲染，不虚构）。 */
const diff = computed(() =>
  props.item.pending ? {} : toolDiffStats(props.item.output),
);
const hasDiff = computed(
  () =>
    diff.value.added !== undefined ||
    diff.value.removed !== undefined ||
    diff.value.size !== undefined,
);

/** 实时轮时长（重放无时间戳不虚构）。 */
const durationText = computed(() =>
  props.item.ts !== undefined && props.item.doneTs !== undefined
    ? formatDuration(props.item.doneTs - props.item.ts)
    : "",
);

const statusWord = computed(() => {
  if (props.item.pending) {
    // G2：悬空未裁决 = 「结果未知」（不是执行中——进程已退出，不会再跑）
    if (props.item.uncertain) return t("tool.unknown");
    return props.awaiting ? t("tool.awaitingApproval") : t("tool.running");
  }
  if (props.item.ok) return "";
  return t("tool.failed");
});

/** §3.28 审阅：edit_diff 的 unified diff 随 args.patch，路径随 args.path。 */
const reviewItem = computed(() => {
  if (props.item.tool !== "edit_diff" || props.item.pending) return null;
  const args = props.item.args as { path?: unknown; patch?: unknown } | null;
  if (typeof args?.path !== "string" || typeof args.patch !== "string") return null;
  return { callId: props.item.callId, path: args.path, diff: args.patch };
});

function onToggle() {
  expanded.value = !expanded.value;
  if (expanded.value) everExpanded.value = true;
}
</script>

<template>
  <div class="tool">
    <ProcessRow
      :icon="kind.icon"
      :type="typeLabel"
      :expanded="expanded"
      :title="desc"
      @toggle="onToggle"
    >
      <template #desc>
        <span class="tool__desc">{{ desc }}</span>
        <span v-if="hasDiff" class="tool__diff">
          <template v-if="diff.added !== undefined || diff.removed !== undefined">
            <span class="tool__add">+{{ diff.added ?? 0 }}</span>
            <span class="tool__del">−{{ diff.removed ?? 0 }}</span>
          </template>
          <span v-if="diff.size" class="tool__size">{{ diff.size }}</span>
        </span>
      </template>
      <template #status>
        <!-- 克制（§3.8）：进行中 = 脉动点 + 弱化词；完成 = 时长或弱对勾；
             失败 = danger 叉 + 红词（唯一高亮态）；G2 结果未知 = warn 点
             + warn 词（不确定态，非失败非运行） -->
        <template v-if="item.pending">
          <span class="dot lw-pulse" :class="{ 'dot--unknown': item.uncertain }" />
          <span :class="{ 'tool__unknown': item.uncertain }">{{ statusWord }}</span>
        </template>
        <template v-else-if="item.ok">
          <span v-if="durationText">{{ durationText }}</span>
          <Icon v-else name="check" :size="12" />
        </template>
        <template v-else>
          <Icon name="close" :size="11" class="tool__fail-icon" />
          <span class="tool__fail">{{ statusWord }}</span>
        </template>
      </template>
    </ProcessRow>

    <!-- 工具产物图片（§8.3）：缩略 + 查看大图/保存（多图时钮作用于
         首图，逐图走缩略点击） -->
    <div v-if="assets.length" class="tool__assets">
      <div class="tool__thumbs">
        <button
          v-for="(a, i) in assets"
          :key="`${a.id}:${i}`"
          class="tool__thumb"
          type="button"
          :aria-label="`${a.name} · ${t('mm.viewLarge')}`"
          :title="`${a.name} · ${t('mm.viewLarge')}`"
          @click="openViewer(a)"
        >
          <img v-if="thumbOf(a)" :src="thumbOf(a)" alt="" draggable="false" />
          <span v-else class="tool__thumb-loading" />
        </button>
      </div>
      <div class="tool__asset-actions">
        <button class="tool__review" type="button" @click="openViewer(assets[0]!)">
          <Icon name="eye" :size="12" />
          {{ t("mm.viewLarge") }}
        </button>
        <button class="tool__review" type="button" @click="saveAsset(assets[0]!)">
          <Icon name="download" :size="12" />
          {{ t("mm.saveAsset") }}
        </button>
      </div>
    </div>

    <!-- 展开体 = 缩进 + 左竖线的子行（不换布局；懒挂载 + v-show 保 DOM，
         首开后收展零重渲染） -->
    <div v-if="everExpanded" v-show="expanded" class="tool__sub">
      <ToolDetailView :item="item" @open-transcript="(id, title) => emit('openTranscript', id, title)" />
      <!-- edit_diff 审阅入口（§3.28）：明细动作行的文本钮 -->
      <button
        v-if="reviewItem"
        class="tool__review"
        type="button"
        @click="reviewItem && emit('openDiff', reviewItem.callId, reviewItem.path, reviewItem.diff)"
      >
        <Icon name="file" :size="12" />
        {{ t("panel.viewDiff") }}
      </button>
    </div>
  </div>
</template>

<style scoped>
.tool {
  /* v4.14：层 2 行 ↔ 层 3 明细 = 8px，父容器 flex+gap 给间距（margin 撤） */
  display: flex;
  flex-direction: column;
  gap: var(--lw-gap-8);
  min-width: 0;
}
.tool__diff {
  display: inline-flex;
  align-items: baseline;
  gap: var(--lw-gap-6);
  margin-left: var(--lw-gap-6);
  font-family: var(--font-mono);
  font-size: var(--lw-fs-11);
}
.tool__add {
  color: var(--lw-ok);
}
.tool__del {
  color: var(--lw-danger);
}
.tool__size {
  color: var(--lw-ink-2);
}
.tool__fail-icon {
  color: var(--lw-danger);
}
.tool__fail {
  color: var(--lw-danger);
}
/* G2 结果未知：warn 点 + warn 词（不确定态，弱于失败的 danger） */
.tool__unknown {
  color: var(--lw-warn);
}
.tool :deep(.dot--unknown) {
  background: var(--lw-warn);
}
/* 运行脉动点 = 层 2 状态字形，跟随次要色（全局 .dot 底 ink-4） */
.tool :deep(.dot) {
  background: var(--lw-ink-2);
}
/* 展开体：缩进 12px + 左竖线（层级语法，保留）；垂直间距由父级 gap 给 */
.tool__sub {
  margin-left: var(--lw-gap-12);
  padding-left: var(--lw-gap-12);
  border-left: 1px solid var(--lw-line-subtle);
  display: flex;
  flex-direction: column;
  gap: var(--lw-gap-8);
}
.tool__assets {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: var(--lw-gap-6);
}
.tool__thumbs {
  display: flex;
  flex-wrap: wrap;
  gap: var(--lw-gap-6);
}
.tool__thumb {
  width: 56px;
  height: 56px;
  overflow: hidden;
  border-radius: var(--lw-r-sm);
  background: var(--lw-wash-2);
  transition: box-shadow var(--dur-fast) var(--ease-standard);
}
.tool__thumb:hover {
  box-shadow: 0 0 0 1px var(--lw-line);
}
.tool__thumb img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}
.tool__thumb-loading {
  display: block;
  width: 100%;
  height: 100%;
}
.tool__asset-actions {
  display: flex;
  gap: var(--lw-gap-6);
}
.tool__review {
  display: inline-flex;
  align-items: center;
  gap: 4px; /* stylelint-disable-line declaration-property-value-disallowed-list -- 光学修正(孤值4,§二⑬) */
  align-self: flex-start;
  padding: 2px 4px; /* stylelint-disable-line declaration-property-value-disallowed-list -- 光学修正(孤值2/4,§二⑬) */
  border-radius: var(--lw-r-2xs);
  font-size: var(--lw-fs-11);
  color: var(--lw-ink-2);
  transition: background var(--dur-fast) var(--ease-standard),
    color var(--dur-fast) var(--ease-standard);
}
.tool__review:hover {
  background: var(--lw-wash-2);
  color: var(--lw-ink-1);
}
</style>
