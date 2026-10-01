<script setup lang="ts">
/**
 * 设置分区：命令终端治理（settings.json `shell` 区，S1；清单 §2.7/§3
 * 约束最小化）。高危命令解锁开关——默认 rm/sudo/chmod 等直接拒绝，
 * 开启后改为进审批（仍要点确认，不是直接放行）；解释器/包装/网络类
 * 保持硬锁。开关经插件加载前的 env 注入，重启后生效。
 */
import { reactive, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useSettingsStore } from "../../stores/settings";

const { t } = useI18n();
const settings = useSettingsStore();

const form = reactive({
  unlockRisky: false,
});

watch(
  () => settings.view,
  (v) => {
    if (!v) return;
    form.unlockRisky = v.shell.unlockRisky;
  },
  { immediate: true },
);

function save() {
  void settings.saveShell({ unlockRisky: form.unlockRisky });
}
</script>

<template>
  <section class="space-y-3 rounded-xl border border-line p-4">
    <h3 class="text-xs font-semibold">{{ t("settings.shellTitle") }}</h3>
    <p class="text-[11px] text-ink-4">{{ t("settings.shellHint") }}</p>

    <label class="flex cursor-pointer items-start gap-2 text-xs">
      <input v-model="form.unlockRisky" type="checkbox" class="mt-0.5 accent-[var(--lw-accent)]" />
      <span>{{ t("settings.shellUnlock") }}</span>
    </label>
    <p class="rounded-lg bg-warn/10 p-2 text-[11px] text-warn">
      ⚠️ {{ t("settings.shellUnlockWarning") }}
    </p>
    <p class="text-[11px] text-ink-5">{{ t("settings.shellRestart") }}</p>

    <div class="flex items-center gap-2">
      <button
        class="rounded-lg bg-wash-2 px-4 py-1.5 text-xs font-semibold transition hover:bg-wash-3 disabled:cursor-not-allowed disabled:opacity-40"
        :disabled="settings.saving"
        @click="save"
      >
        {{ t("settings.save") }}
      </button>
      <span v-if="settings.savedFlash" class="text-xs text-ok">
        {{ t("settings.saved") }}
      </span>
    </div>
    <p v-if="settings.error" class="rounded-lg bg-danger/10 p-2 text-[11px] text-danger">
      {{ settings.error }}
    </p>
  </section>
</template>
