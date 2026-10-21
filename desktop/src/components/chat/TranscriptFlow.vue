<script setup lang="ts">
/**
 * 消息流渲染核心（v2.104 用户裁定：子代理工作过程的分层折叠消息流 =
 * 主对话整个流程原样复用）：从 ChatTranscript 提取的共享渲染体——连续
 * 过程项合成动作块（§3.7 v4.11，决策记录行不渲染）、任务容器（v2.99
 * 「第 0 层」：计时行常驻头部 + 开合覆盖，收束收缩为计时行）、组相位机
 * （L3 live / L2 replied / L1 done）、复制钮判据（只在用户消息与最终
 * 汇报之下）、待决审批索引。父层提供滚动容器：主对话 = ChatTranscript
 * （跟随滚动/回到底部/列宽柄/编辑重发），子代理转录页签 =
 * SubagentTranscriptTab（只读快照）——行为差异全部走 props/emits：
 * - running：最后一轮执行中态（相位机 live 判据）；页签快照不传 =
 *   false，全落 done，与主对话历史重放同形；
 * - stopRequested：「已停止」只标最新过程组；页签恒 false；
 * - editableIds：可编辑用户气泡集合（§3.34）；页签只读不传；
 * - noticeAction：系统条动作上抛（撤销压缩/查看详情等）；页签不接。
 */
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import type { ChatItem, SystemItem, ToolItem } from "../../stores/chat";
import type { TurnState } from "../../types";
import { useSettingsStore } from "../../stores/settings";
import { isCompactItem } from "../../lib/chat/compact-line";
import { Icon } from "../../lib/icons";
import MessageRow from "./MessageRow.vue";
import SystemRow from "./SystemRow.vue";
import ThinkingRow from "./ThinkingRow.vue";
import ToolRow from "./ToolRow.vue";
import TurnProcess from "./TurnProcess.vue";
import TurnTimerLine from "./TurnTimerLine.vue";
import UncertainCard from "./UncertainCard.vue";

const props = defineProps<{
  /** 待渲染条目（主对话 = chat.items；页签 = subagent_transcript 投影）。 */
  items: ChatItem[];
  /** 最后一轮执行中（缺省 false = 历史重放同形）。 */
  running?: boolean;
  /** 「已停止」标记来源（主对话 stopRequested）。 */
  stopRequested?: boolean;
  /** 可编辑用户气泡 id 集（§3.34；缺省 = 全只读）。 */
  editableIds?: Set<number>;
}>();

const emit = defineEmits<{
  edit: [pos: number | undefined, text: string];
  openTranscript: [childId: string, title: string];
  openDiff: [callId: string, path: string, diff: string];
  noticeAction: [item: SystemItem, action: string];
}>();

const { t } = useI18n();
const settings = useSettingsStore();

/** 过程项（§3.7 v4.11 组内都是扁平行）：工具行 / 思考行 / 错误行。
 * 决策记录行 v2.98 全隐藏，不入组。 */
type ProcessItem = Extract<ChatItem, { kind: "tool" | "thinking" | "error" }>;
type TurnBlock = {
  kind: "turn";
  key: number;
  items: ProcessItem[];
  /** 失败的工具调用数（轮头「N 个问题」标记的来源）。 */
  failures: number;
  /** 组内有真过程（工具/思考）= 套组壳；纯错误行平铺不套壳。 */
  hasProcess: boolean;
  startTs?: number;
};
type Block =
  | TurnBlock
  | { kind: "single"; key: number; item: Exclude<ChatItem, ProcessItem> };

