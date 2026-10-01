<script setup lang="ts">
/**
 * 设置分区：记忆注入（settings.json `memory` 区，⑪；Context and
 * Memory.md §6.2）。开关 + 每轮注入条数（1–20，缺省 5）；保存即生效
 * （后端换入 Memory-Inject 活配置，下一轮请求就按新值注入）。
 * 管理入口在顶层"记忆"页（搜索 / 编辑 / 删除 / 清空）。
 */
import { reactive, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useSettingsStore } from "../../stores/settings";

const { t } = useI18n();
const settings = useSettingsStore();

const form = reactive({
  enabled: true,
  topK: 5,
  autoExtract: false,
});

watch(
  () => settings.view,
  (v) => {
    if (!v) return;
    form.enabled = v.memory.enabled;
    form.topK = v.memory.topK;
    form.autoExtract = v.memory.autoExtract;
  },
  { immediate: true },
);

function save() {
  void settings.saveMemory({
    enabled: form.enabled,
    topK: form.topK,
    autoExtract: form.autoExtract,
  });
}
</script>

<template>
  <section class="space-y-3 rounded-xl border border-neutral-800 p-4">
    <h3 class="text-xs font-semibold">{{ t("settings.memoryTitle") }}</h3>
    <p class="text-[11px] text-neutral-500">{{ t("settings.memoryHint") }}</p>

    <label class="flex cursor-pointer items-center gap-2 text-xs">
      <input v-model="form.enabled" type="checkbox" class="accent-emerald-500" />
      <span>{{ t("settings.memoryEnabled") }}</span>
    </label>

    <label class="block space-y-1 text-xs">
      <span class="text-neutral-400">{{ t("settings.memoryTopK") }}</span>
      <input
        v-model.number="form.topK"
        type="number"
        min="1"
        max="20"
        class="block w-32 rounded-lg border border-neutral-700 bg-neutral-800 px-2 py-1.5"
      />
      <span class="text-[11px] text-neutral-600">{{ t("settings.memoryTopKHint") }}</span>
    </label>

    <!-- ⑬ 隐式提取：缺省关（易记垃圾），开启后回合结束异步提事实+经历 -->
    <label class="flex cursor-pointer items-center gap-2 text-xs">
      <input v-model="form.autoExtract" type="checkbox" class="accent-emerald-500" />
      <span>{{ t("settings.memoryAutoExtract") }}</span>
    </label>
    <p class="text-[11px] text-neutral-600">{{ t("settings.memoryAutoExtractHint") }}</p>

    <div class="flex items-center gap-2">
      <button
        class="rounded-lg bg-neutral-700 px-4 py-1.5 text-xs font-semibold transition hover:bg-neutral-600 disabled:cursor-not-allowed disabled:opacity-40"
        :disabled="settings.saving"
        @click="save"
      >
        {{ t("settings.save") }}
      </button>
      <span v-if="settings.savedFlash" class="text-xs text-emerald-400">
        {{ t("settings.saved") }}
      </span>
    </div>
  </section>
</template>
