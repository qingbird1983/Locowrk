/**
 * 三栏响应式断点常量（UI shell §3.40「非对称级联折叠」，v2.109）。
 * 独立 leaf 模块：composable（use-responsive-shell）与 store（work-panel
 * 的开意图视口钳制，v2.110）同吃一份，避免 store → components 反向依赖。
 */
export const BREAKPOINT_XL = 1400;
export const BREAKPOINT_LG = 1100;
export const BREAKPOINT_SM = 800;
