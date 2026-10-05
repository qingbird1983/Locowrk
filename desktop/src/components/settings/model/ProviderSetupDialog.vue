<script setup lang="ts">
/**
 * 添加 / 编辑 AI 服务模态（UI shell 文档 §3.30，参考 PI-Desktop
 * ProviderSetupDialog：一个模态两个视图）。宽 min(1030px, 100vw-48px)
 * 居中，overlay 点击关（保存中阻断）；Esc 逐层退出（高级展开 → 选服务
 * 回表单 → 关闭）。表单 = 头部动作（测试连接·编辑态 / 取消 / 保存服务
 * primary，canSave = 名称 ∧ baseUrl ∧ 模型 ≥ 1）+ 凭据区（服务行更换 /
 * 名称 / baseUrl（预设只读）/ 类型（仅自定义 SettingsSelect）/ API 密钥
 * + 探测状态行）+ ModelPanes 双栏。探测：表单编辑防抖 600ms（密钥留空
 * 回落已存值）、获取列表立即探、请求序号防旧覆新；添加路径首次探测成功
 * 自动全选 ≤ AUTO_PICK_MAX 个，用户手动勾选即退出自动预选。保存：
 * models 首项 = 该档默认模型，密钥留空 = 保持已存。
 */
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { llmProviderSave } from "../../../lib/api";
import { Icon } from "../../../lib/icons";
import {
  CUSTOM_SERVICE,
  endpointLabel,
  monogramLetter,
  PROVIDER_PRESETS,
} from "../../../lib/provider-presets";
import SettingsSelect from "../SettingsSelect.vue";
import ServiceChooser from "./ServiceChooser.vue";
import ModelPanes from "./ModelPanes.vue";
import { useModelProbe } from "./use-model-probe";
import type { ProviderInput, ProviderModelView, ProviderView } from "../../../types";

const AUTO_PICK_MAX = 8;

const props = defineProps<{
  /** null = 添加流；ProviderView = 编辑流。 */
  provider: ProviderView | null;
  /** 复制配置预填（名称已带「副本」后缀、密钥不带）。 */
  draft: { label: string; kind: string; baseUrl: string | null; models: ProviderModelView[] } | null;
}>();
const emit = defineEmits<{ close: []; saved: [] }>();

const { t } = useI18n();

const service = ref<string>("");
const name = ref("");
const baseUrl = ref("");
const kind = ref<"openai" | "anthropic" | "ollama">("openai");
const apiKey = ref("");
const models = ref<ProviderModelView[]>([]);
const choosing = ref(false);
const saving = ref(false);
const testing = ref(false);
const touched = ref(false);

const preset = computed(() => PROVIDER_PRESETS.find((p) => p.id === service.value));
const isCustom = computed(() => service.value === CUSTOM_SERVICE);
const resolvedName = computed(() =>
  preset.value ? name.value.trim() || preset.value.name : name.value,
);
const resolvedBaseUrl = computed(() => preset.value?.baseUrl ?? baseUrl.value.trim());
const chooserOpen = computed(() => choosing.value || !service.value);

/** 探测（请求序号防旧覆新，实现在 use-model-probe）。 */
const { probeState, available, probeError, run: probeRun, reset: resetProbe } = useModelProbe();
let probeTimer: ReturnType<typeof setTimeout> | null = null;

/** 当前表单形态探测一次（ready 后触发自动预选检查）。 */
async function probe(): Promise<void> {
  await probeRun(
    kind.value,
    resolvedBaseUrl.value,
    apiKey.value.trim() || null,
    props.provider?.id ?? null,
    t("settings.modelProbeEmpty"),
  );
  maybeAutoPick();
}

function initFrom(provider: ProviderView | null) {
  if (provider) {
    service.value = matchingPresetId(provider) ?? CUSTOM_SERVICE;
    name.value = provider.label;
    baseUrl.value = provider.baseUrl ?? "";
    kind.value = provider.kind as typeof kind.value;
    models.value = provider.models.map((m) => ({ ...m }));
  } else if (props.draft) {
    service.value = CUSTOM_SERVICE;
    name.value = props.draft.label;
    baseUrl.value = props.draft.baseUrl ?? "";
    kind.value = props.draft.kind as typeof kind.value;
    models.value = props.draft.models.map((m) => ({ ...m }));
  } else {
    service.value = "";
    name.value = "";
    baseUrl.value = "";
    kind.value = "openai";
    models.value = [];
  }
  apiKey.value = "";
  choosing.value = false;
  touched.value = false;
  resetProbe();
}

/** 存档 baseUrl 命中预设表（含别名 host）→ 记为该预设，否则自定义。 */
function matchingPresetId(p: ProviderView): string | null {
  const norm = (u: string) => u.replace(/\/+$/, "").toLowerCase();
  const base = p.baseUrl ? norm(p.baseUrl) : "";
  const hit = PROVIDER_PRESETS.find(
    (x) => x.kind === p.kind && (!base || norm(x.baseUrl) === base || x.name === p.label),
  );
  return hit?.id ?? null;
}

