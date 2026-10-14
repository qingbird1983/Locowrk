<script setup lang="ts">
/**
 * 设置分区：外观（⑰，settings.json `appearance` 区）。主题三档 + 字号基座，
 * 保存即生效（lib/theme.ts 写 <html data-theme>、lib/ui-font.ts 写
 * --lw-ui-font-size；§六 / UI Design System 附录 B）。
 * 行版式（§3.32·七 v3.5）：左行名右控件，就地生效无保存按钮。
 * 分段控件走 SettingsSegmented（v3.6 两色控件体系：白底台面 + 选中灰）；
 * 数字输入走 lw-field lw-field-num（对话列宽与开发者区同一形态）。
 */
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useSettingsStore } from "../../stores/settings";
import { UI_FONT_DEFAULT, UI_FONT_MAX, UI_FONT_MIN } from "../../lib/ui-font";
import SettingsGroup from "./SettingsGroup.vue";
import SettingsRow from "./SettingsRow.vue";
import SettingsSegmented from "./SettingsSegmented.vue";

const { t } = useI18n();
const settings = useSettingsStore();

const theme = computed({
  get: () => settings.view?.appearance.theme ?? "dark",
  set: (v: string) => void settings.setTheme(v),
});

const options = computed(() => [
  { value: "system", label: t("settings.themeSystem") },
  { value: "light", label: t("settings.themeLight") },
  { value: "dark", label: t("settings.themeDark") },
]);

/** 字号基座的**草稿**（队列 Q14 第四刀）。与分段控件不同类：数字框在用户敲字符
 *  的过程中不能每次都发 IPC，所以草稿与投影分开、change（blur / 回车）才提交——
 *  这是对话列宽与开发者区数值行的既有分工，不是本刀新立的形态。 */
const fontDraft = ref<number>(UI_FONT_DEFAULT);

watch(
  () => settings.view?.appearance.uiFontSize,
  (v) => {
    if (typeof v === "number") fontDraft.value = v;
  },
  { immediate: true },
);

/** 提交门：越界（含清空后的空串 / NaN）**不发 IPC**，直接把草稿弹回当前生效值。
 *  这不是给后端加一套平行校验——落盘权威仍在后端（`settings/update.rs` 越界一律拒，
 *  并由 `settings/tests/appearance.rs` 三条用例与变异靶承重）；这里拦的是"注定被拒
 *  的一趟往返"，因为红条说的是 12–18，用户看到的却是一个空白格，比弹回原值更难懂。 */
async function saveFontSize() {
  const px = Math.round(Number(fontDraft.value));
  if (Number.isInteger(px) && px >= UI_FONT_MIN && px <= UI_FONT_MAX) {
    await settings.setUiFontSize(px);
  }
  fontDraft.value = settings.view?.appearance.uiFontSize ?? UI_FONT_DEFAULT;
}
</script>

<template>
  <SettingsGroup>
    <SettingsRow :title="t('settings.themeTitle')" :hint="t('settings.themeHint')">
      <SettingsSegmented
        v-model="theme"
        :options="options"
        :label="t('settings.themeTitle')"
      />
    </SettingsRow>

    <SettingsRow
      :title="t('settings.fontSizeTitle')"
      :hint="t('settings.fontSizeHint', { min: UI_FONT_MIN, max: UI_FONT_MAX })"
      control-id="appearance-font-size-input"
    >
      <input
        id="appearance-font-size-input"
        v-model.number="fontDraft"
        type="number"
        :min="UI_FONT_MIN"
        :max="UI_FONT_MAX"
        step="1"
        class="lw-field lw-field-num px-2 text-xs"
        @change="saveFontSize"
      />
    </SettingsRow>
  </SettingsGroup>
</template>
