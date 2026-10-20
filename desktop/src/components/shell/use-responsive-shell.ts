/**
 * 三栏响应式断点源（UI shell §3.40，v2.112 用户口述定稿）：监听窗口宽，
 * **沿检测**驱动收缩级联——只在跨越断点线的那一刻动作，带内手动展开不被
 * 回弹（否则「按钮可用」是假的）。级联分两级 + 一条带界：
 * - 跨越 <1400：右栏真实关闭 + 开意图消费（持久化 false），回宽**永不**
 *   自动弹回——「右栏永不自动展开」非对称裁定的实现手段，不是疏漏，禁止
 *   改回「持久化意图、回宽恢复」；
 * - 跨越 <1100：左栏瞬态折叠（不落盘），回升自动解除（自动折叠自动恢复）；
 * - 跨越 <800：进 overlay 带——两栏全收（「到达 800 只剩主会话区」），
 *   右栏若在带内手动开着也一并消费；跨越 ≥800 出带：浮层态复位（左栏回
 *   级联管辖，右栏开着则变停靠——手动意图保留，非对称）。
 * 挂载首校准：清窄窗启动的悬空面板开意图（v2.110 语义）。
 */
import { onBeforeUnmount, onMounted } from "vue";
import { useShellStore } from "../../stores/shell";
import { useWorkPanelStore } from "../../stores/work-panel";
import { BREAKPOINT_LG, BREAKPOINT_MD, BREAKPOINT_XL } from "../../lib/breakpoints";

export function useResponsiveShell() {
  const shell = useShellStore();
  const workPanel = useWorkPanelStore();
  let raf = 0;
  let pendingW = 0;
  let prevW = 0;

  function apply() {
    raf = 0;
    const w = pendingW;
    shell.applyViewportWidth(w);
    // 挂载首校准（prevW=0 不触发沿）：窄窗启动清悬空面板开意图
    if (prevW === 0) {
      if (w < BREAKPOINT_XL && workPanel.open) workPanel.setOpen(false);
      prevW = w;
      return;
    }
    // 沿：右栏级联（跨 1400 即收 + 意图消费）
    if (prevW >= BREAKPOINT_XL && w < BREAKPOINT_XL && workPanel.open) {
      workPanel.setOpen(false);
    }
    // 沿：左栏级联（跨 1100 瞬态折叠 / 回升自动展开）
    if (prevW >= BREAKPOINT_LG && w < BREAKPOINT_LG) shell.sidebarAutoFold = true;
    else if (prevW < BREAKPOINT_LG && w >= BREAKPOINT_LG) shell.sidebarAutoFold = false;
    // 沿：overlay 带界（进带两栏全收——右栏带内开着也消费；出带浮层态复位）
    if (prevW >= BREAKPOINT_MD && w < BREAKPOINT_MD) {
      if (workPanel.open) workPanel.setOpen(false);
      shell.resetOverlayState();
    } else if (prevW < BREAKPOINT_MD && w >= BREAKPOINT_MD) {
      shell.resetOverlayState();
    }
    prevW = w;
  }
  function onResize() {
    pendingW = window.innerWidth;
    if (!raf) raf = requestAnimationFrame(apply);
  }

  onMounted(() => {
    onResize(); // 挂载即校准一次（含首帧前把旗标对齐真实窗口）
    window.addEventListener("resize", onResize);
  });
  onBeforeUnmount(() => {
    window.removeEventListener("resize", onResize);
    if (raf) cancelAnimationFrame(raf);
  });
}
