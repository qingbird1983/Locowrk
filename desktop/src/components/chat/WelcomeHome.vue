<script setup lang="ts">
/**
 * 欢迎页（UI shell doc §3.14 v3.9，issue #10⑬⑭）：首页空态独立形态，
 * 与真实对话状态分离——吉祥物 + 四态问候 + composer（Teleport 槽位）组成
 * 居中偏上的块；composer 定宽 720（§3.11 空态变体，列宽柄不渲染）、
 * 双层卡影（底层深色圆角矩形下沉 14px，浮层底边短于暗影）。问候 = 时段
 * 四态（上午好/下午好/晚上好/夜深了）+ 项目名（chat.currentWorkspace，
 * 与工作区 chip 同源，v2.61 用户裁定；无项目 = 不带名的 greetingBare）。
 * 吉祥物
 * 微动效 = 云体浮动 + 周期眨眼，reduced-motion 全停（§6.6 纪律）。
 */
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useChatStore } from "../../stores/chat";
import { workspaceDisplayName } from "../../lib/session-groups";
import { greetingKey } from "../../lib/greeting";

const { t } = useI18n();
const chat = useChatStore();

/** composer 槽位元素：经 defineExpose 交给 App——Teleport 直接吃元素，
 * 不走 document 选择器（初次挂载整棵树还没插进 document，选择器必扑空）。 */
const slotEl = ref<HTMLElement | null>(null);
defineExpose({ slotEl });

const greeting = computed(() => {
  const key = greetingKey();
  // 项目名 = chat.currentWorkspace（与对话框下面工作区 chip **同一真源**，
  // v2.61 用户裁定「把欢迎语项目名跟对话框下面的项目名关联」——此前问候
  // 另有最近会话/fsRoots 回落，chip 显示「选择文件夹」时问候仍挂旧项目名）。
  // null = 未选项目 → 不带名问候（greetingBare）。
  const ws = chat.currentWorkspace;
  if (!ws) return t(`chat.greetingBare.${key}`);
  return t(`chat.greeting.${key}`, { name: workspaceDisplayName(ws) });
});
</script>

<template>
  <div class="welcome">
    <div class="welcome__inner">
      <svg
        class="welcome__mascot"
        viewBox="0 0 72 72"
        fill="none"
        stroke="currentColor"
        stroke-width="1.5"
        stroke-linecap="round"
        stroke-linejoin="round"
        aria-hidden="true"
      >
        <path
          class="welcome__cloud"
          d="M22 47.5a9.5 9.5 0 0 1-1.4-18.9 13 13 0 0 1 25.2-4.6 10 10 0 0 1 1.7 23.5Z"
        />
        <g class="welcome__eyes">
          <circle cx="30.6" cy="32.4" r="1.9" fill="currentColor" stroke="none" />
          <circle cx="41.4" cy="32.4" r="1.9" fill="currentColor" stroke="none" />
        </g>
      </svg>
      <h1 class="welcome__title">{{ greeting }}</h1>
      <!-- composer 槽位（§3.11 空态变体）：实例在 App 挂载，Teleport 进出，
           草稿/浮层状态跨形态保留；暗影层垫底下沉，浮层底边短于暗影 -->
      <div class="welcome__composer">
        <div id="welcome-composer-slot" ref="slotEl" class="welcome__composer-slot" />
      </div>
    </div>
  </div>
</template>

<style scoped>
.welcome {
  display: flex;
  flex: 1 1 auto;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  min-height: 0;
  padding: 0 24px;
}
/* 垂直上移（issue #10）：视觉重心偏上，不是正中 */
.welcome__inner {
  display: flex;
  width: 100%;
  flex-direction: column;
  align-items: center;
  gap: 22px;
  transform: translateY(-7vh);
}
.welcome__mascot {
  width: 72px;
  height: 72px;
  color: var(--lw-ink-1);
  animation: welcome-float 4s var(--ease-standard) infinite;
}
/* 眨眼：两只眼一起压扁（约 4s 一次，120ms 内完成） */
.welcome__eyes {
  transform-origin: 36px 32.4px;
  animation: welcome-blink 4s infinite;
}
@keyframes welcome-float {
  0%,
  100% {
    transform: translateY(0);
  }
  50% {
    transform: translateY(-3px);
  }
}
@keyframes welcome-blink {
  0%,
  92%,
  100% {
    transform: scaleY(1);
  }
  95% {
    transform: scaleY(0.12);
  }
}
.welcome__title {
  font-size: 26px;
  font-weight: 400;
  letter-spacing: -0.02em;
  line-height: 1.2;
  color: var(--lw-ink-1);
  text-align: center;
}
/* 双层卡影（§3.11 v3.9，issue #10）：实底暗影画在卡片自身 box-shadow 上
 * （style.css 欢迎页覆盖）——随卡走，不被卡外工作区底行撑高 */
.welcome__composer {
  position: relative;
  width: min(100%, 720px);
  margin-top: 10px;
}
.welcome__composer-slot {
  position: relative;
}
@media (prefers-reduced-motion: reduce) {
  .welcome__mascot,
  .welcome__eyes {
    animation: none;
  }
}
</style>