/** 添加路径首次探测成功自动全选 ≤ AUTO_PICK_MAX（手动改选即退出）。 */
function maybeAutoPick() {
  if (props.provider || props.draft || touched.value) return;
  if (probeState.value !== "ready" || models.value.length > 0) return;
  if (available.value.length === 0 || available.value.length > AUTO_PICK_MAX) return;
  models.value = available.value.map((id) => ({ id, alias: null, contextWindow: null }));
}

function onChosenUpdate(next: ProviderModelView[]) {
  touched.value = true;
  models.value = next;
}

function onPickService(id: string) {
  choosing.value = false;
  if (id === service.value) return;
  const next = PROVIDER_PRESETS.find((p) => p.id === id);
  // Key 属于它贴上去的服务：换服务清空（编辑态不换预设则保留）。
  apiKey.value = "";
  touched.value = true;
  service.value = id;
  if (next) {
    if (!name.value.trim() || PROVIDER_PRESETS.some((p) => p.name === name.value)) {
      name.value = next.name;
    }
    baseUrl.value = next.baseUrl;
    kind.value = next.kind;
  }
  models.value = [];
  resetProbe();
}

async function testConnection() {
  if (!props.provider) return;
  testing.value = true;
  try {
    await probe();
  } finally {
    testing.value = false;
  }
}

const canSave = computed(
  () =>
    !saving.value &&
    !!resolvedName.value.trim() &&
    !!resolvedBaseUrl.value &&
    models.value.length > 0,
);

async function save() {
  if (!canSave.value) return;
  saving.value = true;
  try {
    // models 首项 = 该档默认模型（后端同规则兜底）；窗口随默认模型走。
    const first = models.value[0];
    const input: ProviderInput = {
      id: props.provider?.id,
      label: resolvedName.value.trim(),
      kind: kind.value,
      // 预设/自定义都发解析后的端点：后端 save 是整档覆盖，undefined 会
      // 把存量 baseUrl 抹成 None（30c-4 走查实抓——行 host 变「—」、聊天
      // 路由回落 OpenAI 默认端点）。
      baseUrl: resolvedBaseUrl.value,
      model: first.id,
      windowTokens: first.contextWindow,
      models: models.value,
    };
    if (apiKey.value.trim()) input.apiKey = apiKey.value.trim();
    await llmProviderSave(input);
    emit("saved");
    emit("close");
  } finally {
    saving.value = false;
  }
}

// 表单编辑防抖探测：编辑态开箱即探（密钥回落已存）；预设切换同规则。
watch([resolvedBaseUrl, kind, apiKey], () => {
  if (chooserOpen.value) return;
  if (probeTimer) clearTimeout(probeTimer);
  probeTimer = setTimeout(() => void probe(), 600);
});
watch(chooserOpen, (open) => {
  if (open) return;
  if (probeTimer) clearTimeout(probeTimer);
  // 回到表单（换服务/首次选定）：立探一次
  void probe();
});

function onKeydown(e: KeyboardEvent) {
  if (e.key !== "Escape" || saving.value) return;
  if (choosing.value && service.value) {
    choosing.value = false;
    return;
  }
  emit("close");
}
onMounted(() => {
  initFrom(props.provider);
  window.addEventListener("keydown", onKeydown);
  if (!chooserOpen.value) void probe();
});
onBeforeUnmount(() => {
  window.removeEventListener("keydown", onKeydown);
  if (probeTimer) clearTimeout(probeTimer);
});

const letter = (s: string) => monogramLetter(s);
const endpoint = (u: string) => endpointLabel(u);
const kindOptions = computed(() => [
  { value: "openai", label: "OpenAI 兼容" },
  { value: "anthropic", label: "Anthropic" },
  { value: "ollama", label: "Ollama" },
]);
</script>

