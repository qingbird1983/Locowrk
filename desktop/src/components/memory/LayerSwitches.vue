<script setup lang="ts">
/**
 * 层区（S1 刀④b · ADR-0007 §18 的界面兑现：「全局记忆 / 项目记忆 / 项目交接层
 * 各关各的」= 三枚行为开关，每层一行）。
 *
 * 为什么独立成一个组件（`MemoryPage.vue` 本批因此从 413 行回到闸内）：三枚开关的形状
 * 完全相同、只有键位与文案不同，写在页面里就是三份并排模板；刀④c 还要在这一区给每层
 * 补「文件数 / 更新时间 / 删除该层」，那一维只需要在**这一处**长。
 *
 * 为什么落点在记忆页而不是设置页：ADR 那句「记忆管理页 = 设置页同款」指的是这一页要
 * 长成"每层一行 + 每层一个开关"。同一枚开关在两页各放一个是两处真相，所以设置页那行
 * `repoContext` 撤了（字段仍随 view 回填、原样保存）。放在事实库档而不是看板档：看板是
 * 只读导航，不该带控件。
 *
 * 为什么**切换即写回**、不给这一区加「保存」行：本页每一个动作——删除、清空、行内编辑
 * ——都是点一下就落盘，它不是表单页；加保存行会让"改了没保存"变成这一页的新状态，
 * 而那正是设置页已经付过的代价。
 */
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import SettingsGroup from "../settings/SettingsGroup.vue";
import SettingsRow from "../settings/SettingsRow.vue";
import SettingsToggle from "../settings/SettingsToggle.vue";
import { useSettingsStore } from "../../stores/settings";

const { t } = useI18n();
const settings = useSettingsStore();
const layerView = computed(() => settings.view?.memory ?? null);

/**
 * 三枚开关 = 一张表。`save` 各写自己的 patch，不用 `{ [key]: on }` 那种计算键——
 * 它在类型面上退化成索引签名，"这三枚字段都属于 MemoryUpdate"这件事就查不出来了
 * （字段名漂一个，编译不红、静默存一个没人读的键）。
 *
 * `?? true` 是**视图还没加载完**那一瞬的显示兜底，不是第二处缺省定义：后端那三枚
 * 缺省常量（`GLOBAL_LAYER_DEFAULT` / `PROJECT_LAYER_DEFAULT` / `REPO_CONTEXT_DEFAULT`）
 * 都是 `true`，加载完成后这一行读回的就是真值。
 */
const switches = [
  {
    key: "globalLayer",
    title: t("settings.memoryGlobalLayer"),
    hint: t("settings.memoryGlobalLayerHint"),
    on: () => layerView.value?.globalLayer ?? true,
    save: (on: boolean) => settings.saveMemory({ globalLayer: on }),
  },
  {
    key: "projectLayer",
    title: t("settings.memoryProjectLayer"),
    hint: t("settings.memoryProjectLayerHint"),
    on: () => layerView.value?.projectLayer ?? true,
    save: (on: boolean) => settings.saveMemory({ projectLayer: on }),
  },
  {
    key: "repoContext",
    title: t("settings.memoryRepoContext"),
    hint: t("settings.memoryRepoContextHint"),
    on: () => layerView.value?.repoContext ?? true,
    save: (on: boolean) => settings.saveMemory({ repoContext: on }),
  },
];
</script>

<template>
  <div class="shrink-0 px-5 pb-2 pt-3">
    <SettingsGroup
      :title="t('settings.memoryLayers')"
      :hint="t('settings.memoryLayersHint')"
    >
      <SettingsRow
        v-for="s in switches"
        :key="s.key"
        :title="s.title"
        :hint="s.hint"
      >
        <SettingsToggle
          :model-value="s.on()"
          :label="s.title"
          @update:model-value="s.save"
        />
      </SettingsRow>
    </SettingsGroup>
  </div>
</template>