/** 连续过程项合成一组；其余条目各自为兄弟节点。 */
const blocks = computed<Block[]>(() => {
  const out: Block[] = [];
  let cur: ProcessItem[] = [];
  const flush = () => {
    if (cur.length === 0) return;
    const tools = cur.filter((i): i is ToolItem => i.kind === "tool");
    // 计时起点 = 组内最早的过程项时刻（工具与思考都算；错误行无时戳）
    const tsList = cur
      .map((i) => ("ts" in i ? i.ts : undefined))
      .filter((v): v is number => v !== undefined);
    // AF-14 终版（用户 2026-10-19 裁定「保留 ✓ + 时长那层，删掉组头那层」）：组头
    // 整层已删 ⇒ 原先喂给组头摘要的 `names`（动作名串联）与 `toolCount`（· N 个工具）
    // 一并作废——同一件事只写一次，轮内动作行自己看得见有几件工具。
    out.push({
      kind: "turn",
      key: cur[0]!.id,
      items: cur,
      failures: tools.filter((i) => i.ok === false).length,
      hasProcess: cur.some((i) => i.kind === "tool" || i.kind === "thinking"),
      // 组内最早的过程项时刻（工具与思考都算——计时从思考开始）
      startTs: tsList.length > 0 ? Math.min(...tsList) : undefined,
    });
    cur = [];
  };
  for (const item of props.items) {
    // v2.98 裁定：审批决策记录行（「已允许 ×」每步反馈）全隐藏
    if (item.kind === "decision") continue;
    if (
      item.kind === "tool" ||
      item.kind === "thinking" ||
      item.kind === "error"
    ) {
      cur.push(item);
    } else {
      flush();
      out.push({ kind: "single", key: item.id, item });
    }
  }
  flush();
  return out;
});

/** 用户条目的事件位（§3.34 编辑重发定位键；模板窄化集中收窄）。 */
function userPos(item: ChatItem): number | undefined {
  return item.kind === "user" ? item.pos : undefined;
}

/** 「已停止」只标最新过程组（stopRequested 轮次收束后保持到下次发送）。 */
const lastProcessKey = computed(() => {
  for (let i = blocks.value.length - 1; i >= 0; i--) {
    const b = blocks.value[i]!;
    if (b.kind === "turn" && b.hasProcess) return b.key;
  }
  return null;
});
/** AF-14 终版（用户 2026-10-19 裁定「保留 ✓ + 时长那层，删掉组头这层」）：原先挂在
 * 组头上的两个标记随组头一起下沉到轮头（容器计时行）——判据不变，落点从「该轮最新
 * 过程组」上移到那一轮的容器行。 */
function turnStopped(turn: TurnView): boolean {
  const isNewest = turn.inner.some(
    (b) => b.kind === "turn" && b.hasProcess && b.key === lastProcessKey.value,
  );
  if (!isNewest) return false;
  // 「已停止」标记（v2.205 起有后端权威态）：该轮的 `turn_header` 读数在场就按它判
  // （`interrupted` = 日志里真记着这件事），读不出（旧日志 / 旧后端）才回落到前端
  // 自算的 `stopRequested`——它不进会话日志、切会话即丢。两个来源不是并列的两把尺：
  // **权威态在场它说话**，回落只是如实降级。
  return turn.turnState ? turn.turnState === "interrupted" : !!props.stopRequested;
}

/** 轮头「N 个问题」：轮内各块失败工具数之和（原为逐块组头各显一份，现在一行汇总）。 */
function turnProblems(turn: TurnView): number {
  return turn.inner.reduce(
    (n, b) => n + (b.kind === "turn" ? b.failures : 0),
    0,
  );
}

// ---- 任务容器（v2.99「第 0 层」用户裁定）----
/** 每条用户消息开启一轮：容器内 = 轮计时行 + 该轮全部动作块与简短回复
 * （执行中恒展开）；容器外尾块 = 轮末尾连续的非过程块（最终报告 + 其后
 * 系统条目）——容器收缩后仍可见、最终报告有复制钮。调度注入轮没有实时
 * 用户消息 → 过程块自成一轮（计时行无读数，只剩发丝线，不虚构起点）。 */
