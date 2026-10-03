<script setup lang="ts">
/**
 * 模型选择器浮层（UI shell 文档 §3.30）：锚在模型 chip 上方（composer
 * 贴底向上开），宽 min(320px, 100vw-32px)、最大高 min(420px, 60vh) 滚动；
 * 顶部搜索字段（§3.19 形态）；结果按服务商档位分组（组标签 11px ink-4
 * 不可点，项 12.5px + 右侧 ✓ 当前项，hover wash-2），打开时滚到当前项；
 * 无结果显示文案 + 「配置服务」动作。
 * ㉚·改版数据源 = **已配置模型清单**（启用档 × 其 models，档无清单回落
 * 其单 model——与 PI 同口径：选择器列「已配置」，不现场探测，离线可用；
 * 探测/获取清单在设置 → 模型配置页）。停用档不出现。选择语义见 chat
 * store.setModel（会话内 = 会话级覆盖，首页 = 写设置缺省）；会话覆盖生效
 * 时顶部一行「跟随默认」清除覆盖。
 */
import { computed, nextTick, onBeforeUnmount, onMounted, ref, Transition } from "vue";
import { useI18n } from "vue-i18n";
import type { ProviderView } from "../../types";
import { Icon } from "../../lib/icons";

const props = defineProps<{
  /** 设置缺省模型（活动档平面投影）。 */
  defaultModel: string;
  /** 当前会话模型覆盖（null = 跟随设置缺省）。 */
  override: { providerId: string; model: string } | null;
  /** 启用档（配置页真源；组件内不二次过滤）。 */
  providers: ProviderView[];
  /** 首页空态 = 向下弹（composer 居中；会话态贴底向上弹，v2.59 用户裁定）。 */
  dropDown?: boolean;
}>();
const emit = defineEmits<{
  close: [];
  select: [providerId: string, model: string];
  clear: [];
  configure: [];
}>();

const { t } = useI18n();

const query = ref("");
const container = ref<HTMLElement | null>(null);

const effective = () => props.override?.model ?? props.defaultModel;

interface Group {
  id: string;
  label: string;
  models: string[];
}
/** 组 = 启用档；档无 models 清单时回落其单 model（不发明空组）。 */
const groups = computed<Group[]>(() =>
  props.providers.map((p) => ({
    id: p.id,
    label: p.label,
    models: p.models.length > 0 ? p.models.map((m) => m.id) : p.model ? [p.model] : [],
  })),
);

/** 过滤后（搜索命中模型名/组名）的分组；空组隐藏。 */
const visible = computed(() => {
  const q = query.value.trim().toLowerCase();
  return groups.value
    .map((g) => ({
      ...g,
      models: q
        ? g.models.filter((m) => m.toLowerCase().includes(q) || g.label.toLowerCase().includes(q))
        : g.models,
    }))
    .filter((g) => g.models.length > 0);
});

// v-if 挂载即打开：滚到当前项（当前项必须可见，§3.30）；挂载期 Esc 收口
// （外点关闭由 Composer 锚点监听——走查实抓：本浮层曾是五弹层中唯一
// 无外点/Esc 收口的）
function onDocKeydown(e: KeyboardEvent) {
  if (e.key === "Escape") emit("close");
}
onMounted(() => {
  document.addEventListener("keydown", onDocKeydown);
  query.value = "";
  void nextTick(() => {
    container.value
      ?.querySelector('[data-current="true"]')
      ?.scrollIntoView({ block: "center" });
  });
});
onBeforeUnmount(() => document.removeEventListener("keydown", onDocKeydown));

function pick(providerId: string, model: string) {
  emit("select", providerId, model);
  emit("close");
}
</script>

