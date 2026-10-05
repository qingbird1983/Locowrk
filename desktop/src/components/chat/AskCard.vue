<script setup lang="ts">
/**
 * 任务选择卡（UI shell doc §3.39①，v4.11）——模型经 `ask_user` 反向提问，
 * composer 坞内渲染（贴合柱面，卡面由 Composer.dock-cards 统一持有）。
 * 版式按用户参考图（ZCode 实卡）：**问题文本本身就是卡头**（13px/600，
 * 无标题行），下接选项行（字母瓦片 A/B/C + label + description；单选点选
 * 即提交，多选点选累积 + 发送提交）+ 底行自由输入（无框 input，没有合适
 * 选项时的兜底）+ 右下「跳过」。子 Agent / 排队数只在存在时以 10.5px
 * 小字行出现在问题上方，常态不干扰。
 *
 * 守夜（后端 AskHub，可配置 tools.askTimeoutSecs，0 = 不限）：卡内首次
 * 交互 ask_snooze 摘除本次倒计时（打字不被超时吞）；倒计时只做显示，
 * 真执行者是后端（到点按跳过收束 → ask-resolved 摘卡）。Esc = 跳过
 * （§3.39⑤ Esc 链第一档，Composer 接线）。
 */
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { askSnooze } from "../../lib/api";
import { useAsksStore } from "../../stores/asks";
import { useChatStore } from "../../stores/chat";
import { Icon } from "../../lib/icons";
import type { AskUserEvent } from "../../types";

const props = defineProps<{
  req: AskUserEvent;
  /** 排队数（除本卡之外的待决数）。 */
  queue: number;
}>();

const { t } = useI18n();
const asks = useAsksStore();
const chat = useChatStore();

/** ⑮ 子 Agent 派生的提问（session_id 非当前会话）打标识。 */
const isSubagent = computed(
  () => !!props.req.session_id && props.req.session_id !== chat.sessionId,
);

/** 倒计时只做显示（§3.39①：无脉冲动效）——0 时后端守夜自动跳过摘卡。 */
const limit = computed(() => props.req.timeout_secs ?? 0);
const secondsLeft = ref(limit.value);
onMounted(() => {
  if (limit.value <= 0) return;
  const timer = window.setInterval(() => {
    if (secondsLeft.value > 0) secondsLeft.value--;
    if (secondsLeft.value <= 0) window.clearInterval(timer);
  }, 1000);
});

/** 首次交互续期（ask_snooze 一次，幂等；后端幂等语义兜底重复调用）。 */
let snoozed = false;
function markInteraction() {
  if (snoozed) return;
  snoozed = true;
  void askSnooze(props.req.request_id).catch(() => {
    /* 续期失败不打断作答 */
  });
}

/** 多选累积集。 */
const picked = ref<Set<number>>(new Set());
const note = ref("");
const submitting = ref(false);
const errorText = ref("");

function toggleOption(i: number) {
  markInteraction();
  errorText.value = "";
  if (props.req.multi_select) {
    const next = new Set(picked.value);
    if (next.has(i)) next.delete(i);
    else next.add(i);
    picked.value = next;
    return;
  }
  // 单选：点选即提交
  void submit([props.req.options[i]!.label]);
}

async function submit(labels: string[]) {
  if (submitting.value) return;
  submitting.value = true;
  errorText.value = "";
  try {
    await asks.answer(props.req.request_id, labels, note.value.trim());
  } catch (err) {
    errorText.value = t("ask.submitFailed", { reason: String(err) });
  } finally {
    submitting.value = false;
  }
}

/** 自由输入行的发送：有选项选择时带选择，纯文本也可（labels 可空）。 */
function sendCustom() {
  markInteraction();
  const labels = [...picked.value].map((i) => props.req.options[i]!.label);
  if (!labels.length && !note.value.trim()) return;
  void submit(labels);
}

function skip() {
  if (submitting.value) return;
  void asks.skip(props.req.request_id).catch((err) => {
    errorText.value = t("ask.submitFailed", { reason: String(err) });
  });
}

