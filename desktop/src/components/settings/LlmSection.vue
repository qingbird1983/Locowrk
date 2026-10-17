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
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Icon } from "../../lib/icons";
import { llmCatalogRefresh, llmCatalogStatus, llmProbeModels, llmProviderReorder, llmSetProxy } from "../../lib/api";
import { useSettingsStore } from "../../stores/settings";
import { useToastStore } from "../../stores/toast";
import { relativeTimeLabel, relativeTimeMs } from "../../lib/format";
import type { CatalogStatus, ProviderView, SubagentPin } from "../../types";
import SettingsGroup from "./SettingsGroup.vue";
import SettingsRow from "./SettingsRow.vue";
import ServiceList from "./model/ServiceList.vue";
import ProviderSetupDialog from "./model/ProviderSetupDialog.vue";
import ImportDialog from "./model/ImportDialog.vue";
import DefaultModelMenu from "./model/DefaultModelMenu.vue";
import SettingsSelect from "./SettingsSelect.vue";

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

// ---- M2 记忆管理器（树 v2 §5.2）：与主模型同款更改浮层（模型选择器
// 全站唯一实现——原 SettingsSelect 第二套形态，用户目检判同类多套）。
// 空选 = 跟随主模型：两键都传空串清除（后端约定，见 MemoryUpdate）。
const managerMenuOpen = ref(false);
const managerAnchorEl = ref<HTMLElement | null>(null);
const recorderProvider = computed(() => settings.view?.memory?.recorderProvider ?? "");
const recorderModel = computed(() => settings.view?.memory?.recorderModel ?? "");
const managerLabel = computed(() => {
  if (!recorderProvider.value || !recorderModel.value) return t("settings.modelDefaultSame");
  const p = providers.value.find((x) => x.id === recorderProvider.value);
  return `${p?.label ?? "—"} · ${recorderModel.value}`;
});

function onSelectRecorder(providerId: string, model: string) {
  void settings.saveMemory(
    providerId && model
      ? { recorderModel: model, recorderProvider: providerId }
      : { recorderModel: "", recorderProvider: "" },
  );
}

// ---- 思考语言（docs/design/archive/language of thought.md）：reasoning 用的
// 语言。会话首请求锁死（前缀缓存红线），中途修改保存后提示「下个会话
// 生效」；同值重选不保存不提示。
const thinkingPref = computed({
  get: () => settings.view?.thinking?.language ?? "follow_ui",
  set: (v: string) => void onThinkingChange(v),
});
const thinkingOptions = computed(() => [
  { value: "follow_ui", label: t("settings.thinkingFollowUi") },
  { value: "zh-CN", label: t("settings.thinkingZh") },
  { value: "en-US", label: t("settings.thinkingEn") },
  { value: "none", label: t("settings.thinkingNone") },
]);
async function onThinkingChange(v: string) {
  if (v === thinkingPref.value) return;
  try {
    await settings.saveThinking({ language: v });
    toast.push("ok", t("settings.thinkingSavedToast"));
  } catch (e) {
    toast.push("danger", String(e));
  }
}

// ---- 模型目录（批 A，供应商稿 §4.5）：models.dev 快照状态 + 手动刷新
// （只进内存，永不落盘；失败旧数据保留）。 ----
const catalogStatus = ref<CatalogStatus | null>(null);
const catalogRefreshing = ref(false);
onMounted(() => {
  llmCatalogStatus()
    .then((s) => (catalogStatus.value = s))
    .catch(() => (catalogStatus.value = null));
});
const catalogDetail = computed(() => {
  if (!catalogStatus.value) return t("settings.catalogLoading");
  const src = t(
    catalogStatus.value.source === "bundled"
      ? "settings.catalogSourceBundled"
      : "settings.catalogSourceRefreshed",
  );
  // updatedMs 是毫秒：旧代码手写 `/ 1000` 再喂按秒算的函数（能对上，但单位靠人记），
  // 现在单位进函数名；旧版还少了 `n === undefined` 分支，just_now 档传 { n: undefined }
  // ——两份语言包的 memory.time.just_now 都不含 {n} 占位，所以输出等价（判据按实文案核）。
  const rel = relativeTimeLabel(relativeTimeMs(catalogStatus.value.updatedMs), (k, p) => t(k, p));
  return `${src} · ${t("settings.catalogModelsCount", { n: catalogStatus.value.models })} · ${rel}`;
});
async function refreshCatalog() {
  catalogRefreshing.value = true;
  try {
    catalogStatus.value = await llmCatalogRefresh();
    toast.push("ok", t("settings.catalogRefreshedToast", { n: catalogStatus.value.models }));
  } catch (e) {
    toast.push("danger", t("settings.catalogRefreshFailToast", { msg: String(e) }));
  } finally {
    catalogRefreshing.value = false;
  }
}

