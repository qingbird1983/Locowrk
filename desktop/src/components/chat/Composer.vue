<script setup lang="ts">
/**
 * composer 坞（UI shell doc §3.11）：绝对定位浮起卡片（r-xl、bg-composer、
 * shadow-composer、无边框），列宽与转录列同源——宽度 = min(100%,
 * var(--lw-content-max))，覆盖值由 ConversationWidthHandles 写在对话区根，
 * 这里不许写死宽度。滚动区在 composer 高度上做底部渐隐 mask（在
 * ChatTranscript），不盖不透明底带。
 *
 * 上行 = 自适应高度文本域（1 行起，透明无边框，14px/1.45，max-h 180）；
 * 下行工具条：左 = 模型 chip（§3.30 模型选择器落地前是状态显示，无菜单、
 * 不收缩除非窄窗）+ 权限模式 chip（随 ㉖ 审批预答后端补位，不发明假档
 * 位）；右 = 用量环（点开 §3.12 浮层）+ 36px 圆形发送键（空闲 accent 实
 * 心上箭头 / 空输入禁用灰 / 运行中换停止字形——轮次中止后端待接入 §8.2，
 * 停止态暂为禁用，如实呈现）。附件（+）与提示词增强（sparkle）随后端能
 * 力补位，不发明占位按钮。Enter 发送 / Shift+Enter 换行（enterToSend 开
 * 关；关 = Ctrl+Enter 发送）；运行中 Enter 忽略。
 */
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useChatStore } from "../../stores/chat";
import { useSettingsStore } from "../../stores/settings";
import { Icon } from "../../lib/icons";
import ContextUsagePopover from "./ContextUsagePopover.vue";

const emit = defineEmits<{ height: [h: number] }>();

const { t } = useI18n();
const chat = useChatStore();
const settings = useSettingsStore();

const draft = ref("");
const ta = ref<HTMLTextAreaElement | null>(null);
const dock = ref<HTMLElement | null>(null);
const showUsage = ref(false);

const enterToSend = computed(() => settings.view?.chat.enterToSend ?? true);
/** 模型 chip = 状态显示（显示当前生效模型，不是愿望显示）。 */
const modelLabel = computed(
  () => settings.view?.llm.model || t("app.modelFallback"),
);

// 用量环：r=5.5 周长 ≈34.56；windowTokens 0 = 窗口未设置（环隐藏）
const pct = computed(() => {
  const u = chat.contextUsage;
  if (!u || u.windowTokens <= 0) return 0;
  return Math.min(100, Math.round((u.used / u.windowTokens) * 100));
});
const ringValue = computed(() => (pct.value / 100) * 34.56);

function autoResize() {
  const el = ta.value;
  if (!el) return;
  el.style.height = "auto";
  el.style.height = `${Math.min(el.scrollHeight, 180)}px`;
}
watch(draft, () => void nextTick(autoResize));

function send() {
  const text = draft.value;
  if (!text.trim() || chat.running) return;
  draft.value = "";
  chat.send(text);
  void nextTick(autoResize);
}

function onKeydown(e: KeyboardEvent) {
  if (e.key !== "Enter" || e.shiftKey) return;
  // 输入法组合中的 Enter 是"确认候选"，不是发送（中文输入必备）
  if (e.isComposing) return;
  const sendKey = enterToSend.value
    ? !e.ctrlKey && !e.metaKey
    : e.ctrlKey || e.metaKey;
  if (!sendKey) return;
  e.preventDefault();
  send();
}

// 坞总高（含底部 16px 内边距）上报：列宽热区下沿要停在 composer 顶部
let ro: ResizeObserver | null = null;
onMounted(() => {
  if (dock.value) {
    ro = new ResizeObserver(() => emit("height", dock.value?.offsetHeight ?? 0));
    ro.observe(dock.value);
    emit("height", dock.value.offsetHeight);
  }
});
onBeforeUnmount(() => ro?.disconnect());

// 浮层外点关闭（§5：小浮层不挂遮罩，点击外部即收）
function onDocPointerDown(e: PointerEvent) {
  if (!showUsage.value) return;
  if (!(dock.value?.contains(e.target as Node) ?? false)) showUsage.value = false;
}
onMounted(() => document.addEventListener("pointerdown", onDocPointerDown));
onBeforeUnmount(() => document.removeEventListener("pointerdown", onDocPointerDown));
</script>

