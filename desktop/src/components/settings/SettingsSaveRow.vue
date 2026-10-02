<script setup lang="ts">
/**
 * 设置分区保存行（§3.32·七 v3.5）：显式保存分区的按钮行——行卡列表之下、
 * 左对齐，「已保存」闪烁随行；#slot 放行内附加按钮（清除 Key 等）。
 * 错误条只在板面级渲染一次（SettingsPage），本组件不重复。
 * 就地生效分区（外观 / 语言）不用本组件。
 */
import { useI18n } from "vue-i18n";
import { useSettingsStore } from "../../stores/settings";

const { t } = useI18n();
const settings = useSettingsStore();

defineEmits<{ save: [] }>();
</script>

<template>
  <div class="flex items-center gap-2 px-1">
    <button
      class="rounded-lg bg-wash-2 px-4 py-1.5 text-xs font-semibold transition hover:bg-wash-3 disabled:cursor-not-allowed disabled:opacity-40"
      :disabled="settings.saving"
      @click="$emit('save')"
    >
      {{ t("settings.save") }}
    </button>
    <slot />
    <span v-if="settings.savedFlash" class="text-xs text-ok">
      {{ t("settings.saved") }}
    </span>
  </div>
</template>
