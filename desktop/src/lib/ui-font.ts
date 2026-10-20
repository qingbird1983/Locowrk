// 字号基座应用唯一执行者（队列 Q14 第四刀；上位《UI Design System》附录 B 的
// 落点 = 设置页「外观」）：settings.json `appearance.uiFontSize` →
// `<html style="--lw-ui-font-size">`。相对制六档（`--lw-fs-ui-*`）全部挂在它上面
// 换算，所以改这一颗 = 整界面同尺移动，组件禁止各自写死字号来"补偿"。
//
// 形状与 lib/theme.ts 同族，三处同源：合法值清单镜像后端
// `settings/appearance.rs` 的 `UI_FONT_{DEFAULT,MIN,MAX}`；生效点只在 DOM 上写一个
// CSS 变量；未知值回落缺省。**纯函数与副作用分开**（`effectiveUiFontPx` /
// `applyUiFontPref`）也是 theme.ts 立的形态——`flipEffectiveTheme` 之所以是纯的，
// 就是为了解析逻辑能脱离 DOM 被测试。
//
// ⚠ **三个数字为什么允许出现在三个地方**：后端常量（校验权威）、`style.css:294`
// 的 `14px` 字面值（**首帧兜底**：JS 还没跑起来时六档必须有值可算，:root 里没法
// import TS 常量）、本文件（前端校验/回落用）。它们是同一条声明的三个投影，不是
// 三个出处——一致性由 `tmp/q14d-shape.mjs` 三面数值对账钉死（任一处漂了即红），
// 单靠注释里写"一一对应"在历批已被证明会漂（第六道闸的立闸理由就是这句）。

export const UI_FONT_DEFAULT = 14;
export const UI_FONT_MIN = 12;
export const UI_FONT_MAX = 18;

/** 设置页三档（S4 刀④ 数值归类）：小 / 标准 / 大，普通用户不填数值。
 *  取值为值域两端 + 缺省（12 / 14 / 18）——缺省必须落在档上，否则打开
 *  设置页时「标准」不高亮（可调项缺省 = 改前写死值）。 */
export const UI_FONT_PRESETS: ReadonlyArray<number> = [
  UI_FONT_MIN,
  UI_FONT_DEFAULT,
  UI_FONT_MAX,
];

/** 投影值 → 分段控件落点：旧 settings.json 手填过的非档值（如 13/15）
 *  就近显示、**不改存量**；用户点选某档才落档。平局取后一档（13 → 标准）。 */
export function nearestUiFontPreset(px: number): number {
  let best = UI_FONT_DEFAULT;
  let bestDist = Number.POSITIVE_INFINITY;
  for (const preset of UI_FONT_PRESETS) {
    const dist = Math.abs(preset - px);
    if (dist <= bestDist) {
      best = preset;
      bestDist = dist;
    }
  }
  return best;
}

/** 写入点唯一：CSS 变量名只在这里出现一次。 */
const CSS_VAR = "--lw-ui-font-size";

/** 解析实效基座：值域外或非整数一律回落缺省，**不钳到边界**。
 * 口径与后端 `valid_ui_font_size` 一致——"给 99 结果界面变成 18px"是把用户的
 * 错误输入悄悄演成一次生效，比"照旧显示"更难查。 */
export function effectiveUiFontPx(px: number): number {
  return (
    Number.isInteger(px) && px >= UI_FONT_MIN && px <= UI_FONT_MAX ? px : UI_FONT_DEFAULT
  );
}

/** 应用字号基座（`settings.init` 与保存动作调用）。 */
export function applyUiFontPref(px: number): void {
  document.documentElement.style.setProperty(CSS_VAR, `${effectiveUiFontPx(px)}px`);
}
