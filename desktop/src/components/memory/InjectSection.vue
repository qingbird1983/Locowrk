<script setup lang="ts">
/**
 * 队列 Q10（ML 稿 §3.1 原则 4 / §5.1 L4）：注入解释**只读**区块——最近一次
 * "真算过四信号"的注入为什么这样排。自持状态，打开拉一次 + 手动刷新
 * （与同页 TreeSection、⑫ 暴露面板同款：不推送不轮询）。
 *
 * ⚠ **占比用 mono 数字表达，不造条形**：四段占比条是《Front-end Design
 * Guidelines》清单外形态（§零.0 自发明判错 + §零.5 特例申请制），本批没有申请，
 * 就不该顺手造一个。数字一样回答"哪一档把这条抬上来的"，而"同类只许一种形态"
 * 这条纪律的价值不在于这次少画一个条。
 *
 * ⚠ **三态分开**（§二⑩ 空态 + UI shell「加载中 ≠ 空态」）：还没拉到 / 库里没有
 * 可解释的行 / 有回执但读不出来，是三个不同结论，各写各的文案。
 */
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { memoryInjectView } from "../../lib/api";
import { relativeTimeLabel, relativeTimeSec } from "../../lib/format";
import type { InjectRow, InjectView } from "../../types";

const { t } = useI18n();

// null = 还没拉到（加载中）；拿到后 explain.present 才决定"有没有可解释的那一次"
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

/** 四档标签**按位**取：顺序 = 内核 `LearnWeights` 字段序（相关性, 强度, 新鲜度, 时间），
 * 与 Rust 侧 `inject_view::ExplainRow::share` 同一个约定，不在前端重排。 */
const TIERS = [
  "memory.injectRelevance",
  "memory.injectStrength",
  "memory.injectFreshness",
  "memory.injectRecency",
] as const;

/** 比率读数：null 与 0 不是一回事（null = 没注入过 / 没键可算，见 InjectOverview）。 */
function ratio(v: number | null): string {
  return v === null ? t("memory.injectUnknown") : `${Math.round(v * 100)}%`;
}

/** 回执里权重四档的读数（面板要说"当时是哪一档拧到多少"）。 */
function weight(i: number): string {
  return String(view.value?.explain.weights[i] ?? 0);
}

function shares(row: InjectRow): string {
  return row.share.map((v, i) => `${t(TIERS[i])} ${v}`).join(" · ");
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

    <template v-else-if="view">
      <!-- L0 派生读数一行：注入次数 / 空块率 / 注入产出率 -->
      <p class="mt-2 text-[length:var(--lw-fs-12)] text-ink-3">
        {{
          t("memory.injectOverview", {
            n: view.overview.injections,
            empty: ratio(view.overview.store_empty_ratio),
            yielded: ratio(view.overview.injection_yield),
            keys: view.overview.injected_keys,
            used: view.overview.yielded_keys,
          })
        }}
      </p>

      <!-- 空态：库里一次可解释的注入都没有（学习档关着 / 还没跑过对话） -->
      <p v-if="!view.explain.present" class="mt-2 text-[length:var(--lw-fs-12)] text-ink-4">
        {{ t("memory.injectEmpty") }}
      </p>

      <!-- 有回执但读不成面板认得的形状：如实说坏，不演成"没有" -->
      <p
        v-else-if="view.explain.unreadable"
        class="mt-2 rounded-[var(--lw-r-xs)] bg-danger/10 px-3 py-2 text-xs text-danger"
      >
        {{ t("memory.injectUnreadable") }}
      </p>

      <template v-else>
        <p class="mt-1 text-[length:var(--lw-fs-11)] text-ink-5">
          {{
            t("memory.injectAt", {
              when: relativeTimeLabel(relativeTimeSec(view.explain.ts), (k, p) => t(k, p)),
              session: view.explain.session_key,
            })
          }}
        </p>
        <p class="mt-1 text-[length:var(--lw-fs-11)] text-ink-5">
          {{
            t("memory.injectWeights", {
              a: weight(0),
              b: weight(1),
              c: weight(2),
              d: weight(3),
            })
          }}
        </p>

        <!-- 空交付：那是"什么都没进上下文"，不是坏数据（Q1 空块率判的就是这种行） -->
        <p v-if="view.explain.rows.length === 0" class="mt-2 text-[length:var(--lw-fs-12)] text-ink-4">
          {{ t("memory.injectNoDelivery") }}
        </p>

        <ul v-else class="mt-2 space-y-2">
          <li
            v-for="row in view.explain.rows"
            :key="row.key"
            class="rounded-[var(--lw-r-md)] bg-wash-1 px-4 py-3"
          >
            <div class="flex items-center gap-2">
              <span class="min-w-0 flex-1 truncate font-mono text-xs text-ink-1" :title="row.key">
                {{ row.key }}
              </span>
              <span class="font-mono text-[length:var(--lw-fs-11)] text-ink-4">
                {{ t("memory.injectTotal", { total: row.total }) }}
              </span>
            </div>
            <p class="mt-1 font-mono text-[length:var(--lw-fs-11)] text-ink-3">
              {{ shares(row) }}
            </p>
          </li>
        </ul>
      </template>
    </template>
  </div>
</template>
