// 台账 T10 的浮层收口件：`escHandler` 判定 + `useDismissOnEsc` 注册面 +
// `dismissHandlers` 的 Esc 半边必须与它同出处 + 「一次 Esc 只许一个消费者」的优先级面。
//
// 这一簇九条各抓一层，彼此不重叠：
//  ① Escape 才关、别的键不关 ⇒ 抓"把任意 keydown 当收起"；
//  ② 注册面挂上就生效、卸载后必须摘干净 ⇒ 抓"卸载后监听器还在 = 关了一个不存在的层"
//     （这是本文件唯一必须 mount 的用例：`onMounted` 脱离组件实例不会执行）；
//  ③ `dismissHandlers`（族 A）的 Esc 半边行为与 `escHandler` 一致 ⇒ 抓"两处各写一份
//     Escape 判定然后漂开"；
//  ④ 关着的时候按 Esc 不许产生第二次"关" ⇒ 阴性对照，防把成对判据写成恒真；
//  ⑤ `isOpen` 门控：关着的层不吃这一键 ⇒ 抓"常驻组件把 Esc 全吃掉"（那正是 T10 卡在
//     裁定前的那个形状——补了 Esc 却让输入框那条链永久哑掉）；
//  ⑥⑦⑧ 优先级登记：挂载即打开的形状占位到卸载 / 有 `open` ref 的形状只在开着时占位 /
//     族 A 走同一份登记 ⇒ 抓"补齐的三处用了新纪律、原本四处没跟上 = 又成两套"；
//  ⑨ 两层叠开时关掉一层，另一层必须还占着 ⇒ 抓"把计数写成布尔"（写成布尔的后果 =
//     关外层时内层还开着却不再让位，双动作回来）。
// 泄漏类断言（⑥⑦）是这条债的**承重面**：`openLayers` 只增不减 ⇒ `anyLayerOpen()` 恒真
// ⇒ 输入框的 Esc 链整条失效，而且没有任何报错。
import { describe, expect, it } from "vitest";
import { defineComponent, h, ref } from "vue";

import {
  anyLayerOpen,
  dismissHandlers,
  escHandler,
  useDismissOnEsc,
  useDismissOnOutside,
} from "../src/lib/use-popover";

/** 只在 mount 前后各读一次，用例之间不残留（每条自己 unmount 干净）。 */
function mountHost(setup: () => void) {
  return defineComponent({ setup: () => (setup(), () => h("div")) });
}

/** mount 一个只装收口件的宿主：`onMounted`/`onBeforeUnmount` 脱离组件实例不执行，
 *  所以登记与摘除这两面必须在真实实例上跑。 */
async function mountWith(setup: () => void) {
  const { mount } = await import("@vue/test-utils");
  return mount(mountHost(setup), { attachTo: document.body });
}

/** 只测**判定**：纯函数，不需要 DOM。 */
describe("escHandler", () => {
  it("只在 Escape 上关，一次一个键只关一次", () => {
    const n = ref(0);
    const h = escHandler(() => (n.value += 1));
    h(new KeyboardEvent("keydown", { key: "Escape" }));
    expect(n.value).toBe(1);
    h(new KeyboardEvent("keydown", { key: "Enter" }));
    h(new KeyboardEvent("keydown", { key: "a" }));
    expect(n.value).toBe(1);
  });
});

/** 注册面必须 mount：`onMounted`/`onBeforeUnmount` 脱离组件实例不执行。 */
describe("useDismissOnEsc", () => {
  it("挂上就生效，卸载后必须摘干净", async () => {
    const { mount } = await import("@vue/test-utils");
    const n = ref(0);
    const Host = defineComponent({
      setup() {
        useDismissOnEsc(() => (n.value += 1));
        return () => h("div");
      },
    });
    const w = mount(Host, { attachTo: document.body });
    document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    expect(n.value).toBe(1);
    w.unmount();
    // 摘不掉的形状 = 组件早没了还继续关东西，而且每 mount 一次多留一份
    document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    expect(n.value).toBe(1);
  });
});

