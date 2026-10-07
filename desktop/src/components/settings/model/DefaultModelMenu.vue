<script setup lang="ts">
/**
 * 默认模型更改浮层（UI shell 文档 §3.30 默认项行）：锚触发钮下方右对齐，
 * 320px 搜索 + 按服务分组（组标签 11px ink-4 不可点；项 = ✓ + 模型名
 * mono，hover wash-2），打开滚到当前项；无结果显示空态文案。数据 =
 * 已配置清单（启用档 × 其 models，档无 models 时回落其单 model）——
 * 与 PI 同口径：默认选择器列「已配置」，不现场探测。纯受控组件。
 */
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Icon } from "../../../lib/icons";
import { hasImageInput } from "../../../lib/model-catalog";
import SearchInput from "../../patterns/SearchInput.vue";
import { formatPricingShort } from "../../../lib/model-catalog";
import type { ProviderModelPricing, ProviderView } from "../../../types";

const props = defineProps<{
  providers: ProviderView[];
  /** 当前生效（activeId, model）。 */
  activeId: string;
  activeModel: string;
  anchor: HTMLElement | null;
  /** 提供时列表顶部常驻一档「跟随缺省」行（activeId="" 选中；选中回传
   * select("", "")）——记忆管理器档复用本菜单用（模型选择器全站唯一实现）。 */
  noneLabel?: string;
}>();
const emit = defineEmits<{
  select: [providerId: string, model: string];
  close: [];
}>();

const { t } = useI18n();
const query = ref("");
const popEl = ref<HTMLElement | null>(null);
const listEl = ref<HTMLElement | null>(null);
const style = ref<Record<string, string>>({});
const searchInput = ref<InstanceType<typeof SearchInput> | null>(null);

interface Row {
  providerId: string;
  label: string;
  model: string;
  /** models.dev 目录价（USD 原值；无目录数据 = null 不渲染，批 A §4.6）。 */
  pricing: ProviderModelPricing | null;
}
/** 组 = 启用档；档无 models 清单时回落其单 model（不发明空组）。 */
const rows = computed<Row[]>(() =>
  props.providers
    .filter((p) => p.enabled)
    .flatMap((p) => {
      const models =
        p.models.length > 0
          ? p.models.map((m) => ({ id: m.id, pricing: m.pricing }))
          : p.model
            ? [{ id: p.model, pricing: null }]
            : [];
      return models.map((m) => ({ providerId: p.id, label: p.label, model: m.id, pricing: m.pricing }));
    }),
);
const visible = computed(() => {
  const needle = query.value.trim().toLowerCase();
  if (!needle) return rows.value;
  return rows.value.filter(
    (r) => r.model.toLowerCase().includes(needle) || r.label.toLowerCase().includes(needle),
  );
});
const isCurrent = (r: Row) => r.providerId === props.activeId && r.model === props.activeModel;
/** 能力徽章（批 B §5.3）：✨ 推理梯子 / 👁 图像输入（数据 = 档位模型行）。 */
const capsOf = (r: Row) => {
  const m = props.providers
    .find((p) => p.id === r.providerId)
    ?.models.find((x) => x.id === r.model);
  return {
    thinking: (m?.reasoningLadder?.length ?? 0) > 0,
    vision: m ? hasImageInput(m) : false,
  };
};

function place() {
  const a = props.anchor;
  const pop = popEl.value;
  if (!a || !pop) return;
  const ar = a.getBoundingClientRect();
  const pr = pop.getBoundingClientRect();
  let left = ar.right - pr.width;
  let top = ar.bottom + 6;
  left = Math.max(8, Math.min(left, window.innerWidth - pr.width - 8));
  if (top + pr.height > window.innerHeight - 8) top = Math.max(8, ar.top - pr.height - 6);
  style.value = { left: `${left}px`, top: `${top}px` };
}

function onDocMouseDown(e: MouseEvent) {
  const t = e.target as Node;
  if (popEl.value?.contains(t) || props.anchor?.contains(t)) return;
  emit("close");
}
function onKeydown(e: KeyboardEvent) {
  if (e.key === "Escape") emit("close");
}

