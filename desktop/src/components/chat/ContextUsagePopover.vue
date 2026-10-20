<script setup lang="ts">
/**
 * 上下文用量浮层（UI shell doc §3.12）：锚在用量环按钮上方、**按环几何
 * 居中**（视口钳 8px；v2.90 用户裁定「按上下文圆环居中上滑出」），
 * 360px elevated popover。分项读数照参考对象——v2.58 数据源落地：
 * 「本轮合计 / 生成速度 / 模型用量（输入/输出/缓存读取/命中率/推理/写入）」
 * 来自 agent://usage 按轮聚合（chat.turnUsage，供应商不报告 = 行整体不
 * 渲染）；「工具上下文」= 本轮调用种数/次数（chat items 自带）+ 暴露
 * 目录估算（context-usage 载荷 toolsEstTokens，0 = 目录缺席不渲染）。
 * headline 口径跟设置 usageReadout（已用 / 剩余）。数字一律过 §3.5。
 */
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { compactNumber } from "../../lib/format";
import { memoryTreeView } from "../../lib/api";
import { useChatStore, type ToolItem } from "../../stores/chat";
import { useSettingsStore } from "../../stores/settings";
import type { TreeDebug } from "../../types";

const props = defineProps<{
  usage: {
    used: number;
    threshold: number;
    windowTokens: number;
    toolsEstTokens: number;
    sliderTokens: number;
    outputReserveTokens: number;
    outputReserveSource: string;
  };
  readout: string;
}>();
defineEmits<{ close: [] }>();

const { t } = useI18n();
const chat = useChatStore();
const settings = useSettingsStore();

// 位置（v2.90 用户裁定「按上下文圆环居中上滑出」）：量实际尺寸后按锚环
// 几何中心落 left（视口钳 8px）——不用 transform:translateX(-50%)，会和
// lw-pop-up 的 transform 过渡互踩；CSS 的 right:8px 只是无量测时的兜底
const el = ref<HTMLElement | null>(null);
const leftPx = ref<number | null>(null);

const pct = computed(() =>
  props.usage.windowTokens > 0
    ? Math.round((props.usage.used / props.usage.windowTokens) * 100)
    : null,
);

const headline = computed(() => {
  if (props.readout === "remaining" && props.usage.windowTokens > 0) {
    const remain = Math.max(0, props.usage.windowTokens - props.usage.used);
    return t("usage.headlineRemaining", { n: compactNumber(remain) });
  }
  return t("usage.headlineUsed", { n: compactNumber(props.usage.used) });
});

const windowText = computed(() => {
  if (props.usage.windowTokens <= 0) {
    return `${compactNumber(props.usage.used)} tokens · ${t("usage.noWindow")}`;
  }
  return `${compactNumber(props.usage.used)} / ${compactNumber(props.usage.windowTokens)} tokens`;
});

/** 预留来源文案（S3 刀④b）：后端标记 → 中英说明。空串 = 旧快照，不渲染。 */
function reserveSourceText(source: string, n: number): string {
  switch (source) {
    case "model":
      return t("usage.reserveFromModel", { n: compactNumber(n) });
    case "capped":
      return t("usage.reserveCapped", { n: compactNumber(n) });
    case "default":
      return t("usage.reserveDefault", { n: compactNumber(n) });
    default:
      return "";
  }
}

/** 扣减链的档位说明：窗口未知 / 滑杆说话 / 窗口空位封顶（AC-S3-8 逐态）。 */
function chainModeText(
  mode: "noWindow" | "sliderWins" | "roomWins",
  slider: number,
  room: number,
): string {
  if (mode === "noWindow") {
    return t("usage.chainNoWindow", { slider: compactNumber(slider) });
  }
  if (mode === "roomWins") {
    return t("usage.chainRoomWins", {
      slider: compactNumber(slider),
      room: compactNumber(room),
    });
  }
  return t("usage.chainSliderWins", {
    slider: compactNumber(slider),
    room: compactNumber(room),
  });
}

