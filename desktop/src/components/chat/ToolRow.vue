<script setup lang="ts">
/**
 * 工具行（UI shell doc §3.8 v4.13，《Tool Execution Process Display》）：
 * flex 行骨（ProcessRow 共享段位）+ 展开明细（左竖线子行 = 明细视图
 * ToolDetailView + subagent 标签值行）。状态克制（设计稿 §4.3）：**成功行尾完全
 * 为空**（既无对勾也无时长——成功不喧哗），失败才高亮（danger 叉 + 红「失败」）；
 * 明细恒默认收起（v2.98，v4.20 起
 * 不接设置开关——开关只作用过程组）。subagent_run = §3.10 紧凑行（大卡片 SubagentCard 已退役）：展开为
 * 任务/状态/轮次/工具/摘要标签值行 + 「查看完整对话」入口（不虚构——父侧
 * 拿不到的字段不渲染，⑮ 黑盒边界不变）。
 */
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Icon } from "../../lib/icons";
import ProcessRow from "./ProcessRow.vue";
import FileIcon from "./FileIcon.vue";
import ToolDetailView from "./ToolDetailView.vue";
import { toolDiffStats, toolImageAssets } from "../../lib/tool-summary";
import { toolView } from "../../lib/chat/render-matrix";
import { isRowExpanded, isRowMounted, toggleRow } from "../../lib/chat/fold-state";
import { toolDescribe } from "../../lib/tool-detail";
import { assetRead, assetSaveAs } from "../../lib/api";
import { routeArtifact } from "../../lib/artifact-routing";
import { previewUrlFor } from "../../lib/preview-url";
import { useToastStore } from "../../stores/toast";
import { useViewerStore } from "../../stores/viewer";
import { useWorkPanelStore } from "../../stores/work-panel";
import type { AssetRefDto } from "../../types";
import type { ToolItem } from "../../stores/chat";

const props = defineProps<{ item: ToolItem }>();

/** §3.10 入口（subagent 转录页签）与 §3.28（edit_diff 审阅页签）。 */
const emit = defineEmits<{
  openTranscript: [childId: string, title: string];
  openDiff: [callId: string, path: string, diff: string];
}>();

const { t } = useI18n();

