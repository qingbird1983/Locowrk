<script lang="ts">
/** §3.10 子卡数据（ChatTranscript 组块 flush 时收集）。 */
export interface SubagentChild {
  childId: string;
  /** 任务 prompt 截断摘要（子卡标题与转录页签名同源，不发明子名）。 */
  prompt: string;
  status: "running" | "done" | "failed";
  /** 实时轮才有；undefined = 历史重放（不显示时长）。 */
  durationText?: string;
}
</script>

<script setup lang="ts">
/**
 * 子 Agent 卡（UI shell doc §3.10）：subagent_run 在父会话转录里的可视形态
 * ——头行（状态 · N 个 subagent · 已完成 k/N · 时长）+ 小型节点图（主 Agent
 * 卡 — 连线双圆点 — 子卡）。卡是入口不是终点：点子卡开工作面板 §3.16 转录
 * 页签（单子卡时头行按原型挂「查看转录」ghost 钮）。
 *
 * 数据诚实边界：⑮ 子内活动黑盒——步骤数 / 子模型档位父侧不可得，不发明
 * （§3.10 的 `Task(<name>)` / `N 个步骤` / `模型 · 档位` 落成 prompt 截断
 * 摘要 + 工具名 + 状态 + 时长）；历史重放无时间戳，时长不虚构。
 */
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Icon } from "../../lib/icons";
import HoverTip from "../settings/HoverTip.vue";

const props = defineProps<{ children: SubagentChild[] }>();
const emit = defineEmits<{ open: [childId: string, prompt: string] }>();

const { t } = useI18n();

const running = computed(() => props.children.some((c) => c.status === "running"));
const doneCount = computed(
  () => props.children.filter((c) => c.status === "done").length,
);
/** 总时长 = 子卡时长最大值（并行子任务的经历跨度）。 */
const durationText = computed(() => {
  const ds = props.children
    .map((c) => c.durationText)
    .filter((v): v is string => !!v);
  return ds.length > 0 ? ds[ds.length - 1]! : "";
});
</script>

<template>
  <div class="subagent" role="region" :aria-label="t('subagent.region')">
    <div class="subagent__head">
      <Icon name="bot" :size="14" class="shrink-0 text-purple" aria-hidden="true" />
      <b>{{ running ? t("subagent.working") : t("subagent.done") }}</b>
      <span>
        ·
        {{ t("subagent.count", { n: children.length }) }}
        ·
        {{ t("subagent.doneCount", { done: doneCount, n: children.length }) }}
        <template v-if="durationText"> · {{ durationText }}</template>
      </span>
      <button
        v-if="children.length === 1"
        class="head-btn"
        type="button"
        @click="emit('open', children[0]!.childId, children[0]!.prompt)"
      >
        <Icon name="bot" :size="12" aria-hidden="true" />{{
          t("panel.viewTranscript")
        }}
      </button>
    </div>

    <div class="subagent__graph">
      <div class="node">
        <div class="node__title">{{ t("subagent.main") }}</div>
        <div class="node__sub">
          {{ t("subagent.coordinating", { n: children.length }) }}
        </div>
      </div>
      <div class="node-col">
        <template v-for="child in children" :key="child.childId">
          <div class="connector" aria-hidden="true" />
          <HoverTip
            class="w-full"
            :text="t('panel.viewTranscript')"
            :label="t('panel.viewTranscript')"
          >
            <button
              class="node node--child"
              type="button"
              @click="emit('open', child.childId, child.prompt)"
            >
              <div class="node__title">{{ child.prompt || t("subagent.child") }}</div>
              <div class="node__tags">
                <span class="node__tag">subagent_run</span>
              </div>
              <div class="node__foot">
                <span
                  class="dot"
                  :class="{
                    'dot--run': child.status === 'running',
                    'dot--ok': child.status === 'done',
                    'dot--fail': child.status === 'failed',
                  }"
                />
                <span>{{
                  child.status === "running"
                    ? t("tool.running")
                    : child.status === "done"
                      ? t("turn.done")
                      : t("tool.failed")
                }}</span>
                <span v-if="child.durationText">{{ child.durationText }}</span>
                <span class="node__open">{{ t("panel.viewTranscript") }}</span>
              </div>
            </button>
          </HoverTip>
        </template>
      </div>
    </div>
  </div>