/** S3 刀④b · AC-S3-8 扣减链：窗口 → −输出侧预留 → 与滑杆取小 = 有效阈值。
 * `sliderTokens <= 0` = 快照早于该字段（旧日志）⇒ 整块不渲染——前端不按
 * 当前设置替历史轮次现算一条链（那是假读数）。阈值本身必须用事件里那颗
 * （后端算好的结果），这里只判"哪一边说话"，不重算 min。 */
const chain = computed(() => {
  const u = props.usage;
  if (u.sliderTokens <= 0) return null;
  const hasWindow = u.windowTokens > 0;
  const mode = !hasWindow
    ? "noWindow"
    : u.threshold < u.sliderTokens
      ? "roomWins"
      : "sliderWins";
  const room = hasWindow ? Math.max(0, u.windowTokens - u.outputReserveTokens) : 0;
  return {
    hasWindow,
    reserve: u.outputReserveTokens,
    threshold: u.threshold,
    headroom: hasWindow ? Math.max(0, u.windowTokens - u.threshold) : 0,
    sourceText: reserveSourceText(u.outputReserveSource, u.outputReserveTokens),
    modeText: chainModeText(mode, u.sliderTokens, room),
  };
});

// M3 §3.37②「未整理尾巴」行：浮层打开时拉一次树投影（打开拉取不轮询，
// ⑫ 同款）；树存在且章节数 > 0 才渲染——影子期大多数会话没有树，不为零
// 状态造行；窗口未知（0）省略 %（增量档本就不依赖窗口，诚实呈现）。
const tree = ref<TreeDebug | null>(null);
onMounted(() => {
  // 居中量测（同步量 offsetWidth 即触发布局，mounted 时已在 DOM）
  const p = el.value;
  const host = p?.offsetParent as HTMLElement | null;
  if (p && host) {
    const hostRect = host.getBoundingClientRect();
    const w = p.offsetWidth;
    const center = hostRect.left + hostRect.width / 2;
    const vpLeft = Math.max(8, Math.min(center - w / 2, window.innerWidth - w - 8));
    leftPx.value = vpLeft - hostRect.left;
  }
  const sid = chat.sessionId;
  if (!sid) return;
  memoryTreeView(sid)
    .then((t) => {
      if (chat.sessionId === sid) tree.value = t;
    })
    .catch(() => {});
});

const treeText = computed(() => {
  const info = tree.value;
  if (!info || !info.exists || info.chapterCount === 0) return null;
  const tail = t("usage.treeTail", { n: compactNumber(info.tailTokensEst) });
  // 尾巴 % 分母 = 会话生效窗口（v2.137 四层链；树投影不再带窗口）
  const pct =
    props.usage.windowTokens > 0
      ? ` ${Math.min(100, Math.round((info.tailTokensEst / props.usage.windowTokens) * 100))}%`
      : "";
  return { chapters: info.chapterCount, text: tail + pct };
});

/** 本轮合计：优先供应商报告 total（0 = 未报告回退输入+输出+写入）。 */
const turnTotal = computed(() => {
  const u = chat.turnUsage;
  if (!u) return null;
  return u.total > 0 ? u.total : u.input + u.output + u.cacheWrite;
});

/** 缓存命中率 = cache_read / 输入（OpenAI 兼容口径 cached ⊆ prompt）。 */
const cacheHitRate = computed(() => {
  const u = chat.turnUsage;
  if (!u || u.input <= 0 || u.cacheRead <= 0) return null;
  return Math.round((u.cacheRead / u.input) * 100);
});

/** 本轮工具上下文：种数 / 次数（chat items）+ 目录估算（后端载荷）。 */
const toolsText = computed(() => {
  const calls = chat.items.filter(
    (i): i is ToolItem => i.kind === "tool",
  );
  if (calls.length === 0) return null;
  const kinds = new Set(calls.map((c) => c.tool)).size;
  const est = props.usage.toolsEstTokens;
  return (
    t("usage.toolsKindsCalls", { kinds, calls: calls.length }) +
    (est > 0 ? ` · ${t("usage.toolsEst", { n: compactNumber(est) })}` : "")
  );
});

