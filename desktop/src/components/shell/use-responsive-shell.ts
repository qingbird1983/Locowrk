/**
 * 三栏响应式断点源（UI shell §3.40，v2.109「非对称级联折叠」）：监听窗口
 * 宽并把三个视口旗标写入 shell store——**只读窗口，不写任何持久化状态**：
 * 断点级联永不覆盖用户的手动折叠/展开意图（§3.40②·5，手动位只在用户
 * 显式操作时翻转）。断点：
 * - ≥1400（xl）三栏可全开（右栏仍需手动展开——§3.40②·3 非对称裁定）
 * - ≥1100（lg）右栏级联折叠，左栏照常
 * - ≥800（md） 左栏也级联折叠
 * - <800（sm） 窗口过窄提示层（应用 minWidth 1024 下仅浏览器开发态可达）
 */
import { onBeforeUnmount, onMounted } from "vue";
import { useShellStore } from "../../stores/shell";

export const BREAKPOINT_XL = 1400;
export const BREAKPOINT_LG = 1100;
export const BREAKPOINT_SM = 800;

export function useResponsiveShell() {
  const shell = useShellStore();
  let raf = 0;
  let pendingW = 0;

  function apply() {
    raf = 0;
    shell.applyViewportWidth(pendingW);
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