// ---- 工具产物图片（批 E §8.3）：output.asset/assets 的 image/*，行头下
// 预览缩略（点击入查看器）+ 查看大图/保存两钮。设计稿三行形态；结果未回
// （`status !== "settled"`）与无产物不渲染。缩略读 asset_read data URL（模块级缓存防重复读盘）。----
const viewer = useViewerStore();
const wp = useWorkPanelStore();
const toast = useToastStore();
const assets = computed<AssetRefDto[]>(() =>
  props.item.status !== "settled" ? [] : toolImageAssets(props.item.output),
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
/** 缩略图点击（用户 2026-10-18：「点击图片 右边 打开图片」）：去右栏图片页签就地看；
 * 全屏交给页签里的「全屏查看」（复用既有 ViewerOverlay），不再抢先弹浮层。 */
function openThumb(a: AssetRefDto) {
  wp.openImageTab(a.name, a.id, a.mime);
}
/** 点文件名（以及明细里的代码块头 / diff 行号）的唯一判定入口（specs/08 批 2、标准
 * §二⑱）：`.html` 去浏览器页签、图去图片页签、其余去文件页签；判定只在
 * `lib/artifact-routing.ts` 一处，本函数只负责「拿到路径后怎么开」。 */
function openArtifactIn(rawPath: string, line?: number, endLine?: number) {
  if (!rawPath) return;
  const route = routeArtifact(rawPath);
  if (route === "browser") {
    wp.openBrowserTab({
      tabId: `br-${rawPath}`,
      kind: "file",
      title: rawPath.split(/[\\/]/).pop() ?? rawPath,
      url: previewUrlFor(rawPath),
      target: rawPath,
    });
    return;
  }
  if (route === "image") {
    wp.openImageTab(rawPath);
    return;
  }
  wp.openFileTab(rawPath, line, endLine);
}

function openArtifact(line?: number, endLine?: number) {
  openArtifactIn(badgePath.value, line, endLine);
}

/** 明细里点代码块头 / diff 行号（WF-01/02/05）：明细只报位置，判定仍走上面的唯一入口。 */
function onOpenFile(path: string, line: number, endLine: number) {
  openArtifactIn(path, line, endLine);
}
async function saveAsset(a: AssetRefDto) {
  const dest = await assetSaveAs(a.id, a.mime, a.name);
  if (dest) toast.push("ok", t("mm.savedAs", { path: dest }));
}
/** 明细恒默认收起（v4.20：设置开关只作用过程组，明细不再接开关）。
 * 展开记忆活在 `lib/chat/fold-state.ts`（台账 AF-12）：行是 `v-for` 出来的，组件内
 * `ref` 会被重投影/回收清掉，记忆必须按 `item.id` 住在组件外。
 * 懒挂载语义不变（挂载过才保 DOM）：多数步骤从不展开，不为它们在挂载期跑
 * marked/DOMPurify、建整棵隐藏大 DOM；首次展开一次性渲染，此后保 DOM 秒开。 */
const expanded = computed(() => isRowExpanded(props.item.id));
const everExpanded = computed(() => isRowMounted(props.item.id));

const view = computed(() => toolView(props.item.tool));
const typeLabel = computed(() => t(`tool.type.${view.value.verb}`));
const desc = computed(() => toolDescribe(props.item.tool, props.item.args));
/** 行内语言徽章认的那个文件：**只在这个身份的矩阵登记 `badge: "file"` 时**才取路径。
 *  `fs_list` 的 path 是目录、不是文件——按身份挂徽章（设计稿 §2.2），
 *  不无条件取参数里的路径。 */
const badgePath = computed(() => {
  if (view.value.badge !== "file") return "";
  const a = props.item.args as Record<string, unknown> | null;
  const p = a?.path ?? a?.file;
  return typeof p === "string" ? p : "";
});

/** 写/编辑类行描述尾的变更统计（取不到不渲染，不虚构）。 */
const diff = computed(() =>
  props.item.status !== "settled" ? {} : toolDiffStats(props.item.output),
);
const hasDiff = computed(
  () =>
    diff.value.added !== undefined ||
    diff.value.removed !== undefined ||
    diff.value.size !== undefined,
);

const statusWord = computed(() => {
  if (props.item.status === "awaiting_approval") return t("tool.state.waiting");
  if (props.item.status === "running") {
    // G2：悬空未裁决 = 「结果未知」（不是执行中——进程已退出，不会再跑）
    if (props.item.uncertain) return t("tool.state.unknown");
    return t("tool.state.running");
  }
  if (props.item.ok) return "";
  return t("tool.state.failed");
});

/** §3.28 审阅：edit_diff 的 unified diff 随 args.patch，路径随 args.path。 */
const reviewItem = computed(() => {
  if (props.item.tool !== "edit_diff" || props.item.status !== "settled") return null;
  const args = props.item.args as { path?: unknown; patch?: unknown } | null;
  if (typeof args?.path !== "string" || typeof args.patch !== "string") return null;
  return { callId: props.item.callId, path: args.path, diff: args.patch };
});

/** 行内 ±N 徽章 = 审阅页签的直接入口（WF-03 小裁①，2026-10-24 决策人拍板）：
 *  判据与展开体「查看 diff」钮同源（`reviewItem`）——只有 edit_diff 且明细拿得到
 *  path/patch 才可点，其余身份的徽章保持纯展示（不虚构）。`.tool__file` 同款
 *  role + 键盘处理；`.stop` 防止点徽章连带整行展开。 */
function openReview() {
  const r = reviewItem.value;
  if (r) emit("openDiff", r.callId, r.path, r.diff);
}

function onToggle() {
  toggleRow(props.item.id);
}

/** 子代理整行可点（specs/08 批 3 / AC-12，用户 2026-10-18：「还有子代理 点击 右栏 展开」）：
 *  判据是**数据**不是身份——`output.child_session_id` 在才给整行入口（父侧拿不到就仍是
 *  普通开关行，不虚构）；标题取任务的 `prompt`，明细里的「查看完整对话」两条路并存。 */
const subagentEntry = computed(() => {
  const o = props.item.output as { child_session_id?: unknown } | null;
  const childId = typeof o?.child_session_id === "string" ? o.child_session_id : "";
  if (!childId) return null;
  const a = props.item.args as { prompt?: unknown } | null;
  const title = typeof a?.prompt === "string" ? a.prompt : props.item.tool;
  return { childId, title };
});

function onActivate() {
  const s = subagentEntry.value;
  if (s) emit("openTranscript", s.childId, s.title);
}
</script>

<template>
  <div class="tool">
    <ProcessRow
      :icon="view.icon"
      :type="typeLabel"
      :expanded="expanded"
      :foldable="view.fold === 'always'"
      :activatable="!!subagentEntry"
      :title="desc"
      @toggle="onToggle"
      @activate="onActivate"
    >
      <template #desc>
        <span class="tool__desc">
          <!-- 文件名可点（用户 2026-10-18：点「写入」后面的文件名跳右栏）：行壳是
               div + role="button"（ProcessRow 去 button 化的原因就是让行内能嵌可点元素），
               这里同款给 role + 键盘处理（同 .panel-tab__close 先例） -->
          <span
            v-if="badgePath"
            class="tool__file"
            role="button"
            tabindex="0"
            :aria-label="t('panel.openFile')"
            @click.stop="openArtifact()"
            @keydown.enter.stop="openArtifact()"
            @keydown.space.stop.prevent="openArtifact()"
          >
            <FileIcon :path="badgePath" />{{ desc }}
          </span>
          <template v-else>{{ desc }}</template>
        </span>
        <span
          v-if="hasDiff"
          class="tool__diff"
          :class="{ 'tool__diff--entry': !!reviewItem }"
          :role="reviewItem ? 'button' : undefined"
          :tabindex="reviewItem ? 0 : undefined"
          :title="reviewItem ? t('panel.viewDiff') : undefined"
          :aria-label="reviewItem ? t('panel.viewDiff') : undefined"
          @click.stop="openReview"
          @keydown.enter.stop="openReview"
          @keydown.space.stop.prevent="openReview"
        >
          <template v-if="diff.added !== undefined || diff.removed !== undefined">
            <span class="tool__add">+{{ diff.added ?? 0 }}</span>
            <span class="tool__del">−{{ diff.removed ?? 0 }}</span>
          </template>
          <span v-if="diff.size" class="tool__size">{{ diff.size }}</span>
        </span>
      </template>
      <template #status>
        <!-- 克制（设计稿 §4.3）：进行中 = 脉动点 + 弱化词；**成功 = 行尾完全
             为空**（既无对勾也无时长——成功不喧哗）；失败 = danger 叉 + 红词
             （整屏唯一高亮态）；G2 结果未知 = warn 点 + warn 词 -->
        <template v-if="item.status !== 'settled'">
          <span class="dot lw-pulse" :class="{ 'dot--unknown': item.uncertain }" />
          <span :class="{ 'tool__unknown': item.uncertain }">{{ statusWord }}</span>
        </template>
        <template v-else-if="!item.ok">
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
          @click="openThumb(a)"
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

    <!-- 展开体 = 公共外壳 `.lw-expand`（发丝竖线落在行首图标居中位、内容左边缘与类型词
         对齐，与思考行同壳；懒挂载 + v-show 保 DOM，首开后收展零重渲染） -->
    <div v-if="everExpanded" v-show="expanded" class="lw-expand tool__sub">
      <ToolDetailView
        :item="item"
        @open-transcript="(id, title) => emit('openTranscript', id, title)"
        @open-file="onOpenFile"
      />
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
/* 语言图标与文件名的间距（图标是行内元素，吃不到 flex gap） */
.tool__desc :deep(.ficon) {
  margin-right: var(--lw-gap-4);
}
/* 截断由**实际可用宽度**决定（台账 AF-15）：`describe.ts` 只留一道病态长值的兜底
 * 上限，不再按工具逐个写死 40/32/24——段位多宽就显多宽，超了这里出省略号。 */
.tool__desc {
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}
/* 文件名 = 可点入口（specs/08 批 2）：hover 抬到主墨色给「可点」的反馈，
   不做下划线也不换底（行本身 hover 已有 wash-2）。 */
.tool__file {
  cursor: pointer;
}
.tool__file:hover,
.tool__file:focus-visible {
  color: var(--lw-ink-1);
}
.tool__diff {
  display: inline-flex;
  align-items: baseline;
  gap: var(--lw-gap-6);
  margin-left: var(--lw-gap-6);
  font-family: var(--font-mono);
  font-size: var(--lw-fs-11);
}
/* 徽章入口态（WF-03 小裁①）：数字段各自带色，容器抬色看不见——
   用下划线给「可点」反馈（数字下划线仍随各自色），不做底色不换字色。 */
.tool__diff--entry {
  cursor: pointer;
}
.tool__diff--entry:hover,
.tool__diff--entry:focus-visible {
  text-decoration: underline;
  text-underline-offset: 2px;
}
.tool__add {
  color: var(--lw-diff-add);
}
.tool__del {
  color: var(--lw-diff-del);
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
/* 展开体：左竖线 + 内容不缩进；垂直间距由父级 gap 给。
   几何在 style.css 的 `.lw-expand`（唯一落点，台账 AF-17）：思考行与工具行共用同一条
   竖线与同一个内容起点，两条路径都不许各自再写一套。这里只管纵向排列。 */
.tool__sub {
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
