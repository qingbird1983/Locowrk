<script setup lang="ts">
/**
 * 【实验室】beautiful-ui-vue ApprovalCard 移植验证件（tmp/bui/approval-card.vue.txt
 * → 本文件，2026-10-06 可用性测试，不挂载任何界面）。
 *
 * 验证目标：①该库 copy-paste 源码能否原样进我们工程过 vue-tsc/vite；②其样式
 * （自有 CSS 变量令牌）重映射 --lw-* 的机械性；③i18n 缺口（原源码 11 处界面
 * 文案硬编码英文、演示数据硬编码）接 vue-i18n + props 的工作量。
 *
 * 移植改动清单（相对原源码）：
 * · QUESTIONS 演示数组 → props.questions（数据由调用方传入 = 内容 i18n 归调用方，
 *   与 AskRequest 后端下发中文同构）；组件状态逻辑原样保留。
 * · 11 处界面文案 → vue-i18n `bui.*` 键（zh/en）。
 * · 令牌重映射：--surface→--lw-bg-elevated、--ink→--lw-ink-1、--ink-2/3→--lw-ink-2/3、
 *   --hover→--lw-wash-2、--line-strong→--lw-line-strong、--field→--lw-wash-3、
 *   --green→--lw-ok、--shadow-btn→--lw-shadow-raised、--shadow-hairline→--lw-elevation、
 *   #fff→--lw-bg。
 * · 数值归梯（零视觉意图，就近吸附）：圆角 7px→r-xs、5px→r-2xs、8px→r-xs、
 *   14px→r-md-plus、999px→r-full；时长 100ms→instant、150ms→fast、200ms→base、
 *   300/350ms→slow + ease 字面 → var(--ease-out)。
 * 原源码自带 prefers-reduced-motion 收敛与完整 aria（aria-pressed/aria-label/
 * aria-current），质量良好，原样保留。
 */
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";

export interface BuiQuestion {
  q: string;
  type: "radio" | "check";
  options: string[];
}

const props = defineProps<{
  /** 问题清单（调用方传入；zh demo 缺省仅为实验室演示）。 */
  questions?: BuiQuestion[];
}>();

const { t } = useI18n();

const QUESTIONS = computed<BuiQuestion[]>(() =>
  props.questions ?? [
    { q: "发布几种口味？", type: "radio", options: ["三种（主力线）", "五种（全口味）", "只出一种招牌"] },
    { q: "搭配哪些配料？", type: "check", options: ["巧克力碎", "华夫粒", "糖针"] },
    { q: "先进哪个市场？", type: "radio", options: ["流动餐车", "商超冷柜", "甜品店"] },
  ],
);

const qi = ref(0);
const answers = ref<Record<number, number[]>>({});
const custom = ref<Record<number, string>>({});
const sent = ref(false);
const open = ref(true);

const last = () => qi.value === QUESTIONS.value.length - 1;
const selected = () => answers.value[qi.value] ?? [];
const hasAnswer = () => selected().length > 0 || Boolean((custom.value[qi.value] ?? "").trim());

function toggle(index: number) {
  const current = selected();
  const question = QUESTIONS.value[qi.value]!;

  if (question.type === "radio") {
    answers.value = { ...answers.value, [qi.value]: [index] };
    custom.value = { ...custom.value, [qi.value]: "" };
    window.setTimeout(() => {
      if (qi.value === QUESTIONS.value.length - 1) {
        sent.value = true;
      } else {
        qi.value = Math.min(QUESTIONS.value.length - 1, qi.value + 1);
      }
    }, 480);
  } else {
    const next = current.includes(index)
      ? current.filter((item) => item !== index)
      : [...current, index];
    answers.value = { ...answers.value, [qi.value]: next };
  }
}

function reset() {
  qi.value = 0;
  answers.value = {};
  custom.value = {};
  sent.value = false;
  open.value = true;
}

function nextQuestion() {
  if (last()) {
    sent.value = true;
  } else {
    qi.value = Math.min(QUESTIONS.value.length - 1, qi.value + 1);
  }
}

function prevQuestion() {
  qi.value = Math.max(0, qi.value - 1);
}
</script>