onMounted(async () => {
  document.addEventListener("mousedown", onDocMouseDown);
  window.addEventListener("keydown", onKeydown);
  await nextTick();
  place();
  // 打开聚焦搜索 + 滚到当前项（必须可见；手设 scrollTop——scrollIntoView
  // 会连祖先滚动容器一起滚，模型设置页目检实抓过整页上移）
  searchInput.value?.focus();
  const list = listEl.value;
  const cur = list?.querySelector("[aria-selected='true']");
  if (list instanceof HTMLElement && cur instanceof HTMLElement) {
    const top = cur.offsetTop - list.offsetTop - list.clientHeight / 2 + cur.offsetHeight / 2;
    list.scrollTop = Math.max(0, top);
  }
});
onBeforeUnmount(() => {
  document.removeEventListener("mousedown", onDocMouseDown);
  window.removeEventListener("keydown", onKeydown);
});
</script>

<template>
  <Teleport to="body">
    <!-- 弹出面板动效（标准 §二①：滑动弹出 + 缓冲收尾，禁止硬切） -->
    <Transition name="lw-pop-down">
      <div
        ref="popEl"
        class="fixed z-[var(--lw-z-pop)] w-[min(320px,calc(100vw-32px))] overflow-hidden rounded-[var(--lw-r-md-plus)] border border-line bg-elevated shadow-[var(--lw-shadow-pop)]"
        :style="style"
        role="listbox"
        :aria-label="t('settings.modelDefaultChange')"
      >
      <div class="flex h-10 items-center gap-2 border-b border-line-subtle px-3">
        <Icon name="search" :size="14" class="shrink-0 text-ink-4" />
        <input
          v-model="query"
          class="w-full bg-transparent text-[length:var(--lw-fs-12half)] text-ink-1 outline-none placeholder:text-ink-5"
          :placeholder="t('settings.modelFilterModels')"
          autofocus
          spellcheck="false"
          autocomplete="off"
        />
      </div>
      <div ref="listEl" class="max-h-[min(420px,60vh)] overflow-y-auto p-1.5">
        <!-- 「跟随缺省」常驻档（noneLabel 提供时）：不吃搜索过滤 -->
        <button
          v-if="props.noneLabel"
          type="button"
          role="option"
          class="mb-0.5 flex w-full items-center gap-2 rounded-[var(--lw-r-sm)] px-2 py-1.5 text-left transition hover:bg-wash-2"
          :aria-selected="activeId === ''"
          @click="emit('select', '', ''); emit('close')"
        >
          <span class="w-4 shrink-0 text-accent">
            <Icon v-if="activeId === ''" name="check" :size="12" />
          </span>
          <span class="min-w-0 flex-1 truncate text-[length:var(--lw-fs-12half)] text-ink-1">{{ props.noneLabel }}</span>
        </button>
        <p v-if="visible.length === 0" class="px-2 py-4 text-center text-[length:var(--lw-fs-12)] text-ink-4">
          {{ t("settings.modelNoMatch") }}
        </p>
        <template v-for="(r, i) in visible" :key="`${r.providerId}:${r.model}`">
          <p
            v-if="i === 0 || visible[i - 1].providerId !== r.providerId"
            class="px-2 pb-1 pt-2 text-[length:var(--lw-fs-11)] text-ink-4"
          >
            {{ r.label }}
          </p>
          <button
            type="button"
            role="option"
            class="flex w-full items-center gap-2 rounded-[var(--lw-r-sm)] px-2 py-1.5 text-left transition hover:bg-wash-2"
            :aria-selected="isCurrent(r)"
            @click="emit('select', r.providerId, r.model); emit('close')"
          >
            <span class="w-4 shrink-0 text-accent">
              <Icon v-if="isCurrent(r)" name="check" :size="12" />
            </span>
            <span class="min-w-0 flex-1 truncate font-mono text-[length:var(--lw-fs-12half)] text-ink-1">{{ r.model }}</span>
            <span v-if="capsOf(r).thinking" class="shrink-0 text-ink-4" :aria-label="t('model.badgeThinking')">
              <Icon name="sparkle" :size="11" />
            </span>
            <span v-if="capsOf(r).vision" class="shrink-0 text-ink-4" :aria-label="t('model.badgeVision')">
              <Icon name="eye" :size="11" />
            </span>
            <span
              v-if="formatPricingShort(r.pricing)"
              class="shrink-0 text-[length:var(--lw-fs-11)] tabular-nums text-ink-5"
            >{{ formatPricingShort(r.pricing) }}</span>
          </button>
        </template>
      </div>
      </div>
    </Transition>
  </Teleport>
</template>
