// 自动压缩阈值的**档位清单**（specs/13 · 设置面那一格）。
//
// 阈值 = **占模型窗口的百分比**，不是绝对 token。为什么要换口径：绝对数不跟
// 窗口走——976K 窗口的模型按 100K 压是「永远压不完」，128K 窗口的模型按 100K
// 压是「永远压不到」。百分比跟着模型走，跨模型可预期（这正是「黑盒」的根因，
// 见 specs/13 的根因表第 2 条）。
//
// 形态是**三档预置**而不是数字输入框（S4 刀④ 的普通分区约束：用户页不出现
// `type="number"`）——这三档是「压多早」的三种量级，不是让人填数值试验的自由量。
//
// ⚠ **两个数字为什么允许出现在两个地方**：后端常量（`settings/context.rs` 的
// `AUTO_COMPACT_PCT_PRESETS` / `AUTO_COMPACT_PCT_DEFAULT`，**校验权威**——写不进
// 这三档的更新会被后端拒收）与本文件（界面那份候选清单）。它们是同一条声明的
// 两个投影，不是两个出处（同 `summary-budget.ts` 那条纪律）。
// ⚠ 两侧对账由**同源闸 SS32** 钉住（缺省与三档逐个相等）——漂了的表现是"选了却
// 存不进去"（后端拒收）或"当前值不在候选里"（分段控件没有选中片）。
//
// ⚠ **档外值为什么只可能来自手改过的文件**：这枚键是新加的，老配置里根本没有它
// ⇒ 没有存量档外值要迁移。界面遇到档外值仍**就近显示、不改存量**（点选才落档），
// 但后端会拒收它——手改过的人打开设置页选一档就把它归位了。
//
// 为什么缺省那档 = 50 而不是上游的 85：上游缺省 85%（窗口的 15% 留空）时主力
// 窗口 ~200K ⇒ 170K 触发；本仓主力模型窗口 976K ⇒ 85% 要 830K 才触发，等于
// 长期不触发、白养一个近百万的上下文。50% ⇒ 488K，是「压得动」的量级。
// 要回到上游口径只改一颗数（后端那颗 + 本文件那颗）。

/** 三档（百分比）：25% / 50%（缺省）/ 85%。 */
export const AUTO_COMPACT_PCT_PRESETS: ReadonlyArray<number> = [25, 50, 85];

/** 缺省那档（50%）——与后端 `AUTO_COMPACT_PCT_DEFAULT` 同一颗。 */
export const AUTO_COMPACT_PCT_DEFAULT = 50;

/** 投影值 → 分段控件落点：非档值（老配置 / 手改过的 settings.json）就近显示、
 *  **不改存量**；用户点选某档才落档。平局取后一档（同 `nearestUiFontPreset`）。 */
export function nearestAutoCompactPct(pct: number): number {
  let best = AUTO_COMPACT_PCT_DEFAULT;
  let bestDist = Number.POSITIVE_INFINITY;
  for (const preset of AUTO_COMPACT_PCT_PRESETS) {
    const dist = Math.abs(preset - pct);
    if (dist <= bestDist) {
      best = preset;
      bestDist = dist;
    }
  }
  return best;
}
