<script setup lang="ts">
/**
 * 看板左栏（队列 Q14 第三刀 · MB §3.1）。
 *
 * 四件东西从上到下：项目下拉（锚定 fsRoots，**只有** `_root` scope 真有投影时才
 * 补末位「未归属」——没树的项目列出来点了是空页）+「打开记忆目录」幽灵按钮
 * （MB §3.1，队列 Q15 第一刀补的：命令 `memory_reveal_project` 已在，所以这里
 * 不是空壳）、按日期分档的会话列表（行 = 标题 + `N 总结 · N 章节` + 整理中徽章）、
 * 底部「返回主界面」（有入口必须有出口）。栏内不用发丝线分区，用间距（§3.1 原话）；
 * 宽度取现役 `--lw-sidebar-w`（§二⑯ 订正清单第 5 条：不另造一档画布侧栏宽）。
 *
 * 失败提示走 toast（与项目菜单「在文件夹中显示」同一条形态，不新增第三种错误
 * 呈现）；未归属档**禁用**按钮而不是让它打开一个空目录——`''` 没有项目记忆目录，
 * 后端那句拒绝正是为了让前端没法假装它有。
 */
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import SettingsSelect from "../../settings/SettingsSelect.vue";
import HoverTip from "../../settings/HoverTip.vue";
import TreeBadge from "../TreeBadge.vue";
import { bucketSessions } from "../../../lib/board-model";
import { sessionTitleOf } from "../../../lib/tree-labels";
import { workspaceDisplayName } from "../../../lib/session-groups";
import { memoryRevealProject } from "../../../lib/api";
import { useChatStore } from "../../../stores/chat";
import { useSettingsStore } from "../../../stores/settings";
import { useShellStore } from "../../../stores/shell";
import { useToastStore } from "../../../stores/toast";
import { sessionKeyOf, useTreeBoardStore } from "../../../stores/tree-board";
import type { DayBucket } from "../../../lib/board-model";
import type { TreeSummaryView } from "../../../types";

const { t } = useI18n();
const board = useTreeBoardStore();
const chat = useChatStore();
const settings = useSettingsStore();
const shell = useShellStore();
const toast = useToastStore();

/** 日期档名（i18n key 走同一张表，不在模板里 switch）。 */
const bucketKey: Record<DayBucket, string> = {
  today: "memory.boardToday",
  yesterday: "memory.boardYesterday",
  week: "memory.boardWeek",
  earlier: "memory.boardEarlier",
};

/** 未归属档只在真有树时出现（点了空下拉 = 界面在骗人）。 */
const hasUnowned = computed(() =>
  (board.sessions ?? []).some((s) => s.workspace === ""),
);

const projectEntries = computed(() => {
  const roots = settings.view?.tools.fsRoots ?? [];
  const list = roots.map((ws) => ({ value: ws, label: workspaceDisplayName(ws) }));
  return hasUnowned.value
    ? [...list, { value: "", label: t("memory.boardUnowned") }]
    : list;
});

/** 分档基准时间随渲染取（列表低频打开，不排定时器）。 */
const buckets = computed(() =>
  bucketSessions(board.projectSessions, Date.now()),
);

function titleOf(s: TreeSummaryView): string {
  return sessionTitleOf(chat.sessions.find((x) => x.id === s.sessionId)?.title, s.sessionId);
}

function isSelected(s: TreeSummaryView): boolean {
  return sessionKeyOf(s) === `${board.workspace}\u0000${board.sessionId}`;
}

async function pick(key: string) {
  await board.selectSession(key);
}

/** 未归属（`''`）没有项目记忆目录：按钮禁用。这里**不写第二道 `if (unowned) return`**
 *  ——本刀先带着它跑过一轮，再删掉它复跑：前端 104 条用例零转红 ⇒ 那道守卫在测试面
 *  不可观察（happy-dom 与真机一样，disabled 的按钮不派发 click），按「变异不掉的就别
 *  留着」删。禁用条件读 store 现值，不在组件里再存一份"选了哪个项目"。 */
const unowned = computed(() => board.workspace === "");

