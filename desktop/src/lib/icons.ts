/**
 * 图标集（UI shell doc §3.19 图标控件 / 原型 index.html sprite 逐字采用）：
 * 统一 16 viewBox、1.5 描边、round cap/join、stroke=currentColor；
 * 默认 15px（窗口控制 12px / 侧栏字形 14px）。新图标先进本表再上组件。
 */
import { defineComponent, h, type PropType } from "vue";

export type IconName =
  | "sparkle"
  | "chevronDown"
  | "chevronRight"
  | "plus"
  | "search"
  | "panel"
  | "sidebar"
  | "settings"
  | "clock"
  | "bell"
  | "close"
  | "min"
  | "max"
  | "dots"
  | "copy"
  | "edit"
  | "sendUp"
  | "stop"
  | "bot"
  | "folder"
  | "terminal"
  | "warn"
  | "check"
  | "trash"
  | "plug"
  | "file"
  | "database"
  | "checklist"
  | "undo"
  | "info";

/** 原型 sprite 的 symbol 内联内容（stroke 属性由外壳 svg 统一给）。 */
const PATHS: Record<IconName, string> = {
  sparkle: '<path d="M8 2.6 9.1 6.3 12.8 7.4 9.1 8.5 8 12.2 6.9 8.5 3.2 7.4 6.9 6.3Z"/>',
  chevronDown: '<path d="M4.6 6.4 8 9.8 11.4 6.4"/>',
  chevronRight: '<path d="M6.4 4.6 9.8 8 6.4 11.4"/>',
  plus: '<path d="M8 3.4v9.2M3.4 8h9.2"/>',
  search: '<circle cx="7.1" cy="7.1" r="4"/><path d="M10.1 10.1 13.4 13.4"/>',
  panel: '<rect x="2.4" y="3.2" width="11.2" height="9.6" rx="2"/><path d="M10 3.2v9.6"/>',
  sidebar: '<rect x="2.4" y="3.2" width="11.2" height="9.6" rx="2"/><path d="M6 3.2v9.6"/>',
  settings:
    '<circle cx="8" cy="8" r="2.3"/><path d="M8 2.2v1.5M8 12.3v1.5M2.2 8h1.5M12.3 8h1.5M3.9 3.9l1.1 1.1M11 11l1.1 1.1M12.1 3.9 11 5M5 11 3.9 12.1"/>',
  clock: '<circle cx="8" cy="8" r="5.4"/><path d="M8 5v3.2l2.2 1.3"/>',
  bell: '<path d="M4.6 11.2V7.6a3.4 3.4 0 0 1 6.8 0v3.6l1.2 1.4H3.4Z"/><path d="M6.6 12.6a1.4 1.4 0 0 0 2.8 0"/>',
  close: '<path d="M4.3 4.3 11.7 11.7M11.7 4.3 4.3 11.7"/>',
  min: '<path d="M3.6 8h8.8"/>',
  max: '<rect x="4" y="4" width="8" height="8" rx="1.2"/>',
  dots: '<circle cx="4" cy="8" r="1" fill="currentColor" stroke="none"/><circle cx="8" cy="8" r="1" fill="currentColor" stroke="none"/><circle cx="12" cy="8" r="1" fill="currentColor" stroke="none"/>',
  copy: '<rect x="6" y="6" width="7.4" height="7.4" rx="1.6"/><path d="M10.2 3.2H4.4a1.4 1.4 0 0 0-1.4 1.4v5.8"/>',
  edit: '<path d="M11.2 2.6l2.2 2.2-7.8 7.8-2.8.6.6-2.8Z"/>',
  sendUp: '<path d="M8 12.6V4M4.4 7.6 8 4l3.6 3.6"/>',
  stop: '<rect x="5.2" y="5.2" width="5.6" height="5.6" rx="1.2" fill="currentColor" stroke="none"/>',
  bot: '<rect x="3.2" y="5.4" width="9.6" height="7.4" rx="2.4"/><circle cx="6.4" cy="9.1" r=".85" fill="currentColor" stroke="none"/><circle cx="9.6" cy="9.1" r=".85" fill="currentColor" stroke="none"/><path d="M8 5.4V3"/>',
  folder:
    '<path d="M2.6 5.2a1.4 1.4 0 0 1 1.4-1.4h2.4l1.4 1.6h4.2a1.4 1.4 0 0 1 1.4 1.4v5a1.4 1.4 0 0 1-1.4 1.4H4a1.4 1.4 0 0 1-1.4-1.4Z"/>',
  terminal:
    '<rect x="2.4" y="3.2" width="11.2" height="9.6" rx="2"/><path d="M5.4 6.8 7 8.4 5.4 10M8.8 10.4h2"/>',
  warn: '<path d="M8 2.8 13.4 12.2H2.6Z"/><path d="M8 6.4v3M8 11.1v.01"/>',
  check: '<path d="M4 8.6 6.7 11.3 12 5.6"/>',
  trash:
    '<path d="M3.4 5.4h9.2M6.6 5.4V3.8h2.8v1.6"/><path d="M4.6 5.4l.6 7.2a1.2 1.2 0 0 0 1.2 1.1h3.2a1.2 1.2 0 0 0 1.2-1.1l.6-7.2"/>',
  plug: '<path d="M6.2 2.8v3M9.8 2.8v3"/><path d="M4.4 5.8h7.2v2.4a3.6 3.6 0 0 1-7.2 0Z"/><path d="M8 12v1.6"/>',
  file: '<path d="M4.4 2.8h4.2l3 3v7.4H4.4Z"/><path d="M8.6 2.8v3h3"/>',
  database:
    '<ellipse cx="8" cy="4.4" rx="5" ry="2"/><path d="M3 4.4v7.2c0 1.1 2.2 2 5 2s5-.9 5-2V4.4"/><path d="M3 8c0 1.1 2.2 2 5 2s5-.9 5-2"/>',
  checklist:
    '<path d="M3.2 5.4l1.1 1.1 1.9-1.9"/><path d="M9.2 5.4h3.6M3.2 10.6l1.1 1.1 1.9-1.9M9.2 10.6h3.6"/>',
  undo: '<path d="M6.6 4 3.2 7.4l3.4 3.4"/><path d="M3.2 7.4h5.4a3.6 3.6 0 0 1 0 7.2H6.2"/>',
  info: '<circle cx="8" cy="8" r="5.4"/><path d="M8 7.4v3.6M8 5.2v.01"/>',
};

/** 统一规格的字形组件：<Icon name="sidebar" :size="14" />。 */
export const Icon = defineComponent({
  name: "Icon",
  props: {
    name: { type: String as PropType<IconName>, required: true },
    size: { type: Number, default: 15 },
  },
  setup(props) {
    // 单根节点：内联 svg 直接作为渲染输出；aria-hidden 由用方标注
    return () =>
      h("svg", {
        width: props.size,
        height: props.size,
        viewBox: "0 0 16 16",
        fill: "none",
        stroke: "currentColor",
        "stroke-width": 1.5,
        "stroke-linecap": "round",
        "stroke-linejoin": "round",
        class: "shrink-0",
        innerHTML: PATHS[props.name],
      });
  },
});