/** 守夜进度（宽度百分比；0 = 不限不渲染）。 */
const timeoutPct = computed(() =>
  limit.value > 0 ? Math.max(0, secondsLeft.value / limit.value) * 100 : 0,
);
</script>

<template>
  <section class="ask-card" role="region" :aria-label="t('ask.cardTitle')">
    <!-- 徽标行：只在有子 Agent / 排队时出现（常态卡面 = 参考图的干净形态） -->
    <div v-if="queue > 0 || isSubagent" class="ask-card__flags">
      <span v-if="isSubagent" class="ask-card__sub">{{
        t("approval.subagentTag")
      }}</span>
      <span v-if="queue > 0">{{ t("ask.queue", { n: queue }) }}</span>
    </div>

    <!-- ① 问题行 = 卡头（参考图：无标题行，问题即标题） -->
    <p class="ask-card__question">{{ req.question }}</p>

    <!-- ③ 选项行（字母瓦片 + label + description；单选点选即提交） -->
    <ul v-if="req.options.length" class="ask-card__options">
      <li v-for="(opt, i) in req.options" :key="i">
        <button
          class="ask-card__opt"
          type="button"
          :aria-pressed="req.multi_select ? picked.has(i) : undefined"
          :data-picked="req.multi_select && picked.has(i) ? '' : undefined"
          :disabled="submitting"
          @click="toggleOption(i)"
        >
          <span class="ask-card__tile">{{ String.fromCharCode(65 + i) }}</span>
          <span class="ask-card__opt-body">
            <span class="ask-card__opt-label">{{ opt.label }}</span>
            <span v-if="opt.description" class="ask-card__opt-desc">{{
              opt.description
            }}</span>
          </span>
          <Icon
            v-if="req.multi_select && picked.has(i)"
            name="check"
            :size="14"
            class="ask-card__opt-check"
          />
        </button>
      </li>
    </ul>

    <!-- ④ 底行（参考图）：无框自由输入 + 右下跳过；多选/有输入时出现发送 -->
    <div class="ask-card__custom">
      <input
        v-model="note"
        class="ask-card__input"
        type="text"
        :placeholder="t('ask.customPlaceholder')"
        @focus="markInteraction"
        @keydown.enter.prevent="sendCustom"
      />
      <button
        v-if="req.multi_select || req.options.length === 0 || picked.size || note.trim()"
        class="ask-card__send"
        type="button"
        :disabled="submitting || (!picked.size && !note.trim())"
        @click="sendCustom"
      >
        {{ t("ask.send") }}
      </button>
      <button
        class="ask-card__skip"
        type="button"
        :disabled="submitting"
        @click="skip"
      >
        {{ t("ask.skip") }}
        <span class="ask-card__keycap">Esc</span>
      </button>
    </div>

    <p v-if="errorText" class="ask-card__error">{{ errorText }}</p>

    <!-- 守夜倒计时（§3.17 同款底缘 2px 细条；0 = 不限不渲染） -->
    <div
      v-if="limit > 0"
      class="ask-card__timeout"
      role="timer"
      :title="t('ask.timeoutHint', { n: secondsLeft })"
    >
      <div class="ask-card__timeout-bar" :style="{ width: timeoutPct + '%' }" />
    </div>
  </section>
</template>

<style scoped>
/* 卡面（底色/圆角/暗影）由 Composer.dock-cards 容器统一持有（贴合柱面
 * 语义），本组件只管内容版式。 */
.ask-card {
  position: relative;
  padding: 12px 14px 10px;
}

/* 徽标行：子 Agent / 排队数，只在存在时出现 */
.ask-card__flags {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 4px;
  font-size: 10.5px;
  color: var(--lw-ink-3);
}
.ask-card__sub {
  padding: 1px 8px;
  border-radius: var(--lw-r-full);
  background: color-mix(in oklab, var(--lw-purple) 16%, transparent);
  color: var(--lw-purple);
}

