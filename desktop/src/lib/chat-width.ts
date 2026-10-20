// 主对话区列宽常量（UI shell doc §2.7 的唯一真源）：转录列、composer 列、
// 每条消息正文宽三处同吃一个变量（--lw-content-max 的对话区覆盖值），
// 实战宽度 = min(主区宽, 偏好值)，偏好值落 settings.json chat.contentMaxWidth。
export const CHAT_CONTENT_WIDTH_DEFAULT = 760;
export const CHAT_CONTENT_WIDTH_MIN = 560;
/** 单侧留白（§6.4 --lw-chat-gutter）：给调宽热区用；上限 = 主区宽 − 2×该值。 */
export const CHAT_CONTENT_WIDTH_GUTTER = 24;

/** 设置页三档（S4 刀④ 数值归类）：窄 / 标准 / 宽。标准 = 缺省 760；
 *  窄/宽各偏一档，拖拽双柄仍可落在 560–4000 的任意值（设置页就近显示档位）。 */
export const CHAT_CONTENT_WIDTH_NARROW = 640;
export const CHAT_CONTENT_WIDTH_WIDE = 960;
export const CHAT_CONTENT_WIDTH_PRESETS: ReadonlyArray<number> = [
  CHAT_CONTENT_WIDTH_NARROW,
  CHAT_CONTENT_WIDTH_DEFAULT,
  CHAT_CONTENT_WIDTH_WIDE,
];

/** 当前偏好值 → 分段控件落点：拖拽出的非档值（如 823）就近显示，不改存量；
 *  点选某档才写回该档数值。 */
export function nearestChatWidthPreset(px: number): number {
  let best = CHAT_CONTENT_WIDTH_DEFAULT;
  let bestDist = Number.POSITIVE_INFINITY;
  for (const preset of CHAT_CONTENT_WIDTH_PRESETS) {
    const dist = Math.abs(preset - px);
    if (dist < bestDist) {
      best = preset;
      bestDist = dist;
    }
  }
  return best;
}
