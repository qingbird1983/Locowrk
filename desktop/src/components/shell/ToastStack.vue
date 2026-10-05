<script setup lang="ts">
/**
 * Toast 栈（UI shell 文档 §3.22 + 前端标准 §二①）：水平居中、垂直居中
 * 偏上一小段；同屏最多 3 条超出排队；左侧 6px 状态点（不整条染底）；自动
 * 消失 4s、hover 暂停计时；入场 = 上方 -8px 滑入缓冲，退场 = 收回淡出
 * （TransitionGroup，离开项移动类管重排）。挂 body 级一次（App.vue）。
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
      <!-- 退场 = 收回淡出（标准 §二①：toast 必须有退场），move 类管重排 -->
      <TransitionGroup name="toast">
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
      </TransitionGroup>
    </div>
  </Teleport>
</template>

<style scoped>
/* 位置（用户裁定 + 2026-10-06 目检修正）：水平居中、垂直居中偏上的上部
 * （40vh 被读成「正居中」，提至 30vh）；宽度统一 min(360px, 100vw−32px)
 * （目检裁定 420 过长） */
.toast-stack {
  position: fixed;
  top: 30vh;
  left: 50%;
  transform: translateX(-50%);
  z-index: var(--lw-z-toast);
  display: flex;
  width: min(360px, calc(100vw - 32px));
  flex-direction: column;
  gap: 8px;
}
.toast {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  min-width: 280px;
  padding: 12px 14px;
  border-radius: var(--lw-r-md-plus);
  background: var(--lw-bg-elevated);
  border: 1px solid var(--lw-line);
  box-shadow: var(--lw-shadow-pop);
}
.toast-enter-active {
  transition:
    opacity var(--dur-base) var(--ease-out),
    transform var(--dur-base) var(--ease-out);
}
.toast-enter-from {
  opacity: 0;
  transform: translateY(-8px);
}
.toast-leave-active {
  transition:
    opacity var(--dur-fast) var(--ease-in),
    transform var(--dur-fast) var(--ease-in);
}
.toast-leave-to {
  opacity: 0;
  transform: translateY(-4px);
}
.toast-move {
  transition: transform var(--dur-base) var(--ease-out);
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
