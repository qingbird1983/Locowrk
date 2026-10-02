<script setup lang="ts">
/**
 * Toast 栈（UI shell 文档 §3.22）：窗口右下，同屏最多 3 条超出排队；
 * 左侧 6px 状态点（不整条染底）；自动消失 4s、hover 暂停计时；入场 =
 * 淡入 + 右侧滑入 8px。挂 body 级一次（App.vue）。
 */
import { onBeforeUnmount, onMounted, watch } from "vue";
import { useToastStore } from "../../stores/toast";

const toasts = useToastStore();
const AUTO_MS = 4000;

// 计时器按 toast id 管理：hover 暂停 = 清除，离开 = 重启 4s；排队项
// 进入 visible 时才起表（自然实现"超出排队"）。
const timers = new Map<number, number>();

function start(id: number) {
  stop(id);
  timers.set(id, window.setTimeout(() => toasts.dismiss(id), AUTO_MS));
}
function stop(id: number) {
  const t = timers.get(id);
  if (t !== undefined) {
    window.clearTimeout(t);
    timers.delete(id);
  }
}

watch(
  () => toasts.visible.map((t) => t.id),
  (ids) => {
    for (const id of [...timers.keys()]) if (!ids.includes(id)) stop(id);
    for (const id of ids) if (!timers.has(id)) start(id);
  },
  { immediate: true },
);

function onKey(e: KeyboardEvent) {
  if (e.key === "Escape" && toasts.items.length > 0) {
    toasts.dismiss(toasts.items[0].id);
  }
}
onMounted(() => window.addEventListener("keydown", onKey));
onBeforeUnmount(() => {
  window.removeEventListener("keydown", onKey);
  for (const t of timers.values()) window.clearTimeout(t);
});
</script>

<template>
  <Teleport to="body">
    <div
      v-if="toasts.items.length > 0"
      class="toast-stack"
      role="status"
      aria-live="polite"
    >
      <div
        v-for="t in toasts.visible"
        :key="t.id"
        class="toast"
        :data-tone="t.tone"
        data-testid="toast"
        @mouseenter="stop(t.id)"
        @mouseleave="start(t.id)"
      >
        <span class="toast__dot" aria-hidden="true" />
        <div class="min-w-0 flex-1">
          <p class="toast__title">{{ t.title }}</p>
          <p v-if="t.body" class="toast__body" :title="t.body">{{ t.body }}</p>
        </div>
        <button
          type="button"
          class="toast__close"
          :aria-label="'关闭'"
          @click="toasts.dismiss(t.id)"
        >
          ✕
        </button>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.toast-stack {
  position: fixed;
  right: 16px;
  bottom: 16px;
  z-index: 50;
  display: flex;
  width: min(420px, calc(100vw - 32px));
  flex-direction: column;
  gap: 8px;
}
.toast {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  min-width: 280px;
  padding: 12px 14px;
  border-radius: var(--lw-r-lg);
  background: var(--lw-bg-elevated);
  border: 1px solid var(--lw-line);
  box-shadow: var(--lw-shadow-dialog);
  animation: toast-in var(--dur-base) var(--ease-out);
}
@keyframes toast-in {
  from {
    opacity: 0;
    transform: translateX(8px);
  }
  to {
    opacity: 1;
    transform: none;
  }
}
@media (prefers-reduced-motion: reduce) {
  .toast {
    animation: none;
  }
}
.toast__dot {
  width: 6px;
  height: 6px;
  flex: 0 0 auto;
  margin-top: 6px;
  border-radius: var(--lw-r-full);
  background: var(--lw-ok);
}
.toast[data-tone="warn"] .toast__dot {
  background: var(--lw-warn);
}
.toast[data-tone="danger"] .toast__dot {
  background: var(--lw-danger);
}
.toast__title {
  font-size: 12.5px;
  color: var(--lw-ink-1);
}
.toast__body {
  margin-top: 2px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 12px;
  color: var(--lw-ink-3);
}
.toast__close {
  flex: 0 0 auto;
  padding: 0 2px;
  font-size: 10px;
  color: var(--lw-ink-4);
}
.toast__close:hover {
  color: var(--lw-ink-2);
}
</style>
