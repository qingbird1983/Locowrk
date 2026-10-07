<script lang="ts">
/** 名单行（将受影响的条目）。 */
export interface ConfirmItem {
  id: string;
  /** 行首字形（可省）。 */
  icon?: "plug" | "file" | "bot";
  name: string;
  /** 行尾 mono 元信息（id · N 个工具）。 */
  meta?: string;
}
</script>

<script setup lang="ts">
/**
 * 确认弹窗（UI shell doc §五：唯一还带遮罩的 Modal——"不答就不能做别的事"
 * 的场景才允许：名单 / 数量 / 破坏范围一次说清；§3.17 批量卸载二次确认是
 * 唯一现行实例）。形态照原型 S12 `.dialog--confirm` 逐值：danger 7% 头段 +
 * 名单井（inset 底、190px 滚动）+ 影响范围 + 右对齐 取消 ghost / 确认
 * primary（§3.17 纪律：不用红色按钮，风险由头段染底承载）。
 */
import { useI18n } from "vue-i18n";
import { Icon } from "../../lib/icons";

defineProps<{
  open: boolean;
  title: string;
  sub?: string;
  /** 名单（将受影响的条目）；空 = 只出 sub 文案。 */
  items?: ConfirmItem[];
  /** 名单标签（如「将卸载（3）」）。 */
  itemsLabel?: string;
  /** 影响范围一句说明。 */
  impact?: string;
  confirmLabel: string;
  cancelLabel?: string;
}>();
const emit = defineEmits<{ confirm: []; cancel: [] }>();

const { t } = useI18n();
</script>

<template>
  <Teleport to="body">
    <Transition name="lw-modal">
      <div
        v-if="open"
        class="fixed inset-0 z-[var(--lw-z-modal)] flex items-center justify-center px-6"
        style="background: var(--lw-scrim)"
        @mousedown.self="emit('cancel')"
      >
        <div
          class="dialog lw-modal-panel"
          role="dialog"
          aria-modal="true"
          :aria-label="title"
        >
          <div class="dialog__head">
            <Icon name="warn" :size="14" class="mt-0.5 shrink-0 text-danger" aria-hidden="true" />
            <div class="min-w-0">
              <div class="dialog__title">{{ title }}</div>
              <p v-if="sub" class="dialog__sub">{{ sub }}</p>
            </div>
          </div>
          <div class="dialog__body">
            <div v-if="items && items.length > 0">
              <div class="dialog__label">{{ itemsLabel }}</div>
              <div class="confirm-list selectable">
                <div
                  v-for="item in items"
                  :key="item.id"
                  class="confirm-list__row"
                >
                  <Icon
                    :name="item.icon ?? 'plug'"
                    :size="12"
                    class="shrink-0"
                    aria-hidden="true"
                  />
                  <span class="truncate">{{ item.name }}</span>
                  <span v-if="item.meta" class="confirm-list__id">{{
                    item.meta
                  }}</span>
                </div>
              </div>
            </div>
            <div v-if="impact">
              <div class="dialog__label">{{ t("confirm.impact") }}</div>
              <p class="dialog__impact">{{ impact }}</p>
            </div>
          </div>
          <div class="dialog__foot">
            <button
              class="btn btn--ghost"
              type="button"
              @click="emit('cancel')"
            >
              {{ cancelLabel ?? t("common.cancel") }}
            </button>
            <button
              class="btn btn--primary"
              type="button"
              @click="emit('confirm')"
            >
              {{ confirmLabel }}
            </button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
/* 弹窗（原型 .dialog / .dialog--confirm 逐值） */
.dialog {
  width: min(100%, 520px);
  overflow: hidden;
  border-radius: var(--lw-r-lg-plus);
  background: var(--lw-bg-elevated);
  border: 1px solid var(--lw-line);
  box-shadow: var(--lw-shadow-dialog);
}
.dialog__head {
  display: flex;
  align-items: flex-start;
  gap: var(--lw-gap-12);
  padding: var(--lw-gap-14) var(--lw-gap-16);
  background: color-mix(in oklab, var(--lw-danger) 7%, transparent);
  border-bottom: 1px solid var(--lw-line-subtle);
}
.dialog__title {
  font-size: var(--lw-fs-14);
  font-weight: 600;
  letter-spacing: -0.02em;
  line-height: 1.3;
  color: var(--lw-ink-1);
}
.dialog__sub {
  margin: 3px 0 0; /* stylelint-disable-line declaration-property-value-disallowed-list -- 光学修正(孤值3,§二⑬) */
  font-size: var(--lw-fs-12);
  line-height: 1.45;
  color: var(--lw-ink-2);
}
.dialog__body {
  padding: var(--lw-gap-16);
  display: flex;
  flex-direction: column;
  gap: var(--lw-gap-12);
}
.dialog__label {
  font-size: var(--lw-fs-11);
  letter-spacing: 0.02em;
  color: var(--lw-ink-4);
  margin-bottom: var(--lw-gap-4);
}
.confirm-list {
  display: flex;
  flex-direction: column;
  gap: var(--lw-gap-6);
  max-height: 190px;
  overflow: auto;
  border-radius: var(--lw-r-sm);
  background: var(--lw-bg-inset);
  padding: var(--lw-gap-10) var(--lw-gap-12);
}
.confirm-list__row {
  display: flex;
  align-items: center;
  gap: var(--lw-gap-8);
  font-size: var(--lw-fs-12half);
  color: var(--lw-ink-2);
}
.confirm-list__id {
  margin-left: auto;
  flex: 0 0 auto;
  font-family: var(--font-mono);
  font-size: var(--lw-fs-12);
  color: var(--lw-ink-4);
}
.dialog__impact {
  margin: 0;
  font-size: var(--lw-fs-12);
  line-height: 1.45;
  color: var(--lw-ink-2);
}
.dialog__foot {
  display: flex;
  justify-content: flex-end;
  gap: var(--lw-gap-8);
  padding: 0 var(--lw-gap-16) var(--lw-gap-16);
}

/* 按钮（§3.19 三变体） */
.btn {
  display: inline-flex;
  align-items: center;
  gap: var(--lw-gap-6);
  border-radius: var(--lw-r-sm);
  padding: 7px var(--lw-gap-12); /* stylelint-disable-line declaration-property-value-disallowed-list -- 光学修正(孤值7,§二⑬) */
  font-size: var(--lw-fs-13);
  font-weight: 500;
  line-height: 1.2;
  white-space: nowrap;
}
.btn:active {
  transform: scale(0.98);
}
.btn--primary {
  background: var(--lw-accent);
  color: var(--lw-accent-ink);
}
.btn--primary:hover {
  background: var(--lw-accent-hover);
}
.btn--ghost {
  color: var(--lw-ink-2);
}
.btn--ghost:hover {
  background: var(--lw-wash-2);
  color: var(--lw-ink-1);
}
</style>
