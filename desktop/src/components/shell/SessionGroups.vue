<script setup lang="ts">
/**
 * 会话树（UI shell 文档 §2.3）：顶层「会话」组（未关联）+ 「项目」组
 * （一级 = 工作区，头行 12px chevron + 14px 文件夹 + 13px 名 + 可选 6px
 * 运行点，可折叠且折叠态持久化进侧栏偏好；二级 = 时间桶，桶内会话行
 * 缩进一档）。行渲染与运营在 SessionRow（§3.6 行尾 `…` 菜单）。
 */
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useChatStore } from "../../stores/chat";
import type { SessionMeta } from "../../types";
import { groupSessions } from "../../lib/session-groups";
import { loadCollapsedGroups, saveCollapsedGroups } from "../../lib/sidebar-preference";
import { Icon } from "../../lib/icons";
import SessionRow from "./SessionRow.vue";

const props = defineProps<{ sessions: SessionMeta[] }>();
const { t } = useI18n();
const chat = useChatStore();

const tree = computed(() =>
  groupSessions(props.sessions, chat.running ? chat.sessionId : null),
);

// 分组头折叠（§2.3）：键 = workspace 存储路径，随偏好持久化
const collapsed = ref(loadCollapsedGroups());

function toggle(key: string) {
  const next = new Set(collapsed.value);
  if (!next.delete(key)) next.add(key);
  collapsed.value = next;
  saveCollapsedGroups(next);
}
</script>

<template>
  <div>
    <!-- 顶层「会话」组（未关联会话；v2.50 前全量在此） -->
    <div class="side-label">{{ t("sidebar.sessions") }}</div>
    <template v-for="g in tree.top" :key="g.key">
      <div class="bucket-label">{{ t(`sidebar.${g.key}`) }}</div>
      <SessionRow v-for="s in g.items" :key="s.id" :session="s" />
    </template>

    <!-- 「项目」组：一级 = 工作区（组内最近活动降序），二级 = 时间桶 -->
    <template v-if="tree.workspaces.length > 0">
      <div class="side-label">{{ t("sidebar.projects") }}</div>
      <div v-for="w in tree.workspaces" :key="w.key">
        <button
          type="button"
          class="ws-head"
          :aria-expanded="!collapsed.has(w.key)"
          :title="w.key"
          @click="toggle(w.key)"
        >
          <Icon
            name="chevronRight"
            :size="12"
            class="ws-head__chevron"
            :class="{ 'is-open': !collapsed.has(w.key) }"
            aria-hidden="true"
          />
          <Icon name="folder" :size="14" aria-hidden="true" />
          <span class="ws-head__name">{{ w.name }}</span>
          <span
            v-if="w.running"
            class="ws-head__dot"
            role="img"
            :aria-label="t('status.running')"
            :title="t('status.running')"
          />
        </button>
        <template v-if="!collapsed.has(w.key)">
          <template v-for="b in w.buckets" :key="b.key">
            <div class="bucket-label bucket-label--nested">
              {{ t(`sidebar.${b.key}`) }}
            </div>
            <SessionRow v-for="s in b.items" :key="s.id" :session="s" nested />
          </template>
        </template>
      </div>
    </template>
  </div>
</template>

<style scoped>
.side-label {
  padding: 10px 8px 4px;
  font-size: 11.5px;
  color: var(--lw-ink-4);
}
.bucket-label {
  padding: 6px 8px 2px 28px;
  font-size: 11.5px;
  color: var(--lw-ink-4);
}
.bucket-label--nested {
  padding-left: 56px;
}

/* 工作区分组头（§2.3）：28px 行 = 12px chevron + 14px 文件夹 + 13px 名 +
 * 可选 6px 状态点；hover wash-2；chevron 展开转 90° */
.ws-head {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
  min-height: 28px;
  padding: 0 8px;
  border-radius: var(--lw-r-xs);
  text-align: left;
}
.ws-head:hover {
  background: var(--lw-wash-2);
}
.ws-head__chevron {
  flex: 0 0 auto;
  color: var(--lw-ink-4);
  transition: transform var(--dur-fast) var(--ease-out);
}
.ws-head__chevron.is-open {
  transform: rotate(90deg);
}
.ws-head__name {
  min-width: 0;
  flex: 1 1 auto;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 13px;
  color: var(--lw-ink-2);
}
.ws-head__dot {
  width: 6px;
  height: 6px;
  flex: 0 0 auto;
  border-radius: var(--lw-r-full);
  background: var(--lw-ok);
}
</style>
