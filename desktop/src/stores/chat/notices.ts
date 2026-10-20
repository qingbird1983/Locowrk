// stores/chat/notices.ts —— 系统通知条的生产口 + M0 观测告警（T22 刀二搬出）。
// S4 刀② 记忆面退役后「整理发丝线」这个生产口删除，只剩通用条 / 压缩发丝线 / 原文条。
// 这一族只做一件事：**往 items 里落「UI 陈述」**（SystemNotice 模式），不发请求、不碰后端。
// 参数名一律 `st`（不是 `s`）：原实现内部的 `.find((s) => …)` 用的是 `s`，
// 把 store 状态参数也叫 `s` 会形成遮蔽 ⇒ 等价比对时看不清哪层是谁。
import { i18n } from "../../i18n";
import { useSettingsStore } from "../settings";
import { useToastStore } from "../toast";
import type { SessionUsage } from "./types";
import { nextId } from "./ids";
import type { ChatState } from "./state";

/** 系统消息入列（SystemNotice 模式的唯一生产入口；首个生产者 ⑨
 * 压缩通知，随后 ⑪ "已记住"）。 */
export function pushNotice(
  st: ChatState,
  key: string,
  params?: Record<string, unknown>,
  actions?: string[],
) {
  st.items.push({ id: nextId(), kind: "system", key, params, actions });
}

/** 发丝线族的**唯一实现**（v2.143 用户裁定「两卡片改一根发丝线，状态居中盖在线上」）：
 * 发起/收束/熔断共用同一根线——在途线被收束事件**就地改写**（一条线从「整理中」变
 * 「已整理」），无在途线（自动整理）直接落收束线。
 *
 * S2 刀⑦ 把两枚常数（`tag` 与"在途那条的 key"）提成参数：`/压缩` 要的是同一根线，
 * 复制一份实现就是"同一属性两个出处"（改写规则、落点、`tone` 三件都会各自漂）。
 * 两个入口各自保留原名，只把参数填进去。 */
/** 收走在途线（发起失败时用——线是「正在做」的陈述，失败后不再成立；就地摘除，
 * 错误由 pushRawSystem 另行陈述）。 */
function dropRunningLine(st: ChatState, tag: string, runningKey: string) {
  for (let i = st.items.length - 1; i >= 0; i--) {
    const it = st.items[i]!;
    if (it.kind === "system" && it.tag === tag && it.key === runningKey) {
      st.items.splice(i, 1);
      return;
    }
  }
}

function pushLine(
  st: ChatState,
  tag: string,
  key: string,
  params?: Record<string, unknown>,
  actions?: string[],
) {
  st.items.push({
    id: nextId(),
    kind: "system",
    key,
    params,
    actions,
    tone: "hairline",
    tag,
  });
}

function rewriteOrPushLine(
  st: ChatState,
  tag: string,
  runningKey: string,
  key: string,
  params?: Record<string, unknown>,
  actions?: string[],
) {
  const items = st.items;
  for (let i = items.length - 1; i >= 0; i--) {
    const it = items[i]!;
    if (it.kind === "system" && it.tag === tag && it.key === runningKey) {
      it.key = key;
      it.params = params;
      it.actions = actions;
      return;
    }
  }
  pushLine(st, tag, key, params, actions);
}

/** 压缩发丝线（tag="compact"，S2 刀⑦ `/压缩`）：发起落「正在压缩」，收束就地改写
 * 成结果线（一次摘要往返要几秒，人得知道按钮生效了）。 */
export function pushCompactNotice(
  st: ChatState,
  key: string,
  params?: Record<string, unknown>,
  actions?: string[],
) {
  rewriteOrPushLine(st, "compact", "notice.compactRunning", key, params, actions);
}

export function dropCompactRunning(st: ChatState) {
  dropRunningLine(st, "compact", "notice.compactRunning");
}

/** 原文系统条（key = "__raw__"，ChatMessages 按 params.text 渲染）：
 * 用于"查看详情"展开压缩摘要等内容型陈述。 */
export function pushRawSystem(st: ChatState, text: string) {
  st.items.push({
    id: nextId(),
    kind: "system",
    key: "__raw__",
    params: { text },
  });
}

/** M0 观测告警（KV v2 §7.4，开发者开关缺省关）：会话累计命中率跌破
 * 阈值时 toast 一次（每会话至多一次）。≥3 步才判——冷启动首请求无
 * 缓存可命不是断裂；供应商从未报告过缓存字段（cacheRead/cacheWrite
 * 全 0）不判——未报告 ≠ 命中率塌方，宁缺勿假。 */
export function checkCacheAlert(st: ChatState, su: SessionUsage) {
  if (st.cacheAlertFired || st.replaying) return;
  const dev = useSettingsStore().view?.developer;
  if (!dev?.cacheHitAlert) return;
  if (su.steps < 3 || su.input <= 0) return;
  if (su.cacheRead + su.cacheWrite <= 0) return;
  if (su.cacheRead / su.input >= dev.cacheHitAlertPercent / 100) return;
  st.cacheAlertFired = true;
  useToastStore().push(
    "warn",
    i18n.global.t("usage.cacheAlert", { pct: dev.cacheHitAlertPercent }),
  );
}
