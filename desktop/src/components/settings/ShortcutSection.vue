<script setup lang="ts">
/**
 * 设置分区：全局快捷键（settings.json `shortcut` 区，保存即重注册）。
 * v3.6（用户裁定"快捷键可自定义"）：录制式改键（SettingsKeyRecorder）+
 * 「恢复默认」（回落 Ctrl+Shift+L = 后端 SHORTCUT_DEFAULT）。提交走
 * saveShortcut（后端 parse 校验 + apply_shortcut 即时重注册；键位被占用
 * 时保存失败、错误条走板面级 settings.error，如实呈现）。
 * v3.7：「恢复默认」与组头说明**同一行**（#action 槽），mr-3 补齐组头
 * px-1 → 右缘与下方录制钮右缘（行瓦片 16px 内边距）对齐；同宽 160px +
 * 前置 undo 图标（§3.32·七，用户裁定"说明与按钮一行"）。
 * 行版式（§3.32·七 v3.5）：左行名右控件。
 */
import { useI18n } from "vue-i18n";
import { Icon } from "../../lib/icons";
import { useSettingsStore } from "../../stores/settings";
import SettingsGroup from "./SettingsGroup.vue";
import SettingsKeyRecorder from "./SettingsKeyRecorder.vue";
import SettingsRow from "./SettingsRow.vue";

/** 与后端 settings.rs SHORTCUT_DEFAULT 一致（恢复默认回落值）。 */
const SHORTCUT_DEFAULT = "Ctrl+Shift+L";

const { t } = useI18n();
const settings = useSettingsStore();

function commit(accelerator: string) {
  void settings.saveShortcut(accelerator);
}

function restoreDefault() {
  void settings.saveShortcut(SHORTCUT_DEFAULT);
}
</script>

<template>
  <SettingsGroup
    :title="t('settings.shortcutTitle')"
    :hint="t('settings.shortcutHint')"
  >
    <template #action>
      <button
        type="button"
        class="lw-btn mr-3 flex w-40 items-center justify-center gap-1.5 px-3 py-1.5 text-xs text-ink-2"
        :disabled="settings.saving"
        @click="restoreDefault"
      >
        <Icon name="undo" :size="13" class="text-ink-3" aria-hidden="true" />
        {{ t("settings.shortcutRestore") }}
      </button>
    </template>
    <SettingsRow
      :title="t('settings.shortcut')"
      :hint="t('settings.shortcutRowHint')"
    >
      <SettingsKeyRecorder
        :model-value="settings.view?.shortcut ?? SHORTCUT_DEFAULT"
        @commit="commit"
      />
    </SettingsRow>
  </SettingsGroup>
</template>
