<script setup lang="ts">
/**
 * 记忆树看板（队列 Q14 第三刀 · MB《Memory Tree Board Design v1》§3）。
 *
 * 形态 = 「深色左栏到顶 + 浅色画布到顶」双栏（§3.1）：**没有**上栏标题带、没有
 * 分色条、没有发丝线分隔——这一页不是「顶栏 + 内容」骨架。画布只读（§3.2 只读
 * 裁定）：点击语义只有展开 / 收起 / 定位三种，编辑面留在「事实库」档的
 * TreeSection.vue（页签怎么分档见 `MemoryPage.vue` 的 tab 注释）。
 *
 * 尺寸与纪律的唯一出处 = `lib/board-model.ts`（列基线、定宽、缩放档、逐层展开
 * 判定、几何），本文件与画布都不重抄这些数——`§二⑯` 那句「节点定宽的唯一理由 =
 * 连线端点几何可预测」要求的就是这件事。
 */
import { onMounted } from "vue";
import BoardCanvas from "./board/BoardCanvas.vue";
import BoardRail from "./board/BoardRail.vue";
import { sessionKeyOf, useTreeBoardStore } from "../../stores/tree-board";
import { useChatStore } from "../../stores/chat";
import { useSettingsStore } from "../../stores/settings";

const board = useTreeBoardStore();
const chat = useChatStore();
const settings = useSettingsStore();

/**
 * 进场装配（顺序是有理由的）：先拉投影清单，再定「缺省项目」——缺省 = 当前会话
 * 所属项目（有树的话直接落到那条会话），否则第一个**有树**的项目。
 * 拿当前会话的项目而不是 fsRoots[0]，是因为用户从对话里点进来时想看的就是手上这
 * 条会话的整理结果（§3.1「默认选中当前会话所属项目」）。
 *
 * ⚠ `sessions !== null` 那道闸是给"切页签回来"用的：数据与选择态都在 store 里，
 * 组件重挂不该把用户刚选的那条会话冲掉——只有**第一次**进场才做缺省装配。
 */
onMounted(async () => {
  if (board.sessions !== null) return;
  const list = await board.openSessions();
  if (list.length === 0) return;
  const here = list.find((s) => s.sessionId === chat.sessionId);
  if (here) {
    await board.selectSession(sessionKeyOf(here));
    return;
  }
  const roots = settings.view?.tools.fsRoots ?? [];
  const owned = roots.find((ws) => list.some((s) => s.workspace === ws));
  const fallback = owned ?? list[0]?.workspace ?? "";
  await board.selectProject(fallback);
});
</script>

<template>
  <!-- 目的地壳给的是**定高块**（不是 flex 容器），所以这里必须带 `h-full`：
       画布要有确定高度，`flex-1` 自己算不出来（量不到内容 = 连线与包围盒全塌）。 -->
  <div class="flex h-full min-h-0 flex-1 overflow-hidden bg-bg">
    <BoardRail />
    <BoardCanvas />
  </div>
</template>
