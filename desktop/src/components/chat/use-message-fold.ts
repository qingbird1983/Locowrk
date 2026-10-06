/**
 * 长消息折叠状态机（组件规格「Line Clamp with Fade Mask」，MessageRow
 * 专用）：超过 N 行（设置 chat.foldLines，3–20 缺省 5）收起——max-height
 * 钉行高 × N + 渐变遮罩 + 文字下方圆形展开钮。要点：
 * - **覆盖面（v2.101 用户裁定）**：只折用户消息（主聊天的与派给子代理
 *   的任务提示都算）——模型输出（主聊天正文与子代理工作显示）恒完整
 *   展示，不接本状态机；设置开关关闭同样整体旁路；
 * - max-height 是唯一可过渡的收口（height:auto 不能过渡）；transition 只
 *   在动画期挂（animating）——none 不是可插值值，常态挂着会在挂载/收束跳变；
 * - 展开动画结束即解除上限（超长消息钉死 2000px 类近似会截断，落实测全高）；
 * - 测量：clamp 高度变了先落样式再测（同拍测的是未夹紧的布局），折叠态
 *   比对 scrollHeight 与 clientHeight（容差 2px 吃亚像素取整）；
 * - 编辑态不折：强制展开，退出编辑才折回。
 * 复制不受影响：恒复制全文（MessageRow 职责）。
 */
import {
  computed,
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
  watch,
  type Ref,
} from "vue";
import { useSettingsStore } from "../../stores/settings";

/** = --dur-base（style.css 动效三档）；超时兜底在 reduced-motion 下接住
 * 不触发的过渡收尾。 */
const ANIM_MS = 200;

export function useMessageFold(
  contentEl: Ref<HTMLElement | null>,
  editing: Ref<boolean>,
) {
  const settings = useSettingsStore();
  const expanded = ref(false);
  const overflowing = ref(false);
  const clampPx = ref(0);
  const animH = ref<string | null>(null);
  const animating = ref(false);
  let animTimer: number | null = null;

  const foldOn = computed(() => settings.view?.chat.foldLongMessages ?? true);
  const foldLines = computed(() => settings.view?.chat.foldLines ?? 5);
  /** 展开判定：用户点开 / 编辑中 / 折叠开关关闭。 */
  const isOpen = computed(
    () => !foldOn.value || expanded.value || editing.value,
  );

  function lineHeightOf(el: HTMLElement): number {
    const lh = Number.parseFloat(getComputedStyle(el).lineHeight);
    if (Number.isFinite(lh) && lh > 0) return lh;
    const fs = Number.parseFloat(getComputedStyle(el).fontSize);
    return (Number.isFinite(fs) ? fs : 14) * 1.55;
  }

  function remeasure() {
    const el = contentEl.value;
    if (!el) return;
    if (!foldOn.value) {
      overflowing.value = false;
      return;
    }
    const px = Math.ceil(lineHeightOf(el) * foldLines.value);
    if (px !== clampPx.value) {
      clampPx.value = px;
      void nextTick(remeasure);
      return;
    }
    if (isOpen.value) return;
    overflowing.value = el.scrollHeight > el.clientHeight + 2;
  }

  const foldStyle = computed(() => {
    if (animH.value) return { maxHeight: animH.value };
    if (!foldOn.value || isOpen.value || !clampPx.value) return undefined;
    return { maxHeight: `${clampPx.value}px` };
  });

  const showFold = computed(
    () => foldOn.value && overflowing.value && !editing.value,
  );

  function settle() {
    animH.value = null;
    animating.value = false;
    animTimer = null;
  }

  /** 展 → 折（live 收束同路）：上限先钉当前高度（同值无跳变），下一帧
   * 落 N 行过渡；is-anim 在超时收尾后才摘，保证过渡走完。 */
  function collapseAnim(el: HTMLElement) {
    animating.value = true;
    animH.value = `${el.scrollHeight}px`;
    void nextTick(() => {
      requestAnimationFrame(() =>
        requestAnimationFrame(() => {
          animH.value = null;
        }),
      );
    });
    animTimer = window.setTimeout(settle, ANIM_MS + 60);
  }

  function onToggleFold() {
    const el = contentEl.value;
    if (!el) return;
    if (animTimer !== null) window.clearTimeout(animTimer);
    if (!isOpen.value) {
      // 折 → 展：上限钉到实测全高过渡，结束后解除（max-height none）
      animating.value = true;
      animH.value = `${el.scrollHeight}px`;
      expanded.value = true;
      animTimer = window.setTimeout(settle, ANIM_MS + 60);
    } else {
      expanded.value = false;
      collapseAnim(el);
    }
  }

  let ro: ResizeObserver | null = null;
  watch(contentEl, (el, prev) => {
    if (prev && ro) ro.unobserve(prev);
    if (el && ro) ro.observe(el);
    if (el) remeasure();
  });
  onMounted(() => {
    ro = new ResizeObserver(remeasure);
    if (contentEl.value) ro.observe(contentEl.value);
    remeasure();
  });
  onBeforeUnmount(() => {
    if (animTimer !== null) window.clearTimeout(animTimer);
    ro?.disconnect();
  });

  watch([foldLines, foldOn], () => remeasure());

  return { isOpen, overflowing, showFold, foldStyle, animating, onToggleFold };
}
