<script setup lang="ts">
/**
 * 模型选择器浮层（UI shell 文档 §3.30）：锚在模型 chip 上方（composer
 * 贴底向上开），宽 min(280px, 100vw-32px)（v2.90 目检裁定收窄；对话态/
 * 首页空态同一实例同尺寸）、最大高 min(420px, 60vh) 滚动；
 * 顶部搜索字段（§3.19 形态）；结果按服务商档位分组（组标签 11px ink-4
 * 不可点，项 12.5px + 右侧 ✓ 当前项，hover wash-2），打开时滚到当前项；
 * 无结果显示文案 + 「配置服务」动作。
 * ㉚·改版数据源 = **已配置模型清单**（启用档 × 其 models，档无清单回落
 * 其单 model——与 PI 同口径：选择器列「已配置」，不现场探测，离线可用；
 * 探测/获取清单在设置 → 模型配置页）。停用档不出现。选择语义见 chat
 * store.setModel（会话内 = 会话级覆盖，首页 = 写设置缺省）；会话覆盖生效
 * 时顶部一行「跟随默认」清除覆盖。
 */
import { computed, nextTick, onMounted, ref, Transition } from "vue";
import { useI18n } from "vue-i18n";
import type { ProviderView } from "../../types";
import { formatPricingShort } from "../../lib/model-catalog";
import { useDismissOnEsc } from "../../lib/use-popover";
import { Icon } from "../../lib/icons";
import SearchInput from "../patterns/SearchInput.vue";

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

/** models.dev 目录价（USD 原值，批 A §4.6）：行尾缀，无数据不渲染。 */
const pricingOf = (providerId: string, model: string): string => {
  const hit = props.providers
    .find((p) => p.id === providerId)
    ?.models.find((m) => m.id === model);
  return formatPricingShort(hit?.pricing ?? null);
};
/** 模型行能力徽章（批 B §5.3）：✨ = 有推理梯子（可调档）、👁 = 图像输入。 */
const modelCaps = (providerId: string, model: string) => {
  const m = props.providers
    .find((p) => p.id === providerId)
    ?.models.find((x) => x.id === model);
  return {
    thinking: (m?.reasoningLadder?.length ?? 0) > 0,
    vision: m?.supportsImage != null ? m.supportsImage : (m?.inputModalities ?? []).includes("image"),
  };
};

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

// v-if 挂载即打开：滚到当前项（当前项必须可见，§3.30）。**只滚弹窗内部
// 清单**（手设 scrollTop）——scrollIntoView 会连祖先滚动容器一起滚，把
// 整个界面拖动（用户目检实抓「点击后整个内容上移」）；挂载期 Esc 收口走
// 全站唯一那份（外点关闭由 Composer 锚点监听——走查实抓：本浮层曾是五弹层
// 中唯一无外点/Esc 收口的）。挂载即打开 = `useDismissOnEsc` 的省略 `open`
// 那种归属，占优先级到卸载为止。
useDismissOnEsc(() => emit("close"));
onMounted(() => {
  query.value = "";
  void nextTick(() => {
    const list = container.value?.querySelector<HTMLElement>(".ms-list");
    const cur = container.value?.querySelector<HTMLElement>('[data-current="true"]');
    if (list && cur) {
      // 手动居中：目标行中线对齐清单中线（等价 block:center，但绝不外溢）
      const target = cur.offsetTop - list.offsetTop - list.clientHeight / 2 + cur.offsetHeight / 2;
      list.scrollTop = Math.max(0, target);
    }
  });
});

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
    <!-- v2.143：搜索胶囊包一层 padding 容器——此前 mx 负距 + 子组件
         width:100%（scoped 特异性高于工具类）在 280px 弹层里恒右溢
         10px（用户目检实抓）；块级容器让 label 拉伸到内容盒，不再出界 -->
    <div class="ms-search">
      <SearchInput
        v-model="query"
        :placeholder="t('model.searchPlaceholder')"
        :label="t('model.searchPlaceholder')"
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
          <span v-if="modelCaps(g.id, m).thinking" class="ms-row__cap" :aria-label="t('model.badgeThinking')">
            <Icon name="sparkle" :size="11" />
          </span>
          <span v-if="modelCaps(g.id, m).vision" class="ms-row__cap" :aria-label="t('model.badgeVision')">
            <Icon name="eye" :size="11" />
          </span>
          <span v-if="pricingOf(g.id, m)" class="ms-row__price">{{ pricingOf(g.id, m) }}</span>
          <!-- v2.143：对勾槽位恒在（非当前行 visibility 隐藏）——此前 v-if
               只在当前行渲染，勾一出现把名称/价格挤移位（用户目检实抓）；
               预留等宽槽位后各行名称宽度一致，切换选择零位移 -->
          <Icon
            name="check"
            :size="14"
            class="ms-row__check"
            :class="{ 'ms-row__check--off': m !== effective() }"
          />
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
  width: min(280px, calc(100vw - 32px));
  display: flex;
  flex-direction: column;
  border-radius: var(--lw-r-md-plus);
  border: 1px solid var(--lw-line);
  background: var(--lw-bg-elevated);
  box-shadow: var(--lw-shadow-pop);
  z-index: var(--lw-z-pop);
  overflow: hidden;
}
/* 首页空态向下弹（v2.59 用户裁定） */
.model-selector--down {
  top: calc(100% + 10px);
  bottom: auto;
}
/* 搜索胶囊容器（v2.143 出界修复）：padding 承载原 mx 间距，子组件
 * width:100% 相对内容盒求值 → 恰好填满不出界 */
