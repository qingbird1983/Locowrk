<script setup lang="ts">
/**
 * 【实验室】形态库试挂面板（2026-10-06）——beautiful-ui-vue 移植件集中
 * 展示，挂开发者分区（7 连击解锁后可见），供用户目检形态语言。本面板
 * 与移植件均不进任何业务界面；「不配/不行」= git revert 基线之后的
 * 实验提交即可整体回退。组件清单随验证批次增长。
 *
 * 位置口径（台账 T4 · 2026-10-18）：这块住在 `desktop/lab/`，**在 `src/` 之外**——
 * 入口仍在本仓唯一的挂载点 `src/components/settings/DeveloperSection.vue`，但那条
 * 挂载由 DEV 编译期常量门控，生产构建里整条折掉 ⇒ 产物不含这 989 行，
 * 生产形态闸（stylelint / check-style）也不扫它：**待裁决的形态样本不受
 * 「同类只许一种形态」管**，这正是"搬出生产目录"的语义之一。哪天某等形态被采纳，
 * 是把它搬回 `src/components/` 并当场过形态闸，而不是在 lab 里直接接业务。
 * 类型面没跟着搬出去：前端 tsconfig 的 include 里加了 `lab` 目录下所有 .vue，
 * `vue-tsc` 照样编它——否则这块会在没人看得见的位置上漂坏（搬出去的是产物归属，
 * 不是"从此没人管"）。（注：这里不能把那条 include 原文抄成 glob——块注释里出现
 * 星号斜杠会提前闭合注释，本行第一版就是这么把 `vue-tsc` 打红的。）
 */
import BuiApprovalCard from "./BuiApprovalCard.vue";
import BuiRecommendationCard from "./BuiRecommendationCard.vue";
</script>

<template>
  <div class="lab-panel">
    <p class="lab-panel__hint">
      形态库实验室（beautiful-ui-vue 移植验证件，仅开发者可见）——看形态语言是否采纳：
    </p>
    <div class="lab-panel__grid">
      <div class="lab-panel__cell">
        <p class="lab-panel__name">ApprovalCard（审批卡·多题向导）</p>
        <BuiApprovalCard />
      </div>
      <div class="lab-panel__cell">
        <p class="lab-panel__name">RecommendationCard（推荐卡·置信度）</p>
        <BuiRecommendationCard />
      </div>
    </div>
  </div>
</template>

<style scoped>
.lab-panel {
  display: flex;
  flex-direction: column;
  gap: var(--lw-gap-10);
}
.lab-panel__hint {
  font-size: var(--lw-fs-12);
  color: var(--lw-ink-3);
}
.lab-panel__grid {
  display: flex;
  flex-wrap: wrap;
  gap: var(--lw-gap-16);
}
.lab-panel__cell {
  display: flex;
  flex-direction: column;
  gap: var(--lw-gap-6);
}
.lab-panel__name {
  font-size: var(--lw-fs-12);
  font-weight: 600;
  color: var(--lw-ink-2);
}
</style>
