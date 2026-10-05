<script setup lang="ts">
/**
 * 设置分区：模型（模型配置页，UI shell 文档 §3.30 ㉚·改版，对齐
 * PI-Desktop ModelConfigPage；settings.json `llm` 区档位制，DESIGN
 * §13.1）。三块：默认项（默认模型行 + 更改浮层，选择 = 写设置缺省）→
 * AI 服务列表（卡片行：字母瓦片/徽章/meta + 开关 + 菜单，行点击进编辑
 * 模态，拖拽排序落盘）→ 添加/编辑服务大模态（ProviderSetupDialog：
 * 选服务瓦片格 ↔ 表单双栏）。API Key 只落盘不出 IPC。目录状态行
 * （models.dev 快照）= 文档记槽位，本页不渲染。
 */
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Icon } from "../../lib/icons";
import { llmProbeModels, llmProviderReorder } from "../../lib/api";
import { useSettingsStore } from "../../stores/settings";
import { useToastStore } from "../../stores/toast";
import type { ProviderView } from "../../types";
import SettingsGroup from "./SettingsGroup.vue";
import SettingsRow from "./SettingsRow.vue";
import SettingsSelect from "./SettingsSelect.vue";
import ServiceList from "./model/ServiceList.vue";
import ProviderSetupDialog from "./model/ProviderSetupDialog.vue";
import DefaultModelMenu from "./model/DefaultModelMenu.vue";

const { t } = useI18n();
const settings = useSettingsStore();
const toast = useToastStore();

const llm = computed(() => settings.view?.llm ?? null);
const providers = computed<ProviderView[]>(() => llm.value?.providers ?? []);
const enabledProviders = computed(() => providers.value.filter((p) => p.enabled));
const activeId = computed(() => llm.value?.active ?? "");
const activeModel = computed(() => llm.value?.model ?? "");
/** 活动档在启用档里已不可解析（被删/停用）→ 值保留 + 灰注。 */
const defaultMissing = computed(() => {
  if (!llm.value || !activeModel.value) return false;
  return !enabledProviders.value.some((p) => p.id === activeId.value);
});

// ---- 默认项：更改浮层 ----
const defaultMenuOpen = ref(false);
const anchorEl = ref<HTMLElement | null>(null);

async function onSelectDefault(providerId: string, model: string) {
  try {
    await settings.saveLlm({ active: providerId, model });
  } catch (e) {
    toast.push("danger", String(e));
  }
}

// ---- M2 会话记录员（树 v2 §5.2）：同主模型 ▾ + 已启用服务的模型清单 ----
const RECORDER_SEP = "::";
const recorderValue = computed(() => {
  const m = settings.view?.memory;
  if (!m?.recorderModel || !m.recorderProvider) return "";
  return `${m.recorderProvider}${RECORDER_SEP}${m.recorderModel}`;
});
const recorderOptions = computed(() => [
  { value: "", label: t("settings.modelDefaultSame") },
  ...enabledProviders.value.flatMap((p) =>
    (p.models.length > 0 ? p.models.map((x) => x.id) : [p.model].filter(Boolean)).map((id) => ({
      value: `${p.id}${RECORDER_SEP}${id}`,
      label: `${p.label} · ${id}`,
    })),
  ),
]);

function onRecorderChange(value: string) {
  // 空选 = 同主模型：两键都传空串清除（后端约定，见 MemoryUpdate）
  if (!value) {
    void settings.saveMemory({ recorderModel: "", recorderProvider: "" });
    return;
  }
  const [providerId, model] = value.split(RECORDER_SEP);
  void settings.saveMemory({
    recorderModel: model ?? "",
    recorderProvider: providerId ?? "",
  });
}

// ---- AI 服务：模态与动作 ----
/** null = 关；"" = 添加；id = 编辑。 */
const setupFor = ref<null | string>(null);
const copyDraft = ref<null | {
  label: string;
  kind: string;
  baseUrl: string | null;
  models: ProviderView["models"];
}>(null);
const editingProvider = computed<ProviderView | null>(() => {
  if (!setupFor.value) return null;
  return providers.value.find((p) => p.id === setupFor.value) ?? null;
});

