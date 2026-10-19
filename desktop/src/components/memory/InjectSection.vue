<script setup lang="ts">
/**
 * 队列 Q10（L4）立的注入侧观测区块 —— **S3 刀③b 之后只剩 L0 那一行读数**。
 * 自持状态，打开拉一次 + 手动刷新（与同页 TreeSection、⑫ 暴露面板同款：不推送不轮询）。
 *
 * ⚠ 摘掉的是"最近一次为什么这样排"那一半：四档占比、当时的权重读数、以及
 * `present` / `unreadable` 两态。它们的 producer（`memory_learn` 写的四信号回执）随
 * 学习档整族退役（裁定 = specs/10 开工三问②）⇒ 留着就是一块恒空的框。
 * 原先这两条纪律记在这里：① **占比用 mono 数字表达，不造条形**（§零.0 自发明判错 +
 * §零.5 特例申请制）——读数没了就没有形态要判，这条随它离场；② **三态分开**（§二⑩
 * 空态 + UI shell「加载中 ≠ 空态」）——现在只剩「还没拉到」与「有数」两态，
 * "库里没有可解释的那一次"与"有回执但读不出来"随数据源一起消失。
 *
 * 为什么这块不整片删：L0 那三个派生读数不依赖任何分数，检索回执照常进分母，
 * 它仍是界面上唯一能回答"记忆到底有没有进过上下文"的读数。将来若要恢复逐条视图，
 * 正解是给索引段埋点（specs/10 欠账 ⓠ）。
 */
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { memoryInjectView } from "../../lib/api";
import type { InjectView } from "../../types";

const { t } = useI18n();

// null = 还没拉到（加载中 ≠ 空态，两态各有文案）
const view = ref<InjectView | null>(null);
const loading = ref(false);
const loadError = ref("");

async function refresh() {
  loading.value = true;
  loadError.value = "";
  try {
    view.value = await memoryInjectView();
  } catch (e) {
    loadError.value = String(e);
  } finally {
    loading.value = false;
  }
}
onMounted(() => void refresh());

/** 比率读数：null 与 0 不是一回事（null = 没注入过 / 没键可算，见 InjectOverview）。 */
function ratio(v: number | null): string {
  return v === null ? t("memory.injectUnknown") : `${Math.round(v * 100)}%`;
}
</script>

<template>
  <div class="border-t border-line px-5 py-3">
    <!-- 区块头：标题 + 口径自证 + 刷新（§二⑨ 区块头 12/600） -->
    <div class="flex items-center gap-2">
      <h3 class="text-xs font-semibold text-ink-1">
        {{ t("memory.injectTitle") }}
      </h3>
      <span class="text-[length:var(--lw-fs-10)] text-ink-5">{{ t("memory.injectHint") }}</span>
      <button
        class="ml-auto rounded-[var(--lw-r-xs)] px-2 py-1 text-[length:var(--lw-fs-11)] text-ink-4 transition hover:bg-wash-2 hover:text-ink-1"
        :disabled="loading"
        @click="refresh"
      >
        {{ t("common.refresh") }}
      </button>
    </div>

    <p v-if="loadError" class="mt-2 rounded-[var(--lw-r-xs)] bg-danger/10 px-3 py-2 text-xs text-danger">
      {{ loadError }}
    </p>

    <!-- 加载中（≠ 空态：文案不共用，否则"正在读"与"库里没货"看不出来） -->
    <p v-else-if="loading && !view" class="mt-2 text-[length:var(--lw-fs-12)] text-ink-4">
      {{ t("memory.injectLoading") }}
    </p>

    <!-- L0 派生读数一行：注入次数 / 空块率 / 注入产出率（零注入时那三个数自己会说话） -->
    <p v-else-if="view" class="mt-2 text-[length:var(--lw-fs-12)] text-ink-3">
      {{
        t("memory.injectOverview", {
          n: view.overview.injections,
          empty: ratio(view.overview.store_empty_ratio),
          // ⚠ 这个插值名原先两边不同源：这里传 `yielded`，而两份文案里写的是 `{yield}`
          // ⇒ vue-i18n 对取不到的参数**吐空串**，于是"注入产出率"这个读数在界面上永远
          // 不显示，只剩后面的（4/5）。这是 S3 刀③b 重写本区块时由新用例（断言 `80%`
          // 上屏）抓出来的既有缺陷——旧用例只断了 title 与占比行，从没断过这个数。
          // 「同一读数两个出处必漂」在 i18n 上的形态：键名也是读数，改一边不改另一边
          // 不报错、不红闸，只表现为界面少一个数。
          rate: ratio(view.overview.injection_yield),
          keys: view.overview.injected_keys,
          used: view.overview.yielded_keys,
        })
      }}
    </p>
  </div>
</template>