type TurnView = {
  /** 容器身份 = 用户消息条目 id（调度轮 = 首个过程组键）。 */
  key: number;
  userItem?: Extract<ChatItem, { kind: "user" }>;
  /** 轮起点 = 用户消息实时时刻（计时行读数起点；undefined = 无从计时）。 */
  startTs?: number;
  /** 轮收束时刻（v2.101，turn_done 轮界事件投影——重放读数终点）。 */
  endTs?: number;
  /** 本轮收束状态（v2.205，`turn_header` 挂在该轮 user 行上）：undefined =
   * 旧日志读不出 ⇒ 「已停止」标记回落到前端 `stopRequested`。 */
  turnState?: TurnState;
  /** 本轮真正干活的毫秒数（v2.205，墙钟区间 − Σ审批等待）：undefined = 读不出
   * ⇒ 计时行回落墙钟差（不当 0）。 */
  activeMs?: number;
  running: boolean;
  inner: Block[];
  /** 压缩线（发丝线族）：**摘到轮容器外**渲染 —— 顶层、永不折叠。
   *
   * 用户 2026-10-22 裁定：「这个压缩内容需要在最顶层。不受那个L0折叠层管理。
   * 属于永不折叠状态。跟模型最后汇报总结消息在一个层级的。」 ⇒ 轮容器（第 0
   * 层，v2.99）收起时它仍在，展开体也跟着在。
   *
   * 为什么只摘**内层**那几根：尾段（`tail`）本来就在容器外（与最终报告
   * 同级），按原位留着即可；内层那几根（自动压缩发生在轮中途时）原本在
   * 容器里，会被 L0 折叠管住 ⇒ 摘出来。 */
  compaction: Extract<Block, { kind: "single" }>[];
  /** 容器外尾块（构造保证只剩 single——最终报告 + 其后系统条目）。 */
  tail: Extract<Block, { kind: "single" }>[];
};
const turnViews = computed<TurnView[]>(() => {
  const views: TurnView[] = [];
  let cur: {
    key: number;
    userItem?: Extract<ChatItem, { kind: "user" }>;
    blocks: Block[];
  } | null = null;
  const close = () => {
    if (!cur) return;
    // 尾块 = 末尾连续的非过程块；过程块一旦再现（调度轮续作）即归还容器
    let i = cur.blocks.length;
    while (i > 0 && cur.blocks[i - 1]!.kind === "single") i--;
    // 内层里的压缩线摘出来（顶层渲染）：判据 = 那一条是不是压缩线
    // （`lib/chat/compact-line.ts` 的唯一判据，不在这里另写一份）。
    const inner: Block[] = [];
    const compaction: Extract<Block, { kind: "single" }>[] = [];
    for (const b of cur.blocks.slice(0, i)) {
      if (b.kind === "single" && b.item.kind === "system" && isCompactItem(b.item)) {
        compaction.push(b);
      } else {
        inner.push(b);
      }
    }
    views.push({
      key: cur.key,
      userItem: cur.userItem,
      startTs: cur.userItem?.ts,
      endTs: cur.userItem?.endTs,
      turnState: cur.userItem?.turnState,
      activeMs: cur.userItem?.activeMs,
      running: false, // 最后一轮在收尾处统一回填
      inner,
      compaction,
      tail: cur.blocks.slice(i) as Extract<Block, { kind: "single" }>[],
    });
    cur = null;
  };
  for (const block of blocks.value) {
    if (block.kind === "single" && block.item.kind === "user") {
      close();
      cur = { key: block.key, userItem: block.item, blocks: [] };
    } else {
      if (!cur) cur = { key: block.key, blocks: [] };
      cur.blocks.push(block);
    }
  }
  close();
  if (views.length > 0) {
    views[views.length - 1]!.running = props.running ?? false;
  }
  return views;
});

/** 容器开合覆盖：手动点击优先。缺省 = 执行中恒开；收束后按「折叠工具
 * 执行过程」开关（v4.20 实机走查裁定补线：容器是第 0 层，不接开关则
 * 关档下轮次照样收缩成计时行，开关形同虚设）——关（缺省）= 容器保持
 * 展开整个轮次直接可见、唯一折叠的是工具明细；开 = 收缩为计时行（点击
 * 重开）。开关即时生效：已挂容器随保存重渲染。 */
const turnBoxOverride = ref(new Map<number, boolean>());
function turnBoxDefault(running: boolean): boolean {
  return running || !settings.view?.chat.collapseToolProcess;
}
function turnBoxOpen(key: number, running: boolean): boolean {
  return turnBoxOverride.value.get(key) ?? turnBoxDefault(running);
}
function toggleTurnBox(key: number, running: boolean) {
  const m = new Map(turnBoxOverride.value);
  m.set(key, !turnBoxOpen(key, running));
  turnBoxOverride.value = m;
}

