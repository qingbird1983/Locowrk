// 工具行的展开记忆（设计稿 §8，台账 AF-12）：行是 `v-for` 出来的，切会话、重投影、
// 列表回收都会重建组件实例 —— 记忆只能按 `item.id` 活在组件外。这条只靠真机很难验
// （要先制造一次重投影），钉在这里：谁把记忆搬回组件内 `ref`，这个用例立刻红。
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import { createPinia } from "pinia";
import ToolRow from "../src/components/chat/ToolRow.vue";
import { i18n } from "../src/i18n";
import { resetRowFolds } from "../src/lib/chat/fold-state";
import type { ToolItem } from "../src/stores/chat";

const item = (id: number): ToolItem => ({
  id,
  kind: "tool",
  callId: `call-${id}`,
  tool: "fs_read",
  args: { path: "/w/a.ts" },
  status: "settled",
  ok: true,
  output: { path: "/w/a.ts", bytes: 2 },
  ts: 1,
  doneTs: 2,
});

const mountRow = (id: number) =>
  mount(ToolRow, { props: { item: item(id) }, global: { plugins: [createPinia(), i18n] } });

describe("工具行展开记忆", () => {
  it("换一个组件实例后仍展开着（记忆不在组件内）", async () => {
    resetRowFolds();
    const first = mountRow(42);
    // 从没展开过 ⇒ 懒挂载都没发生，DOM 里没有明细
    expect(first.find(".tool__sub").exists()).toBe(false);
    await first.find(".prow").trigger("click");
    expect(first.find(".tool__sub").exists()).toBe(true);
    first.unmount();

    const second = mountRow(42);
    expect(second.find(".tool__sub").exists()).toBe(true);
    second.unmount();
    resetRowFolds();
  });

  it("不同行互不串味", async () => {
    resetRowFolds();
    const a = mountRow(1);
    await a.find(".prow").trigger("click");
    a.unmount();
    const b = mountRow(2);
    expect(b.find(".tool__sub").exists()).toBe(false);
    b.unmount();
    resetRowFolds();
  });
});