.ms-search {
  padding: var(--lw-gap-10) var(--lw-gap-10) var(--lw-gap-6);
}
.ms-list {
  max-height: min(420px, 60vh);
  overflow-y: auto;
  padding: var(--lw-gap-2) var(--lw-gap-6) var(--lw-gap-8);
}
.ms-group {
  padding: var(--lw-gap-10) var(--lw-gap-8) 3px; /* stylelint-disable-line declaration-property-value-disallowed-list -- 光学修正(孤值3,§二⑬) */
  font-size: var(--lw-fs-11);
  color: var(--lw-ink-4);
  user-select: none;
}
/* v3.9 用户裁定：分组间淡发丝线，两端不到头（inset 12px） */
.ms-group--ruled {
  border-top: 1px solid var(--lw-line-subtle);
  margin: var(--lw-gap-4) var(--lw-gap-12) 0;
  padding: var(--lw-gap-10) 0 3px; /* stylelint-disable-line declaration-property-value-disallowed-list -- 光学修正(孤值3,§二⑬) */
}
.ms-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--lw-gap-8);
  width: 100%;
  padding: 0 var(--lw-gap-8);
  height: 30px;
  border-radius: var(--lw-r-xs);
  font-size: var(--lw-fs-12half);
  color: var(--lw-ink-1);
  text-align: left;
}
.ms-row:hover {
  background: var(--lw-wash-2);
}
.ms-row--default {
  margin: 0 var(--lw-gap-6) var(--lw-gap-4);
  color: var(--lw-ink-3);
  gap: var(--lw-gap-6);
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
.ms-row__price {
  flex: none;
  font-size: var(--lw-fs-11);
  color: var(--lw-ink-5);
  font-variant-numeric: tabular-nums;
}
.ms-row__cap {
  flex: none;
  display: inline-flex;
  color: var(--lw-ink-4);
}
.ms-row__check {
  color: var(--lw-ink-1);
  flex: 0 0 auto;
}
/* 非当前行：勾不可见但占位（v2.143 对勾挤位移修复） */
.ms-row__check--off {
  visibility: hidden;
}
.ms-empty {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: var(--lw-gap-6);
  padding: var(--lw-gap-16) var(--lw-gap-10);
  font-size: var(--lw-fs-12);
  color: var(--lw-ink-4);
}
.ms-configure {
  font-size: var(--lw-fs-12);
  color: var(--lw-accent);
}
.ms-configure:hover {
  text-decoration: underline;
}
</style>
