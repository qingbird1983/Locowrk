<script setup lang="ts">
/**
 * 动作块（UI shell doc §3.7 + 四层折叠 v2.96→v2.99 修订；AF-14 终版 = 用户
 * 2026-10-19 裁定「保留 `✓ 已工作 N 分 N 秒` 那层，删掉组头这层」）：任务容器内的
 * **块级单元**——把一轮里的若干过程行（工具 / 思考 / 错误）拼成一块，块内行距 8px、
 * 块根带 `turn` 类（跨组件兄弟选择器靠它给块间距，见 `TranscriptFlow.vue` 的
 * `.turn + .msg` / `.turn-timer + .turn`）。
 *
 * 曾经的三层组头整层已删：L3 执行中无头、L2 半折叠一行摘要、L1 极简摘要行——用户
 * 裁定一轮里同一件事只写一次，组头那行是与容器计时行重复的第二行。随之各归其位：
 * 走表读数、「已停止」、「N 个问题」下沉到容器计时行（`TurnTimerLine` 的
 * `stopped` / `problems`）；「· N 个工具」不再渲染（轮内动作行自己看得见）。
 * 原先喂给组头的 `phase / defaultExpanded / stopped / toolCount / failures / names`
 * 六个 prop 一并删（`failures` 改由父层汇总到轮头）。
 *
 * 组体自身的折叠也一并删：没有组头就没有点击目标，「折叠工具执行过程」开关本就只作用
 * 在轮容器（`TranscriptFlow.vue` 的 `turnBoxDefault`），留着那套 grid 折叠机就是谁也
 * 点不到的死状态。收起态的安全三条（`overflow` / `pointer-events` / 高度归零）由容器
 * 的卸载与各组件自己的 `v-show` 承担，这里不再自折叠。
 */
</script>

<template>
  <div class="turn"><slot /></div>
</template>

<style scoped>
/* 块根：垂直堆叠 + 8px 行距（v4.14 用户裁定「三层间距统一 8px」）；与轮头同左缘
 * ——不缩进、无引导线，扁平直排（v4.11 用户裁定，照 ZCode 过程流基准）。 */
.turn {
  display: flex;
  flex-direction: column;
  gap: var(--lw-gap-8);
  min-width: 0;
  min-height: 0;
}
</style>