/** M4 §3.38⑥ 召回质量埋点：本会话 memory_expand 调用次数（前端从转录
 * 工具行统计，> 0 才渲染——该找到的信息模型有没有去展开，实验判据④）。 */
const expandCalls = computed(() =>
  chat.items.filter(
    (i): i is ToolItem => i.kind === "tool" && i.tool === "memory_expand",
  ).length,
);
</script>

<template>
  <!-- 进退场由挂载点 Transition 接管（标准 §二①）；left 由挂载量测写入
       （锚环几何中心 + 视口钳制），right:8px 为量测前兜底 -->
  <div
    ref="el"
    class="ctx-popover"
    role="dialog"
    :aria-label="t('usage.window')"
    :style="leftPx !== null ? { left: `${leftPx}px`, right: 'auto' } : undefined"
  >
    <div class="ctx-popover__head">
      <span>{{ headline }}</span>
      <span v-if="pct !== null" class="kv__v">{{ pct }}%</span>
    </div>
    <div class="kv">
      <span>{{ t("usage.window") }}</span>
      <span class="kv__v">{{ windowText }}</span>
      <span v-if="pct !== null" class="kv__v">{{ pct }}%</span>
    </div>

    <!-- 扣减链（S3 刀④b · AC-S3-8）：窗口 → −输出侧预留 → 与滑杆取小 =
         有效阈值 → 已用 / 有效阈值；外加"窗口 − 阈值"那颗空位（净增超过它
         就撞窗 = ⓟ 说的那条关系）。chain = null（旧快照）⇒ 整块不渲染，
         绝不按当前设置替历史轮次现算。 -->
    <template v-if="chain">
      <div v-if="chain.hasWindow" class="kv kv--stack">
        <span>{{ t("usage.outputReserve") }}</span>
        <span class="kv__v">−{{ compactNumber(chain.reserve) }} tokens</span>
        <span v-if="chain.sourceText" class="chain__note">{{ chain.sourceText }}</span>
      </div>
      <div class="kv kv--stack">
        <span>{{ t("usage.effectiveThreshold") }}</span>
        <span class="kv__v">
          {{ compactNumber(usage.used) }} / {{ compactNumber(chain.threshold) }} tokens
        </span>
        <span class="chain__note">{{ chain.modeText }}</span>
      </div>
      <div v-if="chain.hasWindow" class="kv kv--stack">
        <span>{{ t("usage.headroom") }}</span>
        <span class="kv__v">{{ compactNumber(chain.headroom) }} tokens</span>
        <span class="chain__note">{{ t("usage.headroomHint") }}</span>
      </div>
    </template>

    <!-- 本轮合计 / 生成速度（§3.12；供应商报告了用量才渲染） -->
    <template v-if="chat.turnUsage">
      <div class="kv">
        <span>{{ t("usage.turnTotal") }}</span>
        <span v-if="turnTotal !== null" class="kv__v">
          {{ compactNumber(turnTotal) }} tokens
        </span>
      </div>
      <div v-if="chat.turnUsage.speed !== null" class="kv">
        <span>{{ t("usage.speed") }}</span>
        <span class="kv__v">{{ t("usage.speedValue", { n: chat.turnUsage.speed }) }}</span>
      </div>
      <!-- 模型用量六类（§3.12 行形态；命中率含在行内） -->
      <div class="kv kv--stack">
        <span>{{ t("usage.modelUsage") }}</span>
        <span class="kv__usage">
          <span>{{ t("usage.input") }} {{ compactNumber(chat.turnUsage.input) }}</span>
          <span>{{ t("usage.output") }} {{ compactNumber(chat.turnUsage.output) }}</span>
          <span v-if="chat.turnUsage.cacheRead > 0">
            {{ t("usage.cacheRead") }} {{ compactNumber(chat.turnUsage.cacheRead) }}
          </span>
          <span v-if="cacheHitRate !== null">{{ t("usage.cacheHit") }} {{ cacheHitRate }}%</span>
          <span v-if="chat.turnUsage.reasoning > 0">
            {{ t("usage.reasoning") }} {{ compactNumber(chat.turnUsage.reasoning) }}
          </span>
          <span v-if="chat.turnUsage.cacheWrite > 0">
            {{ t("usage.cacheWrite") }} {{ compactNumber(chat.turnUsage.cacheWrite) }}
          </span>
        </span>
      </div>
    </template>

    <!-- 会话累计（M0 观测 KV v2 §7）：命中率（分母不含 cacheWrite，纪律 7）。
         v2.137：节省估算随两档单价设置退役。chat.cacheStats 为 null =
         供应商未报告缓存字段 → 不渲染；chat.cacheStats 设置（用户层，
         缺省开）控制整行显隐。 -->
    <div v-if="settings.view?.chat.cacheStats && chat.cacheStats" class="kv">
      <span>{{ t("usage.sessionCumulative") }}</span>
      <span class="kv__v">{{ t("usage.cacheHit") }} {{ chat.cacheStats.hitRate }}%</span>
    </div>

    <!-- 工具上下文（本轮调用种数/次数 + 目录估算） -->
    <div v-if="toolsText" class="kv">
      <span>{{ t("usage.toolsContext") }}</span>
      <span class="kv__v">{{ toolsText }}</span>
    </div>

    <!-- 记忆树：已整理章节数 + 未整理尾巴估算（M3 §3.37②；有树才渲染） -->
    <div v-if="treeText" class="kv">
      <span>{{ t("usage.treeRow") }}</span>
      <span class="kv__v">
        {{ t("usage.treeChaptersShort", { n: treeText.chapters }) }} · {{ treeText.text }}
      </span>
    </div>

    <!-- 记忆展开次数（M4 §3.38⑥ 召回埋点；有展开才渲染） -->
    <div v-if="expandCalls > 0" class="kv">
      <span>{{ t("usage.expandRow") }}</span>
      <span class="kv__v">{{ t("usage.expandCalls", { n: expandCalls }) }}</span>
    </div>
  </div>
