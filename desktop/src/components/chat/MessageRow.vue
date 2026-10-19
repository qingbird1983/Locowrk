<script setup lang="ts">
/**
 * 消息行（UI shell doc §8.4 MessageRow）：用户消息 = 右对齐气泡（wash-1
 * 底、圆角 lg-plus、14px/1.55 pre-wrap）；助手消息 = 不是气泡、整列宽
 * markdown 正文（§6.5）。行尾动作区（复制 + 编辑 + 时间）hover 才显形；
 * 时间只在实时轮次有（历史重放无时间戳，不虚构时序）。
 * 编辑态（v2.57 §3.34）：用户气泡原位换自适应文本域，Enter = 提交重发、
 * Shift+Enter 换行、Esc = 取消；可编辑性由父层裁决（出现条件见 §3.34）。
 * 长消息折叠（组件规格「Line Clamp with Fade Mask」+ v2.101 裁定收窄：
 * 只折用户消息，模型正文——主聊天与子代理工作显示——不折，助手分支不
 * 接测量体）：fold-zone 夹紧 + 渐变遮罩 + 圆形展开钮悬浮在气泡内右下角
 * （v2.103 裁定），状态机在 use-message-fold.ts；
 * 复制/编辑/时间行在气泡下方且不受折叠影响（恒复制全文）。
 * 简短回复（copyable=false 的助手条目）整个动作区不渲染——隐藏时间行
 * opacity 0 却常驻占位，曾把组内 8px 间距节奏撑大（2026-10-07 用户裁定
 * 取消）；复制/编辑/时间只随用户消息与最终报告。
 * 遮罩方向（v2.103 目检裁定）：底部基本不透明（完全盖住文字）、渐渐往
 * 上渐渐透明（文字渐渐透出）——两层同形渐变叠出气泡实际表面色。
 */