/** 族 A 的 Esc 半边必须走同一个判定（否则"收口一律 Esc"就有两份实现）。 */
describe("dismissHandlers 的 Esc 半边", () => {
  it("开着时 Escape 关层，且关掉后再按不再产生第二次写", () => {
    const open = ref(true);
    const root = ref<HTMLElement | null>(null);
    const h = dismissHandlers(root, open);
    h.onDocKeydown(new KeyboardEvent("keydown", { key: "Enter" }));
    expect(open.value).toBe(true);
    h.onDocKeydown(new KeyboardEvent("keydown", { key: "Escape" }));
    expect(open.value).toBe(false);
    // 阴性对照：已经关着时再按 Esc，不许把 open 翻回 true
    h.onDocKeydown(new KeyboardEvent("keydown", { key: "Escape" }));
    expect(open.value).toBe(false);
  });

  it("root 还没挂上时按外点算「关」，这是 UsageRing 的 v-if 锚点形状", () => {
    const open = ref(true);
    const root = ref<HTMLElement | null>(null);
    const h = dismissHandlers(root, open);
    h.onDocPointerDown(new PointerEvent("pointerdown"));
    expect(open.value).toBe(false);
  });
});

/** ⑤ 门控：关着的层不吃这一键。这条抓的是 T10 卡在裁定前的那个形状——补了 Esc
 *  却不门控 ⇒ 常驻顶栏的组件（SubagentsMenu 就是）长期占住优先级，输入框那条
 *  Esc 链（停止 / 拒审批）整条哑掉。 */
describe("escHandler 的 isOpen 门控", () => {
  it("关着时既不 close 也不占位", () => {
    const n = ref(0);
    const open = ref(false);
    const onKey = escHandler(
      () => (n.value += 1),
      () => open.value,
    );
    onKey(new KeyboardEvent("keydown", { key: "Escape" }));
    expect(n.value).toBe(0);
    open.value = true;
    onKey(new KeyboardEvent("keydown", { key: "Escape" }));
    expect(n.value).toBe(1);
    // 门控只管"开没开"，键名照旧只认 Escape
    onKey(new KeyboardEvent("keydown", { key: "Enter" }));
    expect(n.value).toBe(1);
  });
});

/** 优先级登记面（⑥⑦⑧⑨）。判据只读得出一个布尔 `anyLayerOpen()`，所以每条都要
 *  在 mount/unmount 与翻 ref 的**前后各读一次**——只看末态会把"只增不减"读成对。 */
describe("anyLayerOpen · 挂载即打开的形状（父 v-if 渲染）", () => {
  it("挂载即占位，卸载必须归还", async () => {
    expect(anyLayerOpen()).toBe(false);
    const w = await mountWith(() => useDismissOnEsc(() => {}));
    expect(anyLayerOpen()).toBe(true);
    w.unmount();
    expect(anyLayerOpen()).toBe(false);
  });
});

describe("anyLayerOpen · 有本地开关的形状", () => {
  it("只在开着的那段占位；开着时被卸载也必须归还", async () => {
    const open = ref(false);
    expect(anyLayerOpen()).toBe(false);
    const w = await mountWith(() => useDismissOnEsc(() => {}, open));
    // 关着挂载 = 不占位（这条是 SubagentsMenu 的实形：常驻顶栏、默认关）
    expect(anyLayerOpen()).toBe(false);
    open.value = true;
    expect(anyLayerOpen()).toBe(true);
    open.value = false;
    expect(anyLayerOpen()).toBe(false);
    // 真挂出去再拆：卸载时无条件归还，漏一次就是输入框 Esc 永久失效
    open.value = true;
    w.unmount();
    expect(anyLayerOpen()).toBe(false);
  });
});

describe("anyLayerOpen · 族 A 走同一份登记", () => {
  it("`useDismissOnOutside` 开着时占位、关掉与卸载都归还", async () => {
    const open = ref(false);
    const root = ref<HTMLElement | null>(null);
    expect(anyLayerOpen()).toBe(false);
    const w = await mountWith(() => useDismissOnOutside(root, open));
    expect(anyLayerOpen()).toBe(false);
    open.value = true;
    expect(anyLayerOpen()).toBe(true);
    // 外点关掉（走真实监听器，不是直接写 ref）：优先级必须随之归还
    document.dispatchEvent(new PointerEvent("pointerdown"));
    expect(open.value).toBe(false);
    expect(anyLayerOpen()).toBe(false);
    w.unmount();
    expect(anyLayerOpen()).toBe(false);
  });
});

describe("anyLayerOpen · 两层叠开", () => {
  it("关掉一层，另一层还占着（抓「把计数写成布尔」）", async () => {
    const a = await mountWith(() => useDismissOnEsc(() => {}));
    const b = await mountWith(() => useDismissOnEsc(() => {}));
    expect(anyLayerOpen()).toBe(true);
    a.unmount();
    // 写成布尔就在这儿红：关外层时内层还开着却不再让位 ⇒ 双动作回来
    expect(anyLayerOpen()).toBe(true);
    b.unmount();
    expect(anyLayerOpen()).toBe(false);
  });
});
