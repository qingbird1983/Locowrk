<script setup lang="ts">
/**
 * 【实验室】形态库试挂面板（2026-10-06）——beautiful-ui-vue 移植件集中
 * 展示，挂开发者分区（7 连击解锁后可见），供用户目检形态语言。本面板
 * 与移植件均不进任何业务界面；「不配/不行」= git revert 基线之后的
 * 实验提交即可整体回退。组件清单随验证批次增长。
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