import { computed, nextTick, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { assetRead, assetReveal } from "../../lib/api";
import { useWorkPanelStore } from "../../stores/work-panel";
import type { AssetRefDto, ContentPartDto } from "../../types";
import { Icon } from "../../lib/icons";
import HoverTip from "../settings/HoverTip.vue";
import { renderMarkdown } from "../../lib/markdown";
import { useCopyFeedback } from "../../lib/use-copy";
import { useMessageFold } from "./use-message-fold";

const props = withDefaults(
  defineProps<{
    kind: "user" | "assistant";
    text: string;
    /** 实时到达时刻；历史重放无此项 → 动作区只留复制钮。 */
    ts?: number;
    /** 用户消息可编辑（v2.57 §3.34：重放消息或最后一条；运行中恒 false）。 */
    editable?: boolean;
    /** 复制钮（v2.96 用户裁定）：只在用户消息与每轮最终汇报/总结之下出现
     * （缺省 true = 用户消息）；任务中间的简短回话由父层传 false 关掉。 */
    copyable?: boolean;
    /** 附件部件（批 D）：用户消息的图片缩略图 / PDF 文件 chip（正文下方）；
     * 缺省 = 纯文本消息（旧消息渲染零变化）。 */
    parts?: ContentPartDto[];
  }>(),
  { copyable: true },
);

const emit = defineEmits<{ edit: [newText: string] }>();

const { t } = useI18n();

const timeText = computed(() => {
  if (!props.ts) return "";
  const d = new Date(props.ts);
  const hh = String(d.getHours()).padStart(2, "0");
  const mm = String(d.getMinutes()).padStart(2, "0");
  return `${hh}:${mm}`;
});

/** 助手正文 markdown（解析 + 消毒 + 代码块增强在 lib/markdown.ts）。 */
const html = computed(() =>
  props.kind === "assistant" ? renderMarkdown(props.text, {
    copyCode: t("chat.copyCode"),
  }) : "",
);

// ---- 附件部件渲染（批 D §9.3）：图片缩略（asset_read data URL，模块级
// 缓存防重复读盘）/ PDF chip；图片点击 = 右栏图片页签（specs/08 批 2，全屏从
// 页签里的「全屏查看」走既有查看器），文件 chip 点击 = 在文件夹中显示 ----
const wp = useWorkPanelStore();
function openImageAsset(asset: AssetRefDto) {
  wp.openImageTab(asset.name, asset.id, asset.mime);
}
const assetCache = new Map<string, string>();
const mediaParts = computed(() => (props.parts ?? []).filter((p) => p.type !== "text"));
const thumbUrls = ref<Record<string, string>>({});
watch(
  mediaParts,
  (list) => {
    for (const p of list) {
      if (p.type !== "image") continue;
      const key = `${p.asset.id}:${p.asset.mime}`;
      const cached = assetCache.get(key);
      if (cached) {
        thumbUrls.value = { ...thumbUrls.value, [key]: cached };
        continue;
      }
      void assetRead(p.asset.id, p.asset.mime).then((url) => {
        assetCache.set(key, url);
        thumbUrls.value = { ...thumbUrls.value, [key]: url };
      });
    }
  },
  { immediate: true },
);
function thumbOf(asset: { id: string; mime: string }): string {
  return thumbUrls.value[`${asset.id}:${asset.mime}`] ?? "";
}
async function revealAsset(asset: { id: string; mime: string }) {
  try {
    await assetReveal(asset.id, asset.mime);
  } catch {
    /* 悬空引用静默（缩略图已在场说明曾有文件；报错不打断浏览） */
  }
}

/** 编辑态（组件本地，不持久化）：进入即全选草稿，离开即丢弃。 */
const editing = ref(false);
const draft = ref("");
const box = ref<HTMLTextAreaElement | null>(null);

function startEdit() {
  if (!props.editable || editing.value) return;
  draft.value = props.text;
  editing.value = true;
  void nextTick(() => {
    box.value?.focus();
    box.value?.setSelectionRange(draft.value.length, draft.value.length);
    autoGrow();
  });
}

function cancelEdit() {
  editing.value = false;
  draft.value = "";
}

function submitEdit() {
  const t2 = draft.value.trim();
  if (!t2) return;
  editing.value = false;
  emit("edit", t2);
}

function autoGrow() {
  const el = box.value;
  if (!el) return;
  el.style.height = "auto";
  el.style.height = `${Math.min(el.scrollHeight, 240)}px`;
}

watch(draft, () => void nextTick(autoGrow));

function onEditKeydown(e: KeyboardEvent) {
  if (e.key === "Escape") {
    e.stopPropagation();
    cancelEdit();
  } else if (e.key === "Enter" && !e.shiftKey && !e.isComposing) {
    e.preventDefault();
    submitEdit();
  }
}

// 长消息折叠（v2.101 裁定：只折用户消息——主聊天的与派给子代理的任务
// 提示都算；模型输出（主聊天正文与子代理工作显示）恒完整展示，助手分
// 支不再接折叠测量体）。测量体 = 气泡内文
const contentEl = ref<HTMLElement | null>(null);
const { isOpen, overflowing, showFold, foldStyle, animating, onToggleFold } =
  useMessageFold(contentEl, editing);

// 复制反馈（§3.7 v4.18）：成功后字形换对勾 ≈1.6s（消息钮走 ref、
// 代码块委托钮走 DOM class，同一 composable）
const { copiedId, copyWithFeedback, copyDelegatedCode } = useCopyFeedback();

async function copySelf() {
  await copyWithFeedback(props.text, "msg");
}

// 代码块复制委托（v-html 里的钮）实现同在 `lib/use-copy.ts`：两组件那段逐字相同。
</script>

<template>
  <div
    class="msg"
    :class="[
      kind === 'user' ? 'msg-user' : 'msg-assistant',
      { 'msg--brief': kind === 'assistant' && !copyable },
    ]"
  >
    <!-- 用户消息：右对齐气泡（编辑态 = 自适应文本域原位替换）；
        助手消息：整列宽 markdown。折叠 = fold-zone 夹紧 + 渐变遮罩 -->
    <textarea
      v-if="editing"
      ref="box"
      v-model="draft"
      class="msg-edit"
      rows="2"
      :aria-label="t('chat.editMessage')"
      @keydown="onEditKeydown"
    />
    <div
      v-else-if="kind === 'user'"
      class="msg-user__bubble"
      :class="{ 'is-clamped': overflowing }"
    >
      <div
        ref="contentEl"
        class="fold-zone"
        :class="{ 'is-anim': animating }"
        :style="foldStyle"
      >{{ text }}</div>
      <div v-if="showFold" class="fold-mask" :class="{ 'is-open': isOpen }" />
      <!-- 悬浮圆钮（v2.103 裁定：气泡内右下角、浮在文字表面——折叠态压
          在渐变遮罩上，展开态压在末行文字上；tips 走 Teleport 不受裁剪 -->
      <HoverTip
        v-if="showFold"
        class="fold-btn-pos"
        :text="isOpen ? t('chat.collapseMessage') : t('chat.expandMessage')"
      >
        <button
          class="fold-btn"
          :class="{ 'is-open': isOpen }"
          type="button"
          :aria-label="isOpen ? t('chat.collapseMessage') : t('chat.expandMessage')"
          :aria-expanded="isOpen"
          @click="onToggleFold"
        >
          <Icon name="chevronDown" :size="14" />
        </button>
      </HoverTip>
    </div>
    <!-- eslint-disable-next-line vue/no-v-html —— 内容已过 DOMPurify -->
    <div v-else class="md" v-html="html" @click="copyDelegatedCode" />

    <!-- 附件部件（批 D §9.3）：正文下、动作区上——图片缩略 96px 圆角 +
         PDF 文件 chip，点击在文件夹中显示。历史重放与实时同形。 -->
    <div v-if="kind === 'user' && mediaParts.length" class="msg-parts">
      <template v-for="(p, i) in mediaParts" :key="i">
        <button
          v-if="p.type === 'image'"
          class="msg-part-img"
          type="button"
          :aria-label="`${p.asset.name} · ${t('panel.openImage')}`"
          :title="`${p.asset.name} · ${t('panel.openImage')}`"
          @click="openImageAsset(p.asset)"
        >
          <img v-if="thumbOf(p.asset)" :src="thumbOf(p.asset)" alt="" draggable="false" />
          <span v-else class="msg-part-img__loading" />
        </button>
        <button
          v-else-if="p.type === 'file'"
          class="msg-part-file"
          type="button"
          :aria-label="`${p.asset.name} · ${t('mm.revealAsset')}`"
          :title="`${p.asset.name} · ${t('mm.revealAsset')}`"
          @click="revealAsset(p.asset)"
        >
          <Icon name="file" :size="12" aria-hidden="true" />
          <span class="msg-part-file__name">{{ p.asset.name }}</span>
        </button>
      </template>
    </div>

    <!-- 动作区只在有内容时渲染：简短回复（copyable=false 且不可编辑）
         整行不渲染——隐藏时间行占位是组内间距虚高的成因之一 -->
    <div v-if="copyable || editable" class="msg__actions">
      <HoverTip v-if="copyable" :text="t('chat.copyMessage')">
        <button
          class="copy-btn"
          :class="{ 'is-copied': copiedId === 'msg' }"
          type="button"
          :aria-label="t('chat.copyMessage')"
          @click="copySelf"
        >
          <Icon :name="copiedId === 'msg' ? 'check' : 'copy'" :size="12" />
        </button>
      </HoverTip>
      <HoverTip :text="t('chat.editMessage')">
        <button
          v-if="editable && !editing"
          class="copy-btn"
          type="button"
          :aria-label="t('chat.editMessage')"
          @click="startEdit"
        >
          <Icon name="edit" :size="12" />
        </button>
      </HoverTip>
      <span v-if="timeText" class="msg__time">{{ timeText }}</span>
    </div>
  </div>