function openMemoryDir() {
  void memoryRevealProject(board.workspace).catch((e) => {
    toast.push("danger", t("toast.revealFailed"), String(e));
  });
}
</script>

<template>
  <!-- 宽度只走内联（`w-[..]` 任意尺寸类是 §二⑯ 明确不授权的事；这里给的是
       现役令牌 var 而不是自造 px） -->
  <aside
    class="flex h-full min-h-0 shrink-0 flex-col gap-2 bg-side px-3 pb-3 pt-2"
    style="width: var(--lw-sidebar-w)"
  >
    <SettingsSelect
      :model-value="board.workspace"
      :entries="projectEntries"
      :label="t('memory.boardProjectLabel')"
      @update:model-value="(v: string) => void board.selectProject(v)"
    />

    <!-- 「打开记忆目录」（MB §3.1 左栏第二件）：与底部出口钮同一种形态——栏内
         幽灵钮，不写 z-index、不加发丝线。禁用态照现役那一条 ghost 钮的写法
         `hover:bg-wash-2 + disabled:opacity-50`（不另造 `enabled:hover:` 那种
         第二形态；判据 D3 钉这一条）。未归属禁用并由 HoverTip 说清为什么。 -->
    <HoverTip
      class="w-full shrink-0"
      :text="unowned ? t('memory.boardOpenDirUnowned') : t('memory.boardOpenDirHint')"
      :label="t('memory.boardOpenDir')"
    >
      <button
        type="button"
        class="flex w-full items-center gap-2 rounded-[var(--lw-r-md)] px-3 py-1.5 text-left text-[length:var(--lw-fs-ui-caption)] text-ink-4 transition-colors hover:bg-wash-2 hover:text-ink-1 disabled:cursor-not-allowed disabled:opacity-50"
        :disabled="unowned"
        @click="openMemoryDir"
      >
        {{ t("memory.boardOpenDir") }}
      </button>
    </HoverTip>

    <p v-if="board.error" class="text-[length:var(--lw-fs-ui-sm)] text-danger">
      {{ board.error }}
    </p>
    <p
      v-else-if="buckets.length === 0"
      class="mt-2 text-center text-[length:var(--lw-fs-ui-sm)] text-ink-4"
    >
      {{ t("memory.boardNoSession") }}
    </p>

    <div v-else class="scroll-y min-h-0 flex-1">
      <section v-for="b in buckets" :key="b.bucket" class="mt-2">
        <p class="px-2 text-[length:var(--lw-fs-ui-xs)] text-ink-5">
          {{ t(bucketKey[b.bucket]) }}
        </p>
        <ul class="mt-1 space-y-1">
          <li v-for="s in b.items" :key="sessionKeyOf(s)">
            <button
              type="button"
              class="flex w-full flex-col gap-1 rounded-[var(--lw-r-md)] px-3 py-2 text-left transition-colors"
              :class="
                isSelected(s)
                  ? 'bg-wash-3'
                  : 'hover:bg-wash-2'
              "
              @click="void pick(sessionKeyOf(s))"
            >
              <span class="w-full truncate text-[length:var(--lw-fs-ui-caption)] text-ink-1">
                {{ titleOf(s) }}
              </span>
              <span class="flex items-center gap-2">
                <span class="text-[length:var(--lw-fs-ui-xs)] text-ink-5">
                  {{
                    t("memory.boardCounts", {
                      summaries: s.summaryCount,
                      chapters: s.chapterCount,
                    })
                  }}
                </span>
                <TreeBadge v-if="s.pending" :text="t('memory.boardOrganizing')" tone="warn" />
              </span>
            </button>
          </li>
        </ul>
      </section>
    </div>

    <button
      type="button"
      class="mt-auto flex shrink-0 items-center gap-2 rounded-[var(--lw-r-md)] px-3 py-1.5 text-[length:var(--lw-fs-ui-caption)] text-ink-4 transition-colors hover:bg-wash-2 hover:text-ink-1"
      @click="shell.setDestination('chat')"
    >
      {{ t("memory.boardBackToChat") }}
    </button>
  </aside>
</template>
