// lib/chat/compact-line.ts —— 压缩线族（发丝线一族）的 tag 与判据（**唯一出处**）。
//
// 为什么单独成文件：这一族同时被**生产面**（chat store 落那几根线）与
// **渲染面**（TranscriptFlow 把压缩线摘到轮容器外顶层）读。两处各写一份
// tag 判据 = 同一属性两个出处 ⇒ 改一处漂一处（本仓一路在反的那件事）。
//
// 三个 tag = 压缩线在界面上的三种身份：
//   manual —— 手动 `/压缩` 那根在途线
//   auto   —— 自动压缩那根在途线（自动路径可能与手动那根**同时在场**）
//   result —— 压成之后的结果线（在途那根被**就地改写**成它 ⇒ 它的 tag 与
//             在途那根不同，"这根还在途吗"因此判得出来）
//
// 形态口径见《Front-end Design Guidelines》§二⑲「压缩线与摘要展开体」。

/** 手动 `/压缩` 那根在途线。 */
export const COMPACT_TAG_MANUAL = "compact";
/** 自动压缩那根在途线（tag 与手动那根**分开**：人按 `/压缩` 的那一刻这一轮
 *  还在跑 ⇒ 两根"正在压缩"会同时存在，共用一个 tag 时自动路径的收束会把
 *  手动那根改写掉，手动那一下的失败提示凭空消失）。 */
export const COMPACT_TAG_AUTO = "compact-auto";
/** 压成之后的结果线（在途那根被就地改写成它）。 */
export const COMPACT_TAG_RESULT = "compact-result";

/** 压缩线族的全部 tag（判"这是不是一根压缩线"只认这一张表）。 */
const COMPACT_TAGS = new Set<string>([
  COMPACT_TAG_MANUAL,
  COMPACT_TAG_AUTO,
  COMPACT_TAG_RESULT,
]);

/** 判据的输入：只要 kind 与 tag 两格 —— 不引 store 的类型（lib 不反向依赖 store）。 */
export type CompactTagged = { kind: string; tag?: string };

/** 这一条是不是压缩线（发丝线族的一根）？ */
export function isCompactItem(item: CompactTagged): boolean {
  return item.kind === "system" && typeof item.tag === "string" && COMPACT_TAGS.has(item.tag);
}