</template>

<style scoped>
.msg {
  padding: var(--lw-gap-12) 0;
}
/* 简短回复（copyable=false 的助手条目）：自身垂直间距归零，兄弟间距全
 * 交父级（TranscriptFlow 兄弟选择器，v4.14 8px 节奏）——自带 12px padding
 * 与父级 8px 双重计算曾把间距撑到 24-30px。须排在 .msg 之后（同特异性
 * 靠源序取胜；.turn + .msg 等父级规则特异性更高，padding-top 仍归父级） */
.msg--brief {
  padding: 0;
}
.msg__actions {
  display: flex;
  align-items: center;
  gap: var(--lw-gap-2);
  margin-top: var(--lw-gap-2);
  opacity: 0;
  transition: opacity var(--dur-fast) var(--ease-out);
}
/* 附件部件行（批 D §9.3）：右对齐随用户气泡；图片 96px 圆角缩略 */
.msg-parts {
  display: flex;
  flex-wrap: wrap;
  gap: var(--lw-gap-6);
  margin-top: var(--lw-gap-6);
  justify-content: flex-end;
}
.msg-user .msg-parts {
  align-items: flex-end;
}
.msg-part-img {
  display: block;
  width: 96px;
  height: 96px;
  padding: 0;
  overflow: hidden;
  border-radius: var(--lw-r-lg);
  background: var(--lw-wash-2);
}
.msg-part-img img {
  display: block;
  width: 100%;
  height: 100%;
  object-fit: cover;
  user-select: none;
}
.msg-part-img__loading {
  display: block;
  width: 100%;
  height: 100%;
  background: var(--lw-wash-3);
}
.msg-part-file {
  display: inline-flex;
  max-width: 220px;
  height: 28px;
  align-items: center;
  gap: var(--lw-gap-6);
  padding: 0 var(--lw-gap-10);
  border-radius: var(--lw-r-full);
  background: var(--lw-wash-1);
  color: var(--lw-ink-2);
  font-size: var(--lw-fs-12half);
}
.msg-part-file__name {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.msg:hover .msg__actions,
.msg:focus-within .msg__actions {
  opacity: 1;
}
.msg__time {
  font-size: var(--lw-fs-11);
  color: var(--lw-ink-4);
  margin-left: var(--lw-gap-4);
}
.msg-user {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
}
.msg-user__bubble {
  position: relative;
  width: fit-content;
  max-width: min(82%, 600px);
  border-radius: var(--lw-r-lg-plus);
  /* v2.102：三圆角 + 右下直角（直角边朝发消息一侧） */
  border-bottom-right-radius: 0;
  padding: var(--lw-gap-10) var(--lw-gap-14);
  background: var(--lw-wash-1);
  font-size: var(--lw-fs-14);
  line-height: 1.55;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}
/* 编辑态文本域（§3.34）：气泡同宽同排版同角，raised 台面 */
.msg-edit {
  width: min(82%, 600px);
  border-radius: var(--lw-r-lg-plus);
  border-bottom-right-radius: 0;
  padding: var(--lw-gap-10) var(--lw-gap-14);
  background: var(--lw-bg-elevated);
  box-shadow: var(--lw-shadow-raised);
  font-size: var(--lw-fs-14);
  line-height: 1.55;
  resize: none;
  outline: none;
  overflow: hidden;
}

/* ---- 长消息折叠（Line Clamp with Fade Mask，只折用户消息）---- */
/* 遮罩 = 两层同形渐变（v2.103 目检裁定方向）：上层 wash-1、下层宿主实
 * 底，都从顶部透明渐到底部不透明——底部叠出气泡的实际表面色（wash-1
 * 半透明叠宿主面）完全盖住文字，往上两层同步渐透明、文字渐渐透出。
 * 单层 fade 到 wash-1 目标色近乎全透明 = 看不见（v2.102 实抓）；第二层
 * 用实底色块则整块盖死文字无渐透（v2.103 实抓）。宿主面可被
 * --lw-msg-host 覆盖（子代理转录页签 = bg-dock）。§六：.vue 禁字面色 */
.msg-user__bubble.is-clamped {
  overflow: hidden;
}
.fold-zone.is-anim {
  transition: max-height var(--dur-base) var(--ease-out);
}
.fold-mask {
  position: absolute;
  left: 0;
  right: 0;
  bottom: 0;
  height: 24px;
  pointer-events: none;
  opacity: 1;
  transition: opacity var(--dur-base) var(--ease-out);
  background:
    linear-gradient(to bottom, transparent, var(--lw-wash-1)),
    linear-gradient(to bottom, transparent, var(--lw-msg-host, var(--lw-bg)));
}
.fold-mask.is-open {
  opacity: 0;
}
/* 悬浮圆钮（v2.103 裁定：右下角右下角右下角）：气泡内右下、浮在表面
 * ——折叠态压遮罩、展开态压末行文字；恰落在右下直角上。
 * v2.105 目检实抓（左下角根因）：类经 $attrs 落在 HoverTip 锚点上，但
 * HoverTip 是双根组件（锚点 + Teleport 气泡）——多根子组件不继承父级
 * scoped 属性，`.fold-btn-pos[data-v-*]` 从未匹配上（v2.102 起定位一直
 * 没生效，恒左下角流内）；:deep 放开作用域约束才真正命中锚点 */
.msg-user__bubble :deep(.fold-btn-pos) {
  position: absolute;
  right: var(--lw-gap-6);
  bottom: var(--lw-gap-6);
  z-index: 2;
}
.fold-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  border-radius: var(--lw-r-full);
  background: var(--lw-bg);
  box-shadow: var(--lw-shadow-raised);
  color: var(--lw-ink-3);
  transition: color var(--dur-fast) var(--ease-standard);
}
.fold-btn:hover {
  color: var(--lw-ink-1);
}
.fold-btn :deep(svg) {
  transition: transform var(--dur-base) var(--ease-out);
}
/* 悬停微动效（规格 §三）：折叠态箭头下移 2px（向下展开），展开态上移 */
.fold-btn:hover :deep(svg) {
  transform: translateY(2px);
}
.fold-btn.is-open :deep(svg) {
  transform: rotate(180deg);
}
/* rotate(180) 后局部 +Y 朝屏幕上方：translateY(2px) = 上移 2px */
.fold-btn.is-open:hover :deep(svg) {
  transform: rotate(180deg) translateY(2px);
}
</style>
