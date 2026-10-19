/** 层 3（单行展开体）的展开记忆（设计稿 §8，台账 AF-12）。
 *
 * 为什么不能住在组件里：过程行是 `v-for` 出来的，切会话、重投影、列表回收都会重建
 * 组件实例，组件内 `ref` 一丢，用户刚展开的明细就自己收起来（「必须先展开上面才能点
 * 下面」是同族手感缺陷）。所以记忆活在组件外，按 `ChatItem.id` 记。
 *
 * 一个 `Map` 同时装两件事：**键在表里** = 这一行的展开体挂载过（原 `everExpanded`
 * 的懒挂载语义），**值为 true** = 现在展开着。键是 `stores/chat/ids.ts` 发的单调
 * 递增号，跨会话不撞号；但记忆不跨会话（设计稿 §8 口径），切会话清空。
 */

import { reactive } from "vue";

const folds = reactive(new Map<number, boolean>());

/** 这一行的展开体挂载过没有（挂过才保 DOM，首开之后收展零重渲染）。 */
export function isRowMounted(id: number): boolean {
  return folds.has(id);
}

/** 这一行现在展开着没有。 */
export function isRowExpanded(id: number): boolean {
  return folds.get(id) === true;
}

/** 展开 ↔ 收起。首次交互即记下「挂载过」。 */
export function toggleRow(id: number): void {
  folds.set(id, !isRowExpanded(id));
}

/** 切会话清空（层 3 记忆只在会话内有效）。 */
export function resetRowFolds(): void {
  folds.clear();
}