<template>
  <Teleport to="body">
    <!-- 模态族统一形态（标准 §二①④）：--lw-scrim 遮罩 + lw-modal 过渡
         （遮罩淡入 / 面板缩放缓冲）+ z=模态档 -->
    <Transition name="lw-modal">
      <div
        class="fixed inset-0 z-[var(--lw-z-modal)] flex items-center justify-center px-6"
        style="background: var(--lw-scrim)"
        role="presentation"
        @click="!saving && emit('close')"
      >
        <div
          class="lw-modal-panel flex max-h-[92vh] w-[min(1030px,100vw-48px)] flex-col overflow-hidden rounded-[var(--lw-r-lg)] bg-elevated p-5 shadow-[var(--lw-shadow-dialog)]"
          role="dialog"
          aria-modal="true"
          @click.stop
        >
        <!-- 选服务视图 -->
        <template v-if="chooserOpen">
          <div class="mb-3 flex shrink-0 items-center">
            <h3 class="text-[15px] font-semibold text-ink-1">
              {{ props.provider ? t("settings.modelChangeService") : t("settings.modelAddTitle") }}
            </h3>
            <button
              type="button"
              class="lw-btn ml-auto px-3 py-1.5 text-xs text-ink-1"
              @click="props.provider ? (choosing = false) : emit('close')"
            >
              {{ t("common.cancel") }}
            </button>
          </div>
          <ServiceChooser :current="service" @pick="onPickService" />
        </template>

        <!-- 表单视图 -->
        <template v-else>
          <div class="mb-4 flex shrink-0 items-center gap-2">
            <h3 class="text-[15px] font-semibold text-ink-1">
              {{
                props.provider
                  ? t("settings.modelEditTitle")
                  : props.draft
                    ? t("settings.modelCopyTitle")
                    : t("settings.modelAddTitle")
              }}
            </h3>
            <div class="ml-auto flex items-center gap-2">
              <button
                v-if="props.provider"
                type="button"
                class="lw-btn px-3 py-1.5 text-xs text-ink-1"
                :disabled="testing || saving"
                @click="testConnection"
              >
                {{ testing ? t("settings.modelTesting") : t("settings.modelTestConn") }}
              </button>
              <button
                type="button"
                class="lw-btn px-3 py-1.5 text-xs text-ink-1"
                :disabled="saving"
                @click="emit('close')"
              >
                {{ t("common.cancel") }}
              </button>
              <button
                type="button"
                class="rounded-[var(--lw-r-xs)] bg-accent px-3.5 py-1.5 text-xs font-semibold text-accent-ink transition hover:opacity-90 disabled:opacity-40"
                :disabled="!canSave"
                @click="save"
              >
                {{ saving ? t("settings.modelSaving") : t("settings.modelSaveService") }}
              </button>
            </div>
          </div>

          <div class="min-h-0 flex-1 overflow-y-auto">
            <!-- 凭据区 -->
            <div class="mb-4 grid grid-cols-2 gap-4">
              <div class="col-span-2 flex items-center gap-2.5 rounded-[var(--lw-r-md)] bg-wash-1 px-3 py-2.5">
                <span
                  class="flex h-9 w-9 shrink-0 items-center justify-center rounded-[var(--lw-r-xs)] bg-wash-2 text-[14px] font-semibold text-ink-2"
                  aria-hidden
                >{{ letter(resolvedName) }}</span>
                <span class="min-w-0">
                  <span class="block truncate text-[13px] font-medium text-ink-1">
                    {{ resolvedName || t("settings.presetCustom") }}
                  </span>
                  <span class="block truncate text-[11px] text-ink-4">{{ endpoint(resolvedBaseUrl) }}</span>
                </span>
                <button
                  type="button"
                  class="lw-btn ml-auto px-2.5 py-1 text-[11.5px] text-ink-1"
                  @click="choosing = true"
                >
                  {{ t("settings.modelReplace") }}
                </button>
              </div>

              <label v-if="isCustom" class="flex items-center gap-2">
                <span class="w-20 shrink-0 text-[12px] text-ink-3">{{ t("settings.modelName") }}</span>
                <input
                  v-model="name"
                  class="lw-field h-9 min-w-0 flex-1 px-2.5 text-[12.5px]"
                  spellcheck="false"
                />
              </label>
              <label v-if="isCustom" class="flex items-center gap-2">
                <span class="w-20 shrink-0 text-[12px] text-ink-3">Base URL</span>
                <input
                  v-model="baseUrl"
                  class="lw-field h-9 min-w-0 flex-1 px-2.5 font-mono text-[12px]"
                  placeholder="https://example.com/v1"
                  spellcheck="false"
                  autocomplete="off"
                />
              </label>
              <label v-if="isCustom" class="flex items-center gap-2">
                <span class="w-20 shrink-0 text-[12px] text-ink-3">{{ t("settings.modelKind") }}</span>
                <SettingsSelect
                  v-model="kind"
                  class="flex-1"
                  :label="t('settings.modelKind')"
                  :entries="kindOptions"
                />
              </label>
              <label class="col-span-2 flex items-center gap-2" :class="isCustom ? '' : 'mt-1'">
                <span class="w-20 shrink-0 text-[12px] text-ink-3">API Key</span>
                <input
                  v-model="apiKey"
                  type="password"
                  class="lw-field h-9 min-w-0 flex-1 px-2.5 font-mono text-[12px]"
                  :placeholder="props.provider?.apiKeySet ? t('settings.modelKeyKeep') : 'sk-…'"
                  autocomplete="off"
                />
              </label>
              <p
                v-if="probeState === 'ready' && available.length > 0"
                class="col-span-2 flex items-center gap-1 text-[11.5px] text-ok"
              >
                <Icon name="check" :size="13" />
                {{ t("settings.modelConnected", { n: available.length }) }}
              </p>
              <p v-else-if="probeState === 'error' && probeError" class="col-span-2 truncate text-[11.5px] text-danger">
                {{ probeError }}
              </p>
            </div>

            <ModelPanes
              :available="available"
              :probe-state="probeState"
              :chosen="models"
              :disabled="saving"
              @fetch="probe"
              @update="onChosenUpdate"
            />
          </div>
        </template>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>