</template>

<style scoped>
/* 卡容器（§3.10）：1px line-subtle（浮层语法豁免）+ 圆角 14 + wash-1 */
.subagent {
  margin: var(--lw-gap-6) 0;
  border: 1px solid var(--lw-line-subtle);
  border-radius: var(--lw-r-md-plus);
  background: var(--lw-wash-1);
  padding: var(--lw-gap-12) var(--lw-gap-14);
}
.subagent__head {
  display: flex;
  align-items: center;
  gap: var(--lw-gap-6);
  font-size: var(--lw-fs-12half);
  color: var(--lw-ink-2);
}
.subagent__head b {
  font-weight: 520;
  color: var(--lw-ink-1);
}
.head-btn {
  display: inline-flex;
  align-items: center;
  gap: var(--lw-gap-4);
  margin-left: auto;
  flex: 0 0 auto;
  border-radius: var(--lw-r-sm);
  padding: 3px var(--lw-gap-8); /* stylelint-disable-line declaration-property-value-disallowed-list -- 光学修正(孤值3,§二⑬) */
  font-size: var(--lw-fs-12);
  color: var(--lw-ink-2);
}
.head-btn:hover {
  background: var(--lw-wash-2);
  color: var(--lw-ink-1);
}

/* 节点图：主卡 + 连线（两端小圆）+ 子卡列 */
.subagent__graph {
  display: flex;
  align-items: stretch;
  margin-top: var(--lw-gap-12);
}
.node-col {
  display: flex;
  min-width: 0;
  flex: 1 1 0;
  flex-direction: column;
  gap: var(--lw-gap-6);
}
.node {
  min-width: 0;
  border-radius: var(--lw-r-md);
  background: var(--lw-raised);
  box-shadow: var(--lw-elevation);
  padding: var(--lw-gap-10) var(--lw-gap-12);
  text-align: left;
}
.subagent__graph > .node {
  flex: 0 0 auto;
}
.node--child {
  width: 100%;
}
button.node--child:hover {
  background: var(--lw-wash-2);
}
.node__title {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: var(--lw-fs-12half);
  font-weight: 520;
  line-height: 1.3;
  color: var(--lw-ink-1);
}
.node__sub {
  margin-top: 3px; /* stylelint-disable-line declaration-property-value-disallowed-list -- 光学修正(孤值3,§二⑬) */
  font-size: var(--lw-fs-12);
  color: var(--lw-ink-3);
}
.node__tags {
  display: flex;
  align-items: center;
  gap: var(--lw-gap-6);
  margin-top: var(--lw-gap-4);
}
.node__tag {
  font-family: var(--font-mono);
  font-size: var(--lw-fs-11);
  color: var(--lw-ink-3);
}
.node__foot {
  display: flex;
  align-items: center;
  gap: var(--lw-gap-6);
  margin-top: var(--lw-gap-6);
  font-size: var(--lw-fs-12);
  color: var(--lw-ink-3);
}
.node__open {
  margin-left: auto;
  flex: 0 0 auto;
}
.node__foot .node__open {
  opacity: 0;
  transition: opacity var(--dur-fast) var(--ease-out);
}
button.node--child:hover .node__open,
button.node--child:focus-visible .node__open {
  opacity: 1;
}

/* 连接线：两端各一个小圆点（原型 .connector） */
.connector {
  flex: 0 0 auto;
  align-self: center;
  width: 28px;
  height: 1px;
  background: var(--lw-line-strong);
  position: relative;
}
.connector::before,
.connector::after {
  content: "";
  position: absolute;
  top: -2px;
  width: 5px;
  height: 5px;
  border-radius: var(--lw-r-full);
  background: var(--lw-line-strong);
}
.connector::before {
  left: -1px;
}
.connector::after {
  right: -1px;
}

/* 状态点（§6.6 ⑧ 脉动 = 唯一常驻动画） */
.dot {
  width: 6px;
  height: 6px;
  flex: 0 0 auto;
  border-radius: var(--lw-r-full);
}
.dot--run {
  background: var(--lw-info);
  animation: lw-pulse 1s ease-in-out infinite;
}
.dot--ok {
  background: var(--lw-ok);
}
.dot--fail {
  background: var(--lw-danger);
}
</style>
