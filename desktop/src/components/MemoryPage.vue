<script setup lang="ts">
/**
 * 记忆页（壳批次骨架，docs/design/UI shell and patterns.md §3.1 列表页
 * 模式第一实例）：只锁结构与空态。数据命令（memory_list / 编辑 / 删除 /
 * 清空）⑪ 接入（Context and Memory.md §4），行内来源徽章（explicit /
 * extracted）与时间列也随 ⑪ 落地。
 */
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import ListPage from "./patterns/ListPage.vue";

const { t } = useI18n();
const search = ref("");
</script>

<template>
  <div class="flex min-h-0 flex-1 flex-col overflow-hidden">
    <ListPage
      v-model:search="search"
      :is-empty="true"
      :empty-text="t('memory.empty')"
    >
      <!-- ⑪：记忆条目行（内容 / 来源徽章 / 时间 / 编辑·删除）在此填充 -->
    </ListPage>
    <p class="px-5 pb-3 text-[11px] text-[var(--lw-text-4)]">
      {{ t("memory.hint") }}
    </p>
  </div>
</template>
