<script setup lang="ts">
/**
 * 设置分区：工具与分发（⑫，settings.json `tools` 区；exposure 文档
 * §6.3/§9.6；v2.49 更名自 DeveloperSection——分区 id 本就是 tools，组件
 * 名对齐 1:1）。上半是截断设置（开关 + 每请求最大工具数 + 调用超时 +
 * 单轮步数上限 + 审批等待时长，保存即热生效）；下半是工作区目录（⑱，
 * 增删即存即生效）。暴露调试面板同批迁入「开发者」分区（开发者信息不
 * 占普通面板）。可配置原则 §七-16①：不确定数值一律可调项。
 * 行版式（§3.32·七 v3.5）：两块组（行组 / 工作区目录）。
 */
import { computed, reactive, watch } from "vue";
import { useI18n } from "vue-i18n";
import { pickDirectory } from "../../lib/api";
import { useSettingsStore } from "../../stores/settings";
import HoverTip from "./HoverTip.vue";
import SettingsGroup from "./SettingsGroup.vue";
import SettingsRow from "./SettingsRow.vue";
import SettingsSaveRow from "./SettingsSaveRow.vue";
import SettingsToggle from "./SettingsToggle.vue";
import SettingsSelect from "./SettingsSelect.vue";

const { t } = useI18n();
const settings = useSettingsStore();

const form = reactive({
  enabled: true,
  maxPerRequest: 20,
  callTimeoutSecs: 30,
  maxTurnSteps: 8,
  approvalTimeoutSecs: 100,
  askTimeoutSecs: 300,
  defaultPermissionMode: "ask" as "plan" | "ask" | "acceptEdits" | "auto",
});

// ㉘·B 新会话权限档缺省（四档；会话级覆盖挂 composer §3.33 切换器）
const PERM_MODES = ["plan", "ask", "acceptEdits", "auto"] as const;
const permModeOptions = computed(() =>
  PERM_MODES.map((m) => ({
    value: m,
    label: t(`perm.short.${m}`),
    desc: t(`perm.desc.${m}`),
  })),
);

watch(
  () => settings.view,
  (v) => {
    if (!v) return;
    form.enabled = v.tools.enabled;
    form.maxPerRequest = v.tools.maxPerRequest;
    form.callTimeoutSecs = v.tools.callTimeoutSecs;
    form.maxTurnSteps = v.tools.maxTurnSteps;
    form.approvalTimeoutSecs = v.tools.approvalTimeoutSecs;
    form.askTimeoutSecs = v.tools.askTimeoutSecs;
    form.defaultPermissionMode = v.tools.defaultPermissionMode ?? "ask";
  },
  { immediate: true },
);

function save() {
  void settings.saveTools({
    enabled: form.enabled,
    maxPerRequest: form.maxPerRequest,
    callTimeoutSecs: form.callTimeoutSecs,
    maxTurnSteps: form.maxTurnSteps,
    approvalTimeoutSecs: form.approvalTimeoutSecs,
    askTimeoutSecs: form.askTimeoutSecs,
    defaultPermissionMode: form.defaultPermissionMode,
  });
}

// ---- ⑱ 工作区目录（fs roots）：增删即存即生效（后端重注入 env，
// fsutil 每次工具调用读取），不参与上方表单的"保存"按钮。 ----
const fsRoots = computed(() => settings.view?.tools.fsRoots ?? []);

async function addRoot() {
  const dir = await pickDirectory();
  if (!dir || fsRoots.value.includes(dir)) return;
  await settings.saveTools({ fsRoots: [...fsRoots.value, dir] });
}

function removeRoot(idx: number) {
  void settings.saveTools({ fsRoots: fsRoots.value.filter((_, i) => i !== idx) });
}
</script>