// ---- 子代理模型（批 F）：钉档 = 子 Agent 专用主模型（缺省跟随会话）；
// 清单 = 只列「可用于子代理」标记的模型（复用 DefaultModelMenu，传过滤
// 后的 providers 副本）；钉档档位随标记模型梯子。 ----
const subPin = computed<SubagentPin | null>(() => llm.value?.subagent ?? null);
const subMenuOpen = ref(false);
const subAnchorEl = ref<HTMLElement | null>(null);
/** 标记池投影：只保留 availableForSubagents === true 的模型行（空组剔除）。 */
const markedProviders = computed<ProviderView[]>(() =>
  providers.value
    .filter((p) => p.enabled)
    .map((p) => ({ ...p, models: p.models.filter((m) => m.availableForSubagents === true) }))
    .filter((p) => p.models.length > 0),
);
const subLabel = computed(() => {
  if (!subPin.value) return t("settings.subagentFollowSession");
  const p = providers.value.find((x) => x.id === subPin.value?.providerId);
  return `${p?.label ?? "—"} · ${subPin.value.model}`;
});
const subMarked = computed(
  () => !!subPin.value && markedProviders.value.some(
    (p) => p.id === subPin.value?.providerId &&
      p.models.some((m) => m.id === subPin.value?.model),
  ),
);
function onSelectSub(providerId: string, model: string) {
  subMenuOpen.value = false;
  if (!providerId || !model) {
    if (subPin.value) void saveSub({ providerId: "", model: "" });
    return;
  }
  void saveSub({ providerId, model });
}
async function saveSub(pin: { providerId: string; model: string; thinkingLevel?: string | null }) {
  try {
    await settings.saveLlm({ subagent: { providerId: pin.providerId, model: pin.model, thinkingLevel: pin.thinkingLevel ?? null } });
  } catch (e) {
    toast.push("danger", String(e));
  }
}
/** 钉档档位选项 = 钉档模型的梯子（空梯子 = 行隐藏）；「跟随会话」= 清除。 */
const subThinking = computed({
  get: () => subPin.value?.thinkingLevel ?? "",
  set: (v: string) => {
    if (!subPin.value) return;
    void saveSub({
      providerId: subPin.value.providerId,
      model: subPin.value.model,
      thinkingLevel: v || null,
    });
  },
});
const subThinkingOptions = computed(() => {
  const p = providers.value.find((x) => x.id === subPin.value?.providerId);
  const ladder = p?.models.find((m) => m.id === subPin.value?.model)?.reasoningLadder ?? [];
  return [
    { value: "", label: t("settings.subagentThinkingFollow") },
    ...ladder.map((l) => ({ value: l, label: l })),
  ];
});

// ---- 导入面板（批 F）：四源扫描预览 → 选中写入档位。 ----
const importOpen = ref(false);


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
    const out = await llmProbeModels(p.kind, p.baseUrl, null, p.id);
    if (out.models.length > 0) {
      toast.push("ok", t("settings.modelConnected", { n: out.models.length }));
    } else {
      toast.push("danger", out.error ?? t("settings.modelProbeEmpty"));
    }
  } catch (e) {
    toast.push("danger", String(e));
  }
}

// ---- 网络代理（批 C §8.5，裁定 D6）：system/direct 即选即存；custom
// 由地址行提交（custom 必带 url）。保存即热换（下一次请求生效）。 ----
const proxyMode = computed(() => llm.value?.networkProxy?.mode ?? "system");
const proxyUrl = ref(llm.value?.networkProxy?.url ?? "");
watch(proxyMode, () => {
  proxyUrl.value = llm.value?.networkProxy?.url ?? "";
});
/** 本地草稿档：切「自定义」只记草稿不落盘（后端校验 custom 必带 url），
 * 地址行以草稿档为门；system/direct 即选即存并清草稿。 */
