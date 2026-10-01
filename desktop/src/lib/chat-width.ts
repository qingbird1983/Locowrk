// 主对话区列宽常量（UI shell doc §2.7 的唯一真源）：转录列、composer 列、
// 每条消息正文宽三处同吃一个变量（--lw-content-max 的对话区覆盖值），
// 实战宽度 = min(主区宽, 偏好值)，偏好值落 settings.json chat.contentMaxWidth。
export const CHAT_CONTENT_WIDTH_DEFAULT = 760;
export const CHAT_CONTENT_WIDTH_MIN = 560;
/** 单侧留白（§6.4 --lw-chat-gutter）：给调宽热区用；上限 = 主区宽 − 2×该值。 */
export const CHAT_CONTENT_WIDTH_GUTTER = 24;