function onEdit(p: ProviderView) {
  copyDraft.value = null;
  setupFor.value = p.id;
}
function onCopy(p: ProviderView) {
  copyDraft.value = {
    label: `${p.label} ${t("settings.modelCopySuffix")}`,
    kind: p.kind,
    baseUrl: p.baseUrl,
    models: p.models.map((m) => ({ ...m })),
  };
  setupFor.value = "";
}
async function onToggle(p: ProviderView) {
  try {
    await settings.saveProvider({
      id: p.id,
      label: p.label,
      kind: p.kind,
      baseUrl: p.baseUrl ?? undefined,
      model: p.model,
      windowTokens: p.windowTokens,
      enabled: !p.enabled,
      models: p.models,
    });
    toast.push(
      "ok",
      t(p.enabled ? "settings.modelDisabledToast" : "settings.modelEnabledToast", { name: p.label }),
    );
  } catch (e) {
    toast.push("danger", String(e));
  }
}
async function onMakeDefault(p: ProviderView) {
  const model = p.models[0]?.id ?? p.model;
  await onSelectDefault(p.id, model);
}
async function onTest(p: ProviderView) {
  try {
    const list = await llmProbeModels(p.kind, p.baseUrl, null, p.id);
    toast.push(
      list.length > 0 ? "ok" : "danger",
      list.length > 0 ? t("settings.modelConnected", { n: list.length }) : t("settings.modelProbeEmpty"),
    );
  } catch (e) {
    toast.push("danger", String(e));
  }
}
async function onRemove(p: ProviderView) {
  try {
    await settings.deleteProvider(p.id);
    toast.push("ok", t("settings.modelRemovedToast", { name: p.label }));
  } catch (e) {
    toast.push("danger", String(e));
  }
}
async function onReorder(list: ProviderView[]) {
  try {
    await llmProviderReorder(list.map((p) => p.id));
    await settings.refresh();
  } catch (e) {
    toast.push("danger", String(e));
  }
}
async function onSaved() {
  // 模态直调 llmProviderSave，不走 store 动作 → 这里统一刷新视图
  // （服务行「N 个模型」/默认项清单/下次打开编辑的初值都吃新投影）。
  await settings.refresh();
  toast.push("ok", t("settings.modelSavedToast"));
}
function closeSetup() {
  setupFor.value = null;
  copyDraft.value = null;
}
</script>

<template>
  <div class="space-y-6">
    <!-- 默认项 -->
    <SettingsGroup :title="t('settings.modelDefaultsGroup')">
      <SettingsRow :title="t('settings.modelDefaultRow')">
        <template #detail>
          <span v-if="activeModel" class="mt-1 flex items-center gap-1.5 text-[12px] text-ink-3">
            <span>{{ providers.find((p) => p.id === activeId)?.label ?? "—" }}</span>
            <span aria-hidden>·</span>
            <span class="font-mono">{{ activeModel }}</span>
            <span v-if="defaultMissing" class="text-warn">{{ t("settings.modelDefaultMissing") }}</span>
          </span>
          <span v-else class="mt-1 block text-[12px] text-ink-4">
            {{ enabledProviders.length === 0 ? t("settings.modelDefaultNone") : t("settings.modelDefaultUnset") }}
          </span>
        </template>
        <button
          ref="anchorEl"
          type="button"
          class="lw-btn flex items-center gap-1 px-3 py-1.5 text-xs text-ink-1"
          :disabled="enabledProviders.length === 0"
          :aria-haspopup="'listbox'"
          :aria-expanded="defaultMenuOpen"
          @click="defaultMenuOpen = !defaultMenuOpen"
        >
          {{ t("settings.modelDefaultChange") }}
          <Icon name="chevronDown" :size="13" />
        </button>
      </SettingsRow>

      <!-- M2 会话记录员（§5.2）：清空 = 同主模型；清单来自已启用服务档 -->
      <SettingsRow
        :title="t('settings.recorderModel')"
        :hint="t('settings.recorderModelHint')"
      >
        <SettingsSelect
          :model-value="recorderValue"
          :entries="recorderOptions"
          :label="t('settings.recorderModel')"
          @update:model-value="onRecorderChange"
        />
      </SettingsRow>
    </SettingsGroup>

    <!-- AI 服务 -->
    <SettingsGroup :title="t('settings.modelServicesGroup', { n: providers.length })">
      <template #action>
        <button
          type="button"
          class="flex items-center gap-1.5 rounded-[var(--lw-r-xs)] bg-accent px-3 py-1.5 text-xs font-semibold text-accent-ink transition hover:opacity-90"
          @click="copyDraft = null; setupFor = ''"
        >
          <Icon name="plus" :size="13" />
          {{ t("settings.modelAddService") }}
        </button>
      </template>

      <div v-if="providers.length === 0" class="flex flex-col items-center gap-1.5 rounded-[var(--lw-r-md-plus)] bg-wash-1 px-4 py-8">
        <span class="flex h-10 w-10 items-center justify-center rounded-[var(--lw-r-md)] bg-wash-2 text-ink-4" aria-hidden>
          <Icon name="server" :size="18" />
        </span>
        <p class="text-[13px] font-medium text-ink-2">{{ t("settings.modelEmptyTitle") }}</p>
        <p class="text-[11.5px] text-ink-4">{{ t("settings.modelEmptyDesc") }}</p>
      </div>
      <ServiceList
        v-else
        :providers="providers"
        :active-id="activeId"
        :busy="setupFor !== null"
        @edit="onEdit"
        @make-default="onMakeDefault"
        @test="onTest"
        @copy="onCopy"
        @toggle="onToggle"
        @remove="onRemove"
        @reorder="onReorder"
      />
    </SettingsGroup>

    <DefaultModelMenu
      v-if="defaultMenuOpen"
      :providers="providers"
      :active-id="activeId"
      :active-model="activeModel"
      :anchor="anchorEl"
      @select="onSelectDefault"
      @close="defaultMenuOpen = false"
    />

    <ProviderSetupDialog
      v-if="setupFor !== null"
      :provider="editingProvider"
      :draft="copyDraft"
      @close="closeSetup"
      @saved="onSaved"
    />
  </div>
</template>