/** 组相位（v2.99 层层折叠）与「计时行让位」集合：AF-14 终版（用户 2026-10-19 裁定
 * 「保留 ✓ + 时长那层，删掉组头这层」）之后一起作废——组头整层已删，一轮只有一个
 * 头行（容器计时行），轮内过程块一律平铺在它下面，没有 live/replied/done 三种形态
 * 可分。 */

/** 复制钮收敛（用户裁定）：只出现在用户消息与每轮最终汇报/总结之下——
 * 判据 = 该助手条目到下一个用户消息之间再无任何过程项（工具/思考/错误；
 * 决策记录行已隐藏不参与判据）；任务执行中间的简短回话后面跟着动作，
 * 不给复制钮。 */
const finalAssistantIds = computed(() => {
  const ids = new Set<number>();
  const items = props.items;
  for (let i = 0; i < items.length; i++) {
    const it = items[i]!;
    if (it.kind !== "assistant") continue;
    let followed = false;
    for (let j = i + 1; j < items.length; j++) {
      const n = items[j]!;
      if (n.kind === "user") break;
      if (n.kind === "tool" || n.kind === "thinking" || n.kind === "error") {
        followed = true;
        break;
      }
    }
    if (!followed) ids.add(it.id);
  }
  return ids;
});

// 「等待审批」的读数改由 agent://tool-status 事件写进行状态（队列 Q12·C1）。
// 这里原先是拿 approvals.pending 反查 call_id——同一件事两个出处，且
// 重放时待决表恒空，历史行永远显不出它当时等过审批（现已一并如实退役：
// 重放本来就不该有等待态，见 events.rs::channels::TOOL_STATUS 的注释）。

/** 系统消息文本：__raw__ 条渲染原文，其余按 i18n key 翻译。 */
function noticeText(item: SystemItem): string {
  if (item.key === "__raw__") return String(item.params?.text ?? "");
  return t(item.key, item.params ?? {});
}

/** 系统条的动作钮（文案 = i18n key `notice.action.<id>`）。
 *
 * 「查看详情」那一枚在**展开态**改说「收起」——同一个钮切换两态（口径见
 * §二⑲：入口唯一 = 线上那一个动作钮，不另加一枚「收起」钮）。 */
function systemActions(item: SystemItem) {
  return (item.actions ?? []).map((a) => ({
    id: a,
    label:
      a === "details" && isCompactItem(item) && item.expanded
        ? t("notice.action.collapse")
        : t(`notice.action.${a}`),
  }));
}

/** 系统条那一行的全部渲染参数（三个落点共用这一份 ⇒ 改一处即改三处）。 */
function systemProps(item: SystemItem) {
  return {
    text: noticeText(item),
    hairline: item.tone === "hairline",
    actions: systemActions(item),
    body: item.body,
    expanded: item.expanded,
  };
}
</script>

