// 浮层「外点关 + Esc 关」的收口件（《CODE-AUDIT-v1》C12 / 台账 T10）。
// 规范出处：《Front-end Design Guidelines》§二③「一律 Esc + 外点自持」+ UI shell doc
// §五「小浮层不挂遮罩」——不挂遮罩就得自己听 document。
//
// 这个文件只抽**逐字同形的那两半**，不抽差异：
//   ① Esc 的判定本身 = `escHandler`（各站点原本各写一份 `if (e.key === "Escape")"`）；
//   ② 「一次 Esc 该由谁吃」= `anyLayerOpen` / `useLayerPriority`（台账 T10 的 (a) 路线，
//      2026-10-18 定）。先有②才有①的普及：`Composer.vue` 的输入框 Esc 链原本只认识
//      `showModels` 一个浮层，别的浮层一补 Esc 就变成"按一次键关一层 + 顺手拒一次审批"
//      ⇒ 不可逆动作进了误触路径。现在链上改问"有没有开着自持 Esc 的浮层"，有就让位，
//      这条让位判定只有一处，站点每多一个都不必再改输入框。
//
// **没有**给族 A 加 `capture?` / `onDismiss?` / `escapeGate?` 那种开关：「关掉谁」的所有权
// （写本地 ref / `emit("close")` / 遮罩 `self`）、外点挂捕获还是冒泡、trigger 要不要豁免
// ——都是真语义差，把差异从十处搬进一处是搬家不是收敛（标准 §七 已登记这三种写法并存）。
// 本文件唯一对外承诺的形状 = 「谁自持 Esc，谁占优先级」，占位与归还都由注册面自己管，
// 站点没有"忘记归还"这个可犯的错误。
import { onBeforeUnmount, onMounted, watch, type Ref } from "vue";

/**
 * 开着且自持 Esc 的浮层数。>0 ⇒ 别的 Esc 消费者（输入框那条链）必须让位。
 *
 * 用模块级计数而不是 Pinia store：这个数只在 keydown 的一瞬被读一次，没有响应式消费者
 * （挂 store 上只会多一条"store 里有个没人订阅的字段"）。
 */
let openLayers = 0;

/** 输入框那条链吃的判定：有没有浮层正开着、准备吃掉这一键。 */
export function anyLayerOpen(): boolean {
  return openLayers > 0;
}

/**
 * 占位/归还（**内部件**：只由下面两个注册面调用，站点不许自己加减）。
 *
 * `open` 省略 = 本组件由父 `v-if` 渲染、挂载即打开（`ModelSelector` / `NotificationPopover`
 * / `DefaultModelMenu` 那一族），整段挂载期占位；传了就只在开着时占位——这两种归属是真
 * 语义差，但差的是"什么时候算开着"，不是另一套计数器。
 * 卸载无条件归还：浮层开着时被父拆掉（切页、`v-if` 条件变假）是最容易漏还的形状，
 * 而这里漏一次的后果是**输入框的 Esc 永久失效**（计数只增不减 ⇒ `anyLayerOpen()` 恒真），
 * 所以 `useLayerPriority` 的归还面由 `popover-esc.spec.ts` 单独盯一条。
 */
function useLayerPriority(open?: Ref<boolean>) {
  let claimed = false;
  function set(v: boolean) {
    if (v && !claimed) {
      claimed = true;
      openLayers += 1;
    } else if (!v && claimed) {
      claimed = false;
      openLayers -= 1;
    }
  }
  // flush:sync —— 关掉的那一帧就归还，不留"已关但还占着优先级"的一帧窗口
  if (open) watch(open, (v) => set(v), { flush: "sync" });
  onMounted(() => set(open ? open.value : true));
  onBeforeUnmount(() => set(false));
}

/**
 * Esc 关层判定的**唯一出处**（标准 §二③ 那句"一律 Esc"里"怎么算按了 Esc"）。
 *
 * `isOpen` 缺省为真 = 挂载即打开的那族不需要再判一次。判定与门控写在一处是为了让
 * "开着才吃"这条纪律不可能被站点漏掉——漏掉的后果不是"关不掉"而是"关了却不占优先级"，
 * 那就退回 T10 修掉前的双动作。
 */
export function escHandler(close: () => void, isOpen: () => boolean = () => true) {
  return (e: KeyboardEvent) => {
    if (e.key === "Escape" && isOpen()) close();
  };
}

/**
 * 两个 document 监听器的行为体，单独导出是为了**可测**：判据要在纯 Node 里把事件派发
 * 进去跑轨迹，而 `onMounted` 脱离组件实例不会执行。注册面本身由 `SS9/SS9b`
 * （`scripts/check-single-source.mjs`）+ 变异验证兜。
 *
 * `root.value` 为 null 时按「点在了外面」处理（`?.` 给出 undefined，`?? false` 落成
 * false，取反即真）——这不是笔误：UsageRing 的锚点挂在 `v-if` 上，浮层开着而锚点被摘掉时
 * 就该关。
 */
export function dismissHandlers(root: Ref<HTMLElement | null>, open: Ref<boolean>) {
  function onDocPointerDown(e: PointerEvent) {
    if (!open.value) return;
    if (!(root.value?.contains(e.target as Node) ?? false)) open.value = false;
  }
  // Esc 半边复用 escHandler：同一判定只许一个出处
  const onDocKeydown = escHandler(() => {
    open.value = false;
  }, () => open.value);
  return { onDocPointerDown, onDocKeydown };
}

/**
 * 只补 Esc 的注册面。这些站点的**外点**各有真实语义差（capture 阶段、trigger 也要豁免、
 * 关掉要 `emit("close")`），不收进族 A 那个形状；缺的是另一半，由这里给——注册与摘除同
 * 一个引用，摘不掉的情况不存在。
 *
 * `open` 省略 = 挂载即打开（父 `v-if` 渲染的浮层）。有本地开关的站点必须传，否则一个
 * 常驻组件会长期占着优先级，把输入框的 Esc 整条链堵死。
 */
export function useDismissOnEsc(close: () => void, open?: Ref<boolean>) {
  useLayerPriority(open);
  const h = escHandler(close, open ? () => open.value : undefined);
  onMounted(() => document.addEventListener("keydown", h));
  onBeforeUnmount(() => document.removeEventListener("keydown", h));
}

/**
 * 族 A 入口：挂载时挂两个 document 监听（冒泡阶段，与合并前四处一致），卸载时摘掉。
 * add/remove 引用同一个 `h` 上的方法，摘不掉的情况不存在。
 */
export function useDismissOnOutside(root: Ref<HTMLElement | null>, open: Ref<boolean>) {
  useLayerPriority(open);
  const h = dismissHandlers(root, open);
  onMounted(() => {
    document.addEventListener("pointerdown", h.onDocPointerDown);
    document.addEventListener("keydown", h.onDocKeydown);
  });
  onBeforeUnmount(() => {
    document.removeEventListener("pointerdown", h.onDocPointerDown);
    document.removeEventListener("keydown", h.onDocKeydown);
  });
}
