/**
 * 轮/组共用的走表时钟（队列 Q12·第二刀 v2.205 收的重复面）。
 *
 * 收敛前「本轮跑了多久」在两个组件里**各写了一遍**：`TurnTimerLine.vue`
 * （第 0 层轮头行，文案「工作中 / 已工作 N 分 N 秒」）与 `TurnProcess.vue`
 * （过程组头行的 `durationText`）各自持有一套完全同形的机器——`now = ref(Date.now())`
 * + `finishedAt` 本地收束戳 + `setInterval` 每秒推进 + 「瞬时翻假回 live 要清戳」
 * 那条 v2.97 实抓来的边沿判断 + 卸载清定时器。同一属性两个出处 = 改一处漏一处的
 * 产地（与批 F·F1 收时钟尺、Q12·C1 收 ±N 同族），这里收成一把尺。
 *
 * ⚠ **只统一机制，不改口径**：`endTs`（重放终点）与 `activeMs`（宿主扣过等待的
 * 权威时长）都是**可选输入**，调用方不给就维持它原来的行为——`TurnProcess` 至今
 * 不读 `endTs`（重放时组级本就无时长），本批不趁收尺偷偷改它的读数。
 *
 * 读数优先级（收束后）：`activeMs` > 本地收束戳 > `endTs` > 无读数。
 * `activeMs` 缺省 = 数据层读不出（旧日志没有轮界时刻或审批两刻）⇒ 回落到墙钟差，
 * 这是**如实降级**不是猜：**绝不允许**把缺省当 0。
 */
import { computed, onBeforeUnmount, ref, watch, type ComputedRef } from "vue";

export interface TurnClockSource {
  /** 计时起点（轮 = 用户消息时刻）。undefined = 无从起算 ⇒ 不给读数。 */
  startTs?: () => number | undefined;
  /** 当轮/组是否在跑；收束即冻结读数。 */
  running: () => boolean;
  /** 重放终点（v2.101 轮界事件投影出的 `endTs`）。不给 = 该容器不吃重放读数。 */
  endTs?: () => number | undefined;
  /** 宿主给的"真正干活"毫秒数（墙钟区间 − Σ审批等待）。只在**已收束**时采用：
   * 轮还在跑时它必然还没有值，采用旧值会把读数冻在上一轮。 */
  activeMs?: () => number | undefined;
}

/** 已用秒数；null = 这个容器此刻不该有读数（组件据此省略整段文本）。 */
export function useTurnClock(src: TurnClockSource): { seconds: ComputedRef<number | null> } {
  const now = ref(Date.now());
  const finishedAt = ref<number | null>(null);
  let timer: number | null = null;
  watch(
    src.running,
    (r, prev) => {
      if (r) {
        // 瞬时翻假回 live（v2.97 实抓）清戳恢复走表；真实收束不回 live 时戳保留。
        if (prev !== undefined) finishedAt.value = null;
        if (timer === null) {
          now.value = Date.now();
          timer = window.setInterval(() => (now.value = Date.now()), 1000);
        }
      } else {
        if (prev !== undefined && prev && finishedAt.value === null) {
          finishedAt.value = Date.now();
        }
        if (timer !== null) {
          window.clearInterval(timer);
          timer = null;
        }
      }
    },
    { immediate: true },
  );
  onBeforeUnmount(() => {
    if (timer !== null) window.clearInterval(timer);
  });

  const seconds = computed<number | null>(() => {
    const start = src.startTs?.();
    if (start === undefined) return null;
    const active = src.activeMs?.();
    if (active != null && !src.running()) return Math.max(0, Math.round(active / 1000));
    const end = finishedAt.value ?? src.endTs?.() ?? (src.running() ? now.value : null);
    if (end == null) return null;
    return Math.max(0, Math.round((end - start) / 1000));
  });
  return { seconds };
}
