<script setup lang="ts">
/**
 * 回收区（S1 刀④c · AC-S1-7 那一档的界面兑现）：`obsolete/` 里那些"已经删掉但还没真删"
 * 的记忆列出来，逐条可恢复；**只有**点「清空回收区」并再点一次确认才是真删。
 *
 * 为什么这块必须长在界面上而不只是一句注释：删除改成软删（刀④a）之后，"我删错了怎么办"
 * 在人的这一侧只有这一个答案；反过来，有人删一条是为了删掉一句含隐私的话——那他要的是
 * 不可逆那一档。所以"先搬进回收区、可以恢复"与"清空才算真删"这两句话必须在页面上写出来
 * （specs/05 的 AC-S1-6 / AC-S1-7 判读登记的就是这条分界）。
 *
 * 数据源 = 真源目录，不发 SQL；与注入解释面板同款"打开拉一次 + 动作后重拉"，不轮询。
 * 恢复动作传回去的是清单给的那条 `rel` 原样（后端 `is_trash_relative` 那道闸认它；
 * 前端递路径这件事在这里同样不存在）。
 */
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import SettingsGroup from "../settings/SettingsGroup.vue";
import {
  memoryObsoleteList,
  memoryObsoletePurge,
  memoryObsoleteRestore,
} from "../../lib/api";
import { relativeTimeLabel, relativeTimeSec } from "../../lib/format";
import { workspaceDisplayName } from "../../lib/session-groups";
import type { ObsoleteView } from "../../types";

/** 恢复 / 清空之后：父页重拉事实列表，层区那一块也要重拉（同一批文件）。 */
const emit = defineEmits<{ changed: [] }>();

const { t } = useI18n();
const view = ref<ObsoleteView | null>(null);
const loadError = ref("");

async function load() {
  loadError.value = "";
  try {
    view.value = await memoryObsoleteList();
  } catch (e) {
    loadError.value = String(e);
  }
}
onMounted(() => void load());
defineExpose({ reload: load });

function deletedLabel(ms: number): string {
  return t("memory.trashDeletedAt", {
    time: relativeTimeLabel(relativeTimeSec(Math.round(ms / 1000)), (k, p) => t(k, p)),
  });
}

async function restore(rel: string) {
  loadError.value = "";
  try {
    await memoryObsoleteRestore(rel);
    await load();
    emit("changed");
  } catch (e) {
    // 原位置已被占（删掉之后又写了同一条）⇒ 后端拒绝且不覆盖，这里如实报错
    loadError.value = String(e);
  }
}

/** 「清空回收区」= 全站唯一一处不可逆动作，所以带"再点一次"确认（§11.3 同款）。 */
const confirmingPurge = ref(false);
async function purge() {
  if (!confirmingPurge.value) {
    confirmingPurge.value = true;
    return;
  }
  confirmingPurge.value = false;
  loadError.value = "";
  try {
    await memoryObsoletePurge();
    await load();
  } catch (e) {
    loadError.value = String(e);
  }
}
</script>

<template>
  <div class="shrink-0 px-5 pb-2 pt-3">
    <SettingsGroup :title="t('memory.trashTitle')" :hint="t('memory.trashHint')">
      <p
        v-if="loadError"
        class="mb-1 rounded-[var(--lw-r-xs)] bg-danger/10 px-2 py-1 text-[length:var(--lw-fs-11)] text-danger"
      >
        {{ loadError }}
      </p>
      <div class="flex items-center gap-2">
        <p class="min-w-0 flex-1 text-[length:var(--lw-fs-11)] text-ink-5">
          {{
            view && view.rows.length
              ? t("memory.trashCount", { n: view.rows.length })
              : t("memory.trashEmpty")
          }}
          <span v-if="view && view.unreadable">
            {{ t("memory.trashUnreadable", { n: view.unreadable }) }}
          </span>
        </p>
        <button
          v-if="view && view.rows.length"
          class="shrink-0 rounded-[var(--lw-r-xs)] px-3 py-1 text-[length:var(--lw-fs-11)] transition"
          :class="
            confirmingPurge
              ? 'bg-danger/16 text-danger'
              : 'text-ink-4 hover:bg-wash-2 hover:text-danger'
          "
          @click="purge"
        >
          {{
            confirmingPurge
              ? t("memory.trashPurgeConfirm")
              : t("memory.trashPurge")
          }}
        </button>
      </div>
      <ul v-if="view && view.rows.length" class="mt-1 max-h-44 space-y-1 overflow-y-auto">
        <li
          v-for="row in view.rows"
          :key="row.rel"
          class="flex items-center gap-2 rounded-[var(--lw-r-md)] bg-wash-1 px-3 py-2"
        >
          <span class="min-w-0 flex-1 truncate font-mono text-xs text-ink-1">
            {{ row.key }}
          </span>
          <span class="shrink-0 text-[length:var(--lw-fs-10)] text-ink-5">
            {{
              row.workspace
                ? workspaceDisplayName(row.workspace)
                : t("memory.globalTier")
            }}
          </span>
          <span class="shrink-0 text-[length:var(--lw-fs-10)] text-ink-5">
            {{ deletedLabel(row.deleted_at_ms) }}
          </span>
          <button
            class="shrink-0 rounded-[var(--lw-r-xs)] px-2 py-1 text-[length:var(--lw-fs-11)] text-ink-4 transition hover:bg-wash-2 hover:text-ink-1"
            @click="restore(row.rel)"
          >
            {{ t("memory.trashRestore") }}
          </button>
        </li>
      </ul>
    </SettingsGroup>
  </div>
</template>
