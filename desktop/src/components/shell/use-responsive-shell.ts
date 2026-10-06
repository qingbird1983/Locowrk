/**
 * 三栏响应式断点源（UI shell §3.40，v2.109「非对称级联折叠」）：监听窗口
 * 宽并把三个视口旗标写入 shell store。左栏手动位（manualSidebarCollapsed）
 * **永不**被断点改写——收窄折叠 ≠ 用户折叠。唯一例外是右栏开意图（§3.40②·2，
 * v2.110 目检修正）：跌破 1400px 的瞬间消费掉 workPanel.open（持久化落
 * false），回宽**不**自动展开——这是 ②·3 非对称产品裁定的硬兑现，不是
 * 疏漏，禁止改回「记忆意图、回宽恢复」。断点：
 * - ≥1400（xl）三栏可全开（右栏仍需手动展开——§3.40②·3 非对称裁定）
 * - ≥1100（lg）右栏级联折叠，左栏照常
 * - ≥800（md） 左栏也级联折叠
 * - <800（sm） 窗口过窄提示层（应用 minWidth 1024 下仅浏览器开发态可达）
 */
import { onBeforeUnmount, onMounted } from "vue";
import { useShellStore } from "../../stores/shell";
import { useWorkPanelStore } from "../../stores/work-panel";
import { BREAKPOINT_XL } from "../../lib/breakpoints";

export function useResponsiveShell() {
  const shell = useShellStore();
  const workPanel = useWorkPanelStore();
  let raf = 0;
  let pendingW = 0;

  function apply() {
    raf = 0;
    shell.applyViewportWidth(pendingW);
    // 右栏开意图被断点折叠消费（挂载首校准也走这里：窄窗口启动时清掉
    // 旧 localStorage 里的悬空 open=true）
    if (pendingW < BREAKPOINT_XL && workPanel.open) workPanel.setOpen(false);
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
