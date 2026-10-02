<script setup lang="ts">
/**
 * 设置分区：记忆注入（settings.json `memory` 区，⑪；Context and
 * Memory.md §6.2）。开关 + 每轮注入条数（1–20，缺省 5）；保存即生效
 * （后端换入 Memory-Inject 活配置，下一轮请求就按新值注入）。
 * 管理入口在顶层"记忆"页（搜索 / 编辑 / 删除 / 清空）。
 * 行版式（§3.32·七 v3.5）：布尔项用 SettingsToggle。
 */
import { reactive, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useSettingsStore } from "../../stores/settings";
import SettingsGroup from "./SettingsGroup.vue";
import SettingsRow from "./SettingsRow.vue";
import SettingsSaveRow from "./SettingsSaveRow.vue";
import SettingsToggle from "./SettingsToggle.vue";

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
  <SettingsGroup :hint="t('settings.memoryHint')">
    <SettingsRow :title="t('settings.memoryEnabled')">
      <SettingsToggle v-model="form.enabled" :label="t('settings.memoryEnabled')" />
    </SettingsRow>

    <SettingsRow
      :title="t('settings.memoryTopK')"
      :hint="t('settings.memoryTopKHint')"
      control-id="memory-topk"
    >
      <input
        id="memory-topk"
        v-model.number="form.topK"
        type="number"
        min="1"
        max="20"
        class="lw-field lw-field-num px-2 text-xs"
      />
    </SettingsRow>

    <!-- ⑬ 隐式提取：缺省关（易记垃圾），开启后回合结束异步提事实+经历 -->
    <SettingsRow
      :title="t('settings.memoryAutoExtract')"
      :hint="t('settings.memoryAutoExtractHint')"
    >
      <SettingsToggle v-model="form.autoExtract" :label="t('settings.memoryAutoExtract')" />
    </SettingsRow>
  </SettingsGroup>

  <SettingsSaveRow class="mt-3" @save="save" />
</template>
