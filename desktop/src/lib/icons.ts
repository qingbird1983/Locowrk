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
  | "hand"
  | "shieldCheck"
  | "shieldAlert"
  | "paperclip";

/** 原型 sprite 的 symbol 内联内容（stroke 属性由外壳 svg 统一给）。
 * 带 <g transform="scale(.6667)"> 的是 lucide 24-viewBox 原路径缩进 16 格
 * （描边 2.25×0.6667 ≈ 1.5，视觉重量与本表原生路径一致）。 */
const PATHS: Record<IconName, string> = {
  sparkle: '<path d="M8 2.6 9.1 6.3 12.8 7.4 9.1 8.5 8 12.2 6.9 8.5 3.2 7.4 6.9 6.3Z"/>',
  chevronDown: '<path d="M4.6 6.4 8 9.8 11.4 6.4"/>',
  chevronRight: '<path d="M6.4 4.6 9.8 8 6.4 11.4"/>',
  plus: '<path d="M8 3.4v9.2M3.4 8h9.2"/>',
  search: '<circle cx="7.1" cy="7.1" r="4"/><path d="M10.1 10.1 13.4 13.4"/>',
  panel: '<rect x="2.4" y="3.2" width="11.2" height="9.6" rx="2"/><path d="M10 3.2v9.6"/>',
  sidebar: '<rect x="2.4" y="3.2" width="11.2" height="9.6" rx="2"/><path d="M6 3.2v9.6"/>',
  settings:
    '<g transform="scale(0.6667)" stroke-width="2.25"><path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"/><circle cx="12" cy="12" r="3"/></g>',
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
  stop: '<rect x="5.2" y="5.2" width="5.6" height="5.6" rx="1.2" fill="currentColor" stroke="none"/>',
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
  folderOpen:
    '<g transform="scale(0.6667)" stroke-width="2.25"><path d="m6 14 1.5-2.9A2 2 0 0 1 9.24 10H20a2 2 0 0 1 1.94 2.5l-1.54 6a2 2 0 0 1-1.95 1.5H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h3.9a2 2 0 0 1 1.69.9l.81 1.2a2 2 0 0 0 1.67.9H18a2 2 0 0 1 2 2v2"/></g>',
  filter: '<path d="M2.8 3.6h10.4L9.6 8.1v3.7l-3.2 1.6V8.1Z"/>',
  bulb:
    '<g transform="scale(0.6667)" stroke-width="2.25"><path d="M15 14c.2-1 .7-1.7 1.5-2.5 1-.9 1.5-2.2 1.5-3.5A6 6 0 0 0 6 8c0 1 .2 2.2 1.5 3.5.7.7 1.3 1.5 1.5 2.5"/><path d="M9 18h6"/><path d="M10 22h4"/></g>',
  hand:
    '<g transform="scale(0.6667)" stroke-width="2.25"><path d="M18 11V6a2 2 0 0 0-2-2v0a2 2 0 0 0-2 2v0"/><path d="M14 10V4a2 2 0 0 0-2-2v0a2 2 0 0 0-2 2v2"/><path d="M10 10.5V6a2 2 0 0 0-2-2v0a2 2 0 0 0-2 2v8"/><path d="M18 8a2 2 0 1 1 4 0v6a8 8 0 0 1-8 8h-2c-2.8 0-4.5-.86-5.99-2.34l-3.6-3.6a2 2 0 0 1 2.83-2.82L7 15"/></g>',
  shieldCheck:
    '<g transform="scale(0.6667)" stroke-width="2.25"><path d="M20 13c0 5-3.5 7.5-7.66 8.95a1 1 0 0 1-.67-.01C7.5 20.5 4 18 4 13V6a1 1 0 0 1 1-1c2 0 4.5-1.2 6.24-2.72a1.17 1.17 0 0 1 1.52 0C14.51 3.81 17 5 19 5a1 1 0 0 1 1 1z"/><path d="m9 12 2 2 4-4"/></g>',
  paperclip:
    '<g transform="scale(0.6667)" stroke-width="2.25"><path d="m21.44 11.05-9.19 9.19a6 6 0 0 1-8.49-8.49l8.57-8.57A4 4 0 1 1 18 8.84l-8.59 8.57a2 2 0 0 1-2.83-2.83l8.49-8.48"/></g>',
  shieldAlert:
    '<g transform="scale(0.6667)" stroke-width="2.25"><path d="M20 13c0 5-3.5 7.5-7.66 8.95a1 1 0 0 1-.67-.01C7.5 20.5 4 18 4 13V6a1 1 0 0 1 1-1c2 0 4.5-1.2 6.24-2.72a1.17 1.17 0 0 1 1.52 0C14.51 3.81 17 5 19 5a1 1 0 0 1 1 1z"/><path d="M12 8v4"/><path d="M12 16h.01"/></g>',
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
