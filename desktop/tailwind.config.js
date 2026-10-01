/** @type {import('tailwindcss').Config} */
// 颜色角色表（UI shell and patterns.md §6.1 / §七-6）：组件层只允许这里的
// 角色 token，hue-shade / neutral-* 类一律禁止（38 阶 hue 映射表已收缩到语义色）。
//
// 两类映射（§6.2 双形态纪律）：
// - 实色 token（accent/ok/warn/danger/info/purple/gray/surface 别名-3）走
//   --lw-rgb-* 三元组，`/16` 等透明度修饰符可用；
// - 自带透明度的 token（page/side/dock/inset/secondary/tertiary/elevated/
//   composer/raised 亮色/wash/ink/line）整值引用 var()——**对这些类加
//   /透明度修饰符会被静默丢弃**（var() 无法注入 alpha），组件里禁止这么写。
const v = (name) => `rgb(var(--lw-rgb-${name}) / <alpha-value>)`;
const t = (name) => `var(--lw-${name})`;

export default {
  content: ["./index.html", "./src/**/*.{vue,ts}"],
  theme: {
    extend: {
      colors: {
        // §6.1 表面层级 + wash（平面内分层，无边框）
        page: t("bg"),
        side: t("bg-side"),
        dock: t("bg-dock"),
        inset: t("bg-inset"),
        secondary: t("bg-secondary"),
        tertiary: t("bg-tertiary"),
        elevated: t("bg-elevated"),
        composer: t("bg-composer"),
        raised: t("raised"),
        wash: { 1: t("wash-1"), 2: t("wash-2"), 3: t("wash-3") },
        // 文本五级（自带 alpha，禁加修饰符）
        ink: { 1: t("ink-1"), 2: t("ink-2"), 3: t("ink-3"), 4: t("ink-4"), 5: t("ink-5") },
        // 线（自带 alpha，禁加修饰符；1px 边框只许浮层用）
        line: { DEFAULT: t("line"), subtle: t("line-subtle"), strong: t("line-strong") },
        // 单色 accent（暗=白 / 亮=近黑）+ accent 上的文字色
        accent: { DEFAULT: v("accent"), hover: v("accent-hover"), ink: v("accent-ink") },
        // 状态色（§3.4 五族；彩度只出现在状态）
        ok: v("ok"),
        warn: v("warn"),
        danger: v("danger"),
        info: v("info"),
        purple: v("purple"),
        // 实色灰阶（头像块 / 发丝环等少量实色需要）
        gray: {
          0: v("gray-0"),
          50: v("gray-50"),
          75: v("gray-75"),
          100: v("gray-100"),
          300: v("gray-300"),
          500: v("gray-500"),
          550: v("gray-550"),
          600: v("gray-600"),
          700: v("gray-700"),
          750: v("gray-750"),
          800: v("gray-800"),
          900: v("gray-900"),
          1000: v("gray-1000"),
        },
        // 过渡别名（⑰ 旧组件仍在用；对应组件被 UI 批次重写时逐个清除）
        surface: {
          0: t("bg"),
          1: t("bg-secondary"),
          2: t("bg-tertiary"),
          3: v("gray-700"),
        },
      },
      boxShadow: {
        // §6.1 阴影只给浮起件（平面内组件不得有阴影）
        dialog: "var(--lw-shadow-dialog)",
        composer: "var(--lw-shadow-composer)",
        raised: "var(--lw-shadow-raised)",
      },
    },
  },
  plugins: [],
};