<template>
  <template v-for="turn of turnViews" :key="turn.key">
    <!-- 用户消息（容器外，§3.34 可编辑由父层给集；复制钮常态有）。
         parts（批 D）：附件缩略图随重放与实时同形恢复 -->
    <MessageRow
      v-if="turn.userItem"
      kind="user"
      :text="turn.userItem.text"
      :ts="turn.userItem.ts"
      :parts="turn.userItem.parts"
      :editable="editableIds?.has(turn.userItem.id) ?? false"
      @edit="(t2: string) => { if (turn.userItem) emit('edit', userPos(turn.userItem), t2); }"
    />

    <!-- 任务容器（第 0 层，v2.99 裁定）：计时行常驻头部（工作中/已
         工作 + 发丝线），执行中内容全开；收束收缩为计时行，点击重开
         逐层看全程。最终报告与其后系统条目在容器外尾段 -->
    <div class="turn-box">
      <!-- AF-14 终版（用户 2026-10-19 裁定「保留 ✓ + 时长那层，删掉组头这层」）：本行
           是一轮**唯一**的头行——组头整层已删，「已停止」「N 个问题」两个标记随之
           落到本行；点击折叠 / 展开整个轮容器。 -->
      <TurnTimerLine
        :start-ts="turn.startTs"
        :end-ts="turn.endTs"
        :active-ms="turn.activeMs"
        :running="turn.running"
        :stopped="turnStopped(turn)"
        :problems="turnProblems(turn)"
        :aria-expanded="turnBoxOpen(turn.key, turn.running)"
        @click="toggleTurnBox(turn.key, turn.running)"
      />
      <template v-if="turnBoxOpen(turn.key, turn.running)">
        <template v-for="block in turn.inner" :key="block.key">
          <!-- 动作块（AF-14 终版）：组头整层已删，块内过程行平铺在轮头下面 -->
          <template v-if="block.kind === 'turn'">
            <TurnProcess v-if="block.hasProcess">
              <template v-for="item in block.items" :key="item.id">
                <ThinkingRow
                  v-if="item.kind === 'thinking'"
                  :text="item.text"
                  :ts="item.ts"
                  :done-ts="item.doneTs"
                />
                <ToolRow
                  v-else-if="item.kind === 'tool'"
                  :item="item"
                  @open-transcript="(id, title) => emit('openTranscript', id, title)"
                  @open-diff="(callId, path, diff) => emit('openDiff', callId, path, diff)"
                />
                <!-- 错误行入组平铺（裁定①：失败通知去卡片底） -->
                <div v-else class="flow-error-row">
                  <Icon name="warn" :size="12" class="shrink-0" />
                  <span>{{ item.text }}</span>
                </div>
              </template>
              <!-- v4.13：SubagentCard 大卡片退役——subagent_run 就是组内一列
                   工具行（§3.10 紧凑行），入口在行明细「查看完整对话」 -->
            </TurnProcess>
            <template v-else>
              <template v-for="item in block.items" :key="item.id">
                <div v-if="item.kind === 'error'" class="error-row">
                  <Icon name="warn" :size="14" class="shrink-0" />
                  <span>{{ item.text }}</span>
                </div>
              </template>
            </template>
          </template>

          <!-- 模型简短回复（容器内，无复制钮——复制钮只随最终报告）；
               模型正文不参与长消息折叠（v2.101 裁定，按 kind 判定） -->
          <MessageRow
            v-else-if="block.item.kind === 'assistant'"
            kind="assistant"
            :text="block.item.text"
            :ts="block.item.ts"
            :copyable="finalAssistantIds.has(block.item.id)"
          />
          <!-- 会话内系统消息（SystemNotice 模式）：组外兄弟节点 -->
          <SystemRow
            v-else-if="block.item.kind === 'system'"
            v-bind="systemProps(block.item as SystemItem)"
            @action="(a: string) => emit('noticeAction', block.item as SystemItem, a)"
          />
          <!-- G2 崩溃恢复 uncertain 决策卡：组外兄弟（折进组会被默认折叠
               藏住，决策面必须可见）；resolved 后收为记录行 -->
          <div v-else-if="block.item.kind === 'uncertain'" class="unc-row">
            <UncertainCard :item="block.item" />
          </div>
        </template>
      </template>
    </div>

    <!-- 压缩线（发丝线族）：**轮容器外**渲染 —— 顶层、永不折叠，与最终报告
         同级（用户 2026-10-22 裁定：不受 L0 折叠层管理）。 -->
    <SystemRow
      v-for="block in turn.compaction"
      :key="block.key"
      v-bind="systemProps(block.item as SystemItem)"
      @action="(a: string) => emit('noticeAction', block.item as SystemItem, a)"
    />

    <!-- 容器外尾块：最终报告（复制钮只在此）与其后系统条目 -->
    <template v-for="block in turn.tail" :key="block.key">
      <MessageRow
        v-if="block.item.kind === 'assistant'"
        kind="assistant"
        :text="block.item.text"
        :ts="block.item.ts"
        :copyable="finalAssistantIds.has(block.item.id)"
      />
      <SystemRow
        v-else-if="block.item.kind === 'system'"
        v-bind="systemProps(block.item as SystemItem)"
        @action="(a: string) => emit('noticeAction', block.item as SystemItem, a)"
      />
      <!-- G2：崩溃会话没有最终报告，uncertain 卡常落尾段（同组外语义） -->
      <div v-else-if="block.item.kind === 'uncertain'" class="unc-row">
        <UncertainCard :item="block.item" />
      </div>
    </template>
  </template>