<template>
  <div ref="dock" class="composer-dock">
    <div class="composer conv-w">
      <textarea
        ref="ta"
        v-model="draft"
        rows="1"
        class="composer__input scroll-y"
        :placeholder="t('chat.placeholder')"
        @keydown="onKeydown"
      />
      <div class="composer__toolbar">
        <div class="composer__left">
          <!-- 模型 × 状态：唯一可收缩的 flex 项（min-width:0 + 省略） -->
          <span class="chip chip--model" :title="modelLabel">
            <Icon name="sparkle" :size="14" class="glyph shrink-0" />
            <span class="chip__text">{{ modelLabel }}</span>
          </span>
        </div>
        <div class="composer__right">
          <button
            v-if="chat.contextUsage"
            class="ring-btn"
            type="button"
            :aria-expanded="showUsage"
            :aria-label="`${t('usage.window')} ${pct}%`"
            @click="showUsage = !showUsage"
          >
            <svg
              class="glyph"
              viewBox="0 0 16 16"
              width="14"
              height="14"
              fill="none"
              stroke-width="1.5"
              stroke-linecap="round"
              aria-hidden="true"
            >
              <circle class="ring__track" cx="8" cy="8" r="5.5" />
              <circle
                class="ring__value"
                cx="8"
                cy="8"
                r="5.5"
                :stroke-dasharray="`${ringValue} ${34.56 - ringValue}`"
                transform="rotate(-90 8 8)"
              />
            </svg>
            <span>{{ pct }}%</span>
          </button>
          <button
            class="send-btn"
            type="button"
            :disabled="!draft.trim() || chat.running"
            :aria-label="
              chat.running ? t('chat.stopGeneration') : t('chat.send')
            "
            @click="send"
          >
            <Icon v-if="!chat.running" name="sendUp" :size="14" />
            <Icon v-else name="stop" :size="14" />
          </button>
        </div>
      </div>

      <ContextUsagePopover
        v-if="showUsage && chat.contextUsage"
        :usage="chat.contextUsage"
        :readout="settings.view?.chat.usageReadout ?? 'used'"
        @close="showUsage = false"
      />
    </div>
  </div>
</template>

<style scoped>
.composer-dock {
  position: absolute;
  inset-inline: 0;
  bottom: 0;
  padding: 0 24px 16px;
  z-index: 12;
  pointer-events: none;
}
.composer {
  position: relative;
  width: min(100%, var(--lw-content-max));
  margin-inline: auto;
  border-radius: var(--lw-r-xl);
  background: var(--lw-bg-composer);
  box-shadow: var(--lw-shadow-composer);
  pointer-events: auto;
}
.composer__input {
  display: block;
  width: 100%;
  border: 0;
  outline: none;
  background: none;
  resize: none;
  overflow-y: auto;
  border-radius: var(--lw-r-xl) var(--lw-r-xl) 0 0;
  padding: 12px 14px 4px;
  font-size: 14px;
  line-height: 1.45;
  max-height: 180px;
}
.composer__input::placeholder {
  color: var(--lw-ink-4);
}
.composer__toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 12px;
}
/* 左组固定不缩；右组可缩——窄窗口下先压模型 chip 的文字，而不是溢出卡片 */
.composer__left,
.composer__right {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}
.composer__left {
  flex: 0 1 auto;
  min-width: 0;
}
.composer__right {
  flex: 0 0 auto;
  margin-left: auto;
  justify-content: flex-end;
}
.chip {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
  height: 28px;
  padding: 4px 10px;
  border-radius: var(--lw-r-full);
  background: var(--lw-wash-2);
  font-size: 12.5px;
  line-height: 1.2;
  color: var(--lw-ink-2);
  white-space: nowrap;
}
.chip .glyph {
  color: var(--lw-ink-3);
}
.chip--model {
  flex: 0 1 auto;
  max-width: 260px;
}
.chip__text {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.ring-btn {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  height: var(--lw-ctl);
  padding: 0 8px;
  border-radius: var(--lw-r-full);
  color: var(--lw-ink-2);
  font-size: 12px;
  font-variant-numeric: tabular-nums;
  flex: 0 0 auto;
  transition: background var(--dur-fast) var(--ease-standard),
    color var(--dur-fast) var(--ease-standard);
}
.ring-btn:hover {
  background: var(--lw-wash-2);
  color: var(--lw-ink-1);
}
.ring-btn .ring__track {
  stroke: var(--lw-line-strong);
}
.ring-btn .ring__value {
  stroke: var(--lw-ink-2);
}
.send-btn {
  width: 36px;
  height: 36px;
  border-radius: var(--lw-r-full);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  background: var(--lw-accent);
  color: var(--lw-accent-ink);
  flex: 0 0 auto;
  transition: background var(--dur-fast) var(--ease-standard),
    color var(--dur-fast) var(--ease-standard),
    transform var(--dur-instant) var(--ease-standard);
}
.send-btn:hover:not(:disabled) {
  background: var(--lw-accent-hover);
}
.send-btn:active:not(:disabled) {
  transform: scale(0.96);
}
.send-btn:disabled {
  background: var(--lw-wash-3);
  color: var(--lw-ink-4);
  cursor: default;
}
</style>
