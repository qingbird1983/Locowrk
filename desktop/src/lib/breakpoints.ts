/**
 * 三栏响应式断点常量（UI shell §3.40，v2.111 V2 规格：**断点只决定展开模式
 * push/overlay，永不决定按钮可用性**）。独立 leaf 模块：composable
 * （use-responsive-shell）与两个 store（shell / work-panel 的互斥收口）同吃
 * 一份，避免 store → components 反向依赖。
 */
/** 左栏自动展开阈值（>LG 自动展开；v2.111 起左栏唯一自动折叠 = overlay
 * 互斥瞬态旗标，在 MD 界解除时即恢复，故本值只作文档锚点、无独立代码路径） */
export const BREAKPOINT_LG = 1100;
/** push ↔ overlay 模式界 = 右栏开意图消费点（§3.40②·2） */
export const BREAKPOINT_MD = 800;
/** 过窄界 = 应用 minWidth（tauri.conf 同值） */
export const BREAKPOINT_SM = 400;
