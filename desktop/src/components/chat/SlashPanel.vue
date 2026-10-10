<script setup lang="ts">
/**
 * 斜杠命令面板（v2.143，UI shell §3.36① 扩展，参考 ZCode 命令面板）：
 * composer 键入 `/` 即在输入卡上方弹出，↑↓ 选择 / Enter 或点击**直接
 * 触发**（不回填文本）；继续键入按命令名/别名过滤，Esc 关闭。键盘导航
 * 状态自持（active 游标），Composer 只负责转发按键与开关。
 * 命令清单 = lib/slash-commands.ts 注册表（唯一真源）。
 */
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { filterSlashCommands, type SlashCommand } from "../../lib/slash-commands";
import { Icon } from "../../lib/icons";

const props = defineProps<{
  /** `/` 之后的键入文本（过滤用）。 */
  query: string;
}>();
const emit = defineEmits<{
  select: [cmd: SlashCommand];
  close: [];
}>();

const { t } = useI18n();

const filtered = computed(() => filterSlashCommands(props.query));
const active = ref(0);
// 过滤结果变化：游标归零（钳界，删除字符时不清空面板）
watch(filtered, (list) => {
  if (active.value >= list.length) active.value = 0;
});

function move(delta: number) {
  const n = filtered.value.length;
  if (n === 0) return;
  active.value = (active.value + delta + n) % n;
}

/** Composer 转发的导航键；返回 true = 已消费（调用方 preventDefault）。 */
function onKey(e: KeyboardEvent): boolean {
  if (e.key === "ArrowDown") {
    move(1);
    return true;
  }
  if (e.key === "ArrowUp") {
    move(-1);
    return true;
  }
  if (e.key === "Enter" || e.key === "Tab") {
    const cmd = filtered.value[active.value];
    if (cmd) emit("select", cmd);
    return true;
  }
  return false;
}
defineExpose({ onKey });
</script>

<template>
  <div class="slash" role="listbox" :aria-label="t('slash.title')">
    <div class="slash__head">{{ t("slash.title") }}</div>
    <div class="slash__list scroll-y">
      <button
        v-for="(c, i) in filtered"
        :key="c.name"
        type="button"
        class="slash__row"
        :class="{ 'is-active': i === active }"
        role="option"
        :aria-selected="i === active"
        @mouseenter="active = i"
        @click="emit('select', c)"
      >
        <span class="slash__name">/{{ c.name }}</span>
        <span class="slash__desc">{{ t(c.descKey) }}</span>
      </button>
      <div v-if="filtered.length === 0" class="slash__empty">{{ t("slash.none") }}</div>
    </div>
    <div class="slash__hint">
      <Icon name="info" :size="12" aria-hidden="true" />
      <span>{{ t("slash.hint") }}</span>
    </div>
  </div>
</template>

<style scoped>
/* 面板 = 输入卡上方的独立卡片（同 composer 卡族：elevated 底 + 发丝线
 * 轮廓 + 弹层影），锚在 composer 顶边上浮（Composer 内绝对定位） */
.slash {
  display: flex;
  flex-direction: column;
  max-height: 264px;
  border-radius: var(--lw-r-lg-plus);
  border: 1px solid var(--lw-line);
  background: var(--lw-bg-elevated);
  box-shadow: var(--lw-shadow-pop);
  overflow: hidden;
}
.slash__head {
  flex: none;
  padding: var(--lw-gap-10) var(--lw-gap-14) var(--lw-gap-4);
  font-size: var(--lw-fs-11);
  color: var(--lw-ink-4);
  user-select: none;
}
.slash__list {
  overflow-y: auto;
  padding: 0 var(--lw-gap-6) var(--lw-gap-4);
}
.slash__row {
  display: flex;
  align-items: baseline;
  gap: var(--lw-gap-10);
  width: 100%;
  padding: var(--lw-gap-8) var(--lw-gap-8);
  border-radius: var(--lw-r-xs);
  text-align: left;
}
.slash__row.is-active {
  background: var(--lw-wash-2);
}
.slash__name {
  flex: none;
  font-size: var(--lw-fs-12half);
  font-weight: 500;
  color: var(--lw-ink-1);
  font-variant-numeric: tabular-nums;
}
.slash__desc {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: var(--lw-fs-12);
  color: var(--lw-ink-4);
}
.slash__empty {
  padding: var(--lw-gap-12) var(--lw-gap-8);
  font-size: var(--lw-fs-12);
  color: var(--lw-ink-4);
}
.slash__hint {
  flex: none;
  display: flex;
  align-items: center;
  gap: var(--lw-gap-6);
  padding: var(--lw-gap-6) var(--lw-gap-14);
  border-top: 1px solid var(--lw-line-subtle);
  font-size: var(--lw-fs-11);
  color: var(--lw-ink-5);
}
.slash__hint svg {
  flex: none;
  color: var(--lw-ink-5);
}
</style>
