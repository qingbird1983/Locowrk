<script setup lang="ts">
/**
 * 消息行（UI shell doc §8.4 MessageRow）：用户消息 = 右对齐气泡（wash-1
 * 底、圆角 lg-plus、14px/1.55 pre-wrap）；助手消息 = 不是气泡、整列宽
 * markdown 正文（§6.5）。行尾动作区（复制 + 编辑 + 时间）hover 才显形；
 * 时间只在实时轮次有（历史重放无时间戳，不虚构时序）。
 * 编辑态（v2.57 §3.34）：用户气泡原位换自适应文本域，Enter = 提交重发、
 * Shift+Enter 换行、Esc = 取消；可编辑性由父层裁决（出现条件见 §3.34）。
 */
import { computed, nextTick, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Icon } from "../../lib/icons";
import { renderMarkdown } from "../../lib/markdown";

const props = defineProps<{
  kind: "user" | "assistant";
  text: string;
  /** 实时到达时刻；历史重放无此项 → 动作区只留复制钮。 */
  ts?: number;
  /** 用户消息可编辑（v2.57 §3.34：重放消息或最后一条；运行中恒 false）。 */
  editable?: boolean;
}>();

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
  <div class="msg" :class="kind === 'user' ? 'msg-user' : 'msg-assistant'">
    <!-- 用户消息：右对齐气泡（编辑态 = 自适应文本域原位替换）；
        助手消息：整列宽 markdown -->
    <textarea
      v-if="editing"
      ref="box"
      v-model="draft"
      class="msg-edit"
      rows="2"
      :aria-label="t('chat.editMessage')"
      @keydown="onEditKeydown"
    />
    <div v-else-if="kind === 'user'" class="msg-user__bubble">{{ text }}</div>
    <!-- eslint-disable-next-line vue/no-v-html —— 内容已过 DOMPurify -->
    <div
      v-else
      class="md"
      v-html="html"
      @click="onBodyClick"
    />

    <div class="msg__actions">
      <button
        class="copy-btn"
        type="button"
        :title="t('chat.copyMessage')"
        :aria-label="t('chat.copyMessage')"
        @click="copySelf"
      >
        <Icon name="copy" :size="12" />
      </button>
      <button
        v-if="editable && !editing"
        class="copy-btn"
        type="button"
        :title="t('chat.editMessage')"
        :aria-label="t('chat.editMessage')"
        @click="startEdit"
      >
        <Icon name="edit" :size="12" />
      </button>
      <span v-if="timeText" class="msg__time">{{ timeText }}</span>
    </div>
  </div>
</template>

<style scoped>
.msg {
  padding: 12px 0;
}
.msg__actions {
  display: flex;
  align-items: center;
  gap: 2px;
  margin-top: 2px;
  opacity: 0;
  transition: opacity var(--dur-fast) var(--ease-out);
}
.msg:hover .msg__actions,
.msg:focus-within .msg__actions {
  opacity: 1;
}
.msg__time {
  font-size: 11px;
  color: var(--lw-ink-4);
  margin-left: 4px;
}
.msg-user {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
}
.msg-user__bubble {
  width: fit-content;
  max-width: min(82%, 600px);
  border-radius: var(--lw-r-lg-plus);
  padding: 10px 14px;
  background: var(--lw-wash-1);
  font-size: 14px;
  line-height: 1.55;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}
/* 编辑态文本域（§3.34）：气泡同宽同排版，raised 台面 */
.msg-edit {
  width: min(82%, 600px);
  border-radius: var(--lw-r-lg-plus);
  padding: 10px 14px;
  background: var(--lw-bg-elevated);
  box-shadow: var(--lw-shadow-raised);
  font-size: 14px;
  line-height: 1.55;
  resize: none;
  outline: none;
  overflow: hidden;
}
</style>
