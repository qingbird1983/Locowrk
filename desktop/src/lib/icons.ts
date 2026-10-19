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
  | "maxDiag"
  | "dots"
  | "copy"
  | "edit"
  | "sendUp"
  | "stop"
  | "shield"
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
  | "info"
  | "globe"
  | "chat"
  | "server"
  | "refresh"
  | "grip"
  | "star"
  | "folderOpen"
  | "filter"
  | "bulb"
  | "gitBranch"
  | "hand"
  | "shieldCheck"
  | "shieldAlert"
  | "paperclip"
  | "camera"
  | "download"
  | "chevronLeft"
  | "eye"
  | "arrowLeft"
  | "arrowRight"
  | "external";

/** 原型 sprite 的 symbol 内联内容（stroke 属性由外壳 svg 统一给）。
 * 全表原生 16 格坐标（lucide 24 格源路径已按 2/3 统一缩放内联——无
 * transform 包裹、无独立描边补偿；描边 1.5 由外壳统一给，标准 §二⑪）。 */
const PATHS: Record<IconName, string> = {
  sparkle: '<path d="M8 2.6 9.1 6.3 12.8 7.4 9.1 8.5 8 12.2 6.9 8.5 3.2 7.4 6.9 6.3Z"/>',
  chevronDown: '<path d="M4.6 6.4 8 9.8 11.4 6.4"/>',
  chevronRight: '<path d="M6.4 4.6 9.8 8 6.4 11.4"/>',
  plus: '<path d="M8 3.4v9.2M3.4 8h9.2"/>',
  search: '<circle cx="7.1" cy="7.1" r="4"/><path d="M10.1 10.1 13.4 13.4"/>',
  panel: '<rect x="2.4" y="3.2" width="11.2" height="9.6" rx="2"/><path d="M10 3.2v9.6"/>',
  sidebar: '<rect x="2.4" y="3.2" width="11.2" height="9.6" rx="2"/><path d="M6 3.2v9.6"/>',
  settings: '<path d="M 8.147 1.333 h -0.293 a 1.333 1.333 0 0 0 -1.333 1.333 v 0.12 a 1.333 1.333 0 0 1 -0.667 1.153 l -0.287 0.167 a 1.333 1.333 0 0 1 -1.333 0 l -0.1 -0.053 a 1.333 1.333 0 0 0 -1.82 0.487 l -0.147 0.253 a 1.333 1.333 0 0 0 0.487 1.82 l 0.1 0.067 a 1.333 1.333 0 0 1 0.667 1.147 v 0.34 a 1.333 1.333 0 0 1 -0.667 1.16 l -0.1 0.06 a 1.333 1.333 0 0 0 -0.487 1.82 l 0.147 0.253 a 1.333 1.333 0 0 0 1.82 0.487 l 0.1 -0.053 a 1.333 1.333 0 0 1 1.333 0 l 0.287 0.167 a 1.333 1.333 0 0 1 0.667 1.153 V 13.333 a 1.333 1.333 0 0 0 1.333 1.333 h 0.293 a 1.333 1.333 0 0 0 1.333 -1.333 v -0.12 a 1.333 1.333 0 0 1 0.667 -1.153 l 0.287 -0.167 a 1.333 1.333 0 0 1 1.333 0 l 0.1 0.053 a 1.333 1.333 0 0 0 1.82 -0.487 l 0.147 -0.26 a 1.333 1.333 0 0 0 -0.487 -1.82 l -0.1 -0.053 a 1.333 1.333 0 0 1 -0.667 -1.16 v -0.333 a 1.333 1.333 0 0 1 0.667 -1.16 l 0.1 -0.06 a 1.333 1.333 0 0 0 0.487 -1.82 l -0.147 -0.253 a 1.333 1.333 0 0 0 -1.82 -0.487 l -0.1 0.053 a 1.333 1.333 0 0 1 -1.333 0 l -0.287 -0.167 a 1.333 1.333 0 0 1 -0.667 -1.153 V 2.667 a 1.333 1.333 0 0 0 -1.333 -1.333 z"/><circle cx="8" cy="8" r="2"/>',
  clock: '<circle cx="8" cy="8" r="5.4"/><path d="M8 5v3.2l2.2 1.3"/>',
  bell: '<path d="M4.6 11.2V7.6a3.4 3.4 0 0 1 6.8 0v3.6l1.2 1.4H3.4Z"/><path d="M6.6 12.6a1.4 1.4 0 0 0 2.8 0"/>',
  close: '<path d="M4.3 4.3 11.7 11.7M11.7 4.3 4.3 11.7"/>',
  min: '<path d="M3.6 8h8.8"/>',
  max: '<rect x="4" y="4" width="8" height="8" rx="1.2"/>',
  // 斜对角双箭头最大化（v2.60 修复批用户裁定，lucide maximize-2 缩 16 格）
  maxDiag:
    '<path d="M10 2h4v4M6 14H2v-4M14 2 9.4 6.6M2 14l4.6-4.6"/>',
  dots: '<circle cx="4" cy="8" r="1" fill="currentColor" stroke="none"/><circle cx="8" cy="8" r="1" fill="currentColor" stroke="none"/><circle cx="12" cy="8" r="1" fill="currentColor" stroke="none"/>',
  copy: '<rect x="6" y="6" width="7.4" height="7.4" rx="1.6"/><path d="M10.2 3.2H4.4a1.4 1.4 0 0 0-1.4 1.4v5.8"/>',
  edit: '<path d="M11.2 2.6l2.2 2.2-7.8 7.8-2.8.6.6-2.8Z"/>',
  sendUp: '<path d="M8 12.6V4M4.4 7.6 8 4l3.6 3.6"/>',
  stop: '<rect x="4" y="4" width="8" height="8" rx="1.4" fill="currentColor" stroke="none"/>',
  shield:
    '<path d="M8 1.8l5.4 2.3v4.1c0 3.6-2.3 6.8-5.4 8-3.1-1.2-5.4-4.4-5.4-8V4.1L8 1.8z" stroke-linejoin="round"/>',
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
  globe:
    '<circle cx="8" cy="8" r="5.4"/><path d="M2.6 8h10.8M8 2.6c1.7 1.9 1.7 8.9 0 10.8M8 2.6c-1.7 1.9-1.7 8.9 0 10.8"/>',
  chat: '<path d="M13.4 8a5.4 5.4 0 0 1-5.4 5.4c-.9 0-1.8-.2-2.5-.6L2.6 13.4l.6-2.9A5.4 5.4 0 1 1 13.4 8Z"/>',
  server:
    '<rect x="2.6" y="3" width="10.8" height="4.2" rx="1.2"/><rect x="2.6" y="8.8" width="10.8" height="4.2" rx="1.2"/><path d="M5.1 5.1v.01M5.1 10.9v.01"/>',
  refresh: '<path d="M13.2 8A5.2 5.2 0 1 1 11.5 4.1"/><path d="M13.4 2.6v2.7h-2.7"/>',
  grip:
    '<circle cx="6" cy="4.6" r="1" fill="currentColor" stroke="none"/><circle cx="10" cy="4.6" r="1" fill="currentColor" stroke="none"/><circle cx="6" cy="8" r="1" fill="currentColor" stroke="none"/><circle cx="10" cy="8" r="1" fill="currentColor" stroke="none"/><circle cx="6" cy="11.4" r="1" fill="currentColor" stroke="none"/><circle cx="10" cy="11.4" r="1" fill="currentColor" stroke="none"/>',
  star:
    '<path d="M8 2.4l1.7 3.4 3.7.5-2.7 2.7.6 3.7L8 11l-3.3 1.7.6-3.7-2.7-2.7 3.7-.5Z"/>',
  folderOpen: '<path d="m 4 9.333 1 -1.933 A 1.333 1.333 0 0 1 6.16 6.667 H 13.333 a 1.333 1.333 0 0 1 1.293 1.667 l -1.027 4 a 1.333 1.333 0 0 1 -1.3 1 H 2.667 a 1.333 1.333 0 0 1 -1.333 -1.333 V 3.333 a 1.333 1.333 0 0 1 1.333 -1.333 h 2.6 a 1.333 1.333 0 0 1 1.127 0.6 l 0.54 0.8 a 1.333 1.333 0 0 0 1.113 0.6 H 12 a 1.333 1.333 0 0 1 1.333 1.333 v 1.333"/>',
  filter: '<path d="M2.8 3.6h10.4L9.6 8.1v3.7l-3.2 1.6V8.1Z"/>',
  bulb: '<path d="M 10 9.333 c 0.133 -0.667 0.467 -1.133 1 -1.667 0.667 -0.6 1 -1.467 1 -2.333 A 4 4 0 0 0 4 5.333 c 0 0.667 0.133 1.467 1 2.333 0.467 0.467 0.867 1 1 1.667"/><path d="M 6 12 h 4"/><path d="M 6.667 14.667 h 2.667"/>',
  // v4.13 过程行类型字形（git-branch 按 2/3 缩放内联，§六图标表）
  gitBranch:
    '<circle cx="4.7" cy="4" r="1.7"/><circle cx="4.7" cy="12" r="1.7"/><circle cx="11.3" cy="5.3" r="1.7"/><path d="M4.7 5.7v4.6M11.3 7c0 2.4-1.6 3.2-3.6 3.6-1.3.25-2 .8-2 2"/>',
  hand: '<path d="M 12 7.333 V 4 a 1.333 1.333 0 0 0 -1.333 -1.333 v 0 a 1.333 1.333 0 0 0 -1.333 1.333 v 0"/><path d="M 9.333 6.667 V 2.667 a 1.333 1.333 0 0 0 -1.333 -1.333 v 0 a 1.333 1.333 0 0 0 -1.333 1.333 v 1.333"/><path d="M 6.667 7 V 4 a 1.333 1.333 0 0 0 -1.333 -1.333 v 0 a 1.333 1.333 0 0 0 -1.333 1.333 v 5.333"/><path d="M 12 5.333 a 1.333 1.333 0 1 1 2.667 0 v 4 a 5.333 5.333 0 0 1 -5.333 5.333 h -1.333 c -1.867 0 -3 -0.573 -3.993 -1.56 l -2.4 -2.4 a 1.333 1.333 0 0 1 1.887 -1.88 L 4.667 10"/>',
  shieldCheck: '<path d="M 13.333 8.667 c 0 3.333 -2.333 5 -5.107 5.967 a 0.667 0.667 0 0 1 -0.447 -0.007 C 5 13.667 2.667 12 2.667 8.667 V 4 a 0.667 0.667 0 0 1 0.667 -0.667 c 1.333 0 3 -0.8 4.16 -1.813 a 0.78 0.78 0 0 1 1.013 0 C 9.673 2.54 11.333 3.333 12.667 3.333 a 0.667 0.667 0 0 1 0.667 0.667 z"/><path d="m 6 8 1.333 1.333 2.667 -2.667"/>',
  camera: '<path d="M5.6 4.4 6.4 3h3.2l.8 1.4h2.4a1.2 1.2 0 0 1 1.2 1.2v6.2a1.2 1.2 0 0 1-1.2 1.2H3.2a1.2 1.2 0 0 1-1.2-1.2V5.6a1.2 1.2 0 0 1 1.2-1.2Z"/><circle cx="8" cy="8.2" r="2.6"/>',
  download: '<path d="M8 3.2v7.6M4.8 7.6 8 10.8l3.2-3.2"/><path d="M3.4 12.8h9.2"/>',
  chevronLeft: '<path d="M9.6 4.6 6.2 8l3.4 3.4"/>',
  arrowLeft: '<path d="M13 8H3.4"/><path d="M6.8 4.6 3.4 8l3.4 3.4"/>',
  arrowRight: '<path d="M3 8h9.6"/><path d="M9.2 4.6 12.6 8l-3.4 3.4"/>',
  external: '<path d="M12.6 9.2v2.8a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V4.4a1 1 0 0 1 1-1h2.8"/><path d="M9.2 2.6H13.4V6.8"/><path d="M13.2 2.8 7.4 8.6"/>',
  paperclip: '<path d="m 14.293 7.367 -6.127 6.127 a 4 4 0 0 1 -5.66 -5.66 l 5.713 -5.713 A 2.667 2.667 0 1 1 12 5.893 l -5.727 5.713 a 1.333 1.333 0 0 1 -1.887 -1.887 l 5.66 -5.653"/>',
  eye:
    '<path d="M1.6 8s2.3-4.5 6.4-4.5S14.4 8 14.4 8s-2.3 4.5-6.4 4.5S1.6 8 1.6 8Z"/><circle cx="8" cy="8" r="1.9"/>',
  shieldAlert: '<path d="M 13.333 8.667 c 0 3.333 -2.333 5 -5.107 5.967 a 0.667 0.667 0 0 1 -0.447 -0.007 C 5 13.667 2.667 12 2.667 8.667 V 4 a 0.667 0.667 0 0 1 0.667 -0.667 c 1.333 0 3 -0.8 4.16 -1.813 a 0.78 0.78 0 0 1 1.013 0 C 9.673 2.54 11.333 3.333 12.667 3.333 a 0.667 0.667 0 0 1 0.667 0.667 z"/><path d="M 8 5.333 v 2.667"/><path d="M 8 10.667 h 0.007"/>',
};

/** 统一规格的字形组件：<Icon name="sidebar" :size="14" />。缺省 14
 * （尺寸阶梯 12/14/16 三档 + 发送键 18 特例，标准 §二⑪；原缺省 15 按
 * 勾⑦ 吸附归 14）。 */
export const Icon = defineComponent({
  name: "Icon",
  props: {
    name: { type: String as PropType<IconName>, required: true },
    size: { type: Number, default: 14 },
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