<template>
  <Transition :name="props.dropDown ? 'lw-pop-down' : 'lw-pop-up'">
  <div
    ref="container"
    class="model-selector"
    :class="{ 'model-selector--down': props.dropDown }"
    role="dialog"
    :aria-label="t('model.selector')"
  >
    <div class="ms-search">
      <Icon name="search" :size="13" class="ms-search__icon" />
      <input
        v-model="query"
        type="text"
        class="ms-search__input"
        :placeholder="t('model.searchPlaceholder')"
      />
    </div>

    <button
      v-if="override"
      type="button"
      class="ms-row ms-row--default"
      @click="emit('clear'); emit('close')"
    >
      <Icon name="undo" :size="12" />
      <span class="ms-row__name">{{ t("model.followDefault", { model: defaultModel }) }}</span>
    </button>

    <div class="ms-list scroll-y">
      <template v-for="(g, gi) in visible" :key="g.id">
        <div class="ms-group" :class="{ 'ms-group--ruled': gi > 0 }">
          {{ g.label }}
        </div>
        <button
          v-for="m in g.models"
          :key="`${g.id}:${m}`"
          type="button"
          class="ms-row"
          :data-current="m === effective() ? 'true' : undefined"
          @click="pick(g.id, m)"
        >
          <span class="ms-row__name">{{ m }}</span>
          <Icon v-if="m === effective()" name="check" :size="13" class="ms-row__check" />
        </button>
      </template>

      <div v-if="visible.length === 0" class="ms-empty">
        <span>{{ t("model.empty") }}</span>
        <button type="button" class="ms-configure" @click="emit('configure'); emit('close')">
          {{ t("model.configure") }}
        </button>
      </div>
    </div>
  </div>
  </Transition>
</template>

<style scoped>
.model-selector {
  position: absolute;
  right: 0;
  bottom: calc(100% + 10px);
  width: min(320px, calc(100vw - 32px));
  display: flex;
  flex-direction: column;
  border-radius: var(--lw-r-md);
  border: 1px solid var(--lw-line);
  background: var(--lw-bg-elevated);
  box-shadow: var(--lw-shadow-dialog);
  z-index: 40;
  overflow: hidden;
}
/* 首页空态向下弹（v2.59 用户裁定） */
.model-selector--down {
  top: calc(100% + 10px);
  bottom: auto;
}
.ms-search {
  display: flex;
  align-items: center;
  gap: 6px;
  margin: 10px 10px 6px;
  padding: 0 10px;
  height: 30px;
  border-radius: var(--lw-r-full);
  background: var(--lw-wash-1);
  color: var(--lw-ink-4);
}
.ms-search__input {
  flex: 1;
  min-width: 0;
  border: none;
  outline: none;
  background: transparent;
  font-size: 12.5px;
  color: var(--lw-ink-1);
}
.ms-search__input::placeholder {
  color: var(--lw-ink-5);
}
.ms-list {
  max-height: min(420px, 60vh);
  overflow-y: auto;
  padding: 2px 6px 8px;
}
.ms-group {
  padding: 10px 8px 3px;
  font-size: 11px;
  color: var(--lw-ink-4);
  user-select: none;
}
/* v3.9 用户裁定：分组间淡发丝线，两端不到头（inset 12px） */
.ms-group--ruled {
  border-top: 1px solid var(--lw-line-subtle);
  margin: 4px 12px 0;
  padding: 10px 0 3px;
}
.ms-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  width: 100%;
  padding: 0 8px;
  height: 30px;
  border-radius: 8px;
  font-size: 12.5px;
  color: var(--lw-ink-1);
  text-align: left;
}
.ms-row:hover {
  background: var(--lw-wash-2);
}
.ms-row--default {
  margin: 0 6px 4px;
  color: var(--lw-ink-3);
  gap: 6px;
}
.ms-row--default:hover {
  color: var(--lw-ink-1);
}
.ms-row__name {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-variant-numeric: tabular-nums;
}
.ms-row__check {
  color: var(--lw-ink-1);
  flex: 0 0 auto;
}
.ms-empty {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 6px;
  padding: 16px 10px;
  font-size: 12px;
  color: var(--lw-ink-4);
}
.ms-configure {
  font-size: 12px;
  color: var(--lw-accent);
}
.ms-configure:hover {
  text-decoration: underline;
}
</style>
