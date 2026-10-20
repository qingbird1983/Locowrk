<script setup lang="ts">
/**
 * 设置分区：外观（⑰，settings.json `appearance` 区）。主题三档 + 字号三档，
 * 保存即生效（lib/theme.ts 写 <html data-theme>、lib/ui-font.ts 写
 * --lw-ui-font-size；§六 / UI Design System 附录 B）。
 * 行版式（§3.32·七 v3.5）：左行名右控件，就地生效无保存按钮。
 * 分段控件走 SettingsSegmented（v3.6 两色控件体系：白底台面 + 选中灰）。
 *
 * S4 刀④ 数值归类：字号从 12–18 数字框收成小/中/大三档（决策人 2026-10-20
 * 原话：用户视角只留开/关或预置，不填数值猜最优）。存量非档值（旧手填的
 * 13/15）由 nearestUiFontPreset 就近显示，**不改磁盘值**——点选才落档。
 */
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useSettingsStore } from "../../stores/settings";
import {
  nearestUiFontPreset,
  UI_FONT_DEFAULT,
  UI_FONT_MAX,
  UI_FONT_MIN,
  UI_FONT_PRESETS,
} from "../../lib/ui-font";
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

/** 档位 → 文案键：以数值为键，组件不知道 lib/ui-font.ts 的内部顺序。 */
const FONT_LABEL_KEYS: Record<number, string> = {
  [UI_FONT_MIN]: "settings.fontSizeSmall",
  [UI_FONT_DEFAULT]: "settings.fontSizeMedium",
  [UI_FONT_MAX]: "settings.fontSizeLarge",
};

const fontOptions = computed(() =>
  UI_FONT_PRESETS.map((px) => ({
    value: String(px),
    label: t(FONT_LABEL_KEYS[px] ?? "settings.fontSizeTitle"),
  })),
);

/** 分段控件绑 string；点选 = 立即落该档。投影里的非档值就近显示（见头注）。 */
const fontPreset = computed({
  get: () =>
    String(
      nearestUiFontPreset(settings.view?.appearance.uiFontSize ?? UI_FONT_DEFAULT),
    ),
  set: (v: string) => void settings.setUiFontSize(Number(v)),
});
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

    <SettingsRow :title="t('settings.fontSizeTitle')" :hint="t('settings.fontSizeHint')">
      <SettingsSegmented
        v-model="fontPreset"
        :options="fontOptions"
        :label="t('settings.fontSizeTitle')"
      />
    </SettingsRow>
  </SettingsGroup>
</template>
