// 浮层「外点关 + Esc 关」唯一实现（《CODE-AUDIT-v1》C12 族 A）。
// 规范出处：UI shell doc §五「小浮层不挂遮罩」——不挂遮罩就得自己听 document。
//
// 射程只有族 A（逐字同形的四处：AttachButton / PermissionModePicker /
// UsageRing / WorkspacePicker）：document **冒泡**阶段、关掉 = 置本地那个 ref。
// 审计登记说这条有「10 处」，逐组件取证后实测分三族，族 B/C 有意**不收**：
//   族 B（SubagentsMenu / SettingsSelect / DefaultModelMenu / NotificationPopover /
//   RowMenu）= pop 与 trigger 两个 ref 各自豁免 + `capture` 阶段 + 关掉是
//   `emit("close")`（close 的所有权在父组件），其中两处原来只挂 pointerdown 不挂 Esc
//   ——那是**缺**而不是差异（标准 §二③ 明写「收口：一律 Esc + 外点自持」），
//   台账 T10 已补齐：补齐用的是下面那个 `useDismissOnEsc`，不是给族 A 加开关；
//   族 C（对话框 saving 中不关 / Composer 的 Esc 要让位给 isComposing 与既有 Esc 链）
//   = Esc 分支带条件。
// 一个 composable 若同时吃掉 A/B/C 就得带一组开关（capture?: boolean、
// onDismiss?: () => void、escapeGate?: () => boolean…）= 把差异从十处搬进一处，
// 那是搬家不是收敛。capture 与冒泡的时机差、"谁拥有 close"都是真语义差，不许拧。
// 唯一被抽出来共用的是 **Esc 判定本身**（`escHandler`）：它在三族里逐字同形，
// 不同的只是"关掉谁"——那正好是一个回调参数的射程。
import { onBeforeUnmount, onMounted, type Ref } from "vue";

/**
 * Esc 关层判定的**唯一出处**（台账 T10 补齐标准 §二③ 那句"一律 Esc + 外点自持"）。
 *
 * 族 A 与族 B/C 在"怎么关"上确实不同——写本地那个 ref 还是 `emit("close")`
 * （close 的所有权在父组件，父用 `v-if` 渲染浮层）——那是真语义差，不许拧；
 * 但"document keydown + `key === "Escape"` + 卸载摘同引用"这一半在三处逐字同形，
 * 而补齐标准债的办法不能是"每个组件再抄一遍 `if (e.key === "Escape")"`"，
 * 那只是把同一件事的出处从一处变成四处。
 *
 * 纯函数、吃一个 `close` 回调：与 `dismissHandlers` 同理，判据要在纯 Node 里
 * 把事件派发进去跑轨迹（`onMounted` 脱离组件实例不会执行）。
 */
export function escHandler(close: () => void) {
  return (e: KeyboardEvent) => {
    if (e.key === "Escape") close();
  };
}

/**
 * 两个 document 监听器的行为体，单独导出是为了**可测**：本仓没有前端单测
 * runner，判据（tmp/v174-equiv.mts）要在纯 Node 里把事件派发进去跑轨迹，而
 * `onMounted` 脱离组件实例不会执行。注册面本身由静态断言 + 变异验证兜。
 *
 * `root.value` 为 null 时按「点在了外面」处理（`?.` 给出 undefined，`?? false`
 * 落成 false，取反即真）——这不是笔误：UsageRing 的锚点挂在 `v-if` 上，浮层
 * 开着而锚点被摘掉时就该关。
 */
export function dismissHandlers(root: Ref<HTMLElement | null>, open: Ref<boolean>) {
  function onDocPointerDown(e: PointerEvent) {
    if (!open.value) return;
    if (!(root.value?.contains(e.target as Node) ?? false)) open.value = false;
  }
  // Esc 半边复用 escHandler：同一判定只许一个出处
  const onDocKeydown = escHandler(() => {
    if (open.value) open.value = false;
  });
  return { onDocPointerDown, onDocKeydown };
}

/**
 * 只补 Esc 的注册面（标准 §二③ 收口要求，族 B/C 用）。这些站点的**外点**各有
 * 真实语义差（capture 阶段、trigger 也要豁免、关掉要 `emit("close")`），不收进
 * 族 A 那个形状；缺的是另一半，由这里给——注册与摘除同一个引用，摘不掉的情况不存在。
 */
export function useDismissOnEsc(close: () => void) {
  const h = escHandler(close);
  onMounted(() => document.addEventListener("keydown", h));
  onBeforeUnmount(() => document.removeEventListener("keydown", h));
}

/**
 * 组件侧入口：挂载时挂两个 document 监听（冒泡阶段，与合并前四处一致），
 * 卸载时摘掉。add/remove 引用同一个 `h` 上的方法，摘不掉的情况不存在。
 */
export function useDismissOnOutside(root: Ref<HTMLElement | null>, open: Ref<boolean>) {
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
