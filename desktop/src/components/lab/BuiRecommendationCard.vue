<script setup lang="ts">
/**
 * 【实验室】beautiful-ui-vue RecommendationCard 移植验证件之二
 * （tmp/bui/recommendation-card.vue.txt，2026-10-06，不挂载业务界面）。
 * 移植改动同 BuiApprovalCard（文件头有全表）：演示数据 → props、
 * chrome 文案 → vue-i18n `bui.rc.*`、令牌重映射 --lw-*（--inset→井底、
 * --accent 系→紫系、--shadow-hairline→发丝环）、数值归梯（7px→r-xs、
 * 4px→r-3xs、999px→r-full；100/120ms→instant、180/200ms→base、350ms→slow）、
 * 字面 rgba → color-mix(var(--lw-))。
 */
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";

export interface BuiRcOption {
  key: string;
  description: string;
  code?: string;
  tail?: string;
  tailCode?: string;
  suffix?: string;
  short: string;
  label: string;
  confidence: number;
  tone: string;
  action: string;
}

const props = defineProps<{
  title?: string;
  options?: BuiRcOption[];
}>();

const { t } = useI18n();

const OPTIONS = computed<BuiRcOption[]>(() =>
  props.options ?? [
    { key: "high", description: "从", code: "cone_king", tail: "补货甜筒，交期", tailCode: "7_days", suffix: "。", short: "cone_king · 7 天交期补货", label: "高置信", confidence: 3, tone: "var(--lw-ok)", action: "采纳" },
    { key: "review", description: "旺季把香草切换为", code: "vanilla_madagascar", tail: "。", short: "切换到 vanilla_madagascar", label: "需要复核", confidence: 2, tone: "var(--lw-warn)", action: "去配置" },
    { key: "none", description: "退回全 SKU 整仓补货。", short: "全 SKU 整仓补货", label: "无信号", confidence: 0, tone: "var(--lw-ink-3)", action: "采纳整仓补货" },
  ],
);

const alternativesOpen = ref(false);
const selected = ref(0);
const accepted = ref(false);
const active = computed(() => OPTIONS.value[selected.value]!);
const alts = computed(() =>
  OPTIONS.value.map((option, index) => ({ option, index })).filter((item) => item.index !== selected.value),
);

function choose(index: number) {
  selected.value = index;
  accepted.value = false;
  alternativesOpen.value = false;
}
</script>

<template>
  <div class="bui-rc">
    <div class="bui-rc__card">
      <div class="bui-rc__body">
        <span class="bui-rc__title">{{ props.title ?? t("bui.rc.defaultTitle") }}</span>
        <p :key="active.key" class="bui-rc__desc">
          {{ active.description }}
          <code v-if="active.code" class="bui-rc__param">{{ active.code }}</code>
          {{ active.tail }}
          <code v-if="active.tailCode" class="bui-rc__param">{{ active.tailCode }}</code>{{ active.suffix }}
        </p>
      </div>

      <div
        class="bui-rc__alts-panel"
        :class="{ 'bui-rc__alts-panel--open': alternativesOpen }"
      >
        <div class="bui-rc__alts-body">
          <p class="bui-rc__alts-label">{{ t("bui.rc.otherOptions") }}</p>
          <button
            v-for="alt in alts"
            :key="alt.option.key"
            type="button"
            class="bui-rc__alt"
            @click="choose(alt.index)"
          >
            <span class="bui-rc__alt-bars">
              <span
                v-for="n in 3"
                :key="n"
                class="bui-rc__alt-bar"
                :style="{ background: n <= alt.option.confidence ? alt.option.tone : 'var(--lw-line-strong)' }"
              />
            </span>
            <span class="bui-rc__alt-title">{{ alt.option.short }}</span>
            <span class="bui-rc__alt-label">{{ alt.option.label }}</span>
          </button>
        </div>
      </div>

      <div class="bui-rc__footer">
        <span class="bui-rc__meter">
          <span class="bui-rc__meter-bars">
            <span
              v-for="n in 3"
              :key="n"
              class="bui-rc__meter-bar"
              :style="{ background: n <= active.confidence ? active.tone : 'var(--lw-line-strong)' }"
            />
          </span>
          <span class="bui-rc__meter-label">{{ active.label }}</span>
        </span>
        <span class="bui-rc__actions">
          <button
            type="button"
            class="bui-rc__alt-btn"
            :aria-expanded="alternativesOpen"
            @click="alternativesOpen = !alternativesOpen"
          >
            {{ t("bui.rc.alternatives") }}
          </button>
          <button
            type="button"
            class="bui-rc__accept"
            :class="{ 'bui-rc__accept--accepted': accepted, 'bui-rc__accept--accent': active.key === 'high' && !accepted }"
            @click="accepted = true"
          >
            {{ accepted ? t("bui.rc.accepted") : active.action }}
          </button>
        </span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.bui-rc {
  width: 100%;
  max-width: 380px;
}

.bui-rc__card {
  overflow: hidden;
  border-radius: var(--lw-r-md-plus);
  background: var(--lw-bg-elevated);
  box-shadow: var(--lw-elevation);
}

.bui-rc__body {
  padding: var(--lw-gap-12);
}

.bui-rc__title {
  font-size: var(--lw-fs-13);
  font-weight: 600;
  color: var(--lw-ink-1);
}

