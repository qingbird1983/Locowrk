<script setup lang="ts">
/**
 * 崩溃恢复 uncertain 决策卡（G2，可靠性稿 §5）：进程在工具执行中退出、
 * 副作用未知（已执行/部分执行/未执行无法从本地证据判断）。三个裁决按
 * 设计稿 §5.4 收窄——Rerun 不设（「未执行」裁决后模型在下一轮自然重做，
 * 走全部既有审批/守卫门）；裁决经 session_resolve_uncertain 落日志并
 * 实时回推（uncertain_resolved 收束本卡为记录行 + 合成 tool_result 闭合
 * 工具行）。组外兄弟节点（同 error/notice 语义——折进过程组会被默认
 * 折叠藏住，决策面必须可见）。
 */
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Icon } from "../../lib/icons";
import { toolAction } from "../../lib/tool-summary";
import { useChatStore } from "../../stores/chat";
import { useToastStore } from "../../stores/toast";
import type { ChatItem } from "../../stores/chat";

type UncertainItem = Extract<ChatItem, { kind: "uncertain" }>;

const props = defineProps<{ item: UncertainItem }>();

const { t } = useI18n();
const chat = useChatStore();
const toast = useToastStore();

/** 动作名（§3.18 中文动作名，表外回落工具 id；细节缺失 = 如实渲染）。 */
const actionLabel = computed(() => {
  if (!props.item.tool) return t("uncertain.detailMissing");
  const a = toolAction(props.item.tool);
  return a ? t(a.key, a.params ?? {}) : props.item.tool;
});

const deciding = ref(false);
const noteMode = ref(false);
const note = ref("");

async function resolve(decision: "completed" | "not_executed" | "inspected") {
  if (deciding.value) return;
  deciding.value = true;
  try {
    await chat.resolveUncertain(props.item.callId, decision, note.value.trim());
  } catch (err) {
    // 裁决失败如实提示（忙锁/已裁决等后端守卫的报文原样透出）
    toast.push("danger", t("notice.editResendFailed", { reason: String(err) }));
  } finally {
    deciding.value = false;
    noteMode.value = false;
  }
}
</script>

<template>
  <div class="unc-card" role="group" :aria-label="t('uncertain.title')">
    <template v-if="!item.resolved">
      <div class="unc-card__head">
        <Icon name="warn" :size="14" />
        <span class="unc-card__title">{{ t("uncertain.title") }}</span>
        <span class="unc-card__action">{{ actionLabel }}</span>
      </div>
      <p class="unc-card__desc">{{ t("uncertain.desc") }}</p>
      <div v-if="!noteMode" class="unc-card__actions">
        <button class="unc-btn" type="button" :disabled="deciding" @click="resolve('completed')">
          {{ t("uncertain.completed") }}
        </button>
        <button
          class="unc-btn"
          type="button"
          :disabled="deciding"
          @click="resolve('not_executed')"
        >
          {{ t("uncertain.notExecuted") }}
        </button>
        <button
          class="unc-btn"
          type="button"
          :disabled="deciding"
          @click="noteMode = true"
        >
          {{ t("uncertain.inspect") }}
        </button>
      </div>
      <div v-else class="unc-card__note">
        <input
          v-model="note"
          class="unc-card__input lw-field"
          type="text"
          :placeholder="t('uncertain.notePlaceholder')"
          @keydown.enter.prevent="resolve('inspected')"
        />
        <button
          class="unc-btn unc-btn--primary"
          type="button"
          :disabled="deciding || !note.trim()"
          @click="resolve('inspected')"
        >
          {{ t("uncertain.confirm") }}
        </button>
      </div>
    </template>
    <div v-else class="unc-card__resolved">
      <Icon name="check" :size="12" />
      <span class="unc-card__resolved-label">{{ t("uncertain.resolved") }}</span>
      <span>{{
        item.resolved.decision === "completed"
          ? t("uncertain.rCompleted")
          : item.resolved.decision === "not_executed"
            ? t("uncertain.rNotExecuted")
            : t("uncertain.rInspected")
      }}</span>
      <span v-if="item.resolved.note" class="unc-card__resolved-note">{{
        item.resolved.note
      }}</span>
    </div>
  </div>
</template>

<style scoped>
.unc-card {
  padding: var(--lw-gap-10) var(--lw-gap-12);
  border-radius: var(--lw-r-md);
  background: color-mix(in oklab, var(--lw-warn) 9%, transparent);
}
.unc-card__head {
  display: flex;
  align-items: center;
  gap: 7px; /* stylelint-disable-line declaration-property-value-disallowed-list -- 光学修正(孤值7,§二⑬) */
  min-width: 0;
  color: var(--lw-warn);
}
.unc-card__title {
  flex: 0 0 auto;
  font-size: var(--lw-fs-12);
  font-weight: 600;
}
.unc-card__action {
  flex: 1 1 auto;
  min-width: 0;
  font-size: var(--lw-fs-12);
  color: var(--lw-ink-3);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.unc-card__desc {
  margin: var(--lw-gap-4) 0 0;
  font-size: var(--lw-fs-12);
  line-height: 1.5;
  color: var(--lw-ink-3);
}
.unc-card__actions {
  display: flex;
  flex-wrap: wrap;
  gap: var(--lw-gap-6);
  margin-top: var(--lw-gap-8);
}
.unc-btn {
  padding: var(--lw-gap-4) var(--lw-gap-10);
  border-radius: var(--lw-r-2xs);
  background: var(--lw-ctl-hover);
  font-size: var(--lw-fs-12);
  color: var(--lw-ink-1);
  cursor: pointer;
}
.unc-btn:hover:enabled {
  background: color-mix(in oklab, var(--lw-warn) 22%, transparent);
}
.unc-btn:disabled {
  opacity: 0.55;
  cursor: default;
}
.unc-btn--primary {
  background: var(--lw-accent);
  color: var(--lw-accent-ink);
}
.unc-card__note {
  display: flex;
  gap: var(--lw-gap-6);
  margin-top: var(--lw-gap-8);
}
.unc-card__input {
  flex: 1 1 auto;
  min-width: 0;
}
.unc-card__resolved {
  display: flex;
  align-items: baseline;
  gap: 7px; /* stylelint-disable-line declaration-property-value-disallowed-list -- 光学修正(孤值7,§二⑬) */
  font-size: var(--lw-fs-12);
  color: var(--lw-ink-2);
}
.unc-card__resolved > svg {
  color: var(--lw-warn);
  align-self: center;
}
.unc-card__resolved-label {
  color: var(--lw-ink-4);
}
.unc-card__resolved-note {
  min-width: 0;
  color: var(--lw-ink-3);
  overflow-wrap: anywhere;
}
</style>
