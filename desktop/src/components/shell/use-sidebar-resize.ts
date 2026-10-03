// 侧栏拖宽交互（UI shell doc §2.3，v3.9 修订）：指针（setPointerCapture +
// rAF 节流，只在 240–520 内自由调宽——拖到最小值钳制停住，不触发折叠）+
// 键盘（←/→ 16px、Home/End 两端）+ 双击复位；拖拽中根节点
// data-sidebar-resizing 全局锁光标禁选（style.css 基线层）。折叠只经折叠
// 按钮 / Ctrl+B 显式触发；宽度状态与持久化在 shell store。
import { onBeforeUnmount, ref, useTemplateRef } from "vue";
import { useShellStore } from "../../stores/shell";
import { SIDEBAR_MAX, SIDEBAR_MIN } from "../../lib/sidebar-preference";

export function useSidebarResize() {
  const shell = useShellStore();
  // 模板 ref="grip" 经名字绑定（Vue 3.5 useTemplateRef）；指针捕获需要元素
  const grip = useTemplateRef<HTMLElement | null>("grip");
  const dragging = ref(false);
  let dragStartX = 0;
  let dragStartWidth = 0;
  let raf = 0;
  let pendingX = 0;

  function rootResizing(on: boolean) {
    if (on) document.documentElement.dataset.sidebarResizing = "true";
    else delete document.documentElement.dataset.sidebarResizing;
  }

  /** 实时上限 = min(520, 视口 − 320)（§6.4 clamp 的动态项）。 */
  function maxEffective(): number {
    return Math.min(SIDEBAR_MAX, window.innerWidth - 320);
  }

  function onPointerDown(e: PointerEvent) {
    if (shell.sidebarCollapsed) return;
    grip.value?.setPointerCapture(e.pointerId);
    dragging.value = true;
    dragStartX = e.clientX;
    dragStartWidth = shell.sidebarWidth;
    rootResizing(true);
  }

  function applyDrag() {
    raf = 0;
    const desired = dragStartWidth + (pendingX - dragStartX); // 手柄在右缘：右拖 = 变宽
    // 低于最小值钳制停住（setSidebarWidth 内也钳）——拖拽不得隐藏侧栏（issue #1）
    shell.setSidebarWidth(desired);
  }

  function onPointerMove(e: PointerEvent) {
    if (!dragging.value) return;
    pendingX = e.clientX;
    if (!raf) raf = requestAnimationFrame(applyDrag); // 每帧至多一次渲染
  }

  function endDrag(commit: boolean) {
    if (!dragging.value) return;
    dragging.value = false;
    if (raf) cancelAnimationFrame(raf);
    raf = 0;
    rootResizing(false);
    if (commit) shell.commitSidebarWidth();
  }

  function onPointerUp(e: PointerEvent) {
    endDrag(true);
    grip.value?.releasePointerCapture(e.pointerId);
  }

  function onKeyDown(e: KeyboardEvent) {
    const step = 16;
    let next: number | null = null;
    if (e.key === "ArrowLeft") next = shell.sidebarWidth - step;
    if (e.key === "ArrowRight") next = shell.sidebarWidth + step;
    if (e.key === "Home") next = SIDEBAR_MIN;
    if (e.key === "End") next = maxEffective();
    if (next !== null) {
      e.preventDefault();
      shell.setSidebarWidth(next);
      shell.commitSidebarWidth();
    }
  }

  /** 双击 = 复位缺省 275（§2.3）。 */
  function onDoubleClick() {
    shell.setSidebarWidth(275);
    shell.commitSidebarWidth();
  }

  onBeforeUnmount(() => {
    endDrag(false);
    rootResizing(false);
  });

  return {
    dragging,
    maxEffective,
    onPointerDown,
    onPointerMove,
    onPointerUp,
    endDrag,
    onKeyDown,
    onDoubleClick,
  };
}
