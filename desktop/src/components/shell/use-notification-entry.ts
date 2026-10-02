/**
 * 侧栏通知入口（UI shell 文档 §2.4 第 5 图标 + §3.13）的开合状态与锚定：
 * 打开时量触发钮视口坐标（Popover fixed 锚其上方），再开合。
 */
import { ref } from "vue";

export function useNotificationEntry() {
  const open = ref(false);
  const btn = ref<HTMLButtonElement | null>(null);
  const anchor = ref({ x: 0, y: 0 });

  /** 模板函数引用（`:ref="setBtn"`）——按钮元素由此进来。 */
  function setBtn(el: unknown) {
    btn.value = el as HTMLButtonElement | null;
  }

  function toggle() {
    if (open.value) {
      open.value = false;
      return;
    }
    const r = btn.value?.getBoundingClientRect();
    if (r) anchor.value = { x: r.left, y: r.top };
    open.value = true;
  }

  return { open, anchor, setBtn, toggle };
}