<template>
  <div class="space-y-6">
    <div class="space-y-3">
      <SettingsGroup :hint="t('settings.developerHint')">
        <SettingsRow :title="t('settings.toolsEnabled')">
          <SettingsToggle v-model="form.enabled" :label="t('settings.toolsEnabled')" />
        </SettingsRow>

        <SettingsRow
          :title="t('settings.toolsMax')"
          :hint="t('settings.toolsMaxHint')"
          control-id="tools-max"
        >
          <input
            id="tools-max"
            v-model.number="form.maxPerRequest"
            type="number"
            min="1"
            max="200"
            class="lw-field lw-field-num px-2 text-xs"
          />
        </SettingsRow>

        <!-- S2（清单 §3 约束最小化）：宿主单次工具调用超时可配 -->
        <SettingsRow
          :title="t('settings.callTimeout')"
          :hint="t('settings.callTimeoutHint')"
          control-id="tools-call-timeout"
        >
          <input
            id="tools-call-timeout"
            v-model.number="form.callTimeoutSecs"
            type="number"
            min="5"
            max="3600"
            class="lw-field lw-field-num px-2 text-xs"
          />
        </SettingsRow>

        <!-- ㉕·微调③：单轮步数上限（runner 兜底，超过强制收束本轮） -->
        <SettingsRow
          :title="t('settings.maxTurnSteps')"
          :hint="t('settings.maxTurnStepsHint')"
          control-id="tools-max-turn-steps"
        >
          <input
            id="tools-max-turn-steps"
            v-model.number="form.maxTurnSteps"
            type="number"
            min="1"
            max="100"
            class="lw-field lw-field-num px-2 text-xs"
          />
        </SettingsRow>

        <!-- v2.49 可配置原则：审批守夜阈值（0 = 不限，卡片不渲染倒计时） -->
        <SettingsRow
          :title="t('settings.approvalTimeout')"
          :hint="t('settings.approvalTimeoutHint')"
          control-id="tools-approval-timeout"
        >
          <input
            id="tools-approval-timeout"
            v-model.number="form.approvalTimeoutSecs"
            type="number"
            min="0"
            max="86400"
            class="lw-field lw-field-num px-2 text-xs"
          />
        </SettingsRow>

        <!-- v4.11 §3.39① 可配置原则：ask_user 选择守夜（0 = 不限） -->
        <SettingsRow
          :title="t('settings.askTimeout')"
          :hint="t('settings.askTimeoutHint')"
          control-id="tools-ask-timeout"
        >
          <input
            id="tools-ask-timeout"
            v-model.number="form.askTimeoutSecs"
            type="number"
            min="0"
            max="86400"
            class="lw-field lw-field-num px-2 text-xs"
          />
        </SettingsRow>

        <!-- ㉘·B 新会话权限档缺省（会话级覆盖存会话旁车，切换器即时生效） -->
        <SettingsRow
          :title="t('settings.defaultPermissionMode')"
          :hint="t('settings.defaultPermissionModeHint')"
          control-id="tools-perm-mode"
        >
          <SettingsSelect
            v-model="form.defaultPermissionMode"
            :entries="permModeOptions"
            :label="t('settings.defaultPermissionMode')"
          />
        </SettingsRow>
      </SettingsGroup>

      <SettingsSaveRow @save="save" />
    </div>

    <!-- ⑱ 工作区目录（fs roots）：增删即存即生效，不走上方"保存" -->
    <SettingsGroup
      :title="t('settings.fsRootsTitle')"
      :hint="t('settings.fsRootsHint')"
    >
      <template #action>
        <button
          type="button"
          class="lw-btn px-3 py-1.5 text-xs text-ink-2"
          :disabled="settings.saving"
          @click="addRoot"
        >
          {{ t("settings.fsRootsAdd") }}
        </button>
      </template>
      <p v-if="!fsRoots.length" class="px-1 py-1 text-[11.5px] text-ink-4">
        {{ t("settings.fsRootsEmpty") }}
      </p>
      <div
        v-for="(root, i) in fsRoots"
        :key="root"
        class="flex items-center justify-between gap-2 rounded-[var(--lw-r-md-plus)] bg-wash-1 px-4 py-2.5"
      >
        <HoverTip :text="root" :label="root" class="min-w-0">
          <span class="min-w-0 break-all text-[11.5px] text-ink-2">{{ root }}</span>
        </HoverTip>
        <button
          type="button"
          class="shrink-0 rounded-[var(--lw-r-3xs)] px-1.5 text-[11px] text-ink-4 transition hover:bg-wash-2 hover:text-danger"
          :disabled="settings.saving"
          :aria-label="`${t('settings.fsRootsRemove')} ${root}`"
          @click="removeRoot(i)"
        >
          ✕
        </button>
      </div>
    </SettingsGroup>
  </div>
</template>
