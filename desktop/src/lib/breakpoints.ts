/**
 * 三栏响应式断点常量（UI shell §3.40，v2.112 用户口述定稿：**收缩看级联、
 * 展开看档位、400 是硬限位不是警告位**）。独立 leaf 模块：composable 与两个
 * store（shell / work-panel）同吃一份，避免 store → components 反向依赖。
 */
/** 右栏级联线：跌破即收 + 开意图消费（回宽永不自动弹回——非对称裁定） */
export const BREAKPOINT_XL = 1400;
/** 左栏级联线：跌破瞬态折叠（不落盘），回升自动展开 */
export const BREAKPOINT_LG = 1100;
/** overlay 带顶：跌破进浮层档（两栏全收，展开 = 悬浮式一次一边） */
export const BREAKPOINT_MD = 800;
/** overlay 带底 = 应用 minWidth（tauri.conf 同值；硬限位，无提示层） */
export const BREAKPOINT_SM = 400;
