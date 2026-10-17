// 台账 T10 的浮层收口件：`escHandler` 判定 + `useDismissOnEsc` 注册面 +
// `dismissHandlers` 的 Esc 半边必须与它同出处。
//
// 这一簇四条各抓一层，彼此不重叠：
//  ① Escape 才关、别的键不关 ⇒ 抓"把任意 keydown 当收起"；
//  ② 注册面挂上就生效、卸载后必须摘干净 ⇒ 抓"卸载后监听器还在 = 关了一个不存在的层"
//     （这是本文件唯一必须 mount 的用例：`onMounted` 脱离组件实例不会执行）；
//  ③ `dismissHandlers`（族 A）的 Esc 半边行为与 `escHandler` 一致 ⇒ 抓"两处各写一份
//     Escape 判定然后漂开"；
//  ④ 关着的时候按 Esc 不许产生第二次"关" ⇒ 阴性对照，防把成对判据写成恒真。
import { describe, expect, it } from "vitest";
import { defineComponent, h, ref } from "vue";

import { dismissHandlers, escHandler, useDismissOnEsc } from "../src/lib/use-popover";

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