/* 问题即卡头（参考图：无标题行） */
.ask-card__question {
  font-size: 13px;
  font-weight: 600;
  line-height: 1.55;
  color: var(--lw-ink-1);
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}

.ask-card__options {
  list-style: none;
  margin: 8px 0 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.ask-card__opt {
  width: 100%;
  display: flex;
  align-items: flex-start;
  gap: 10px;
  padding: 8px 10px;
  border-radius: var(--lw-r-sm);
  text-align: left;
  transition: background var(--dur-fast) var(--ease-standard);
}
.ask-card__opt:hover:not(:disabled) {
  background: var(--lw-wash-2);
}
.ask-card__opt[data-picked] {
  background: var(--lw-wash-3);
}
.ask-card__opt:disabled {
  opacity: 0.5;
  cursor: default;
}
.ask-card__tile {
  flex: 0 0 26px;
  height: 26px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--lw-r-sm);
  background: var(--lw-wash-3);
  font-size: 12px;
  font-weight: 600;
  color: var(--lw-ink-2);
}
.ask-card__opt[data-picked] .ask-card__tile {
  background: var(--lw-accent);
  color: var(--lw-accent-ink);
}
.ask-card__opt-body {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 1px;
}
.ask-card__opt-label {
  font-size: 12.5px;
  line-height: 1.45;
  color: var(--lw-ink-1);
  overflow-wrap: anywhere;
}
.ask-card__opt-desc {
  font-size: 11.5px;
  line-height: 1.45;
  color: var(--lw-ink-3);
  overflow-wrap: anywhere;
}
.ask-card__opt-check {
  margin-left: auto;
  flex: 0 0 auto;
  color: var(--lw-accent);
}

/* 底行（参考图）：无框自由输入 + 右下跳过 */
.ask-card__custom {
  margin-top: 9px;
  display: flex;
  align-items: center;
  gap: 8px;
}
.ask-card__input {
  flex: 1;
  min-width: 0;
  height: 30px;
  padding: 0 2px;
  background: transparent;
  font-size: 12.5px;
  color: var(--lw-ink-1);
}
.ask-card__input:focus {
  outline: none;
}
.ask-card__input::placeholder {
  color: var(--lw-ink-4);
}
.ask-card__send {
  flex: 0 0 auto;
  height: 28px;
  padding: 0 12px;
  border-radius: var(--lw-r-sm);
  background: var(--lw-accent);
  color: var(--lw-accent-ink);
  font-size: 12px;
  font-weight: 600;
}
.ask-card__send:hover:not(:disabled) {
  background: var(--lw-accent-hover);
}
.ask-card__send:disabled {
  opacity: 0.4;
  cursor: default;
}
.ask-card__skip {
  flex: 0 0 auto;
  margin-left: auto;
  display: inline-flex;
  align-items: center;
  gap: 5px;
  height: 26px;
  padding: 0 8px;
  border-radius: var(--lw-r-sm);
  font-size: 11.5px;
  color: var(--lw-ink-3);
  transition: background var(--dur-fast) var(--ease-standard),
    color var(--dur-fast) var(--ease-standard);
}
.ask-card__skip:hover:not(:disabled) {
  background: var(--lw-wash-2);
  color: var(--lw-ink-1);
}
.ask-card__skip:disabled {
  opacity: 0.4;
}
.ask-card__keycap {
  font-size: 9.5px;
  padding: 0 4px;
  border-radius: var(--lw-r-3xs);
  background: var(--lw-wash-3);
  color: var(--lw-ink-3);
}

.ask-card__error {
  margin-top: 6px;
  font-size: 12px;
  color: var(--lw-danger);
}

.ask-card__timeout {
  position: absolute;
  inset: auto 0 0 0;
  height: 2px;
  background: transparent;
}
.ask-card__timeout-bar {
  height: 100%;
  background: color-mix(in oklab, var(--lw-ink-3) 45%, transparent);
}
</style>