<template>
  <div class="bui-approval">
    <button
      v-if="!open"
      type="button"
      class="bui-approval__open-btn"
      @click="open = true"
    >
      {{ t("bui.openApproval") }}
    </button>

    <div v-else class="bui-approval__wrap">
      <div class="bui-approval__card">
        <div v-if="sent" class="bui-approval__sent">
          <span class="bui-approval__sent-check">
            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round">
              <path d="M20 6L9 17l-5-5" />
            </svg>
          </span>
          <span class="bui-approval__sent-text">{{ t("bui.answersSent") }}</span>
          <button type="button" class="bui-approval__start-over" @click="reset">
            {{ t("bui.startOver") }}
          </button>
        </div>

        <div v-else :key="qi" class="bui-approval__pad">
          <div class="bui-approval__header">
            <span class="bui-approval__question">{{ QUESTIONS[qi]!.q }}</span>
            <button
              type="button"
              :aria-label="t('bui.dismiss')"
              class="bui-approval__dismiss"
              @click="open = false"
            >
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round">
                <path d="M18 6L6 18M6 6l12 12" />
              </svg>
            </button>
          </div>

          <div class="bui-approval__options">
            <button
              v-for="(option, i) in QUESTIONS[qi]!.options"
              :key="option"
              type="button"
              :aria-pressed="selected().includes(i)"
              class="bui-approval__option"
              @click="toggle(i)"
            >
              <span
                class="bui-approval__option-indicator"
                :class="{
                  'bui-approval__option-indicator--radio': QUESTIONS[qi]!.type === 'radio',
                  'bui-approval__option-indicator--check': QUESTIONS[qi]!.type === 'check',
                  'bui-approval__option-indicator--on': selected().includes(i),
                }"
              >
                <span
                  v-if="QUESTIONS[qi]!.type === 'radio'"
                  class="bui-approval__option-dot"
                />
                <svg
                  v-else
                  width="12"
                  height="12"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="3"
                  stroke-linecap="round"
                  stroke-linejoin="round"
                >
                  <path d="M20 6L9 17l-5-5" />
                </svg>
              </span>
              <span
                class="bui-approval__option-label"
                :class="{ 'bui-approval__option-label--on': selected().includes(i) }"
              >{{ option }}</span>
            </button>

            <label class="bui-approval__custom">
              <span aria-hidden="true" class="bui-approval__custom-spacer" />
              <input
                :value="custom[qi] ?? ''"
                class="bui-approval__custom-input"
                :placeholder="t('bui.customPlaceholder')"
                :aria-label="t('bui.customAnswer')"
                @input="
                  (e) => {
                    const val = (e.target as HTMLInputElement).value
                    custom = { ...custom, [qi]: val }
                    if (QUESTIONS[qi]!.type === 'radio') answers = { ...answers, [qi]: [] }
                  }
                "
              />
            </label>
          </div>
        </div>

        <div class="bui-approval__footer">
          <span class="bui-approval__pager">
            <button
              type="button"
              :aria-label="t('bui.prev')"
              :disabled="qi === 0 || sent"
              class="bui-approval__pager-btn"
              @click="prevQuestion"
            >
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
                <path d="M15 18l-6-6 6-6" />
              </svg>
            </button>

            <span class="bui-approval__dots">
              <button
                v-for="(_, i) in QUESTIONS"
                :key="i"
                type="button"
                :aria-label="t('bui.gotoQuestion', { n: i + 1 })"
                :aria-current="i === qi && !sent ? 'step' : undefined"
                :disabled="sent"
                class="bui-approval__dot"
                :class="{
                  'bui-approval__dot--active': i === qi && !sent,
                  'bui-approval__dot--past': sent || i < qi,
                  'bui-approval__dot--future': i > qi && !sent,
                }"
                @click="qi = i"
              />
            </span>

            <button
              type="button"
              :aria-label="t('bui.next')"
              :disabled="last() || sent"
              class="bui-approval__pager-btn"
              @click="nextQuestion"
            >
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
                <path d="M9 6l6 6-6 6" />
              </svg>
            </button>
          </span>

          <button
            v-if="!sent"
            type="button"
            :aria-label="last() ? t('bui.sendAnswers') : t('bui.nextQuestion')"
            :disabled="!hasAnswer()"
            class="bui-approval__send"
            @click="nextQuestion"
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
              <path d="M12 19V5M5 12l7-7 7 7" />
            </svg>
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
/* 令牌重映射见文件头清单；数值归梯为就近吸附（标准 §二 圆角/动效阶梯）。 */
.bui-approval {
  display: flex;
  min-height: 196px;
  width: 100%;
  max-width: 320px;
  flex-direction: column;
  align-items: stretch;
}

