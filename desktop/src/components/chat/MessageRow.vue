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
 * 接测量体）：fold-zone 夹紧 + 渐变遮罩 + 圆形展开钮悬浮在气泡内中下
 * （v2.102 裁定：钮在气泡里浮于文字表面），状态机在 use-message-fold.ts；
 * 复制/编辑/时间行在气泡下方且不受折叠影响（恒复制全文）。
 * 置顶钉（v2.102 裁定）：最近一条用户消息由父层判 pinned——sticky 常驻
 * 转录列顶端，模型工作消息增长不把它挤出视口；不透明底随宿主面
 * （--lw-msg-host，缺省 --lw-bg；子代理页签盖 bg-dock）。
 */
import { computed, nextTick, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Icon } from "../../lib/icons";
import HoverTip from "../settings/HoverTip.vue";
import { renderMarkdown } from "../../lib/markdown";
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
    /** 置顶钉（v2.102）：最近一条用户消息 sticky 常驻转录列顶端，父层
     * （ChatTranscript / 子代理转录页签）判定传入。 */
    pinned?: boolean;
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

async function copySelf() {
  try {
    await navigator.clipboard.writeText(props.text);
  } catch {
    /* 剪贴板不可用不打断 */
  }
}

/** 代码块头行的复制按钮挂在 v-html 里，事件在容器上委托。 */
async function onBodyClick(e: MouseEvent) {
  const target = (e.target as HTMLElement).closest?.(".copy-btn");
  if (!target) return;
  const fig = target.closest(".code-block");
  const code = fig?.querySelector("pre code")?.textContent ?? "";
  try {
    await navigator.clipboard.writeText(code);
  } catch {
    /* 剪贴板不可用不打断 */
  }
}
</script>

<template>
  <div
    class="msg"
    :class="[kind === 'user' ? 'msg-user' : 'msg-assistant', { 'is-pinned': pinned }]"
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
      <!-- 悬浮圆钮（v2.102 裁定）：气泡内中下、浮在文字表面——折叠态压在
          渐变遮罩上，展开态压在末行文字上；tips 走 Teleport 不受气泡裁剪 -->
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
    <div v-else class="md" v-html="html" @click="onBodyClick" />

    <div class="msg__actions">
      <HoverTip v-if="copyable" :text="t('chat.copyMessage')">
        <button
          class="copy-btn"
          type="button"
          :aria-label="t('chat.copyMessage')"
          @click="copySelf"
        >
          <Icon name="copy" :size="12" />
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
.msg__actions {
  display: flex;
  align-items: center;
  gap: var(--lw-gap-2);
  margin-top: var(--lw-gap-2);
  opacity: 0;
  transition: opacity var(--dur-fast) var(--ease-out);
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

/* ---- 置顶钉（v2.102 裁定）---- */
/* 最近一条用户消息 sticky 常驻转录列顶端：模型工作消息增长不把它挤出
 * 视口。不透明底 = 宿主面（主聊天区 --lw-bg），后继内容从其下方滑过被
 * 遮住；子代理转录页签经 --lw-msg-host 盖 bg-dock */
.msg.is-pinned {
  position: sticky;
  top: 0;
  z-index: 5;
  background: var(--lw-msg-host, var(--lw-bg));
}

/* ---- 长消息折叠（Line Clamp with Fade Mask，只折用户消息）---- */
/* 遮罩双层背景：wash-1 渐变叠宿主实底。目标色必须是气泡的实际表面色
 * （半透明 wash-1 叠宿主面）——单层 fade 到 wash-1 因目标色近乎全透明
 * 而不可见（v2.102 目检实抓）。宿主面可被 --lw-msg-host 覆盖。
 * §六：.vue 禁字面色 */
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
    var(--lw-msg-host, var(--lw-bg));
}
.fold-mask.is-open {
  opacity: 0;
}
/* 悬浮圆钮（v2.102 裁定）：气泡内中下、浮在表面——绝对定位于气泡，
 * 折叠态压遮罩、展开态压末行文字 */
.fold-btn-pos {
  position: absolute;
  left: 50%;
  bottom: var(--lw-gap-6);
  transform: translateX(-50%);
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
