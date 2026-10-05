/**
 * Locowrk 前端统一标准机械门禁（《Front-end Design Guidelines.md》v2.1 §六）。
 * 一级（error，令牌已齐无借口）：颜色/圆角/阴影/动效时长一律 var(--lw-*)；
 * 裸 px 字号/内外边距（v2.87 起 lw-fs/lw-gap 令牌已建齐，两级全部 error）。
 * 豁免 = stylelint-disable-line 行注释（引用 §七 批准条目或 §二⑬ 光学修正），
 * 无凭证豁免按判错处理。
 * 常驻循环动画白名单（§二①）：lw-pulse 1s / lw-indeterminate 720ms / welcome-float·blink 4s。
 * @type {import('stylelint').Config}
 */
module.exports = {
  customSyntax: "postcss-html",
  rules: {
    "declaration-property-value-allowed-list": {
      "/^(color|background|background-color|background-image|border|border-left|border-right|border-top|border-bottom|outline|outline-color|fill|stroke|caret-color|accent-color|text-decoration-color|column-rule-color|mask|mask-image|-webkit-mask-image|-webkit-mask)$/i":
        [
          "/var\\(--lw-/i",
          "/^transparent$/i",
          "/^currentColor$/i",
          "/^none$/i",
          "/^#c42b1c$/i",
          "/^#b12a1c$/i",
          "/^0$/",
        ],
      "/^(border|border-top|border-right|border-bottom|border-left)-radius$/i":
        ["/^(0|var\\(--lw-r-[a-z0-9-]+\\))(\\s+(0|var\\(--lw-r-[a-z0-9-]+\\)))*$/"],
      "/^box-shadow$/i": ["/^none$/i", "/var\\(--lw-/i", "/color-mix\\(in oklab, var\\(--lw-/i"],
      "/^(transition|animation)(-duration)?$/i": [
        "/var\\(--dur-/i",
        "/^none$/i",
        "/^\\S+\\s+1s\\s+.*/",
        "/^lw-indeterminate\\s+720ms\\s+.*/",
        "/^welcome-(float|blink)\\s+4s\\s+.*infinite$/",
        "/^0\\.01ms$/",
      ],
    },
    "declaration-property-value-disallowed-list": [
      {
        // 裸 px 判定 = 全值不含 var(/calc( 时才判（tempered 前缀）——
        // calc(var(--x) + 40px) 这类令牌运算不火线（ignoreValues 选项对本
        // 规则无效，实测已弃用，改由正则表达豁免）
        "/^font-size$/i": ["/^\\d+(\\.\\d+)?px$/"],
        "/^(padding|margin)(-(top|right|bottom|left|inline|block))?(-start|-end)?$/i": [
          "/^(?!.*(?:var|calc)\\().*(?:^|\\s|,)\\d+(\\.\\d+)?px/",
        ],
        "/^gap$/i": [
          "/^(?!.*(?:var|calc)\\().*(?:^|\\s|,)\\d+(\\.\\d+)?px/",
        ],
      },
      { severity: "error" },
    ],
  },
};
