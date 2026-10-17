// 台账 T10 (a) 路线的**承重面**：一次 Esc 只许有一个消费者 = 开着的那层吃掉它。
//
// 为什么必须 mount 真实 `Composer` 而不是只测 composable：这条债的形状是"两个消费者
// 抢同一键"，只有把输入框那条链（`Composer.vue` 的 Esc 链）与浮层放在同一份 DOM 里跑，
// 才看得见"让位"到底有没有发生。`popover-esc.spec.ts` 测的是登记与判定那两面（纯函数 +
// 合成宿主），它**碰不到**让位这一半——拿那边的绿灯当这边的证据是本仓明令要说破的事。
//
// 三条各抓一层：
//  ① 没有浮层时按 Esc ⇒ 链照常走到「拒绝审批」⇒ 抓"让位判定写恒真，把整条链堵死"
//     （那正是 SS9b 那条下限要防的反面：链不能死，只是要让位）；
//  ② 浮层开着时按 Esc ⇒ decide 一次都不许被调 ⇒ 本刀真正的缺陷形状（改前给这三处补
//     Esc 就会同时拒掉一次审批 = 不可逆动作进误触路径）；
//  ③ 浮层关掉后再按 ⇒ 又走到「拒绝审批」⇒ 抓"让位只生效一次"与"关掉后优先级没归还"。
// 第四条打在补齐的那一站上：`PermissionCard` 的 scope 下拉按 Esc **只关下拉**，
// 并证明它占/还了优先级（同一份 document 事件，不靠手写 ref）。
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", async () => {
  const stub = await import("./tauri-stub");
  return { invoke: stub.invoke };
});

import { nextTick, ref } from "vue";
import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";

import Composer from "../src/components/chat/Composer.vue";
import PermissionCard from "../src/components/chat/PermissionCard.vue";
import { anyLayerOpen, useDismissOnEsc } from "../src/lib/use-popover";
import { useApprovalsStore } from "../src/stores/approvals";
import { useChatStore } from "../src/stores/chat";
import { i18n } from "../src/i18n";
import type { ApprovalRequest } from "../src/types";

// happy-dom 没有 ResizeObserver，而 Composer 的坞高上报在 `onMounted` 里 `new` 一个
// （生产里它由 WebView2 提供）。桩只补"存在"，不补行为：本文件断言的是按键路由，
// 高度上报不在射程内。
(
  globalThis as unknown as { ResizeObserver?: unknown }
).ResizeObserver ??= class {
  observe(): void {}
  unobserve(): void {}
  disconnect(): void {}
};

/** 字段照 `src/types/events-agent.ts:202` 与生产事件写，不凭想象编（历批第 117 条）。
 *  `external`/`sensitive` 都不给 ⇒ 非硬门卡 ⇒ 才会渲染带 chevron 的分体主钮。 */
const req = (id = "r1"): ApprovalRequest => ({
  request_id: id,
  tool: "shell",
  args: { command: "ls" },
  side_effect: "irreversible",
  session_id: "s1",
  description: "列目录",
  timeout_secs: 0,
  workspace: "F:/w/demo",
});

/** 一个"开着自持 Esc 的浮层"：真实站点的形状（有本地开关 ⇒ 必须传 `open`）。 */
function mountLayer(openAtMount: boolean) {
  const open = ref(openAtMount);
  const w = mount(
    {
      setup: () => {
        useDismissOnEsc(
          () => {
            open.value = false;
          },
          open,
        );
        return () => null;
      },
    },
    { attachTo: document.body },
  );
  return { open, wrapper: w };
}

beforeEach(() => {
  setActivePinia(createPinia());
  i18n.global.locale.value = "zh-CN";
});

describe("Composer 的 Esc 链 · 让位给开着的浮层", () => {
  it("① 没有浮层时照常走到「拒绝审批」；② 浮层开着时一次都不许；③ 关掉后又照常", async () => {
    const approvals = useApprovalsStore();
    approvals.pending = [req()];
    const chat = useChatStore();
    chat.running = false;
    const decide = vi
      .spyOn(approvals, "decide")
      .mockResolvedValue(undefined as never);
    const composer = mount(Composer, {
      global: { plugins: [i18n] },
      attachTo: document.body,
    });
    await flushPromises();
    const ta = composer.find("textarea");
    expect(ta.exists()).toBe(true);

    // ① 链是活的（不是被让位判定堵死）
    expect(anyLayerOpen()).toBe(false);
    await ta.trigger("keydown", { key: "Escape" });
    expect(decide).toHaveBeenCalledTimes(1);
    expect(decide.mock.calls[0]?.[1]).toBe("deny");
    decide.mockClear();

    // ② 浮层开着 ⇒ 这一键归浮层，链不许动
    const layer = mountLayer(true);
    expect(anyLayerOpen()).toBe(true);
    await ta.trigger("keydown", { key: "Escape" });
    expect(decide).not.toHaveBeenCalled();
    // 浮层自己确实吃掉了这一键（关掉的是它，不是审批）
    expect(layer.open.value).toBe(false);
    expect(anyLayerOpen()).toBe(false);
    decide.mockClear();

    // ③ 关掉之后链恢复原样（让位不是一次性的，且优先级已归还）
    await ta.trigger("keydown", { key: "Escape" });
    expect(decide).toHaveBeenCalledTimes(1);

    composer.unmount();
    layer.wrapper.unmount();
  });
});

describe("PermissionCard 的 scope 下拉 · Esc 只关下拉", () => {
  it("开层占优先级，Esc 关层且不调 decide，关掉后归还", async () => {
    const approvals = useApprovalsStore();
    approvals.pending = [req()];
    const decide = vi
      .spyOn(approvals, "decide")
      .mockResolvedValue(undefined as never);
    const card = mount(PermissionCard, {
      props: { req: req(), queue: 0 },
      global: { plugins: [i18n] },
      attachTo: document.body,
    });
    await flushPromises();
    expect(anyLayerOpen()).toBe(false);

    await card.find(".approval-card__split-chev").trigger("click");
    expect(card.find(".approval-card__scope-pop").exists()).toBe(true);
    expect(anyLayerOpen()).toBe(true);

    document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    await nextTick();
    expect(card.find(".approval-card__scope-pop").exists()).toBe(false);
    expect(anyLayerOpen()).toBe(false);
    // 决定性那条：按 Esc 关的是下拉，**不是**把这张卡拒掉
    expect(decide).not.toHaveBeenCalled();

    card.unmount();
  });
});
