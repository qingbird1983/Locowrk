/**
 * 三栏响应式断点源（UI shell §3.40，v2.111 V2 规格）：监听窗口宽把视口
 * 旗标（overlayMode/tooNarrow）写入 shell store，**永不碰任何持久化状态**
 * ——按钮可用性与断点解耦（§3.40 开头评审尺度：限制不碰「能不能用」）。
 * 只做三件模式事：
 * 1. 写旗标（overlay 400–799 / 过窄 <400）；
 * 2. 入 overlay 时双开保左收右——右栏 open 落 false **持久化**（开意图
 *    消费，§3.40②·2），回宽**不**自动弹回——这是「右栏永不自动展开」
 *    非对称裁定（②·3）在 800 界的实现手段，不是疏漏，禁止改回
 *    「记忆意图、回宽恢复」；
 * 3. 出 overlay 时解除左栏互斥瞬态折叠（自动折叠自动恢复，②·1）。
 * v2.109 的级联折叠（<1400 收右栏、<1100 收左栏）与挂载清悬空 open 退役
 * ——open 在任何宽度都是合法状态。
 */
import { onBeforeUnmount, onMounted } from "vue";
import { useShellStore } from "../../stores/shell";
import { useWorkPanelStore } from "../../stores/work-panel";
import { BREAKPOINT_MD } from "../../lib/breakpoints";

export function useResponsiveShell() {
  const shell = useShellStore();
  const workPanel = useWorkPanelStore();
  let raf = 0;
  let pendingW = 0;

  function apply() {
    raf = 0;
    const w = pendingW;
    shell.applyViewportWidth(w);
    if (w < BREAKPOINT_MD) {
      // 入 overlay（挂载首校准同规）：双开保左收右，右栏开意图被消费
      if (workPanel.open && !shell.sidebarCollapsed) workPanel.setOpen(false);
    } else if (shell.overlaySidebarFold) {
      // 出 overlay：互斥瞬态折叠解除（左栏自动恢复——「之前是自动折叠的」）
      shell.setPanelExclusiveFold(false);
    }
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
