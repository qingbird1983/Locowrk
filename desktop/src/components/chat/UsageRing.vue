<script setup lang="ts">
/**
 * 用量环（UI shell doc §3.12 触发钮，自 Composer 拆出以守单文件上限）：
 * r=5.5 周长 ≈34.56 的环形读数钮 + §3.12 浮层；windowTokens 0 = 窗口未
 * 设置（环隐藏）。开合/外点关闭/Esc 自持，浮层锚钮上方（lw-pop-up 滑动
 * 过渡，§6.6 ②）。
 */
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useChatStore } from "../../stores/chat";
import { useSettingsStore } from "../../stores/settings";
import ContextUsagePopover from "./ContextUsagePopover.vue";

const { t } = useI18n();
const chat = useChatStore();
const settings = useSettingsStore();

const show = ref(false);
const root = ref<HTMLElement | null>(null);

const pct = computed(() => {
  const u = chat.contextUsage;
  if (!u || u.windowTokens <= 0) return 0;
  return Math.min(100, Math.round((u.used / u.windowTokens) * 100));
});
const ringValue = computed(() => (pct.value / 100) * 34.56);

function onDocPointerDown(e: PointerEvent) {
  if (!show.value) return;
  if (!(root.value?.contains(e.target as Node) ?? false)) show.value = false;
}
function onDocKeydown(e: KeyboardEvent) {
  if (show.value && e.key === "Escape") show.value = false;
}
onMounted(() => {
  document.addEventListener("pointerdown", onDocPointerDown);
  document.addEventListener("keydown", onDocKeydown);
});
onBeforeUnmount(() => {
  document.removeEventListener("pointerdown", onDocPointerDown);
  document.removeEventListener("keydown", onDocKeydown);
});
</script>

<template>
  <div v-if="chat.contextUsage" ref="root" class="ring-anchor">
    <button
      class="ring-btn"
      type="button"
      :aria-expanded="show"
      :aria-label="`${t('usage.window')} ${pct}%`"
      @click="show = !show"
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
    <!-- 浮层进退场（标准 §二①）：退场由 Transition 接管（禁止无退场） -->
    <Transition name="lw-pop-up">
      <ContextUsagePopover
        v-if="show && chat.contextUsage"
        :usage="chat.contextUsage"
        :readout="settings.view?.chat.usageReadout ?? 'used'"
        @close="show = false"
      />
    </Transition>
  </div>
</template>

<style scoped>
.ring-anchor {
  position: relative;
  display: inline-flex;
  flex: 0 0 auto;
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
.ring__track {
  stroke: var(--lw-line-strong);
}
.ring__value {
  stroke: var(--lw-ink-2);
}
</style>