</template>

<style scoped>
/* 任务容器（v2.99「第 0 层」）：计时行常驻头部 + 动作块/简短回复；收束
 * 收缩为计时行，最终报告在容器外——容器↔报告间距归兄弟规则（v4.15 用户
 * 裁定 12px：全 8px 分不出最终报告起头；旧 16px 容器底 padding + 报告自
 * 带 12px = 28px 退役） */
.turn-box {
  padding-bottom: 0;
}
/* 间距（v4.14 用户裁定：三层间距统一 8px——动作块之间、动作块↔过渡文本、
 * 过渡文本→下一动作块全部同值；旧 8/8/12 三值退役。跨组件兄弟选择器
 * （组件根都带本层 data-v）；.turn 垂直 padding 已归零，间距全由这
 * 里给。计时行属容器 chrome（第 0 层），不在三层之列，保持 2px）。
 * 简短回复 .msg--brief 自身 padding 归零（2026-10-07：隐藏时间行占位 +
 * 自带 12px padding 与此处 8px 双重计算，实际间距曾达 24-30px）——相邻
 * 对在这里补全：简短↔简短、简短/动作块↔系统条目双向。 */
.turn + .msg {
  padding-top: var(--lw-gap-8);
}
.msg + .turn {
  padding-top: var(--lw-gap-8);
}
.turn + .turn {
  padding-top: var(--lw-gap-8);
}
.msg + .msg,
.msg + .sys-row,
.sys-row + .msg,
.sys-row + .sys-row,
.turn + .sys-row,
.sys-row + .turn,
.msg + .unc-row,
.unc-row + .msg,
.turn + .unc-row,
.unc-row + .turn,
.sys-row + .unc-row,
.unc-row + .sys-row,
.unc-row + .unc-row {
  padding-top: var(--lw-gap-8);
}
.turn-timer + .turn,
.turn-timer + .msg {
  padding-top: var(--lw-gap-2);
}
.turn-timer + .sys-row {
  padding-top: var(--lw-gap-2);
}
/* 容器（第 0 层）↔ 容器外尾块（最终报告 / 其后系统条目）= 12px（v4.15
 * 用户裁定：节奏整体 8px，唯此处加大一档让最终报告的起头可分辨）；下一
 * 轮用户消息紧跟容器时同规 */
.turn-box + .msg,
.turn-box + .sys-row {
  padding-top: var(--lw-gap-12);
}
.error-row {
  display: flex;
  align-items: flex-start;
  gap: 7px; /* stylelint-disable-line declaration-property-value-disallowed-list -- 光学修正(孤值7,§二⑬) */
  padding: var(--lw-gap-2) 0;
  font-size: var(--lw-fs-12);
  line-height: 1.5;
  color: var(--lw-danger);
}
/* v4.13 与组内平铺错误行同形（去卡片底——红线「不要卡片」）；与栅格行同一左缘 */
.error-row span {
  min-width: 0;
  overflow-wrap: anywhere;
}
.error-row > svg {
  color: var(--lw-danger);
}
/* §3.7 组内平铺错误行：无卡片底，扁平左对齐（与工具行/思考行同一左缘）；
 * 决策记录行已全隐藏（v2.98 裁定），其平铺样式随之退役 */
.flow-error-row {
  display: flex;
  align-items: flex-start;
  gap: 7px; /* stylelint-disable-line declaration-property-value-disallowed-list -- 光学修正(孤值7,§二⑬) */
  padding: var(--lw-gap-2) 0;
  font-size: var(--lw-fs-12);
  line-height: 1.5;
  color: var(--lw-danger);
}
.flow-error-row span {
  min-width: 0;
  overflow-wrap: anywhere;
}
.flow-error-row > svg {
  color: var(--lw-danger);
}
</style>
