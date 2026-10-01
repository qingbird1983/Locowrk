/** @type {import('tailwindcss').Config} */
// 颜色全部绑定 style.css 的 --lw-rgb-* 三元组（UI shell and patterns.md
// §六）：暗色 = :root 缺省值（= Tailwind 原调色板值，像素级不变），亮色 =
// [data-theme="light"] 覆盖。三元组形态是 bg-x/60 半透明修饰符能工作的前提。
// 扩表纪律：新 (hue,shade) 组合先在这里登记（两主题都给值），再上组件。
const v = (name) => `rgb(var(--lw-rgb-${name}) / <alpha-value>)`;
const shades = (hue, list) =>
  Object.fromEntries(list.map((s) => [s, v(`${hue}-${s}`)]));

export default {
  content: ["./index.html", "./src/**/*.{vue,ts}"],
  theme: {
    extend: {
      colors: {
        // 语义角色（组件层颜色类规范：中性灰阶一律用角色类）
        surface: { 0: v("surface-0"), 1: v("surface-1"), 2: v("surface-2"), 3: v("surface-3") },
        ink: { 1: v("ink-1"), 2: v("ink-2"), 3: v("ink-3"), 4: v("ink-4"), 5: v("ink-5") },
        line: { DEFAULT: v("line"), strong: v("line-strong") },
        accent: { DEFAULT: v("accent"), hover: v("accent-hover") },
        // 中性兜底：漏网的灰阶类不至于破坏主题（新代码禁止用）
        neutral: shades("neutral", ["100", "300", "400", "500", "600", "700", "800", "900"]),
        // 语义 hue 实用阶（徽章族/提示条/按钮；只允许本表已列组合）
        red: shades("red", ["200", "300", "400", "500", "600", "700", "800", "900", "950"]),
        amber: shades("amber", ["300", "400", "500", "700", "900"]),
        yellow: shades("yellow", ["200", "300", "900", "950"]),
        emerald: shades("emerald", ["200", "300", "400", "500", "600", "700", "800", "900"]),
        indigo: shades("indigo", ["300", "500", "600", "900"]),
        sky: shades("sky", ["300", "950"]),
        teal: shades("teal", ["300", "900"]),
        purple: shades("purple", ["200", "300", "900", "950"]),
      },
    },
  },
  plugins: [],
};