.bui-approval__open-btn {
  padding: var(--lw-gap-8) var(--lw-gap-12);
  border: 0;
  border-radius: var(--lw-r-xs);
  background: var(--lw-bg-elevated);
  color: var(--lw-ink-1);
  font-size: var(--lw-fs-12half);
  font-weight: 500;
  box-shadow: var(--lw-shadow-raised);
  cursor: pointer;
  transition: background-color var(--dur-fast) var(--ease-standard);
}

.bui-approval__open-btn:hover {
  background: var(--lw-wash-2);
}

.bui-approval__wrap {
  width: 100%;
}

.bui-approval__card {
  width: 100%;
  overflow: hidden;
  border-radius: var(--lw-r-md-plus);
  background: var(--lw-bg-elevated);
  box-shadow: var(--lw-elevation);
}

.bui-approval__pad {
  padding: var(--lw-gap-14);
  animation: bui-fade-up var(--dur-slow) var(--ease-out) both;
}

.bui-approval__header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--lw-gap-12);
}

.bui-approval__question {
  font-size: var(--lw-fs-13);
  font-weight: 500;
  color: var(--lw-ink-1);
}

.bui-approval__dismiss {
  display: flex;
  width: 24px;
  height: 24px;
  flex-shrink: 0;
  align-items: center;
  justify-content: center;
  padding: 0;
  border: 0;
  border-radius: var(--lw-r-xs);
  background: transparent;
  color: var(--lw-ink-3);
  cursor: pointer;
  transition: background-color var(--dur-instant) var(--ease-standard),
    color var(--dur-instant) var(--ease-standard);
}

.bui-approval__dismiss:hover {
  background: var(--lw-wash-2);
  color: var(--lw-ink-1);
}

.bui-approval__options {
  display: flex;
  flex-direction: column;
  gap: var(--lw-gap-2);
  margin-top: var(--lw-gap-8);
}

.bui-approval__option {
  display: flex;
  align-items: center;
  gap: var(--lw-gap-8);
  margin: 0 -6px; /* stylelint-disable-line declaration-property-value-disallowed-list -- 光学修正(负margin,§二⑬) */
  padding: var(--lw-gap-4) var(--lw-gap-6);
  border: 0;
  border-radius: var(--lw-r-xs);
  background: transparent;
  text-align: left;
  cursor: pointer;
  transition: background-color var(--dur-instant) var(--ease-standard);
}

.bui-approval__option:hover {
  background: var(--lw-wash-2);
}

.bui-approval__option-indicator {
  display: flex;
  width: 16px;
  height: 16px;
  flex-shrink: 0;
  align-items: center;
  justify-content: center;
  transition: background-color var(--dur-base) var(--ease-standard),
    box-shadow var(--dur-base) var(--ease-standard),
    color var(--dur-base) var(--ease-standard);
  color: transparent;
}

.bui-approval__option-indicator--radio {
  border-radius: var(--lw-r-full);
}

.bui-approval__option-indicator--check {
  border-radius: var(--lw-r-2xs);
}

.bui-approval__option-indicator--on {
  background: var(--lw-ink-1);
  color: var(--lw-bg-elevated);
}

.bui-approval__option-indicator:not(.bui-approval__option-indicator--on) {
  box-shadow: inset 0 0 0 1.5px var(--lw-line-strong);
}

.bui-approval__option-dot {
  display: block;
  width: 6px;
  height: 6px;
  border-radius: var(--lw-r-full);
  background: var(--lw-bg-elevated);
  transition: transform var(--dur-base) var(--ease-standard);
  transform: scale(0);
}

.bui-approval__option-indicator--on .bui-approval__option-dot {
  transform: scale(1);
}

.bui-approval__option-label {
  font-size: var(--lw-fs-13);
  color: var(--lw-ink-2);
  transition: color var(--dur-base) var(--ease-standard);
}

.bui-approval__option-label--on {
  color: var(--lw-ink-1);
}

.bui-approval__custom {
  display: flex;
  align-items: center;
  gap: var(--lw-gap-8);
  margin: 0 -6px; /* stylelint-disable-line declaration-property-value-disallowed-list -- 光学修正(负margin,§二⑬) */
  padding: var(--lw-gap-4) var(--lw-gap-6);
  border-radius: var(--lw-r-xs);
  transition: background-color var(--dur-instant) var(--ease-standard);
  cursor: text;
}

.bui-approval__custom:hover,
.bui-approval__custom:focus-within {
  background: var(--lw-wash-2);
}

.bui-approval__custom-spacer {
  display: block;
  width: 16px;
  height: 16px;
  flex-shrink: 0;
}

