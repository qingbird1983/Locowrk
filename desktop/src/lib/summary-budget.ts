// 摘要调用预算的**档位清单**（specs/12 刀② · 设置面那一格）。
//
// 形态是**三档预置**而不是数字输入框（S4 刀④ 的普通分区约束：用户页不出现
// `type="number"`）——这三档是"摘要能等多久"的三种量级，不是让人填数值试验的
// 自由量。30s 适合小上下文快速摘要，300s 留给大上下文慢模型。
//
// ⚠ **两个数字为什么允许出现在两个地方**：后端常量（`settings/context.rs` 的
// `SUMMARY_BUDGET_PRESETS_MS` / `SUMMARY_BUDGET_DEFAULT_MS`，**校验权威**——
// 写不进这三档的更新会被后端拒收）与本文件（界面那份候选清单）。它们是同一条
// 声明的两个投影，不是两个出处：漂了的表现是"选了却存不进去"（后端拒收）或
// "当前值不在候选里"（分段控件没有选中片）。
//
// 为什么中间那档 = 缺省：可调项的缺省必须落在档上，否则打开设置页时那一档不高亮
// （同 `ui-font.ts` 那条理由）。

/** 三档（毫秒）：30s / 90s（缺省）/ 300s。 */
export const SUMMARY_BUDGET_PRESETS_MS: ReadonlyArray<number> = [
  30_000, 90_000, 300_000,
];

/** 缺省那档（90s）——与后端 `SUMMARY_BUDGET_DEFAULT_MS` 同一颗。 */
export const SUMMARY_BUDGET_DEFAULT_MS = 90_000;

/** 投影值 → 分段控件落点：非档值（老配置 / 手改过的 settings.json）就近显示、
 *  **不改存量**；用户点选某档才落档。平局取后一档（同 `nearestUiFontPreset`）。 */
export function nearestSummaryBudget(ms: number): number {
  let best = SUMMARY_BUDGET_DEFAULT_MS;
  let bestDist = Number.POSITIVE_INFINITY;
  for (const preset of SUMMARY_BUDGET_PRESETS_MS) {
    const dist = Math.abs(preset - ms);
    if (dist <= bestDist) {
      best = preset;
      bestDist = dist;
    }
  }
  return best;
}