const proxyModeDraft = ref<string | null>(null);
const proxyOptions = computed(() => [
  { value: "system", label: t("settings.proxySystem") },
  { value: "direct", label: t("settings.proxyDirect") },
  { value: "custom", label: t("settings.proxyCustom") },
]);
const proxyModePref = computed({
  get: () => proxyModeDraft.value ?? proxyMode.value,
  set: (v: string) => void onProxyMode(v),
});
async function onProxyMode(v: string) {
  if (v === (proxyModeDraft.value ?? proxyMode.value)) return;
  if (v === "custom") {
    proxyModeDraft.value = "custom";
    proxyUrl.value = llm.value?.networkProxy?.url ?? "";
    return;
  }
  proxyModeDraft.value = null;
  await saveProxy(v as "system" | "direct", null);
}
async function onProxyUrlCommit() {
  const url = proxyUrl.value.trim();
  if (!url) return;
  await saveProxy("custom", url);
  proxyModeDraft.value = null;
}
async function saveProxy(mode: "system" | "direct" | "custom", url: string | null) {
  try {
    await llmSetProxy(mode, url);
    await settings.refresh();
    toast.push("ok", t("settings.proxySavedToast"));
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
          <span v-if="activeModel" class="mt-1 flex items-center gap-1.5 text-[length:var(--lw-fs-12)] text-ink-3">
            <span>{{ providers.find((p) => p.id === activeId)?.label ?? "—" }}</span>
            <span aria-hidden>·</span>
            <span class="font-mono">{{ activeModel }}</span>
            <span v-if="defaultMissing" class="text-warn">{{ t("settings.modelDefaultMissing") }}</span>
          </span>
          <span v-else class="mt-1 block text-[length:var(--lw-fs-12)] text-ink-4">
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
          <Icon name="chevronDown" :size="14" />
        </button>
      </SettingsRow>

      <!-- M2 记忆管理器（§5.2）：与主模型行同款「当前值 + 更改」结构，
           清单 = 同一个 DefaultModelMenu（+「跟随主模型」常驻档） -->
      <SettingsRow
        :title="t('settings.memoryManagerModel')"
        :hint="t('settings.memoryManagerModelHint')"
      >
        <template #detail>
          <span
            class="mt-1 block text-[length:var(--lw-fs-12)]"
            :class="recorderProvider && recorderModel ? 'text-ink-3' : 'text-ink-4'"
          >
            {{ managerLabel }}
          </span>
        </template>
        <button
          ref="managerAnchorEl"
          type="button"
          class="lw-btn flex items-center gap-1 px-3 py-1.5 text-xs text-ink-1"
          :disabled="enabledProviders.length === 0"
          :aria-haspopup="'listbox'"
          :aria-expanded="managerMenuOpen"
          @click="managerMenuOpen = !managerMenuOpen"
        >
          {{ t("settings.modelDefaultChange") }}
          <Icon name="chevronDown" :size="14" />
        </button>
      </SettingsRow>

      <!-- 思考语言：模型 reasoning 的语言；会话内锁死（前缀缓存红线），
           修改保存即提示下个会话生效 -->
      <SettingsRow
        :title="t('settings.thinkingLanguage')"
        :hint="t('settings.thinkingHint')"
      >
        <SettingsSelect
          v-model="thinkingPref"
          :entries="thinkingOptions"
          :label="t('settings.thinkingLanguage')"
        />
      </SettingsRow>
    </SettingsGroup>

    <!-- 子代理模型（批 F）：钉档 = 子 Agent 主模型（缺省跟随会话）；
         清单只列「可用于子代理」标记的模型 -->
    <SettingsGroup :title="t('settings.subagentTitle')">
      <SettingsRow :title="t('settings.subagentRow')" :hint="t('settings.subagentHint')">
        <template #detail>
          <span
            class="mt-1 block text-[length:var(--lw-fs-12)]"
            :class="subPin && subMarked ? 'text-ink-3' : 'text-ink-4'"
          >
            {{ subLabel }}
          </span>
        </template>
        <button
          ref="subAnchorEl"
          type="button"
          class="lw-btn flex items-center gap-1 px-3 py-1.5 text-xs text-ink-1"
          :disabled="markedProviders.length === 0"
          :aria-haspopup="'listbox'"
          :aria-expanded="subMenuOpen"
          @click="subMenuOpen = !subMenuOpen"
        >
          {{ t("settings.modelDefaultChange") }}
          <Icon name="chevronDown" :size="14" />
        </button>
      </SettingsRow>
      <SettingsRow
        v-if="subPin && subThinkingOptions.length > 1"
        :title="t('settings.subagentThinkingRow')"
      >
        <SettingsSelect
          v-model="subThinking"
          class="w-44"
          :entries="subThinkingOptions"
          :label="t('settings.subagentThinkingRow')"
        />
      </SettingsRow>
    </SettingsGroup>

    <!-- 模型目录（批 A）：快照状态行 + 刷新按钮 -->
    <SettingsGroup :title="t('settings.catalogTitle')">
      <SettingsRow :title="t('settings.catalogRow')" :hint="t('settings.catalogHint')">
        <template #detail>
          <span class="mt-1 block text-[length:var(--lw-fs-12)] text-ink-4">{{ catalogDetail }}</span>
        </template>
        <button
          type="button"
          class="lw-btn flex items-center gap-1 px-3 py-1.5 text-xs text-ink-1"
          :disabled="catalogRefreshing"
          @click="refreshCatalog"
        >
          <Icon name="refresh" :size="14" />
          {{ catalogRefreshing ? t("settings.catalogRefreshing") : t("settings.catalogRefresh") }}
        </button>
      </SettingsRow>
    </SettingsGroup>

    <!-- 网络代理（批 C §8.5）：clash 类常驻代理的显式可控档 -->
    <SettingsGroup :title="t('settings.proxyTitle')">
      <SettingsRow :title="t('settings.proxyRow')" :hint="t('settings.proxyHint')">
        <SettingsSelect
          v-model="proxyModePref"
          class="w-44"
          :entries="proxyOptions"
          :label="t('settings.proxyRow')"
        />
      </SettingsRow>
      <SettingsRow v-if="(proxyModeDraft ?? proxyMode) === 'custom'" :title="t('settings.proxyUrlRow')">
        <input
          v-model="proxyUrl"
          class="lw-field h-9 w-full px-2.5 font-mono text-[length:var(--lw-fs-12)]"
          placeholder="http://127.0.0.1:7890"
          spellcheck="false"
          autocomplete="off"
          @change="onProxyUrlCommit"
        />
      </SettingsRow>
    </SettingsGroup>

    <!-- AI 服务 -->
    <SettingsGroup :title="t('settings.modelServicesGroup', { n: providers.length })">
      <template #action>
        <div class="flex items-center gap-2">
          <button
            type="button"
            class="lw-btn flex items-center gap-1 px-3 py-1.5 text-xs text-ink-1"
            @click="importOpen = true"
          >
            <Icon name="download" :size="14" />
            {{ t("settings.importButton") }}
          </button>
          <button
            type="button"
            class="flex items-center gap-1.5 rounded-[var(--lw-r-xs)] bg-accent px-3 py-1.5 text-xs font-semibold text-accent-ink transition hover:opacity-90"
            @click="copyDraft = null; setupFor = ''"
          >
            <Icon name="plus" :size="14" />
            {{ t("settings.modelAddService") }}
          </button>
        </div>
      </template>

      <div v-if="providers.length === 0" class="flex flex-col items-center gap-1.5 rounded-[var(--lw-r-md-plus)] bg-wash-1 px-4 py-8">
        <span class="flex h-10 w-10 items-center justify-center rounded-[var(--lw-r-md)] bg-wash-2 text-ink-4" aria-hidden>
          <Icon name="server" :size="18" />
        </span>
        <p class="text-[length:var(--lw-fs-13)] font-medium text-ink-2">{{ t("settings.modelEmptyTitle") }}</p>
        <p class="text-[length:var(--lw-fs-12)] text-ink-4">{{ t("settings.modelEmptyDesc") }}</p>
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

    <!-- 记忆管理器更改浮层：与主模型同一实现；空 active = 「跟随主模型」档选中 -->
    <DefaultModelMenu
      v-if="managerMenuOpen"
      :providers="providers"
      :active-id="recorderProvider"
      :active-model="recorderModel"
      :anchor="managerAnchorEl"
      :none-label="t('settings.modelDefaultSame')"
      @select="onSelectRecorder"
      @close="managerMenuOpen = false"
    />

    <!-- 子代理模型更改浮层（批 F）：清单 = 只列「可用于子代理」标记的模型；
         首项「跟随会话」= 清除钉档 -->
    <DefaultModelMenu
      v-if="subMenuOpen"
      :providers="markedProviders"
      :active-id="subPin?.providerId ?? ''"
      :active-model="subPin?.model ?? ''"
      :anchor="subAnchorEl"
      :none-label="t('settings.subagentFollowSession')"
      @select="onSelectSub"
      @close="subMenuOpen = false"
    />

    <!-- 导入面板（批 F）：四源扫描预览 → 选中写入档位 -->
    <ImportDialog
      v-if="importOpen"
      @close="importOpen = false"
      @imported="settings.refresh()"
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