.bui-approval__custom-input {
  min-width: 0;
  flex: 1;
  border: 0;
  background: transparent;
  color: var(--lw-ink-1);
  font-size: var(--lw-fs-13);
  outline: none;
}

.bui-approval__custom-input::placeholder {
  color: var(--lw-ink-3);
}

.bui-approval__footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--lw-gap-14);
}

.bui-approval__pager {
  display: flex;
  align-items: center;
  gap: var(--lw-gap-8);
}

.bui-approval__pager-btn {
  display: flex;
  width: 24px;
  height: 24px;
  align-items: center;
  justify-content: center;
  padding: 0;
  border: 0;
  border-radius: var(--lw-r-2xs);
  background: transparent;
  color: var(--lw-ink-3);
  cursor: pointer;
  transition: background-color var(--dur-instant) var(--ease-standard),
    color var(--dur-instant) var(--ease-standard);
}

.bui-approval__pager-btn:hover:enabled {
  background: var(--lw-wash-2);
  color: var(--lw-ink-2);
}

.bui-approval__pager-btn:disabled {
  opacity: 0.35;
  cursor: default;
}

.bui-approval__dots {
  display: flex;
  align-items: center;
  gap: var(--lw-gap-4);
}

.bui-approval__dot {
  width: 7px;
  height: 7px;
  padding: 0;
  border: 1.5px solid var(--lw-ink-3);
  border-radius: var(--lw-r-full);
  background: transparent;
  cursor: pointer;
  transition: all var(--dur-slow) var(--ease-standard);
}

.bui-approval__dot--active {
  width: 9px;
  height: 9px;
  border-width: 2.5px;
  border-color: var(--lw-ink-1);
}

.bui-approval__dot--past {
  background: var(--lw-ink-3);
  border-color: var(--lw-ink-3);
}

.bui-approval__dot:disabled {
  cursor: default;
}

.bui-approval__send {
  display: flex;
  width: 28px;
  height: 28px;
  align-items: center;
  justify-content: center;
  margin-right: -2px; /* stylelint-disable-line declaration-property-value-disallowed-list -- 光学修正(负margin,§二⑬) */
  padding: 0;
  border: 0;
  border-radius: var(--lw-r-xs);
  cursor: pointer;
  transition: background-color var(--dur-base) var(--ease-standard),
    color var(--dur-base) var(--ease-standard),
    transform var(--dur-base) var(--ease-standard);
  background: var(--lw-wash-3);
  color: var(--lw-ink-3);
  box-shadow: var(--lw-shadow-raised);
}

.bui-approval__send:enabled {
  background: var(--lw-ink-1);
  color: var(--lw-bg-elevated);
  box-shadow: inset 0 1px 0 color-mix(in oklab, var(--lw-bg-elevated) 14%, transparent);
}

.bui-approval__send:enabled:active {
  transform: scale(0.96);
}

.bui-approval__sent {
  display: flex;
  height: 148px;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--lw-gap-8);
}

.bui-approval__sent-check {
  display: flex;
  width: 24px;
  height: 24px;
  align-items: center;
  justify-content: center;
  border-radius: var(--lw-r-full);
  background: var(--lw-ok);
  color: var(--lw-bg);
  animation: bui-pop-in var(--dur-slow) var(--ease-out) both;
}

.bui-approval__sent-text {
  font-size: var(--lw-fs-13);
  font-weight: 500;
  color: var(--lw-ink-1);
  animation: bui-fade-up var(--dur-slow) var(--ease-out) 100ms both;
}

.bui-approval__start-over {
  margin-top: var(--lw-gap-4);
  padding: 0;
  border: 0;
  background: transparent;
  color: var(--lw-ink-2);
  font-size: var(--lw-fs-12);
  font-weight: 500;
  cursor: pointer;
}

.bui-approval__start-over:hover {
  text-decoration: underline;
  color: var(--lw-ink-1);
}

@keyframes bui-fade-up {
  from {
    opacity: 0;
    transform: translateY(6px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

@keyframes bui-pop-in {
  from {
    opacity: 0;
    transform: scale(0.92);
  }
  to {
    opacity: 1;
    transform: scale(1);
  }
}

@media (prefers-reduced-motion: reduce) {
  .bui-approval__pad,
  .bui-approval__sent-check,
  .bui-approval__sent-text {
    animation-duration: 0.01ms !important;
    animation-iteration-count: 1 !important;
  }
}
</style>
