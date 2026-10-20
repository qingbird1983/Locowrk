<script setup lang="ts">
/**
 * 设置分区：工具与分发（⑫，settings.json `tools` 区；exposure 文档
 * §6.3/§9.6；v2.49 更名自 DeveloperSection——分区 id 本就是 tools，组件
 * 名对齐 1:1）。普通用户可见面只剩三块：目录截断开关、新会话权限档、
 * 工作区目录；项目粘性授权规则管理（v2.145）也在本区。
 *
 * S4 刀④ 数值归类：每次调用数 / 调用超时 / 单轮步数 / 审批等待 / 问询
 * 等待五枚数值移入「开发者」分区（值不变）；本区不再有数字输入。
 * 可配置原则 §七-16①：不确定数值一律进设置页，但落在开发者档。
 * 行版式（§3.32·七 v3.5）：行组 / 工作区目录 / 项目权限规则。
 */
import { computed, onMounted, reactive, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import {
  deleteProjectRules,
  listProjectRules,
  pickDirectory,
  projectRulesDir,
  revealInExplorer,
  type ProjectRulesView,
} from "../../lib/api";
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
    form.defaultPermissionMode = v.tools.defaultPermissionMode ?? "ask";
  },
  { immediate: true },
);

function save() {
  void settings.saveTools({
    enabled: form.enabled,
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

// ---- v2.145 项目粘性授权规则管理：审批卡「本项目始终允许」的持久化
// 面。逐工作区列出规则计数，清除即时生效（后端每次查询直读文件）；
// 「打开文件夹」出口供手工编辑/备份。 ----
const rules = ref<ProjectRulesView[]>([]);

async function reloadRules() {
  try {
    rules.value = await listProjectRules();
  } catch {
    rules.value = [];
  }
}

async function removeRules(workspace: string) {
  await deleteProjectRules(workspace);
  await reloadRules();
}

async function openRulesDir() {
  const dir = await projectRulesDir();
  await revealInExplorer(dir);
}

onMounted(reloadRules);
</script>

<template>
  <div class="space-y-6">
    <div class="space-y-3">
      <SettingsGroup :hint="t('settings.toolsGroupHint')">
        <SettingsRow :title="t('settings.toolsEnabled')">
          <SettingsToggle v-model="form.enabled" :label="t('settings.toolsEnabled')" />
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
      <p v-if="!fsRoots.length" class="px-1 py-1 text-[length:var(--lw-fs-12)] text-ink-4">
        {{ t("settings.fsRootsEmpty") }}
      </p>
      <div
        v-for="(root, i) in fsRoots"
        :key="root"
        class="flex items-center justify-between gap-2 rounded-[var(--lw-r-md-plus)] bg-wash-1 px-4 py-2.5"
      >
        <HoverTip :text="root" :label="root" class="min-w-0">
          <span class="min-w-0 break-all text-[length:var(--lw-fs-12)] text-ink-2">{{ root }}</span>
        </HoverTip>
        <button
          type="button"
          class="shrink-0 rounded-[var(--lw-r-3xs)] px-1.5 text-[length:var(--lw-fs-11)] text-ink-4 transition hover:bg-wash-2 hover:text-danger"
          :disabled="settings.saving"
          :aria-label="`${t('settings.fsRootsRemove')} ${root}`"
          @click="removeRoot(i)"
        >
          ✕
        </button>
      </div>
    </SettingsGroup>

    <!-- v2.145 项目粘性授权规则（「本项目始终允许」的管理面） -->
    <SettingsGroup
      :title="t('settings.rulesTitle')"
      :hint="t('settings.rulesHint')"
    >
      <template #action>
        <button
          type="button"
          class="lw-btn px-3 py-1.5 text-xs text-ink-2"
          @click="openRulesDir"
        >
          {{ t("settings.rulesOpen") }}
        </button>
      </template>
      <p v-if="!rules.length" class="px-1 py-1 text-[length:var(--lw-fs-12)] text-ink-4">
        {{ t("settings.rulesEmpty") }}
      </p>
      <div
        v-for="r in rules"
        :key="r.workspace"
        class="flex items-center justify-between gap-2 rounded-[var(--lw-r-md-plus)] bg-wash-1 px-4 py-2.5"
      >
        <HoverTip :text="r.workspace" :label="r.workspace" class="min-w-0">
          <span class="min-w-0 truncate text-[length:var(--lw-fs-12)] text-ink-2">
            {{ r.workspace }}
            <span class="ml-2 text-ink-4">
              {{ t("settings.rulesCounts", { allow: r.allow, deny: r.deny }) }}
            </span>
          </span>
        </HoverTip>
        <button
          type="button"
          class="shrink-0 rounded-[var(--lw-r-3xs)] px-1.5 text-[length:var(--lw-fs-11)] text-ink-4 transition hover:bg-wash-2 hover:text-danger"
          :aria-label="`${t('settings.rulesClear')} ${r.workspace}`"
          @click="removeRules(r.workspace)"
        >
          {{ t("settings.rulesClear") }}
        </button>
      </div>
    </SettingsGroup>
  </div>
</template>