</template>
<style scoped>
/* v2.60 修复批：v2.59 UsageRing 拆分时整块 <style scoped> 被截丢（浮层
 * 失去定位/实底，在工具条里平铺）——按 9ab95ae^ 原样修回 */
.ctx-popover {
  position: absolute;
  right: 8px;
  bottom: calc(100% + 10px);
  width: 360px;
  max-width: calc(100vw - 48px);
  border-radius: var(--lw-r-md-plus);
  border: 1px solid var(--lw-line);
  background: var(--lw-bg-elevated);
  box-shadow: var(--lw-shadow-pop);
  padding: var(--lw-gap-12) var(--lw-gap-14);
  z-index: var(--lw-z-pop);
  font-size: var(--lw-fs-12half);
  line-height: 1.45;
}
.ctx-popover__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-weight: 500;
  color: var(--lw-ink-1);
}
.kv {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--lw-gap-8);
  margin-top: var(--lw-gap-8);
  padding-top: 7px; /* stylelint-disable-line declaration-property-value-disallowed-list -- 光学修正(孤值7,§二⑬) */
  border-top: 1px solid var(--lw-line-subtle);
  color: var(--lw-ink-3);
}
.kv--stack {
  flex-direction: column;
  align-items: flex-start;
  gap: var(--lw-gap-4);
}
.kv__usage {
  display: flex;
  flex-wrap: wrap;
  gap: var(--lw-gap-4) var(--lw-gap-12);
  color: var(--lw-ink-1);
  font-variant-numeric: tabular-nums;
}
.kv__v {
  color: var(--lw-ink-1);
  font-variant-numeric: tabular-nums;
}
/* 扣减链的档位 / 来源说明（S3 刀④b）：贴在它所解释的那一行下面，
   不另造行形态（§3.32 同类一种形态）。 */
.chain__note {
  color: var(--lw-ink-4);
  font-size: var(--lw-fs-11);
}
</style>