.bui-rc__desc {
  margin: var(--lw-gap-6) 0 0;
  font-size: var(--lw-fs-12half);
  line-height: 1.55;
  color: var(--lw-ink-2);
  min-height: 40px;
  animation: bui-rc-fade var(--dur-base) var(--ease-out) both;
}

.bui-rc__param {
  padding: 1px 4px; /* stylelint-disable-line declaration-property-value-disallowed-list -- 光学修正(孤值1,§二⑬) */
  border-radius: var(--lw-r-3xs);
  background: color-mix(in oklab, var(--lw-purple) 12%, transparent);
  font-family: var(--font-mono);
  font-size: var(--lw-fs-11);
  color: var(--lw-purple);
}

.bui-rc__alts-panel {
  display: grid;
  grid-template-rows: 0fr;
  opacity: 0;
  transition: grid-template-rows var(--dur-slow) var(--ease-out),
    opacity var(--dur-slow) var(--ease-out);
}

.bui-rc__alts-panel--open {
  grid-template-rows: 1fr;
  opacity: 1;
}

.bui-rc__alts-body {
  overflow: hidden;
  padding: var(--lw-gap-8);
  border-top: 1px solid var(--lw-line);
  background: var(--lw-bg-inset);
}

.bui-rc__alts-label {
  margin: 0 0 var(--lw-gap-6);
  font-size: var(--lw-fs-11);
  color: var(--lw-ink-3);
}

.bui-rc__alt {
  display: flex;
  width: 100%;
  align-items: center;
  gap: var(--lw-gap-10);
  padding: var(--lw-gap-6) var(--lw-gap-8);
  border: 0;
  border-radius: var(--lw-r-xs);
  background: transparent;
  color: var(--lw-ink-1);
  font-size: var(--lw-fs-12);
  text-align: left;
  cursor: pointer;
  transition: background-color var(--dur-instant) var(--ease-standard);
}

.bui-rc__alt:hover {
  background: var(--lw-wash-2);
}

.bui-rc__alt + .bui-rc__alt {
  margin-top: var(--lw-gap-2);
}

.bui-rc__alt-bars {
  display: flex;
  align-items: flex-end;
  gap: var(--lw-gap-2);
  flex-shrink: 0;
}

.bui-rc__alt-bar {
  width: 4px;
  height: 10px;
  border-radius: var(--lw-r-full);
  background: var(--lw-line-strong);
  transition: background-color var(--dur-base) var(--ease-standard);
}

.bui-rc__alt-title {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.bui-rc__alt-label {
  flex-shrink: 0;
  font-size: var(--lw-fs-11);
  color: var(--lw-ink-3);
}

.bui-rc__footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--lw-gap-10) var(--lw-gap-12);
  border-top: 1px solid var(--lw-line);
  background: var(--lw-bg-inset);
}

.bui-rc__meter {
  display: flex;
  align-items: center;
  gap: var(--lw-gap-8);
}

.bui-rc__meter-bars {
  display: flex;
  align-items: flex-end;
  gap: var(--lw-gap-2);
}

.bui-rc__meter-bar {
  width: 4px;
  height: 10px;
  border-radius: var(--lw-r-full);
  background: var(--lw-line-strong);
  transition: background-color var(--dur-base) var(--ease-standard);
}

.bui-rc__meter-label {
  font-size: var(--lw-fs-12half);
  color: var(--lw-ink-2);
}

.bui-rc__actions {
  display: flex;
  align-items: center;
  gap: var(--lw-gap-8);
}

.bui-rc__alt-btn {
  height: 30px;
  padding: 0 11px; /* stylelint-disable-line declaration-property-value-disallowed-list -- 光学修正(孤值11,§二⑬) */
  border: 0;
  border-radius: var(--lw-r-xs);
  background: var(--lw-bg-elevated);
  color: var(--lw-ink-1);
  font-size: var(--lw-fs-12half);
  font-weight: 500;
  box-shadow: var(--lw-shadow-raised);
  cursor: pointer;
  transition: background-color var(--dur-instant) var(--ease-standard),
    color var(--dur-instant) var(--ease-standard);
}

.bui-rc__alt-btn:hover {
  background: var(--lw-wash-2);
  color: var(--lw-ink-1);
}

.bui-rc__accept {
  height: 30px;
  padding: 0 var(--lw-gap-14);
  border: 0;
  border-radius: var(--lw-r-xs);
  background: var(--lw-ink-1);
  color: var(--lw-bg-elevated);
  font-size: var(--lw-fs-12half);
  font-weight: 500;
  box-shadow: inset 0 1px 0 color-mix(in oklab, var(--lw-bg-elevated) 14%, transparent);
  cursor: pointer;
  transition: transform var(--dur-instant) var(--ease-standard);
}
.bui-rc__accept--accent {
  background: var(--lw-accent);
  color: var(--lw-bg);
}
.bui-rc__accept--accepted {
  background: var(--lw-ok);
  color: var(--lw-bg);
}

@keyframes bui-rc-fade {
  from {
    opacity: 0;
  }
  to {
    opacity: 1;
  }
}

.bui-rc__accept:active {
  transform: scale(0.96);
}

@media (prefers-reduced-motion: reduce) {
  .bui-rc__alts-panel,
  .bui-rc__alts-panel--open {
    transition-duration: 0.01ms !important;
  }
}
</style>
